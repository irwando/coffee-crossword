// ── Engine public API ─────────────────────────────────────────────────────────
// This is the only file external callers (CLI, Tauri commands, future Python/WASM)
// should need to know about. All implementation details are in sub-modules.

pub mod ast;
pub(crate) mod describe;
pub(crate) mod grouping;
pub(crate) mod matcher;
pub(crate) mod normalize;
pub(crate) mod parser;

#[cfg(test)]
pub(crate) mod test_utils;
#[cfg(test)]
mod tests;

// Re-export the public API symbols so callers can use engine::search_words etc.
pub use ast::MatchGroup;
pub use normalize::normalize;

// Re-export the public functions under a mod_pub alias so tests can import
// them cleanly without ambiguity
pub mod mod_pub {
    pub use super::{search_words, validate_pattern, describe_pattern, normalize};
}

/// Search a word list using a pattern string.
/// Handles all pattern types including logical operations.
/// This is the main entry point for plain-text word lists and tests.
pub fn search_words(
    words: &[String],
    pattern: &str,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
) -> Vec<MatchGroup> {
    match parser::parse_logical(pattern) {
        Some(expr) => grouping::search(words, &expr, min_len, max_len, normalize_mode),
        None => Vec::new(),
    }
}

/// Search a memory-mapped cache using a pattern string.
/// This is the high-performance entry point for .tsc cache files.
/// Uses length-bucketed access to avoid scanning the full list.
pub fn search_cache(
    cache: &crate::cache::CacheHandle,
    pattern: &str,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
) -> Vec<MatchGroup> {
    let expr = match parser::parse_logical(pattern) {
        Some(e) => e,
        None => return Vec::new(),
    };

    grouping::search_cache(cache, &expr, min_len, max_len, normalize_mode)
}

/// Streaming + cancellable variant for the Tauri search command.
/// Calls `on_batch` with slices of results as they are found so the UI can
/// display partial results immediately. Batches are capped at MAX_BATCH_SIZE
/// entries each to keep IPC events small.
///
/// `max_results`: stop after this many total matches (0 = unlimited).
///
/// Returns `(complete_groups, truncated)`.
pub(crate) fn search_cache_cancellable_streaming<F>(
    cache: &crate::cache::CacheHandle,
    pattern: &str,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
    cancel: &std::sync::atomic::AtomicBool,
    max_results: usize,
    on_batch: F,
) -> (Vec<MatchGroup>, bool)
where
    F: Fn(Vec<MatchGroup>),
{
    let expr = match parser::parse_logical(pattern) {
        Some(e) => e,
        None => return (Vec::new(), false),
    };
    grouping::search_cache_streaming(cache, &expr, min_len, max_len, normalize_mode, cancel, max_results, on_batch)
}


/// Validate a pattern string.
/// Returns Ok(()) if valid, Err(reason) if not.
pub fn validate_pattern(pattern: &str) -> Result<(), String> {
    let input = pattern.trim();
    if input.is_empty() {
        return Err("Pattern is empty".to_string());
    }
    match parser::parse_logical(input) {
        Some(_) => Ok(()),
        None => Err("Invalid pattern".to_string()),
    }
}

/// Return a human-readable description of a pattern.
/// Returns None if the pattern is empty or invalid.
pub fn describe_pattern(pattern: &str) -> Option<String> {
    describe::describe_pattern(pattern)
}
