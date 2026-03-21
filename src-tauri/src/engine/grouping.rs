// ── Grouping ──────────────────────────────────────────────────────────────────
// Runs the search loop over a word list, groups results by normalized key,
// and deduplicates variants.
// RawMatch is private — it's an intermediate type only used within this file.

use std::collections::HashMap;
use crate::engine::ast::{LogicalExpr, MatchGroup};
use crate::engine::matcher::eval_expr;
use crate::engine::normalize::matching_form;

/// Intermediate match result before grouping — private to this module.
struct RawMatch {
    original: String,
    normalized_key: String,
    balance: Option<String>,
}

/// Search a word list against a LogicalExpr and return grouped, deduplicated results.
/// pub(crate) — called from mod.rs's search_words().
pub(crate) fn search(
    words: &[String],
    expr: &LogicalExpr,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
) -> Vec<MatchGroup> {
    let mut raw: Vec<RawMatch> = Vec::new();

    for word in words {
        let matched_form = matching_form(word, normalize_mode);
        let word_len = matched_form.chars().count();

        if word_len < min_len || word_len > max_len {
            continue;
        }

        if let Some(balance_str) = eval_expr(&matched_form, word_len, expr) {
            raw.push(RawMatch {
                original: word.clone(),
                normalized_key: matched_form,
                balance: if balance_str.is_empty() { None } else { Some(balance_str) },
            });
        }
    }

    // Group by normalized key, collecting variants
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

    // Collect in insertion order, then sort by length then alphabetically
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
