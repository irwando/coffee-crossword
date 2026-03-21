// ── Normalization ─────────────────────────────────────────────────────────────

/// Normalize a word: strip non-letter, non-digit characters and lowercase.
/// Part of the public API — used by callers who want to normalize words
/// before comparing them to search results.
pub fn normalize(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphabetic() || c.is_ascii_digit())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// The form used for pattern matching: lowercased, optionally normalized.
/// pub(crate) because grouping.rs needs it to build the matching form
/// before passing words to eval_expr.
pub(crate) fn matching_form(word: &str, normalize_mode: bool) -> String {
    if normalize_mode {
        normalize(word)
    } else {
        word.to_ascii_lowercase()
    }
}
