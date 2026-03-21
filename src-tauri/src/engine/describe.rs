// ── Pattern description ───────────────────────────────────────────────────────
// Generates human-readable descriptions of pattern strings.
// All helpers are private — only describe_pattern is pub(crate).

use crate::engine::parser::{expand_macros, parse_logical};

/// Return a human-readable description of a pattern string.
/// Returns None if the pattern is empty or invalid.
/// pub(crate) — re-exported as pub from mod.rs.
pub(crate) fn describe_pattern(pattern: &str) -> Option<String> {
    let input = pattern.trim();
    if input.is_empty() {
        return None;
    }
    // Stub for logical expressions — full description deferred
    if input.contains(" & ") || input.contains(" | ") || input.contains('!') {
        return Some("Complex pattern".to_string());
    }
    // Validate the pattern parses before describing it
    parse_logical(input)?;
    Some(describe_simple(input))
}

fn describe_simple(input: &str) -> String {
    let expanded = expand_macros(input);
    let val = expanded.trim();

    let semi_pos = val.find(';');

    if let Some(semi) = semi_pos {
        let tmpl = &val[..semi];
        let anagram_part = &val[semi + 1..];
        let (letters, dots, has_wildcard, choice_descs) =
            parse_anagram_for_description(anagram_part);

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
                if plain_dots == 1 {
                    s += " plus 1 unknown letter";
                } else if plain_dots > 1 {
                    s += &format!(" plus {} unknown letters", plain_dots);
                }
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
            if plain_dots == 1 {
                s += " plus 1 unknown letter";
            } else if plain_dots > 1 {
                s += &format!(" plus {} unknown letters", plain_dots);
            }
        }
        return s;
    }

    // Pure template
    describe_template_part(val)
}

fn describe_template_part(tmpl: &str) -> String {
    let has_wild = tmpl.contains('*');
    let fixed_len = count_template_len(tmpl);
    let first_desc = describe_first_char(tmpl);
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
        if let Some(ref fd) = first_desc {
            desc += &format!(" {}", fd);
        }
        if let Some(last) = last_literal {
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
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
        } else if chars[i] != '*' {
            count += 1;
        }
        i += 1;
    }
    count
}

fn describe_choice_list_inner(inner: &str) -> String {
    let negated = inner.starts_with('^');
    let letters = inner.trim_start_matches('^').to_uppercase();
    if letters == "AEIOU" {
        return if negated {
            "any consonant".to_string()
        } else {
            "any vowel".to_string()
        };
    }
    if negated {
        format!(
            "any letter except {}",
            letters
                .chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        format!(
            "one of: {}",
            letters
                .chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn parse_anagram_for_description(
    anagram_part: &str,
) -> (String, usize, bool, Vec<String>) {
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
                    inner.push(chars[i]);
                    i += 1;
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
