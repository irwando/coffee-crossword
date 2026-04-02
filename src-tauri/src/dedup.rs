// ── Deduplication ────────────────────────────────────────────────────────────
// When dedup is enabled, a word that appears in multiple active lists is shown
// only in the highest-priority list that contains it. Lower-priority results
// for that normalized key are removed in-place.
//
// Priority order = the order of results in the Vec (index 0 = highest priority).

use std::collections::HashSet;
use serde::Serialize;
use crate::engine::MatchGroup;

/// Search results for one word list.
#[derive(Debug, Clone, Serialize)]
pub struct ListSearchResult {
    pub list_id: String,
    pub list_name: String,
    pub results: Vec<MatchGroup>,
    pub truncated: bool,
    pub error: Option<String>,
}

/// Remove duplicate normalized keys from lower-priority lists in-place.
/// The first occurrence (highest-priority list) is always kept.
/// Lists with errors are left unchanged.
pub fn deduplicate(results: &mut Vec<ListSearchResult>) {
    let mut seen: HashSet<String> = HashSet::new();

    for list_result in results.iter_mut() {
        if list_result.error.is_some() {
            continue;
        }
        list_result
            .results
            .retain(|group| seen.insert(group.normalized.clone()));
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_group(normalized: &str) -> MatchGroup {
        MatchGroup {
            normalized: normalized.to_string(),
            variants: vec![],
            balance: None,
        }
    }

    fn make_result(list_id: &str, words: &[&str]) -> ListSearchResult {
        ListSearchResult {
            list_id: list_id.to_string(),
            list_name: list_id.to_string(),
            results: words.iter().map(|w| make_group(w)).collect(),
            truncated: false,
            error: None,
        }
    }

    fn make_error_result(list_id: &str) -> ListSearchResult {
        ListSearchResult {
            list_id: list_id.to_string(),
            list_name: list_id.to_string(),
            results: vec![make_group("cat")],
            truncated: false,
            error: Some("build failed".to_string()),
        }
    }

    #[test]
    fn test_dedup_removes_from_lower_priority() {
        let mut results = vec![
            make_result("list1", &["cat", "dog"]),
            make_result("list2", &["cat", "bird"]), // "cat" is a dupe
        ];
        deduplicate(&mut results);

        let list1: Vec<&str> = results[0].results.iter().map(|r| r.normalized.as_str()).collect();
        let list2: Vec<&str> = results[1].results.iter().map(|r| r.normalized.as_str()).collect();

        assert_eq!(list1, vec!["cat", "dog"]);
        assert_eq!(list2, vec!["bird"]); // "cat" removed
    }

    #[test]
    fn test_dedup_respects_priority_order() {
        // list2 has higher priority (index 0), list1 lower (index 1).
        let mut results = vec![
            make_result("list2", &["bird"]),
            make_result("list1", &["cat", "bird"]), // "bird" is a dupe of list2
        ];
        deduplicate(&mut results);

        let list2: Vec<&str> = results[0].results.iter().map(|r| r.normalized.as_str()).collect();
        let list1: Vec<&str> = results[1].results.iter().map(|r| r.normalized.as_str()).collect();

        assert_eq!(list2, vec!["bird"]); // kept
        assert_eq!(list1, vec!["cat"]);  // "bird" removed
    }

    #[test]
    fn test_dedup_word_only_in_one_list() {
        let mut results = vec![
            make_result("list1", &["cat"]),
            make_result("list2", &["dog"]),
        ];
        deduplicate(&mut results);

        assert_eq!(results[0].results.len(), 1);
        assert_eq!(results[1].results.len(), 1);
    }

    #[test]
    fn test_dedup_three_lists() {
        let mut results = vec![
            make_result("list1", &["cat", "dog"]),
            make_result("list2", &["cat", "bird"]),
            make_result("list3", &["cat", "dog", "fish"]),
        ];
        deduplicate(&mut results);

        let r1: Vec<&str> = results[0].results.iter().map(|r| r.normalized.as_str()).collect();
        let r2: Vec<&str> = results[1].results.iter().map(|r| r.normalized.as_str()).collect();
        let r3: Vec<&str> = results[2].results.iter().map(|r| r.normalized.as_str()).collect();

        assert_eq!(r1, vec!["cat", "dog"]);
        assert_eq!(r2, vec!["bird"]);
        assert_eq!(r3, vec!["fish"]);
    }

    #[test]
    fn test_dedup_empty_lists() {
        let mut results = vec![
            make_result("list1", &[]),
            make_result("list2", &["cat"]),
        ];
        deduplicate(&mut results);
        assert_eq!(results[1].results.len(), 1);
    }

    #[test]
    fn test_dedup_leaves_error_results_unchanged() {
        let mut results = vec![
            make_result("list1", &["cat"]),
            make_error_result("list2"), // has "cat" but marked as error
        ];
        deduplicate(&mut results);

        // Error result should be untouched.
        assert_eq!(results[1].results.len(), 1);
        assert_eq!(results[1].results[0].normalized, "cat");
    }

    #[test]
    fn test_dedup_disabled_all_results_unchanged() {
        // Callers are responsible for not calling deduplicate when disabled.
        // This test verifies the function itself doesn't modify results when
        // called on disjoint sets.
        let mut results = vec![
            make_result("list1", &["cat"]),
            make_result("list2", &["dog"]),
        ];
        let before: Vec<usize> = results.iter().map(|r| r.results.len()).collect();
        deduplicate(&mut results);
        let after: Vec<usize> = results.iter().map(|r| r.results.len()).collect();
        assert_eq!(before, after);
    }

    #[test]
    fn test_dedup_single_list_unchanged() {
        let mut results = vec![make_result("list1", &["cat", "dog", "bird"])];
        deduplicate(&mut results);
        assert_eq!(results[0].results.len(), 3);
    }
}
