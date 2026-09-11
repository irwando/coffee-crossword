// ── Grouping ──────────────────────────────────────────────────────────────────
// Runs the search loop over a word list or cache, groups results by normalized
// key, and deduplicates variants.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::engine::ast::{LogicalExpr, MatchGroup};
use crate::engine::matcher::eval_expr;
use crate::engine::normalize::{fold_accents, matching_form};

/// Matching form for one cache entry. Three of the four (normalize, fold)
/// combinations are zero-alloc borrows straight from precomputed cache
/// fields (`cache.rs` builds `norm`, `orig_lower`, and `fold` — the
/// accent-folded `norm` — once per list build, not once per search).
/// `normalize=false` + fold-accents-on is the one allocating combination
/// left: it's the narrowest of the four (normalize=off is already a niche
/// punctuation-sensitive mode), so a 5th precomputed field wasn't worth it.
fn candidate_word<'a>(
    entry: &crate::cache::CacheEntry<'a>,
    normalize_mode: bool,
    fold_accents_mode: bool,
) -> Cow<'a, str> {
    match (normalize_mode, fold_accents_mode) {
        (true, true) => Cow::Borrowed(entry.fold),
        (true, false) => Cow::Borrowed(entry.norm),
        (false, false) => Cow::Borrowed(entry.orig_lower),
        (false, true) => Cow::Owned(fold_accents(entry.orig_lower)),
    }
}

/// Intermediate match result before grouping — private to this module.
#[derive(Clone)]
struct RawMatch {
    original: String,
    normalized_key: String,
    balance: Option<String>,
}

/// Search a plain word list against a LogicalExpr.
/// pub(crate) — called from mod.rs search_words().
pub(crate) fn search(
    words: &[String],
    expr: &LogicalExpr,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
    fold_accents_mode: bool,
) -> Vec<MatchGroup> {
    let mut raw: Vec<RawMatch> = Vec::new();

    for word in words {
        let norm_word = matching_form(word, normalize_mode, fold_accents_mode);
        let raw_word = if fold_accents_mode { fold_accents(&word.to_lowercase()) } else { word.to_lowercase() };
        let word_len = norm_word.chars().count();

        if word_len < min_len || word_len > max_len {
            continue;
        }

        if let Some(balance_str) = eval_expr(&raw_word, &norm_word, word_len, expr) {
            raw.push(RawMatch {
                original: word.clone(),
                normalized_key: norm_word,
                balance: if balance_str.is_empty() { None } else { Some(balance_str) },
            });
        }
    }

    build_groups(raw)
}

/// Search a memory-mapped cache against a LogicalExpr.
/// Uses length-bucketed access for template patterns to avoid full scans
/// when possible; falls back to full scan for wildcard and logical patterns
/// that span multiple lengths.
pub(crate) fn search_cache(
    cache: &crate::cache::CacheHandle,
    expr: &LogicalExpr,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
    fold_accents_mode: bool,
) -> Vec<MatchGroup> {
    static NEVER_CANCEL: AtomicBool = AtomicBool::new(false);
    search_cache_inner(cache, expr, min_len, max_len, normalize_mode, fold_accents_mode, &NEVER_CANCEL)
}


/// Maximum entries per on_batch call — prevents multi-MB IPC events from
/// stalling the frontend when many results are found in a single length bucket.
const MAX_BATCH_SIZE: usize = 500;

/// Streaming variant — calls `on_batch` with slices of results as they are
/// found, so the caller can forward partial results to the UI immediately.
///
/// Batches are emitted at two boundaries:
///   1. Every MAX_BATCH_SIZE matches (prevents huge single IPC events).
///   2. At the end of each length bucket (natural grouping boundary).
///
/// `max_results`: stop after this many total matches and set truncated=true.
///   Pass 0 for unlimited (CLI / test use).
///
/// Returns `(complete_groups, truncated)`. The complete_groups is the full
/// grouped result for dedup and the final emit; truncated signals the UI.
pub(crate) fn search_cache_streaming<F>(
    cache: &crate::cache::CacheHandle,
    expr: &LogicalExpr,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
    fold_accents_mode: bool,
    cancel: &AtomicBool,
    max_results: usize,
    on_batch: F,
) -> (Vec<MatchGroup>, bool)
where
    F: Fn(Vec<MatchGroup>),
{
    let mut builder = GroupBuilder::new();
    let mut entry_count: u32 = 0;
    let mut total_matches: usize = 0;
    let mut since_flush: usize = 0;
    let mut truncated = false;

    'outer: for len in min_len..=max_len.min(255) {
        let (start, end) = cache.length_bucket(len);
        if start >= end {
            continue;
        }

        for i in start..end {
            entry_count = entry_count.wrapping_add(1);
            if entry_count & 0x1FFF == 0 && cancel.load(Ordering::Relaxed) {
                return (Vec::new(), false);
            }

            let entry = cache.get_entry(i);
            let word = candidate_word(&entry, normalize_mode, fold_accents_mode);

            let word_len = word.chars().count();
            if word_len < min_len || word_len > max_len {
                continue;
            }

            if let Some(balance_str) = eval_expr(&word, &word, word_len, expr) {
                let balance = if balance_str.is_empty() { None } else { Some(balance_str) };
                builder.insert(entry.orig, word.into_owned(), balance);
                total_matches += 1;
                since_flush += 1;

                // Flush to frontend when we hit the per-batch cap.
                if since_flush >= MAX_BATCH_SIZE {
                    on_batch(builder.drain_batch());
                    since_flush = 0;
                }

                // Stop collecting once the result cap is reached.
                if max_results > 0 && total_matches >= max_results {
                    truncated = true;
                    if since_flush > 0 {
                        on_batch(builder.drain_batch());
                    }
                    break 'outer;
                }
            }
        }

        // End of bucket — flush any remaining entries.
        if since_flush > 0 {
            on_batch(builder.drain_batch());
            since_flush = 0;
        }
    }

    (builder.finish(), truncated)
}

