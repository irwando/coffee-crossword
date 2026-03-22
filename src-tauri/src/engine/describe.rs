// ── Pattern description ───────────────────────────────────────────────────────
// Generates human-readable descriptions of pattern strings.
// All helpers are private — only describe_pattern is pub(crate).

use crate::engine::ast::{LogicalExpr, Pattern, SubPattern, TemplateChar};
use crate::engine::parser::{expand_macros, parse_logical, parse_pattern};

/// Return a human-readable description of a pattern string.
/// Returns None if the pattern is empty or invalid.
/// pub(crate) — re-exported as pub from mod.rs.
pub(crate) fn describe_pattern(pattern: &str) -> Option<String> {
    let input = pattern.trim();
    if input.is_empty() {
        return None;
    }

    // Validate the pattern parses before describing it
    let expanded = expand_macros(input);
    let expr = parse_logical(&expanded)?;

    Some(describe_expr(&expr))
}

/// Describe a logical expression tree.
fn describe_expr(expr: &LogicalExpr) -> String {
    match expr {
        LogicalExpr::Single(pattern) => describe_pattern_node(pattern),
        LogicalExpr::And(left, right) => {
            // Check if right is a NOT — handle "A and not B" as "A, excluding B"
            if let LogicalExpr::Not(inner) = right.as_ref() {
                format!("{}, excluding {}",
                    describe_expr(left),
                    describe_expr(inner))
            } else {
                format!("{}, and {}",
                    describe_expr(left),
                    describe_expr(right))
            }
        }
        LogicalExpr::Or(left, right) => {
            format!("{}, or {}",
                describe_expr(left),
                describe_expr(right))
        }
        LogicalExpr::Not(inner) => {
            format!("not {}", describe_expr(inner))
        }
    }
}

/// Describe a single Pattern node (no logical operators).
fn describe_pattern_node(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Template(template) => {
            let has_punct = template_has_punct(template);
            let desc = describe_template_part(template);
            if has_punct {
                format!("{} (includes punctuation)", desc)
            } else {
                desc
            }
        }
        Pattern::Anagram(anagram_chars, dots, has_wildcard) => {
            describe_anagram_part(anagram_chars, dots, *has_wildcard)
        }
        Pattern::TemplateWithAnagram(template, anagram_chars, dots) => {
            let has_punct = template_has_punct(template);
            let tmpl_desc = describe_template_part(template);
            let anagram_desc = describe_anagram_constraint(anagram_chars, dots);
            let combined = format!("{}, {}", tmpl_desc, anagram_desc);
            if has_punct {
                format!("{} (includes punctuation)", combined)
            } else {
                combined
            }
        }
    }
}

/// Returns true if a template contains any Punct or CasedLiteral characters.
fn template_has_punct(template: &[TemplateChar]) -> bool {
    template.iter().any(|t| matches!(t, TemplateChar::Punct(_) | TemplateChar::CasedLiteral(_)))
}

/// Describe a template pattern (the positional part).
fn describe_template_part(template: &[TemplateChar]) -> String {
    let has_wildcard = template.iter().any(|t| matches!(t, TemplateChar::Wildcard));
    let has_subpattern = template.iter().any(|t| matches!(t, TemplateChar::SubPattern(_)));

    let fixed_len = template_fixed_len(template);
    let first_desc = describe_first_char(template);
    let last_literal = get_last_literal(template);

    // Build sub-pattern notes if present
    let sub_notes = describe_template_subpatterns(template);

    let base = if has_wildcard {
        let mut desc = "Words".to_string();
        if let Some(ref fd) = first_desc {
            desc += &format!(" {}", fd);
        }
        if let Some(last) = last_literal {
            desc += &format!(", ending with \"{}\"", last.to_uppercase());
        }
        if first_desc.is_none() && last_literal.is_none() && !has_subpattern {
            desc += " of any length";
        }
        desc
    } else {
        let mut desc = format!("{}-letter words", fixed_len);
        if let Some(ref fd) = first_desc {
            desc += &format!(" {}", fd);
        }
        if let Some(last) = last_literal {
            // Only mention ending if it's different from the start
            let first_char = template.iter().find_map(|t| {
                if let TemplateChar::Literal(c) = t { Some(*c) } else { None }
            });
            if first_char != Some(last) {
                desc += &format!(", ending with \"{}\"", last.to_uppercase());
            }
        }
        desc
    };

    if sub_notes.is_empty() {
        base
    } else {
        format!("{}, {}", base, sub_notes.join(", "))
    }
}

