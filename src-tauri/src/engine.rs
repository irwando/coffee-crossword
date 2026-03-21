use std::collections::HashMap;

// ── Public API ────────────────────────────────────────────────────────────────
// These are the only four functions external callers (CLI, Python, WASM) need.

/// Search a word list using a pattern string. Handles all pattern types
/// including logical operations. This is the main entry point for all callers.
pub fn search_words(
    words: &[String],
    pattern: &str,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
) -> Vec<MatchGroup> {
    match parse_logical(pattern) {
        Some(expr) => search(words, &expr, min_len, max_len, normalize_mode),
        None => Vec::new(),
    }
}

/// Validate a pattern string. Returns Ok(()) if valid, Err(reason) if not.
pub fn validate_pattern(pattern: &str) -> Result<(), String> {
    let input = pattern.trim();
    if input.is_empty() {
        return Err("Pattern is empty".to_string());
    }
    match parse_logical(input) {
        Some(_) => Ok(()),
        None => Err("Invalid pattern".to_string()),
    }
}

/// Return a human-readable description of a pattern.
/// Returns None if the pattern is empty or invalid.
pub fn describe_pattern(pattern: &str) -> Option<String> {
    let input = pattern.trim();
    if input.is_empty() {
        return None;
    }
    // Check for logical operators — stub for now
    if input.contains(" & ") || input.contains(" | ") || input.contains('!') {
        return Some("Complex pattern".to_string());
    }
    parse_logical(input)?;
    Some(describe_simple(input))
}

/// Normalize a word: strip non-letter, non-digit characters and lowercase.
pub fn normalize(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphabetic() || c.is_ascii_digit())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// A group of words that normalize to the same canonical form.
#[derive(Debug, serde::Serialize, Clone)]
pub struct MatchGroup {
    pub normalized: String,
    pub variants: Vec<String>,
    pub balance: Option<String>,
}

// ── Internal types ────────────────────────────────────────────────────────────

/// Top-level expression tree supporting logical operations
#[derive(Debug)]
enum LogicalExpr {
    Single(Pattern),
    And(Box<LogicalExpr>, Box<LogicalExpr>),
    Or(Box<LogicalExpr>, Box<LogicalExpr>),
    Not(Box<LogicalExpr>),
}

/// A parsed pattern ready for matching
#[derive(Debug)]
enum Pattern {
    Template(Vec<TemplateChar>),
    /// letters, dot_count, has_wildcard
    Anagram(Vec<char>, Option<usize>, bool),
    TemplateWithAnagram(Vec<TemplateChar>, Vec<char>, Option<usize>),
}

/// A single position in a template pattern
#[derive(Debug, Clone)]
enum TemplateChar {
    Literal(char),
    Any,
    Wildcard,
    ChoiceList(Vec<char>, bool), // (letters, negated)
    Variable(u8),                // digit 0-9
}

/// Carries letter variable bindings through template matching
#[derive(Clone)]
struct MatchContext {
    variables: HashMap<u8, char>,
}

impl MatchContext {
    fn new() -> Self {
        MatchContext { variables: HashMap::new() }
    }

    /// Try to bind a variable to a character. Returns false if already bound
    /// to a different character.
    fn bind(&mut self, var: u8, ch: char) -> bool {
        match self.variables.get(&var) {
            Some(&existing) => existing == ch,
            None => { self.variables.insert(var, ch); true }
        }
    }
}

struct RawMatch {
    original: String,
    normalized_key: String,
    balance: Option<String>,
}

// ── Macro expansion ───────────────────────────────────────────────────────────

/// Expand @ and # macros before parsing. This makes macros work everywhere
/// — templates, anagrams, choice lists — transparently.
fn expand_macros(input: &str) -> String {
    input.replace('@', "[aeiou]").replace('#', "[^aeiou]")
}

// ── Pattern description ───────────────────────────────────────────────────────

