// ── AST types ─────────────────────────────────────────────────────────────────
// All pattern and expression types used across the engine.
// MatchContext and RawMatch live in the files that use them (matcher.rs, grouping.rs)
// because they don't cross module boundaries.

use serde::Serialize;

/// A group of words that normalize to the same canonical form.
/// This is part of the public API — callers receive Vec<MatchGroup>.
#[derive(Debug, Serialize, Clone)]
pub struct MatchGroup {
    pub normalized: String,
    pub variants: Vec<String>,
    pub balance: Option<String>,
}

/// Top-level expression tree supporting logical operations.
/// Internal only — callers use search_words(pattern_str) not LogicalExpr directly.
#[derive(Debug)]
pub(crate) enum LogicalExpr {
    Single(Pattern),
    And(Box<LogicalExpr>, Box<LogicalExpr>),
    Or(Box<LogicalExpr>, Box<LogicalExpr>),
    Not(Box<LogicalExpr>),
}

/// A parsed pattern — one arm of a LogicalExpr::Single.
#[derive(Debug)]
pub(crate) enum Pattern {
    Template(Vec<TemplateChar>),
    /// letters, dot_count, has_wildcard
    Anagram(Vec<AnagramChar>, Option<usize>, bool),
    TemplateWithAnagram(Vec<TemplateChar>, Vec<AnagramChar>, Option<usize>),
}

/// A single position in a template pattern.
#[derive(Debug, Clone)]
pub(crate) enum TemplateChar {
    /// A literal letter that must match exactly
    Literal(char),
    /// A dot or question mark — matches any single letter
    Any,
    /// A wildcard * — matches zero or more letters
    Wildcard,
    /// A choice list [abc] or negated [^abc]
    ChoiceList(Vec<char>, bool), // (letters, negated)
    /// A letter variable — digit 0-9; same digit must match same letter
    Variable(u8),
    /// A sub-pattern — switches mode: anagram sub-pattern within a template
    /// The usize is the number of characters this sub-pattern consumes
    SubPattern(SubPattern),
}

/// A sub-pattern that switches matching mode.
#[derive(Debug, Clone)]
pub(crate) enum SubPattern {
    /// (;xxx) inside a template — the next N chars must be an anagram of xxx
    Anagram(Vec<char>),
    /// (xxx) inside an anagram — the letters xxx must appear consecutively in order
    Template(Vec<TemplateChar>),
    /// (;xxx) inside an anagram — the letters xxx must appear as an anagram
    /// (functionally same as adding to anagram letters, but explicit)
    AnagramInAnagram(Vec<char>),
}

/// A single element in an anagram pattern.
/// Most are just letters, but sub-patterns can appear too.
#[derive(Debug, Clone)]
pub(crate) enum AnagramChar {
    /// A plain letter that must appear somewhere in the word
    Letter(char),
    /// A dot/? — one unknown letter (counts toward length)
    Blank,
    /// A choice list in the anagram — one letter from the set must appear
    ChoiceList(Vec<char>, bool),
    /// A sub-pattern — a sequence of letters that must appear consecutively
    SubPattern(SubPattern),
}