/// Describe sub-patterns embedded in a template.
fn describe_template_subpatterns(template: &[TemplateChar]) -> Vec<String> {
    let mut notes = Vec::new();
    let mut pos = 0usize;

    for t in template {
        match t {
            TemplateChar::SubPattern(SubPattern::Anagram(letters)) => {
                let len = letters.len();
                let letters_upper: String = letters.iter().map(|c| c.to_ascii_uppercase()).collect();
                notes.push(format!(
                    "positions {}–{} as an anagram of {}",
                    pos + 1,
                    pos + len,
                    letters_upper
                ));
                pos += len;
            }
            TemplateChar::Wildcard => {
                // wildcard — position tracking not meaningful
            }
            _ => {
                pos += 1;
            }
        }
    }
    notes
}

/// Describe the first character constraint of a template.
fn describe_first_char(template: &[TemplateChar]) -> Option<String> {
    match template.first()? {
        TemplateChar::Literal(c) => Some(format!("starting with \"{}\"", c.to_ascii_uppercase())),
        TemplateChar::CasedLiteral(c) => Some(format!("starting with \"{}\" (exact case)", c)),
        TemplateChar::ChoiceList(letters, negated) => Some(describe_choice_first(letters, *negated)),
        _ => None,
    }
}

fn describe_choice_first(letters: &[char], negated: bool) -> String {
    let upper: String = letters.iter().map(|c| c.to_ascii_uppercase()).collect();
    if letters == ['a', 'e', 'i', 'o', 'u'] || letters == ['A', 'E', 'I', 'O', 'U'] {
        return if negated {
            "starting with any consonant".to_string()
        } else {
            "starting with any vowel".to_string()
        };
    }
    if negated {
        format!("starting with any letter except {}", upper)
    } else {
        format!("starting with one of: {}", upper)
    }
}

/// Get the last literal character of a template (if it ends with one).
fn get_last_literal(template: &[TemplateChar]) -> Option<char> {
    match template.last()? {
        TemplateChar::Literal(c) => Some(*c),
        TemplateChar::CasedLiteral(c) => Some(*c),
        _ => None,
    }
}

fn template_fixed_len(template: &[TemplateChar]) -> usize {
    template
        .iter()
        .map(|t| match t {
            TemplateChar::Wildcard => 0,
            TemplateChar::SubPattern(SubPattern::Anagram(letters)) => letters.len(),
            TemplateChar::SubPattern(SubPattern::Template(tmpl)) => template_fixed_len(tmpl),
            TemplateChar::SubPattern(SubPattern::AnagramInAnagram(letters)) => letters.len(),
            _ => 1,
        })
        .sum()
}