fn describe_choice_list_inner(inner: &str) -> String {
    let negated = inner.starts_with('^');
    let letters = inner.trim_start_matches('^').to_uppercase();
    if letters == "AEIOU" {
        return if negated { "any consonant".to_string() } else { "any vowel".to_string() };
    }
    if negated {
        format!("any letter except {}", letters.chars().collect::<Vec<_>>().iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "))
    } else {
        format!("one of: {}", letters.chars().collect::<Vec<_>>().iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "))
    }
}

fn describe_simple(input: &str) -> String {
    let expanded = expand_macros(input);
    let val = expanded.trim();

    let semi_pos = val.find(';');

    if let Some(semi) = semi_pos {
        let tmpl = &val[..semi];
        let anagram_part = &val[semi + 1..];
        let (letters, dots, has_wildcard, choice_descs) = parse_anagram_for_description(anagram_part);

        if tmpl.is_empty() {
            // Pure anagram
            let mut s = if letters.is_empty() {
                "Anagram search".to_string()
            } else {
                format!("Anagrams of \"{}\"", letters.to_uppercase())
            };
            if !choice_descs.is_empty() {
                s += &format!(" plus {}", choice_descs.join(" and "));
            }
            if has_wildcard {
                s += " (any number of extra letters)";
            } else {
                let plain_dots = dots.saturating_sub(choice_descs.len());
                if plain_dots == 1 { s += " plus 1 unknown letter"; }
                else if plain_dots > 1 { s += &format!(" plus {} unknown letters", plain_dots); }
            }
            return s;
        }

        // Template + anagram
        let mut s = describe_template_part(tmpl);
        if !letters.is_empty() {
            s += &format!(", containing the letters \"{}\"", letters.to_uppercase());
        }
        if !choice_descs.is_empty() {
            s += &format!(" and {}", choice_descs.join(" and "));
        }
        if has_wildcard {
            s += " (any number of extra letters)";
        } else {
            let plain_dots = dots.saturating_sub(choice_descs.len());
            if plain_dots == 1 { s += " plus 1 unknown letter"; }
            else if plain_dots > 1 { s += &format!(" plus {} unknown letters", plain_dots); }
        }
        return s;
    }

    // Pure template
    describe_template_part(val)
}

fn describe_template_part(tmpl: &str) -> String {
    let has_wild = tmpl.contains('*');
    let fixed_len = count_template_len(tmpl);

    // Detect first character description
    let first_desc = describe_first_char(tmpl);
    // Detect last literal char
    let last_literal = get_last_literal(tmpl);

    if has_wild {
        let mut desc = "Words".to_string();
        if let Some(ref fd) = first_desc {
            desc += &format!(" {}", fd);
        }
        if let Some(last) = last_literal {
            desc += &format!(" ending with \"{}\"", last.to_uppercase());
        }
        if first_desc.is_none() && last_literal.is_none() {
            desc += " of any length";
        }
        desc
    } else {
        let mut desc = format!("{}-letter words", fixed_len);
        if let Some(fd) = first_desc {
            desc += &format!(" {}", fd);
        }
        if let Some(last) = last_literal {
            // Only mention last if it's different from what first_desc covers
            let first_char = tmpl.chars().next();
            if first_char != Some(last) {
                desc += &format!(" ending with \"{}\"", last.to_uppercase());
            }
        }
        desc
    }
}

fn describe_first_char(tmpl: &str) -> Option<String> {
    if tmpl.starts_with('[') {
        if let Some(end) = tmpl.find(']') {
            let inner = &tmpl[1..end];
            return Some(descr_first_choice(inner));
        }
    }
    let first = tmpl.chars().next()?;
    if first.is_alphabetic() {
        Some(format!("starting with \"{}\"", first.to_uppercase()))
    } else {
        None
    }
}

fn descr_first_choice(inner: &str) -> String {
    let negated = inner.starts_with('^');
    let letters = inner.trim_start_matches('^');
    if letters == "aeiou" || letters == "AEIOU" {
        return if negated {
            "starting with any consonant".to_string()
        } else {
            "starting with any vowel".to_string()
        };
    }
    if negated {
        format!("starting with any letter except {}", letters.to_uppercase())
    } else {
        format!("starting with one of: {}", letters.to_uppercase())
    }
}

fn get_last_literal(tmpl: &str) -> Option<char> {
    // Walk the template string, find the last char
    let last = tmpl.chars().last()?;
    if last.is_alphabetic() { Some(last) } else { None }
}

fn count_template_len(tmpl: &str) -> usize {
    let mut count = 0;
    let chars: Vec<char> = tmpl.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            count += 1;
            while i < chars.len() && chars[i] != ']' { i += 1; }
        } else if chars[i] != '*' {
            count += 1;
        }
        i += 1;
    }
    count
}

fn parse_anagram_for_description(anagram_part: &str) -> (String, usize, bool, Vec<String>) {
    let mut letters = String::new();
    let mut dots = 0usize;
    let mut has_wildcard = false;
    let mut choice_descs: Vec<String> = Vec::new();
    let chars: Vec<char> = anagram_part.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => { has_wildcard = true; i += 1; }
            '.' | '?' => { dots += 1; i += 1; }
            '[' => {
                dots += 1;
                i += 1;
                let mut inner = String::new();
                while i < chars.len() && chars[i] != ']' {
                    inner.push(chars[i]); i += 1;
                }
                if i < chars.len() { i += 1; }
                choice_descs.push(describe_choice_list_inner(&inner));
            }
            c if c.is_alphabetic() => { letters.push(c); i += 1; }
            _ => { i += 1; }
        }
    }
    (letters, dots, has_wildcard, choice_descs)
}

// ── Logical expression parser ─────────────────────────────────────────────────

fn parse_logical(input: &str) -> Option<LogicalExpr> {
    let input = expand_macros(input.trim());
    let input = input.trim();
    if input.is_empty() { return None; }
    parse_or(input)
}

/// Parse OR expressions (lowest precedence)
fn parse_or(input: &str) -> Option<LogicalExpr> {
    // Split on top-level | (not inside brackets or parens)
    let parts = split_logical(input, '|');
    if parts.len() > 1 {
        let mut iter = parts.into_iter();
        let mut left = parse_and(iter.next()?.trim())?;
        for part in iter {
            let right = parse_and(part.trim())?;
            left = LogicalExpr::Or(Box::new(left), Box::new(right));
        }
        return Some(left);
    }
    parse_and(input)
}

/// Parse AND expressions
fn parse_and(input: &str) -> Option<LogicalExpr> {
    let parts = split_logical(input, '&');
    if parts.len() > 1 {
        let mut iter = parts.into_iter();
        let mut left = parse_not(iter.next()?.trim())?;
        for part in iter {
            let right = parse_not(part.trim())?;
            left = LogicalExpr::And(Box::new(left), Box::new(right));
        }
        return Some(left);
    }
    parse_not(input)
}

/// Parse NOT expressions
fn parse_not(input: &str) -> Option<LogicalExpr> {
    let input = input.trim();
    if input.starts_with('!') {
        let inner = parse_not(input[1..].trim())?;
        return Some(LogicalExpr::Not(Box::new(inner)));
    }
    parse_atom(input)
}

/// Parse a single pattern or parenthesized group
fn parse_atom(input: &str) -> Option<LogicalExpr> {
    let input = input.trim();
    if input.starts_with('(') && input.ends_with(')') {
        // Check it's a matched pair
        let inner = &input[1..input.len() - 1];
        if let Some(expr) = parse_or(inner) {
            return Some(expr);
        }
    }
    // Single pattern
    let pattern = parse_pattern(input)?;
    Some(LogicalExpr::Single(pattern))
}

