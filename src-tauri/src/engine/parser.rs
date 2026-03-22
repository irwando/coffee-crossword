// ── Parser ────────────────────────────────────────────────────────────────────
// Turns raw pattern strings into LogicalExpr / Pattern AST nodes.
// All helpers below parse_logical are private — callers use parse_logical only.

use crate::engine::ast::{AnagramChar, LogicalExpr, Pattern, SubPattern, TemplateChar};

/// Expand @ and # macros before any other parsing.
/// pub(crate) because describe.rs also needs to expand macros before
/// describing a pattern.
pub(crate) fn expand_macros(input: &str) -> String {
    input.replace('@', "[aeiou]").replace('#', "[^aeiou]")
}

/// Parse a raw pattern string into a LogicalExpr tree.
/// This is the main entry point for all pattern parsing.
/// pub(crate) — called from mod.rs (search_words, validate_pattern)
/// and describe.rs (describe_pattern).
pub(crate) fn parse_logical(input: &str) -> Option<LogicalExpr> {
    let expanded = expand_macros(input.trim());
    let input = expanded.trim();
    if input.is_empty() {
        return None;
    }
    parse_or(input)
}

/// Parse OR expressions (lowest precedence)
fn parse_or(input: &str) -> Option<LogicalExpr> {
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

/// Parse a single pattern or parenthesized logical group.
fn parse_atom(input: &str) -> Option<LogicalExpr> {
    let input = input.trim();
    // Only treat as logical grouping if the parens wrap the entire expression
    // and the contents contain logical operators
    if input.starts_with('(') && input.ends_with(')') {
        let inner = &input[1..input.len() - 1];
        if inner.contains(" & ") || inner.contains(" | ") {
            if let Some(expr) = parse_or(inner) {
                return Some(expr);
            }
        }
    }
    let pattern = parse_pattern(input)?;
    Some(LogicalExpr::Single(pattern))
}

/// Split input on a logical operator character, respecting brackets, parens,
/// and sub-patterns.
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

/// Parse a single pattern (no logical operators).
pub(crate) fn parse_pattern(input: &str) -> Option<Pattern> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    if let Some(semi_pos) = find_anagram_semi(input) {
        let template_part = &input[..semi_pos];
        let anagram_part = &input[semi_pos + 1..];

        let (anagram_chars, dot_count, has_wildcard) = parse_anagram_part(anagram_part);
        let dots = if dot_count > 0 { Some(dot_count) } else { None };

        if template_part.is_empty() {
            return Some(Pattern::Anagram(anagram_chars, dots, has_wildcard));
        } else {
            let template = parse_template(template_part);
            return Some(Pattern::TemplateWithAnagram(template, anagram_chars, dots));
        }
    }

    Some(Pattern::Template(parse_template(input)))
}

