// ── Matcher ───────────────────────────────────────────────────────────────────
// Evaluates LogicalExpr and Pattern against individual words.
// MatchContext is private to this module — no other module needs it.

use std::collections::HashMap;
use crate::engine::ast::{LogicalExpr, Pattern, TemplateChar};

/// Carries letter variable bindings through template matching.
/// Private to matcher.rs — callers use eval_expr() which handles context internally.
#[derive(Clone)]
struct MatchContext {
    variables: HashMap<u8, char>,
}

impl MatchContext {
    fn new() -> Self {
        MatchContext { variables: HashMap::new() }
    }

    /// Try to bind a variable to a character.
    /// Returns false if already bound to a different character.
    fn bind(&mut self, var: u8, ch: char) -> bool {
        match self.variables.get(&var) {
            Some(&existing) => existing == ch,
            None => {
                self.variables.insert(var, ch);
                true
            }
        }
    }
}

/// Evaluate a logical expression against a single (already normalized) word.
/// Returns Some(balance_string) if the word matches, None if it doesn't.
/// pub(crate) — called from grouping.rs's internal search().
pub(crate) fn eval_expr(word: &str, word_len: usize, expr: &LogicalExpr) -> Option<String> {
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
            if matches_template(word, template) {
                Some(String::new())
            } else {
                None
            }
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
                    template
                        .iter()
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

fn template_fixed_len(template: &[TemplateChar]) -> usize {
    template
        .iter()
        .filter(|t| !matches!(t, TemplateChar::Wildcard))
        .count()
}

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
        if word_chars.len() != template.len() {
            return false;
        }
        return template
            .iter()
            .zip(word_chars.iter())
            .all(|(t, &w)| char_matches(w, t, &mut ctx));
    }

    matches_template_wildcard(&word_chars, template, &mut ctx)
}

fn matches_template_wildcard(
    word: &[char],
    template: &[TemplateChar],
    ctx: &mut MatchContext,
) -> bool {
    if template.is_empty() {
        return word.is_empty();
    }

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
            if word.is_empty() {
                return false;
            }
            if char_matches(word[0], t, ctx) {
                matches_template_wildcard(&word[1..], &template[1..], ctx)
            } else {
                false
            }
        }
    }
}

fn matches_anagram_exact(
    word: &str,
    letters: &[char],
    dot_count: Option<usize>,
    has_wildcard: bool,
) -> Option<String> {
    let word_chars: Vec<char> = word.chars().collect();

    if !has_wildcard {
        let expected_len = letters.len() + dot_count.unwrap_or(0);
        if word_chars.len() != expected_len {
            return None;
        }
    }

    let mut available: HashMap<char, i32> = HashMap::new();
    for &ch in letters {
        *available.entry(ch).or_insert(0) += 1;
    }

    let mut needed: HashMap<char, i32> = HashMap::new();
    for &ch in &word_chars {
        let avail = available.entry(ch).or_insert(0);
        if *avail > 0 {
            *avail -= 1;
        } else {
            *needed.entry(ch).or_insert(0) += 1;
        }
    }

    // All required letters must have been found in the word
    let missing_required: i32 = available.values().filter(|&&v| v > 0).map(|&v| v).sum();
    if missing_required > 0 {
        return None;
    }

    // Extra letters (in word but not in required set)
    let extra_count: i32 = needed.values().sum();
    if !has_wildcard {
        let blanks_available = dot_count.unwrap_or(0) as i32;
        if extra_count > blanks_available {
            return None;
        }
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
        for ch in &omitted {
            balance.push(ch.to_ascii_uppercase());
        }
    }
    if !added.is_empty() {
        balance.push('+');
        for ch in &added {
            balance.push(ch.to_ascii_uppercase());
        }
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
    for &ch in letters {
        *available.entry(ch).or_insert(0) += 1;
    }

    let mut extra: Vec<char> = Vec::new();
    for &ch in &word_chars {
        let avail = available.entry(ch).or_insert(0);
        if *avail > 0 {
            *avail -= 1;
        } else {
            extra.push(ch);
        }
    }

    // All required letters must have been consumed
    for &remaining in available.values() {
        if remaining > 0 {
            return None;
        }
    }

    if extra.len() > dot_count.unwrap_or(0) {
        return None;
    }

    extra.sort();
    let balance = if extra.is_empty() {
        String::new()
    } else {
        let mut s = String::from("+");
        for ch in &extra {
            s.push(ch.to_ascii_uppercase());
        }
        s
    };
    Some(balance)
}
