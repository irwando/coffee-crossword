// ── Test utilities ────────────────────────────────────────────────────────────
// Shared helpers for all engine tests.
// This file is only compiled in test builds (#[cfg(test)] in mod.rs).

use crate::engine::ast::MatchGroup;

/// The standard word list used across all engine tests.
/// When adding a new test, verify the word list contains at least one word
/// that satisfies the test pattern. Add words here if needed.
pub(crate) fn word_list() -> Vec<String> {
    vec![
        // Core test words
        "electron", "canter", "nectar", "recant", "trance",
        "aardvark", "elephant", "cat", "act", "arc",
        "drinker", "beside", "bodice", "edible",
        "maharaja", "quick", "quack", "quirk", "quark",
        "escalator", "explorer's", "Escargots", "escargots", "escargot's",
        "catch-22", "escapists", "ultra",
        // Choice list / macro tests
        "arts", "rest", "rust", "sort", "star", "stir",
        "llama", "lynch", "lymph", "lyric",
        "yoga", "zinc",
        // Letter variable tests — palindromes and tautonyms
        "level", "radar", "civic", "refer", "repaper",
        "murmur", "beriberi",
        // Sub-pattern tests
        "patronage", "readable", "beryllium",
        // Punctuation tests
        "pick-me-up", "well-to-do", "fly-by-night", "dead end", "oh boy!", "Ascot",
        // Logical op tests
        "cats", "cast", "scat", "copycats", "scatter",
        "carbon", "carrot", "catch",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Extract normalized keys from a results vec for easy assertion.
pub(crate) fn keys(results: &[MatchGroup]) -> Vec<&str> {
    results.iter().map(|r| r.normalized.as_str()).collect()
}