/// Find the position of the anagram semicolon in a pattern.
/// Must not be inside brackets or parens.
fn find_anagram_semi(input: &str) -> Option<usize> {
    let mut depth_bracket = 0i32;
    let mut depth_paren = 0i32;
    for (i, ch) in input.char_indices() {
        match ch {
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            ';' if depth_bracket == 0 && depth_paren == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Parse the anagram part of a pattern (after the semicolon).
/// Returns (anagram_chars, plain_dot_count, has_wildcard).
fn parse_anagram_part(anagram_part: &str) -> (Vec<AnagramChar>, usize, bool) {
    let mut anagram_chars: Vec<AnagramChar> = Vec::new();
    let mut dot_count = 0usize;
    let mut has_wildcard = false;

    let chars: Vec<char> = anagram_part.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                has_wildcard = true;
                i += 1;
            }
            '.' | '?' => {
                dot_count += 1;
                anagram_chars.push(AnagramChar::Blank);
                i += 1;
            }
            '[' => {
                // Choice list tracked as AnagramChar::ChoiceList, not as a blank slot
                i += 1;
                let negated = i < chars.len() && chars[i] == '^';
                if negated {
                    i += 1;
                }
                let mut letters = Vec::new();
                while i < chars.len() && chars[i] != ']' {
                    if chars[i].is_alphabetic() {
                        letters.push(chars[i].to_ascii_lowercase());
                    }
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }
                anagram_chars.push(AnagramChar::ChoiceList(letters, negated));
            }
            '(' => {
                // Sub-pattern inside anagram
                i += 1;
                let is_anagram_sub = i < chars.len() && chars[i] == ';';
                if is_anagram_sub {
                    i += 1;
                }
                let mut sub_chars = Vec::new();
                let mut depth = 1i32;
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '(' => { depth += 1; sub_chars.push(chars[i]); }
                        ')' => {
                            depth -= 1;
                            if depth > 0 {
                                sub_chars.push(chars[i]);
                            }
                        }
                        c => sub_chars.push(c),
                    }
                    i += 1;
                }
                let sub_str: String = sub_chars.into_iter().collect();
                if is_anagram_sub {
                    let letters: Vec<char> = sub_str
                        .chars()
                        .filter(|c| c.is_alphabetic())
                        .map(|c| c.to_ascii_lowercase())
                        .collect();
                    anagram_chars.push(AnagramChar::SubPattern(SubPattern::AnagramInAnagram(letters)));
                } else {
                    let template_chars = parse_template(&sub_str);
                    anagram_chars.push(AnagramChar::SubPattern(SubPattern::Template(template_chars)));
                }
            }
            c if c.is_alphabetic() => {
                anagram_chars.push(AnagramChar::Letter(c.to_ascii_lowercase()));
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    (anagram_chars, dot_count, has_wildcard)
}

/// Returns true if a character has special meaning in patterns and must be
/// escaped with \ to be matched literally.
fn is_metacharacter(ch: char) -> bool {
    matches!(ch, '.' | '?' | '*' | ';' | '[' | ']' | '^' | '(' | ')' | '&' | '|' | '!' | '@' | '#' | '0'..='9')
}

/// Parse a template string into a Vec<TemplateChar>.
/// pub(crate) — used by parse_pattern and potentially tests.
pub(crate) fn parse_template(s: &str) -> Vec<TemplateChar> {
    let mut result = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                // Escape character — next char is matched literally or case-sensitively
                i += 1;
                if i < chars.len() {
                    let next = chars[i];
                    if next.is_alphabetic() {
                        // \A matches capital A exactly (case-sensitive)
                        result.push(TemplateChar::CasedLiteral(next));
                    } else {
                        // \! \- \' etc — match that punctuation or metachar literally
                        result.push(TemplateChar::Punct(next));
                    }
                    i += 1;
                }
            }
            '.' | '?' => {
                result.push(TemplateChar::Any);
                i += 1;
            }
            '*' => {
                result.push(TemplateChar::Wildcard);
                i += 1;
            }
            '[' => {
                i += 1;
                let negated = i < chars.len() && chars[i] == '^';
                if negated {
                    i += 1;
                }
                let mut letters = Vec::new();
                while i < chars.len() && chars[i] != ']' {
                    if chars[i].is_alphabetic() {
                        letters.push(chars[i].to_ascii_lowercase());
                    }
                    i += 1;
                }
                if i < chars.len() {
                    i += 1;
                }
                result.push(TemplateChar::ChoiceList(letters, negated));
            }
            '(' => {
                // Sub-pattern in template: (;xxx) means anagram sub-pattern
                i += 1;
                let is_anagram_sub = i < chars.len() && chars[i] == ';';
                if is_anagram_sub {
                    i += 1;
                }
                let mut sub_chars = Vec::new();
                let mut depth = 1i32;
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '(' => { depth += 1; sub_chars.push(chars[i]); }
                        ')' => {
                            depth -= 1;
                            if depth > 0 {
                                sub_chars.push(chars[i]);
                            }
                        }
                        c => sub_chars.push(c),
                    }
                    i += 1;
                }
                let sub_str: String = sub_chars.into_iter().collect();
                if is_anagram_sub {
                    let letters: Vec<char> = sub_str
                        .chars()
                        .filter(|c| c.is_alphabetic())
                        .map(|c| c.to_ascii_lowercase())
                        .collect();
                    result.push(TemplateChar::SubPattern(SubPattern::Anagram(letters)));
                } else {
                    let inner = parse_template(&sub_str);
                    result.extend(inner);
                }
            }
            c if c.is_ascii_digit() => {
                result.push(TemplateChar::Variable(c as u8 - b'0'));
                i += 1;
            }
            c if c.is_alphabetic() => {
                result.push(TemplateChar::Literal(c.to_ascii_lowercase()));
                i += 1;
            }
            c if !is_metacharacter(c) => {
                // Non-letter, non-digit, non-metacharacter — treat as literal punctuation
                result.push(TemplateChar::Punct(c));
                i += 1;
            }
            _ => {
                // Remaining metacharacters in template context — skip
                i += 1;
            }
        }
    }
    result
}