/// Split input on a logical operator character, respecting brackets and parens
fn split_logical(input: &str, op: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_bracket = 0i32;
    let mut depth_paren = 0i32;
    let mut last = 0;
    let chars: Vec<char> = input.chars().collect();
    let bytes: Vec<usize> = input.char_indices().map(|(i, _)| i).collect();

    for (idx, &ch) in chars.iter().enumerate() {
        match ch {
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            c if c == op && depth_bracket == 0 && depth_paren == 0 => {
                let byte_pos = bytes[idx];
                parts.push(&input[last..byte_pos]);
                last = byte_pos + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&input[last..]);
    parts
}

// ── Pattern parser ────────────────────────────────────────────────────────────

fn parse_pattern(input: &str) -> Option<Pattern> {
    let input = input.trim();
    if input.is_empty() { return None; }

    if let Some(semi_pos) = input.find(';') {
        let template_part = &input[..semi_pos];
        let anagram_part = &input[semi_pos + 1..];

        let mut anagram_letters: Vec<char> = Vec::new();
        let mut dot_count = 0usize;
        let mut has_wildcard = false;

        let anagram_chars: Vec<char> = anagram_part.chars().collect();
        let mut i = 0;
        while i < anagram_chars.len() {
            match anagram_chars[i] {
                '*' => { has_wildcard = true; i += 1; }
                '.' | '?' => { dot_count += 1; i += 1; }
                '[' => {
                    dot_count += 1;
                    i += 1;
                    while i < anagram_chars.len() && anagram_chars[i] != ']' { i += 1; }
                    if i < anagram_chars.len() { i += 1; }
                }
                c if c.is_alphabetic() => { anagram_letters.push(c.to_ascii_lowercase()); i += 1; }
                _ => { i += 1; }
            }
        }

        let dots = if dot_count > 0 { Some(dot_count) } else { None };

        if template_part.is_empty() {
            return Some(Pattern::Anagram(anagram_letters, dots, has_wildcard));
        } else {
            let template = parse_template(template_part);
            return Some(Pattern::TemplateWithAnagram(template, anagram_letters, dots));
        }
    }

    Some(Pattern::Template(parse_template(input)))
}

fn parse_template(s: &str) -> Vec<TemplateChar> {
    let mut result = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '.' | '?' => { result.push(TemplateChar::Any); i += 1; }
            '*' => { result.push(TemplateChar::Wildcard); i += 1; }
            '[' => {
                i += 1;
                let negated = i < chars.len() && chars[i] == '^';
                if negated { i += 1; }
                let mut letters = Vec::new();
                while i < chars.len() && chars[i] != ']' {
                    if chars[i].is_alphabetic() {
                        letters.push(chars[i].to_ascii_lowercase());
                    }
                    i += 1;
                }
                if i < chars.len() { i += 1; }
                result.push(TemplateChar::ChoiceList(letters, negated));
            }
            c if c.is_ascii_digit() => {
                result.push(TemplateChar::Variable(c as u8 - b'0'));
                i += 1;
            }
            c => { result.push(TemplateChar::Literal(c.to_ascii_lowercase())); i += 1; }
        }
    }
    result
}

fn template_fixed_len(template: &[TemplateChar]) -> usize {
    template.iter().filter(|t| !matches!(t, TemplateChar::Wildcard)).count()
}

// ── Template matching ─────────────────────────────────────────────────────────

fn char_matches(ch: char, t: &TemplateChar, ctx: &mut MatchContext) -> bool {
    match t {
        TemplateChar::Literal(c) => *c == ch,
        TemplateChar::Any => true,
        TemplateChar::Wildcard => unreachable!(),
        TemplateChar::ChoiceList(letters, negated) => {
            let contains = letters.contains(&ch);
            if *negated { !contains } else { contains }
        }
        TemplateChar::Variable(v) => ctx.bind(*v, ch),
    }
}

fn matches_template(word: &str, template: &[TemplateChar]) -> bool {
    let word_chars: Vec<char> = word.chars().collect();
    let has_wildcard = template.iter().any(|t| matches!(t, TemplateChar::Wildcard));
    let mut ctx = MatchContext::new();

    if !has_wildcard {
        if word_chars.len() != template.len() { return false; }
        return template.iter().zip(word_chars.iter()).all(|(t, &w)| char_matches(w, t, &mut ctx));
    }

    matches_template_wildcard(&word_chars, template, &mut ctx)
}

fn matches_template_wildcard(word: &[char], template: &[TemplateChar], ctx: &mut MatchContext) -> bool {
    if template.is_empty() { return word.is_empty(); }

    match &template[0] {
        TemplateChar::Wildcard => {
            for i in 0..=word.len() {
                let mut ctx_clone = ctx.clone();
                if matches_template_wildcard(&word[i..], &template[1..], &mut ctx_clone) {
                    *ctx = ctx_clone;
                    return true;
                }
            }
            false
        }
        t => {
            if word.is_empty() { return false; }
            if char_matches(word[0], t, ctx) {
                matches_template_wildcard(&word[1..], &template[1..], ctx)
            } else {
                false
            }
        }
    }
}

// ── Anagram matching ──────────────────────────────────────────────────────────

fn matches_anagram_exact(
    word: &str,
    letters: &[char],
    dot_count: Option<usize>,
    has_wildcard: bool,
) -> Option<String> {
    let word_chars: Vec<char> = word.chars().collect();

    if !has_wildcard {
        let expected_len = letters.len() + dot_count.unwrap_or(0);
        if word_chars.len() != expected_len { return None; }
    }

    let mut available: HashMap<char, i32> = HashMap::new();
    for &ch in letters { *available.entry(ch).or_insert(0) += 1; }

    let mut needed: HashMap<char, i32> = HashMap::new();
    for &ch in &word_chars {
        let avail = available.entry(ch).or_insert(0);
        if *avail > 0 { *avail -= 1; }
        else { *needed.entry(ch).or_insert(0) += 1; }
    }

    let missing_required: i32 = available.values().filter(|&&v| v > 0).map(|&v| v).sum();
    if missing_required > 0 { return None; }

    let extra_count: i32 = needed.values().sum();
    if !has_wildcard {
        let blanks_available = dot_count.unwrap_or(0) as i32;
        if extra_count > blanks_available { return None; }
    }

    let mut omitted: Vec<char> = available
        .iter()
        .filter(|(_, &v)| v > 0)
        .flat_map(|(&ch, &count)| std::iter::repeat(ch).take(count as usize))
        .collect();
    omitted.sort();

    let mut added: Vec<char> = needed
        .iter()
        .flat_map(|(&ch, &count)| std::iter::repeat(ch).take(count as usize))
        .collect();
    added.sort();

    let mut balance = String::new();
    if !omitted.is_empty() {
        balance.push('-');
        for ch in &omitted { balance.push(ch.to_ascii_uppercase()); }
    }
    if !added.is_empty() {
        balance.push('+');
        for ch in &added { balance.push(ch.to_ascii_uppercase()); }
    }
    Some(balance)
}