/// Describe a pure anagram pattern.
fn describe_anagram_part(
    anagram_chars: &[crate::engine::ast::AnagramChar],
    dots: &Option<usize>,
    has_wildcard: bool,
) -> String {
    use crate::engine::ast::AnagramChar;

    let mut plain_letters: Vec<char> = Vec::new();
    let mut sub_notes: Vec<String> = Vec::new();
    let mut choice_descs: Vec<String> = Vec::new();
    let mut blank_count = 0usize;

    for ac in anagram_chars {
        match ac {
            AnagramChar::Letter(c) => plain_letters.push(*c),
            AnagramChar::Blank => blank_count += 1,
            AnagramChar::ChoiceList(letters, negated) => {
                choice_descs.push(describe_choice_inline(letters, *negated));
            }
            AnagramChar::SubPattern(SubPattern::Template(tmpl)) => {
                // (xxx) in anagram — consecutive sequence
                let seq: String = tmpl.iter().filter_map(|t| {
                    if let TemplateChar::Literal(c) = t { Some(c.to_ascii_uppercase()) } else { None }
                }).collect();
                if !seq.is_empty() {
                    sub_notes.push(format!("containing \"{}\" consecutively", seq));
                }
            }
            AnagramChar::SubPattern(SubPattern::Anagram(letters)) |
            AnagramChar::SubPattern(SubPattern::AnagramInAnagram(letters)) => {
                let letters_upper: String = letters.iter().map(|c| c.to_ascii_uppercase()).collect();
                sub_notes.push(format!("with an anagram of {} present", letters_upper));
            }
        }
    }

    let letters_upper: String = plain_letters.iter().map(|c| c.to_ascii_uppercase()).collect();

    let mut s = if plain_letters.is_empty() {
        "Anagram search".to_string()
    } else {
        format!("Anagrams of \"{}\"", letters_upper)
    };

    if !choice_descs.is_empty() {
        s += &format!(" plus {}", choice_descs.join(" and "));
    }

    if has_wildcard {
        s += " (any number of extra letters)";
    } else {
        let plain_dots = dots.unwrap_or(0).saturating_sub(choice_descs.len());
        if plain_dots == 1 {
            s += " plus 1 unknown letter";
        } else if plain_dots > 1 {
            s += &format!(" plus {} unknown letters", plain_dots);
        }
    }

    for note in &sub_notes {
        s += &format!(", {}", note);
    }

    s
}

/// Describe the anagram constraint portion of a template+anagram pattern.
fn describe_anagram_constraint(
    anagram_chars: &[crate::engine::ast::AnagramChar],
    dots: &Option<usize>,
) -> String {
    use crate::engine::ast::AnagramChar;

    let mut plain_letters: Vec<char> = Vec::new();
    let mut sub_notes: Vec<String> = Vec::new();

    for ac in anagram_chars {
        match ac {
            AnagramChar::Letter(c) => plain_letters.push(*c),
            AnagramChar::Blank => {}
            AnagramChar::ChoiceList(letters, negated) => {
                sub_notes.push(format!("containing {}", describe_choice_inline(letters, *negated)));
            }
            AnagramChar::SubPattern(SubPattern::Template(tmpl)) => {
                let seq: String = tmpl.iter().filter_map(|t| {
                    if let TemplateChar::Literal(c) = t { Some(c.to_ascii_uppercase()) } else { None }
                }).collect();
                if !seq.is_empty() {
                    sub_notes.push(format!("containing \"{}\" consecutively", seq));
                }
            }
            AnagramChar::SubPattern(SubPattern::Anagram(letters)) |
            AnagramChar::SubPattern(SubPattern::AnagramInAnagram(letters)) => {
                let letters_upper: String = letters.iter().map(|c| c.to_ascii_uppercase()).collect();
                sub_notes.push(format!("with an anagram of {} present", letters_upper));
            }
        }
    }

    let letters_upper: String = plain_letters.iter().map(|c| c.to_ascii_uppercase()).collect();

    let mut parts: Vec<String> = Vec::new();

    if !plain_letters.is_empty() {
        let extra = dots.unwrap_or(0);
        if extra > 0 {
            parts.push(format!("containing the letters \"{}\" plus {} unknown",
                letters_upper, extra));
        } else {
            parts.push(format!("containing the letters \"{}\"", letters_upper));
        }
    }

    parts.extend(sub_notes);

    if parts.is_empty() {
        String::new()
    } else {
        parts.join(", ")
    }
}

fn describe_choice_inline(letters: &[char], negated: bool) -> String {
    let upper: String = letters.iter().map(|c| c.to_ascii_uppercase()).collect();
    if letters == ['a', 'e', 'i', 'o', 'u'] {
        return if negated { "any consonant".to_string() } else { "any vowel".to_string() };
    }
    if negated {
        format!("any letter except {}", upper)
    } else {
        format!("one of: {}", upper)
    }
}