// ── Grouping ──────────────────────────────────────────────────────────────────
// Runs the search loop over a word list or cache, groups results by normalized
// key, and deduplicates variants.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::engine::ast::{LogicalExpr, MatchGroup};
use crate::engine::matcher::eval_expr;
use crate::engine::normalize::matching_form;

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
) -> Vec<MatchGroup> {
    let mut raw: Vec<RawMatch> = Vec::new();

    for word in words {
        let norm_word = matching_form(word, normalize_mode);
        let raw_word = word.to_lowercase();
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
) -> Vec<MatchGroup> {
    static NEVER_CANCEL: AtomicBool = AtomicBool::new(false);
    search_cache_inner(cache, expr, min_len, max_len, normalize_mode, &NEVER_CANCEL)
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
    cancel: &AtomicBool,
    max_results: usize,
    on_batch: F,
) -> (Vec<MatchGroup>, bool)
where
    F: Fn(Vec<MatchGroup>),
{
    let mut all_raw: Vec<RawMatch> = Vec::new();
    let mut entry_count: u32 = 0;
    let mut total_matches: usize = 0;
    let mut truncated = false;

    'outer: for len in min_len..=max_len.min(255) {
        let (start, end) = cache.length_bucket(len);
        if start >= end {
            continue;
        }

        let mut bucket_raw: Vec<RawMatch> = Vec::new();

        for i in start..end {
            entry_count = entry_count.wrapping_add(1);
            if entry_count & 0x1FFF == 0 && cancel.load(Ordering::Relaxed) {
                return (Vec::new(), false);
            }

            let entry = cache.get_entry(i);

            let (raw_word, norm_word) = if normalize_mode {
                (entry.norm.to_lowercase(), entry.norm.to_string())
            } else {
                let orig_lower = entry.orig.to_lowercase();
                let norm = matching_form(&orig_lower, false);
                (orig_lower, norm)
            };

            let word_len = norm_word.chars().count();
            if word_len < min_len || word_len > max_len {
                continue;
            }

            if let Some(balance_str) = eval_expr(&raw_word, &norm_word, word_len, expr) {
                let raw_match = RawMatch {
                    original: entry.orig.to_string(),
                    normalized_key: norm_word,
                    balance: if balance_str.is_empty() { None } else { Some(balance_str) },
                };
                bucket_raw.push(raw_match.clone());
                all_raw.push(raw_match);
                total_matches += 1;

                // Flush to frontend when we hit the per-batch cap.
                if bucket_raw.len() >= MAX_BATCH_SIZE {
                    on_batch(build_groups(std::mem::take(&mut bucket_raw)));
                }

                // Stop collecting once the result cap is reached.
                if max_results > 0 && total_matches >= max_results {
                    truncated = true;
                    if !bucket_raw.is_empty() {
                        on_batch(build_groups(std::mem::take(&mut bucket_raw)));
                    }
                    break 'outer;
                }
            }
        }

        // End of bucket — flush any remaining entries.
        if !bucket_raw.is_empty() {
            on_batch(build_groups(bucket_raw));
        }
    }

    (build_groups(all_raw), truncated)
}

fn search_cache_inner(
    cache: &crate::cache::CacheHandle,
    expr: &LogicalExpr,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
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

            // Choose matching form based on normalize mode.
            let (raw_word, norm_word) = if normalize_mode {
                (entry.norm.to_lowercase(), entry.norm.to_string())
            } else {
                let orig_lower = entry.orig.to_lowercase();
                let norm = matching_form(&orig_lower, false);
                (orig_lower, norm)
            };

            let word_len = norm_word.chars().count();
            if word_len < min_len || word_len > max_len {
                continue;
            }

            if let Some(balance_str) = eval_expr(&raw_word, &norm_word, word_len, expr) {
                raw.push(RawMatch {
                    original: entry.orig.to_string(),
                    normalized_key: norm_word,
                    balance: if balance_str.is_empty() { None } else { Some(balance_str) },
                });
            }
        }
    }

    build_groups(raw)
}

/// Group raw matches by normalized key, collecting variants.
fn build_groups(raw: Vec<RawMatch>) -> Vec<MatchGroup> {
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, MatchGroup> = HashMap::new();

    for raw_match in raw {
        let key = raw_match.normalized_key.clone();

        if let Some(group) = groups.get_mut(&key) {
            let original_lower = raw_match.original.to_ascii_lowercase();
            if original_lower != key {
                group.variants.push(raw_match.original);
            }
        } else {
            group_order.push(key.clone());
            let original_lower = raw_match.original.to_ascii_lowercase();
            let variants = if original_lower != key {
                vec![raw_match.original]
            } else {
                vec![]
            };
            groups.insert(
                key.clone(),
                MatchGroup {
                    normalized: key,
                    variants,
                    balance: raw_match.balance,
                },
            );
        }
    }

    let mut result: Vec<MatchGroup> = group_order
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