fn matches_anagram_within(
    word: &str,
    letters: &[char],
    dot_count: Option<usize>,
) -> Option<String> {
    let word_chars: Vec<char> = word.chars().collect();

    let mut available: HashMap<char, i32> = HashMap::new();
    for &ch in letters { *available.entry(ch).or_insert(0) += 1; }

    let mut extra: Vec<char> = Vec::new();
    for &ch in &word_chars {
        let avail = available.entry(ch).or_insert(0);
        if *avail > 0 { *avail -= 1; }
        else { extra.push(ch); }
    }

    for &remaining in available.values() {
        if remaining > 0 { return None; }
    }

    if extra.len() > dot_count.unwrap_or(0) { return None; }

    extra.sort();
    let balance = if extra.is_empty() {
        String::new()
    } else {
        let mut s = String::from("+");
        for ch in &extra { s.push(ch.to_ascii_uppercase()); }
        s
    };
    Some(balance)
}

// ── Search ────────────────────────────────────────────────────────────────────

fn matching_form(word: &str, normalize_mode: bool) -> String {
    if normalize_mode { normalize(word) } else { word.to_ascii_lowercase() }
}

/// Evaluate a logical expression against a single word.
/// Returns Some(balance) if the word matches, None if it doesn't.
fn eval_expr(word: &str, word_len: usize, expr: &LogicalExpr) -> Option<String> {
    match expr {
        LogicalExpr::Single(pattern) => eval_pattern(word, word_len, pattern),
        LogicalExpr::And(left, right) => {
            eval_expr(word, word_len, left)?;
            eval_expr(word, word_len, right)
        }
        LogicalExpr::Or(left, right) => {
            eval_expr(word, word_len, left)
                .or_else(|| eval_expr(word, word_len, right))
        }
        LogicalExpr::Not(inner) => {
            if eval_expr(word, word_len, inner).is_some() {
                None
            } else {
                Some(String::new())
            }
        }
    }
}

fn eval_pattern(word: &str, word_len: usize, pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Template(template) => {
            if matches_template(word, template) { Some(String::new()) } else { None }
        }
        Pattern::Anagram(letters, dots, has_wildcard) => {
            matches_anagram_exact(word, letters, *dots, *has_wildcard)
        }
        Pattern::TemplateWithAnagram(template, letters, dots) => {
            let has_wildcard = template.iter().any(|t| matches!(t, TemplateChar::Wildcard));
            let length_ok = if has_wildcard {
                true
            } else {
                word_len == template_fixed_len(template)
            };
            if length_ok && matches_template(word, template) {
                let free_positions = if has_wildcard {
                    word_len.saturating_sub(letters.len())
                } else {
                    template.iter()
                        .filter(|t| !matches!(t, TemplateChar::Literal(_)))
                        .count()
                };
                let effective_dots = Some(free_positions + dots.unwrap_or(0));
                matches_anagram_within(word, letters, effective_dots)
            } else {
                None
            }
        }
    }
}