fn search_cache_inner(
    cache: &crate::cache::CacheHandle,
    expr: &LogicalExpr,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
    fold_accents_mode: bool,
    cancel: &AtomicBool,
) -> Vec<MatchGroup> {
    let mut raw: Vec<RawMatch> = Vec::new();
    let mut entry_count: u32 = 0;

    for len in min_len..=max_len.min(255) {
        let (start, end) = cache.length_bucket(len);
        if start >= end {
            continue;
        }

        for i in start..end {
            // Check cancel flag every 8192 entries (bitmask avoids division).
            entry_count = entry_count.wrapping_add(1);
            if entry_count & 0x1FFF == 0 && cancel.load(Ordering::Relaxed) {
                return Vec::new();
            }

            let entry = cache.get_entry(i);
            let word = candidate_word(&entry, normalize_mode, fold_accents_mode);

            let word_len = word.chars().count();
            if word_len < min_len || word_len > max_len {
                continue;
            }

            if let Some(balance_str) = eval_expr(&word, &word, word_len, expr) {
                raw.push(RawMatch {
                    original: entry.orig.to_string(),
                    normalized_key: word.into_owned(),
                    balance: if balance_str.is_empty() { None } else { Some(balance_str) },
                });
            }
        }
    }

    build_groups(raw)
}

/// Group raw matches by normalized key, collecting variants.
fn build_groups(raw: Vec<RawMatch>) -> Vec<MatchGroup> {
    let mut builder = GroupBuilder::new();
    for m in raw {
        builder.insert(&m.original, m.normalized_key, m.balance);
    }
    builder.finish()
}

/// Incrementally groups matches by normalized key as they're found, merging
/// variants into the same `MatchGroup` in one pass. `build_groups` is a thin
/// wrapper over this for the single-shot (`search`/`search_cache_inner`)
/// callers; `search_cache_streaming` uses it directly so the whole result set
/// is only grouped once — not once per streamed batch and again in full at
/// the end (which also required cloning every match to keep two parallel
/// accumulations).
struct GroupBuilder {
    order: Vec<String>,
    groups: HashMap<String, MatchGroup>,
    /// Keys touched since the last `drain_batch()` call, in touch order
    /// (may contain duplicates — deduplicated when drained).
    dirty_since_flush: Vec<String>,
}

impl GroupBuilder {
    fn new() -> Self {
        GroupBuilder { order: Vec::new(), groups: HashMap::new(), dirty_since_flush: Vec::new() }
    }

    /// Merge one match into its group, creating the group on first sight.
    /// `original` is only ever copied into an owned `String` when it's
    /// actually a case/diacritic variant worth recording — not for every
    /// match, unlike storing it in an intermediate struct up front would.
    fn insert(&mut self, original: &str, normalized_key: String, balance: Option<String>) {
        let original_lower = original.to_ascii_lowercase();
        match self.groups.get_mut(&normalized_key) {
            Some(group) => {
                if original_lower != normalized_key {
                    group.variants.push(original.to_string());
                }
            }
            None => {
                self.order.push(normalized_key.clone());
                let variants = if original_lower != normalized_key {
                    vec![original.to_string()]
                } else {
                    vec![]
                };
                self.groups.insert(
                    normalized_key.clone(),
                    MatchGroup { normalized: normalized_key.clone(), variants, balance },
                );
            }
        }
        self.dirty_since_flush.push(normalized_key);
    }

    /// Groups touched since the last call, deduplicated, in their current
    /// (possibly variant-updated) state — for a streaming partial batch.
    fn drain_batch(&mut self) -> Vec<MatchGroup> {
        let dirty = std::mem::take(&mut self.dirty_since_flush);
        let mut seen: HashSet<String> = HashSet::new();
        let mut out = Vec::new();
        for key in dirty {
            if seen.insert(key.clone()) {
                if let Some(group) = self.groups.get(&key) {
                    out.push(group.clone());
                }
            }
        }
        out
    }

    /// Final, length-then-normalized-sorted result. Consumes the builder —
    /// no separate re-grouping pass over raw matches needed.
    fn finish(self) -> Vec<MatchGroup> {
        let mut groups = self.groups;
        let mut result: Vec<MatchGroup> = self
            .order
            .into_iter()
            .filter_map(|k| groups.remove(&k))
            .collect();

        result.sort_by(|a, b| {
            a.normalized
                .len()
                .cmp(&b.normalized.len())
                .then(a.normalized.cmp(&b.normalized))
        });

        result
    }
}
