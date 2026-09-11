// ── Normalization ─────────────────────────────────────────────────────────────

use unicode_normalization::UnicodeNormalization;

/// Normalize a word: strip non-letter, non-digit characters and lowercase.
/// Part of the public API — used by callers who want to normalize words
/// before comparing them to search results.
pub fn normalize(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphabetic() || c.is_ascii_digit())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Fold accented letters to their plain equivalents (e.g. "café" -> "cafe")
/// via Unicode NFD decomposition + stripping combining marks (U+0300-U+036F).
/// Length-preserving for the common case (one precomposed accented letter ->
/// one base letter). Used for the pattern string (folded once before parsing
/// — see `mod.rs`) and the plain-word-list / normalize=off search paths,
/// neither of which can use the cache's precomputed `fold`/`orig_lower`
/// fields. Duplicated in `cache.rs` (which must stay engine-free) — see that
/// copy's doc comment.
pub(crate) fn fold_accents(s: &str) -> String {
    s.nfd().filter(|c| !('\u{0300}'..='\u{036f}').contains(c)).collect()
}

/// The form used for pattern matching: lowercased, optionally normalized and
/// accent-folded. pub(crate) because grouping.rs needs it to build the
/// matching form before passing words to eval_expr.
pub(crate) fn matching_form(word: &str, normalize_mode: bool, fold_accents_mode: bool) -> String {
    let base = if normalize_mode {
        normalize(word)
    } else {
        word.to_ascii_lowercase()
    };
    if fold_accents_mode {
        fold_accents(&base)
    } else {
        base
    }
}