fn search(
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
        if word_len < min_len || word_len > max_len { continue; }

        if let Some(balance_str) = eval_expr(&matched_form, word_len, expr) {
            raw.push(RawMatch {
                original: word.clone(),
                normalized_key: matched_form,
                balance: if balance_str.is_empty() { None } else { Some(balance_str) },
            });
        }
    }

    // Group by normalized key
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, MatchGroup> = HashMap::new();

    for raw_match in raw {
        let key = raw_match.normalized_key.clone();
        if let Some(group) = groups.get_mut(&key) {
            let original_lower = raw_match.original.to_ascii_lowercase();
            if original_lower != key { group.variants.push(raw_match.original); }
        } else {
            group_order.push(key.clone());
            let original_lower = raw_match.original.to_ascii_lowercase();
            let variants = if original_lower != key { vec![raw_match.original] } else { vec![] };
            groups.insert(key.clone(), MatchGroup { normalized: key, variants, balance: raw_match.balance });
        }
    }

    let mut result: Vec<MatchGroup> = group_order
        .into_iter()
        .filter_map(|k| groups.remove(&k))
        .collect();

    result.sort_by(|a, b| {
        a.normalized.len().cmp(&b.normalized.len()).then(a.normalized.cmp(&b.normalized))
    });

    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn word_list() -> Vec<String> {
        vec![
            // Core test words
            "electron", "canter", "nectar", "recant", "trance",
            "aardvark", "elephant", "cat", "act", "arc",
            "drinker", "beside", "bodice", "edible",
            "maharaja", "quick", "quack", "quirk", "quark",
            "escalator", "explorer's", "Escargots", "escargots", "escargot's",
            "catch-22", "escapists", "ultra",
            // Choice list tests
            "arts", "rest", "rust", "sort", "star", "stir",
            "llama", "lynch", "lymph", "lyric",
            "yoga", "zinc",
            // Letter variable tests — palindromes and tautonyms
            "level", "radar", "civic", "refer", "repaper",
            "murmur", "beriberi",
            // Logical op tests
            "cats", "cast", "scat", "copycats", "scatter",
            "carbon", "carrot", "catch",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    // Helper: run search_words with defaults
    fn sw(pattern: &str) -> Vec<MatchGroup> {
        search_words(&word_list(), pattern, 1, 50, true)
    }

    fn keys(results: &[MatchGroup]) -> Vec<&str> {
        results.iter().map(|r| r.normalized.as_str()).collect()
    }

    // ── Template ──────────────────────────────────────────────────────────

    #[test]
    fn test_template_basic() {
        let r = sw(".l...r.n");
        assert!(keys(&r).contains(&"electron"));
    }

    #[test]
    fn test_template_question_marks() {
        let r = sw("q???k");
        let k = keys(&r);
        assert!(k.contains(&"quack"));
        assert!(k.contains(&"quick"));
        assert!(k.contains(&"quirk"));
        assert!(k.contains(&"quark"));
    }

    #[test]
    fn test_template_length_exact() {
        // Only 3-letter words
        let r = sw("...");
        for result in &r { assert_eq!(result.normalized.len(), 3); }
    }

    // ── Wildcard ──────────────────────────────────────────────────────────

    #[test]
    fn test_wildcard_basic() {
        let r = sw("m*ja");
        assert!(keys(&r).contains(&"maharaja"));
    }

    #[test]
    fn test_wildcard_start() {
        // words starting with 'e', any length
        let r = sw("e*");
        assert!(keys(&r).contains(&"electron"));
        assert!(keys(&r).contains(&"escalator"));
        assert!(keys(&r).contains(&"elephant"));
    }

    #[test]
    fn test_wildcard_end() {
        let r = sw("*t");
        for result in &r { assert!(result.normalized.ends_with('t')); }
    }

    // ── Anagram ───────────────────────────────────────────────────────────

    #[test]
    fn test_anagram_exact() {
        let r = sw(";acenrt");
        let k = keys(&r);
        assert!(k.contains(&"canter"));
        assert!(k.contains(&"nectar"));
        assert!(k.contains(&"recant"));
        assert!(k.contains(&"trance"));
        assert_eq!(r.len(), 4);
    }

    #[test]
    fn test_anagram_with_blank() {
        let r = sw(";eiknrr.");
        let drinker = r.iter().find(|r| r.normalized == "drinker");
        assert!(drinker.is_some());
        assert_eq!(drinker.unwrap().balance, Some("+D".to_string()));
    }

    #[test]
    fn test_anagram_wildcard() {
        let r = sw(";cats*");
        let k = keys(&r);
        assert!(k.contains(&"escalator"), "should find escalator");
        assert!(k.contains(&"escapists"), "should find escapists");
        assert!(r.len() >= 2);
    }

    // ── Template + anagram ────────────────────────────────────────────────

    #[test]
    fn test_template_with_anagram_basic() {
        let r = sw("e........;cats");
        assert!(keys(&r).contains(&"escalator"));
    }

    #[test]
    fn test_template_with_anagram_balance() {
        let r = sw("e........;cats");
        let escapists = r.iter().find(|r| r.normalized == "escapists");
        assert!(escapists.is_some());
        assert!(escapists.unwrap().balance.as_deref().unwrap_or("").starts_with('+'));
    }

    #[test]
    fn test_template_with_anagram_length_enforced() {
        let r = sw("e........;cats");
        for result in &r {
            assert_eq!(result.normalized.len(), 9,
                "wrong length: {}", result.normalized);
        }
    }

    #[test]
    fn test_wildcard_with_anagram() {
        let r = sw("e*;cats");
        let k = keys(&r);
        assert!(k.contains(&"escalator"));
        assert!(k.contains(&"escapists"));
    }

    // ── Choice lists ──────────────────────────────────────────────────────

    #[test]
    fn test_choice_list_vowel_start() {
        let r = sw("[aeiou]....");
        assert!(!r.is_empty());
        for result in &r {
            assert_eq!(result.normalized.len(), 5);
            assert!("aeiou".contains(result.normalized.chars().next().unwrap()),
                "should start with vowel: {}", result.normalized);
        }
    }

    #[test]
    fn test_choice_list_negated() {
        let r = sw("[^aeiou]...");
        assert!(!r.is_empty());
        for result in &r {
            assert_eq!(result.normalized.len(), 4);
            assert!(!"aeiou".contains(result.normalized.chars().next().unwrap()),
                "should start with consonant: {}", result.normalized);
        }
    }

    #[test]
    fn test_choice_list_middle() {
        let r = sw(".[aeiou].");
        assert!(!r.is_empty());
        for result in &r {
            assert_eq!(result.normalized.len(), 3);
            assert!("aeiou".contains(result.normalized.chars().nth(1).unwrap()));
        }
    }

    #[test]
    fn test_choice_list_end() {
        let r = sw("....[ck]");
        assert!(!r.is_empty());
        for result in &r {
            assert_eq!(result.normalized.len(), 5);
            let last = result.normalized.chars().last().unwrap();
            assert!("ck".contains(last), "should end in c or k: {}", result.normalized);
        }
    }

    #[test]
    fn test_choice_list_in_anagram() {
        let r = sw(";str[aeiou]");
        assert!(!r.is_empty());
        for result in &r {
            assert_eq!(result.normalized.len(), 4,
                "wrong length: {}", result.normalized);
        }
    }

    #[test]
    fn test_choice_list_with_template_and_anagram() {
        let r = sw("[ea]......;rct");
        for result in &r {
            assert_eq!(result.normalized.len(), 8);
            assert!("ea".contains(result.normalized.chars().next().unwrap()));
            assert!(result.normalized.contains('r'));
            assert!(result.normalized.contains('c'));
            assert!(result.normalized.contains('t'));
        }
    }

    // ── Wildcard × choice list ────────────────────────────────────────────

    #[test]
    fn test_wildcard_with_choice_list() {
        // words starting with vowel, ending in t — "art" qualifies (a + r + t)
        let r = sw("[aeiou]*t");
        assert!(!r.is_empty());
        for result in &r {
            assert!("aeiou".contains(result.normalized.chars().next().unwrap()),
                "should start with vowel: {}", result.normalized);
            assert!(result.normalized.ends_with('t'),
                "should end with t: {}", result.normalized);
        }
    }

    #[test]
    fn test_choice_list_with_wildcard() {
        // words starting with l then a consonant
        let r = sw("l[^aeiou]*");
        assert!(!r.is_empty());
        for result in &r {
            assert!(result.normalized.starts_with('l'));
            let second = result.normalized.chars().nth(1).unwrap();
            assert!(!"aeiou".contains(second),
                "second letter should be consonant: {}", result.normalized);
        }
    }

    // ── Anagram wildcard × template/choice ───────────────────────────────

    #[test]
    fn test_anagram_wildcard_with_template() {
        // e-starting words containing c, a, t, s
        let r = sw("e*;cats*");
        assert!(!r.is_empty());
        for result in &r {
            assert!(result.normalized.starts_with('e'));
            assert!(result.normalized.contains('c'));
            assert!(result.normalized.contains('a'));
            assert!(result.normalized.contains('t'));
            assert!(result.normalized.contains('s'));
        }
    }

    #[test]
    fn test_choice_list_with_anagram_wildcard() {
        // anagram of str + any vowel + any extras
        let r = sw(";str[aeiou]*");
        assert!(!r.is_empty());
        for result in &r {
            assert!(result.normalized.contains('s'));
            assert!(result.normalized.contains('t'));
            assert!(result.normalized.contains('r'));
        }
    }

    // ── Macros ────────────────────────────────────────────────────────────

    #[test]
    fn test_macro_at_vowel_template() {
        // @ = [aeiou], so @.... = 5-letter words starting with vowel
        // same as [aeiou]....
        let r_macro = sw("@....");
        let r_explicit = sw("[aeiou]....");
        let k_macro: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
        let k_explicit: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
        assert_eq!(k_macro, k_explicit, "@ should expand to [aeiou]");
    }

    #[test]
    fn test_macro_hash_consonant_template() {
        // # = [^aeiou], so #... = 4-letter words starting with consonant
        let r_macro = sw("#...");
        let r_explicit = sw("[^aeiou]...");
        let k_macro: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
        let k_explicit: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
        assert_eq!(k_macro, k_explicit, "# should expand to [^aeiou]");
    }

    #[test]
    fn test_macro_in_anagram() {
        // ;str@ = anagram of str + any vowel (same as ;str[aeiou])
        let r_macro = sw(";str@");
        let r_explicit = sw(";str[aeiou]");
        let k_macro: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
        let k_explicit: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
        assert_eq!(k_macro, k_explicit);
    }

    #[test]
    fn test_macro_multiple_in_pattern() {
        // @#@# = vowel consonant vowel consonant — 4-letter words
        // e.g. "arts" = a(v) r(c) t(c)... no. "yoga" = y(c) o(v)... no
        // "acts" = a(v) c(c) t(c) s(c) — only 1 vowel. "arcs" = a(v) r(c) c(c) s(c)
        // Let's use @# = vowel+consonant 2-letter — too short for our words
        // Use @#@# = 4 chars starting vowel,consonant,vowel,consonant
        // "arts" doesn't fit. Use @@## = 2 vowels then 2 consonants
        // "arts" = a,r,t,s — 1 vowel only. Need words with 2 vowels first.
        // Let's just verify it produces only words matching the pattern
        let r = sw("@#@#");
        for result in &r {
            assert_eq!(result.normalized.len(), 4);
            let cs: Vec<char> = result.normalized.chars().collect();
            assert!("aeiou".contains(cs[0]), "pos 0 should be vowel: {}", result.normalized);
            assert!(!"aeiou".contains(cs[1]), "pos 1 should be consonant: {}", result.normalized);
            assert!("aeiou".contains(cs[2]), "pos 2 should be vowel: {}", result.normalized);
            assert!(!"aeiou".contains(cs[3]), "pos 3 should be consonant: {}", result.normalized);
        }
    }

    #[test]
    fn test_macro_with_wildcard() {
        // @* = words starting with vowel, any length
        let r = sw("@*");
        assert!(!r.is_empty());
        for result in &r {
            assert!("aeiou".contains(result.normalized.chars().next().unwrap()),
                "should start with vowel: {}", result.normalized);
        }
    }

    #[test]
    fn test_macro_with_letter_variable() {
        // @1..1 = vowel start, same letter at positions 2 and 5
        // "level" = l(c) — doesn't start with vowel
        // Just verify it doesn't crash and filters correctly
        let r = sw("@1..1");
        for result in &r {
            assert_eq!(result.normalized.len(), 5);
            assert!("aeiou".contains(result.normalized.chars().next().unwrap()));
            let cs: Vec<char> = result.normalized.chars().collect();
            assert_eq!(cs[1], cs[4], "positions 2 and 5 should match: {}", result.normalized);
        }
    }

    #[test]
    fn test_macro_with_anagram_wildcard() {
        // ;str@* = anagram of str + any vowel + any extra letters
        let r = sw(";str@*");
        assert!(!r.is_empty());
        for result in &r {
            assert!(result.normalized.contains('s'));
            assert!(result.normalized.contains('t'));
            assert!(result.normalized.contains('r'));
        }
    }

    // ── Letter variables ──────────────────────────────────────────────────

    #[test]
    fn test_letter_variable_palindrome_5() {
        // 12321 = 5-letter palindrome
        // "level" = l,e,v,e,l ✓  "radar" = r,a,d,a,r ✓  "civic" = c,i,v,i,c ✓
        let r = sw("12321");
        let k = keys(&r);
        assert!(k.contains(&"level"), "should find level");
        assert!(k.contains(&"radar"), "should find radar");
        assert!(k.contains(&"civic"), "should find civic");
        for result in &r {
            let cs: Vec<char> = result.normalized.chars().collect();
            assert_eq!(cs[0], cs[4], "pos 0==4 in {}", result.normalized);
            assert_eq!(cs[1], cs[3], "pos 1==3 in {}", result.normalized);
        }
    }

    #[test]
    fn test_letter_variable_palindrome_7() {
        // 1234321 = 7-letter palindrome — "repaper" = r,e,p,a,p,e,r ✓
        let r = sw("1234321");
        let k = keys(&r);
        assert!(k.contains(&"repaper"), "should find repaper");
        for result in &r {
            let cs: Vec<char> = result.normalized.chars().collect();
            assert_eq!(cs[0], cs[6]);
            assert_eq!(cs[1], cs[5]);
            assert_eq!(cs[2], cs[4]);
        }
    }

    #[test]
    fn test_letter_variable_same_first_last() {
        // 1..1 = 4-letter words where first == last
        // "murmur" is 6 letters, need 4-letter. "that","deed","noon" not in list
        // "refer" is 5 letters. "civic" is 5. "cats" c!=s. "cast" c!=t
        // Let's check what we have: need a 4-letter word where first==last
        // "noon", "deed", "that" not in list. Let's add "radar"... that's 5
        // Actually test with 5-letter: 1...1
        let r = sw("1...1");
        assert!(!r.is_empty(), "should find words where first==last");
        for result in &r {
            assert_eq!(result.normalized.len(), 5);
            let cs: Vec<char> = result.normalized.chars().collect();
            assert_eq!(cs[0], cs[4], "first should equal last: {}", result.normalized);
        }
    }

    #[test]
    fn test_letter_variable_tautonym() {
        // 123123 = 6-letter tautonym (first half repeats)
        // "murmur" = m,u,r,m,u,r ✓
        let r = sw("123123");
        let k = keys(&r);
        assert!(k.contains(&"murmur"), "should find murmur");
        for result in &r {
            let cs: Vec<char> = result.normalized.chars().collect();
            assert_eq!(result.normalized.len(), 6);
            assert_eq!(cs[0], cs[3]);
            assert_eq!(cs[1], cs[4]);
            assert_eq!(cs[2], cs[5]);
        }
    }

    #[test]
    fn test_letter_variable_with_wildcard() {
        // 1*1 = words starting and ending with same letter, any length
        let r = sw("1*1");
        assert!(!r.is_empty());
        for result in &r {
            let cs: Vec<char> = result.normalized.chars().collect();
            assert!(cs.len() >= 1);
            assert_eq!(cs[0], *cs.last().unwrap(),
                "first should equal last: {}", result.normalized);
        }
    }

    #[test]
    fn test_letter_variable_with_choice_list() {
        // [aeiou]1..1 = 5-letter, starts with vowel, pos 2==pos 5
        let r = sw("[aeiou]1..1");
        for result in &r {
            assert_eq!(result.normalized.len(), 5);
            assert!("aeiou".contains(result.normalized.chars().next().unwrap()));
            let cs: Vec<char> = result.normalized.chars().collect();
            assert_eq!(cs[1], cs[4], "pos 1==4 in {}", result.normalized);
        }
    }

    #[test]
    fn test_letter_variable_with_macro() {
        // @1..1 = vowel start, pos 2==pos 5 — same as [aeiou]1..1
        let r_macro = sw("@1..1");
        let r_explicit = sw("[aeiou]1..1");
        let k_m: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
        let k_e: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
        assert_eq!(k_m, k_e);
    }

    #[test]
    fn test_letter_variable_with_anagram() {
        // 1..1;cat = 4-letter template (first==last) containing c,a,t
        // Need a 4-letter word where first==last and contains c,a,t
        // Not likely in our small list — verify no crash, correct structure
        let r = sw("1..1;cat");
        for result in &r {
            assert_eq!(result.normalized.len(), 4);
            let cs: Vec<char> = result.normalized.chars().collect();
            assert_eq!(cs[0], cs[3]);
            assert!(result.normalized.contains('c'));
            assert!(result.normalized.contains('a'));
            assert!(result.normalized.contains('t'));
        }
    }

    // ── Logical operations ────────────────────────────────────────────────

    #[test]
    fn test_logical_and_basic() {
        // c* & *s = words starting with c AND ending with s
        // "cats" starts with c, ends with s ✓
        let r = sw("c* & *s");
        assert!(!r.is_empty());
        for result in &r {
            assert!(result.normalized.starts_with('c'),
                "should start with c: {}", result.normalized);
            assert!(result.normalized.ends_with('s'),
                "should end with s: {}", result.normalized);
        }
    }

    #[test]
    fn test_logical_or_basic() {
        // c... | ...s = 4-letter words starting with c OR ending with s
        let r = sw("c... | ...s");
        assert!(!r.is_empty());
        for result in &r {
            assert_eq!(result.normalized.len(), 4);
            let starts_c = result.normalized.starts_with('c');
            let ends_s = result.normalized.ends_with('s');
            assert!(starts_c || ends_s,
                "should start with c or end with s: {}", result.normalized);
        }
    }

    #[test]
    fn test_logical_not_basic() {
        // c* & !cat* = words starting with c but NOT starting with cat
        let r = sw("c* & !cat*");
        assert!(!r.is_empty());
        for result in &r {
            assert!(result.normalized.starts_with('c'));
            assert!(!result.normalized.starts_with("cat"),
                "should not start with cat: {}", result.normalized);
        }
    }

    #[test]
    fn test_logical_grouped_or() {
        // (c* | *r) & ....  = 4-letter words starting with c or ending with r
        let r = sw("(c... | ...r)");
        assert!(!r.is_empty());
        for result in &r {
            assert_eq!(result.normalized.len(), 4);
            let starts_c = result.normalized.starts_with('c');
            let ends_r = result.normalized.ends_with('r');
            assert!(starts_c || ends_r,
                "should start with c or end with r: {}", result.normalized);
        }
    }

    #[test]
    fn test_logical_and_with_anagram() {
        // ;cats & c* = anagrams of cats that start with c
        // "cats" and "cast" and "scat" — only cats,cast start with c
        let r = sw(";cats & c*");
        assert!(!r.is_empty());
        for result in &r {
            assert!(result.normalized.starts_with('c'),
                "should start with c: {}", result.normalized);
            // must contain c,a,t,s
            assert!(result.normalized.contains('c'));
            assert!(result.normalized.contains('a'));
            assert!(result.normalized.contains('t'));
            assert!(result.normalized.contains('s'));
        }
    }

    #[test]
    fn test_logical_and_with_wildcard() {
        // c* & *s = words starting with c, ending with s, any length
        let r = sw("c* & *s");
        for result in &r {
            assert!(result.normalized.starts_with('c'));
            assert!(result.normalized.ends_with('s'));
        }
    }

    #[test]
    fn test_logical_and_with_choice_list() {
        // [aeiou]... & *t = 4-letter vowel-start words ending in t
        // "arts" starts with a(vowel), ends in s. "rest" starts with r(consonant).
        // Need vowel-start + ends-in-t. Not guaranteed in word list but verify structure
        let r = sw("[aeiou]... & *t");
        for result in &r {
            assert_eq!(result.normalized.len(), 4);
            assert!("aeiou".contains(result.normalized.chars().next().unwrap()));
            assert!(result.normalized.ends_with('t'));
        }
    }

    #[test]
    fn test_logical_and_with_macro() {
        // @... & *t = same as above with macro
        let r_macro = sw("@... & *t");
        let r_explicit = sw("[aeiou]... & *t");
        let k_m: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
        let k_e: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
        assert_eq!(k_m, k_e);
    }

    #[test]
    fn test_logical_and_with_letter_variable() {
        // 1..1 & c* = 4-letter words where first==last AND starts with c
        // "civic" is 5 letters. Need 4-letter c...c — not in list likely
        // Just verify structure
        let r = sw("1..1 & c*");
        for result in &r {
            assert_eq!(result.normalized.len(), 4);
            assert!(result.normalized.starts_with('c'));
            let cs: Vec<char> = result.normalized.chars().collect();
            assert_eq!(cs[0], cs[3]);
        }
    }

    #[test]
    fn test_logical_or_with_anagram() {
        // ;cats | ;dogs = anagrams of cats OR dogs
        // dogs not in word list anagram-wise but cats/cast/scat are
        let r = sw(";cats | ;arts");
        let _k = keys(&r);
        // should find cats,cast,scat (anagrams of cats) and arts,rats,star,tars,etc
        assert!(!r.is_empty());
        // all results should be anagrams of either cats or arts
        for result in &r {
            let is_cats_anagram = {
                let mut chars: Vec<char> = result.normalized.chars().collect();
                chars.sort();
                chars == vec!['a', 'c', 's', 't']
            };
            let is_arts_anagram = {
                let mut chars: Vec<char> = result.normalized.chars().collect();
                chars.sort();
                chars == vec!['a', 'r', 's', 't']
            };
            assert!(is_cats_anagram || is_arts_anagram,
                "should be anagram of cats or arts: {}", result.normalized);
        }
    }

    #[test]
    fn test_logical_not_with_wildcard() {
        // c* & !*s = words starting with c but NOT ending with s
        let r = sw("c* & !*s");
        for result in &r {
            assert!(result.normalized.starts_with('c'));
            assert!(!result.normalized.ends_with('s'),
                "should not end with s: {}", result.normalized);
        }
    }

    #[test]
    fn test_logical_complex_grouped() {
        // (c* | *r) & .... = 4-letter words starting with c OR ending with r
        let r = sw("(c* | *r) & ....");
        assert!(!r.is_empty());
        for result in &r {
            assert_eq!(result.normalized.len(), 4);
            let starts_c = result.normalized.starts_with('c');
            let ends_r = result.normalized.ends_with('r');
            assert!(starts_c || ends_r);
        }
    }

    // ── Normalization / deduplication ─────────────────────────────────────

    #[test]
    fn test_deduplication_groups_variants() {
        let r = sw("e........");
        let escargots = r.iter().find(|r| r.normalized == "escargots");
        assert!(escargots.is_some());
        assert_eq!(escargots.unwrap().variants.len(), 1);
        assert!(escargots.unwrap().variants.contains(&"escargot's".to_string()));
    }

    #[test]
    fn test_normalize_off_no_grouping() {
        let r = search_words(&word_list(), "e........", 1, 50, false);
        for result in &r { assert!(result.variants.is_empty()); }
    }

    #[test]
    fn test_sort_by_length() {
        let r = sw(".*");
        for i in 1..r.len() {
            assert!(r[i].normalized.len() >= r[i-1].normalized.len());
        }
    }

    // ── Public API ────────────────────────────────────────────────────────

    #[test]
    fn test_validate_pattern_valid() {
        assert!(validate_pattern(";acenrt").is_ok());
        assert!(validate_pattern(".l...r.n").is_ok());
        assert!(validate_pattern("c* & !cat*").is_ok());
        assert!(validate_pattern("@....").is_ok());
        assert!(validate_pattern("12321").is_ok());
    }

    #[test]
    fn test_validate_pattern_empty() {
        assert!(validate_pattern("").is_err());
        assert!(validate_pattern("   ").is_err());
    }

    #[test]
    fn test_describe_pattern_template() {
        let d = describe_pattern(".l...r.n").unwrap();
        assert!(d.contains("8"), "should mention 8 letters: {}", d);
    }

    #[test]
    fn test_describe_pattern_anagram() {
        let d = describe_pattern(";acenrt").unwrap();
        assert!(d.to_lowercase().contains("anagram"), "should say anagram: {}", d);
        assert!(d.contains("ACENRT") || d.contains("acenrt"), "should mention letters: {}", d);
    }

    #[test]
    fn test_describe_pattern_empty() {
        assert!(describe_pattern("").is_none());
    }

    #[test]
    fn test_describe_pattern_logical_stub() {
        let d = describe_pattern("c* & !cat*").unwrap();
        assert_eq!(d, "Complex pattern");
    }

    #[test]
    fn test_describe_pattern_macro() {
        let d = describe_pattern("@....").unwrap();
        // Should describe as vowel-start 5-letter words
        assert!(d.contains("5") || d.contains("vowel"), "should describe macro: {}", d);
    }
}
