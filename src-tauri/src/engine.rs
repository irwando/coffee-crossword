use std::collections::HashMap;

/// A parsed pattern ready for matching
#[derive(Debug)]
pub enum Pattern {
    /// Pure template: e.g. ".l...r.n"
    Template(Vec<TemplateChar>),
    /// Pure anagram: e.g. ";acenrt"
    Anagram(Vec<char>, Option<usize>), // letters, optional dot count
    /// Template + anagram combined: e.g. "e....;cats"
    TemplateWithAnagram(Vec<TemplateChar>, Vec<char>, Option<usize>),
}

/// A single position in a template pattern
#[derive(Debug, Clone)]
pub enum TemplateChar {
    /// A literal letter that must match exactly
    Literal(char),
    /// A dot or question mark — matches any single letter
    Any,
    /// A wildcard * — matches zero or more letters
    Wildcard,
}

/// The result of a single word match
#[derive(Debug, serde::Serialize)]
pub struct MatchResult {
    pub word: String,
    pub balance: Option<String>, // e.g. "+D" or "-JX" for anagram balances
}

/// Parse a raw pattern string into a Pattern enum
pub fn parse_pattern(input: &str) -> Option<Pattern> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    // Check for anagram (semicolon separator)
    if let Some(semi_pos) = input.find(';') {
        let template_part = &input[..semi_pos];
        let anagram_part = &input[semi_pos + 1..];

        // Count dots in anagram part (wildcards for unknown letters)
        let mut anagram_letters: Vec<char> = Vec::new();
        let mut dot_count = 0usize;

        for ch in anagram_part.chars() {
            match ch {
                '.' | '?' => dot_count += 1,
                '*' => {
                    dot_count += 1;
                }
                c if c.is_alphabetic() => anagram_letters.push(c.to_ascii_lowercase()),
                _ => {}
            }
        }

        let dots = if dot_count > 0 { Some(dot_count) } else { None };

        if template_part.is_empty() {
            // Pure anagram: ";acenrt"
            return Some(Pattern::Anagram(anagram_letters, dots));
        } else {
            // Combined: "e....;cats"
            let template = parse_template(template_part);
            return Some(Pattern::TemplateWithAnagram(template, anagram_letters, dots));
        }
    }

    // Pure template
    Some(Pattern::Template(parse_template(input)))
}

/// Parse a template string into a Vec of TemplateChar
fn parse_template(s: &str) -> Vec<TemplateChar> {
    s.chars()
        .map(|ch| match ch {
            '.' | '?' => TemplateChar::Any,
            '*' => TemplateChar::Wildcard,
            c => TemplateChar::Literal(c.to_ascii_lowercase()),
        })
        .collect()
}

/// Check if a word matches a template pattern
fn matches_template(word: &str, template: &[TemplateChar]) -> bool {
    let word_chars: Vec<char> = word
        .chars()
        .filter(|c| c.is_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect();

    // If no wildcards, template length must equal word length
    let has_wildcard = template.iter().any(|t| matches!(t, TemplateChar::Wildcard));

    if !has_wildcard {
        if word_chars.len() != template.len() {
            return false;
        }
        return template.iter().zip(word_chars.iter()).all(|(t, w)| match t {
            TemplateChar::Literal(c) => c == w,
            TemplateChar::Any => true,
            TemplateChar::Wildcard => unreachable!(),
        });
    }

    // Has wildcards — use recursive matching
    matches_template_wildcard(&word_chars, template)
}

/// Recursive wildcard matching
fn matches_template_wildcard(word: &[char], template: &[TemplateChar]) -> bool {
    if template.is_empty() {
        return word.is_empty();
    }

    match &template[0] {
        TemplateChar::Literal(c) => {
            !word.is_empty()
                && word[0] == *c
                && matches_template_wildcard(&word[1..], &template[1..])
        }
        TemplateChar::Any => {
            !word.is_empty() && matches_template_wildcard(&word[1..], &template[1..])
        }
        TemplateChar::Wildcard => {
            // Try matching zero or more characters
            for i in 0..=word.len() {
                if matches_template_wildcard(&word[i..], &template[1..]) {
                    return true;
                }
            }
            false
        }
    }
}

/// Check if a word can be formed from the given letters (anagram match).
/// Returns Some(balance) if matched, None if not matched.
/// balance shows added letters (+D) and omitted letters (-JX).
///
/// exact_length: true for pure anagrams (word length must equal letters+dots),
///               false for template+anagram (template already constrains length)
fn matches_anagram(
    word: &str,
    letters: &[char],
    dot_count: Option<usize>,
    exact_length: bool,
) -> Option<String> {
    let word_chars: Vec<char> = word
        .chars()
        .filter(|c| c.is_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect();

    // For pure anagrams, word length must equal letters + dots exactly
    // For template+anagram, the template already constrains length
    if exact_length {
        let expected_len = letters.len() + dot_count.unwrap_or(0);
        if word_chars.len() != expected_len {
            return None;
        }
    }

    // Count available letters
    let mut available: HashMap<char, i32> = HashMap::new();
    for &ch in letters {
        *available.entry(ch).or_insert(0) += 1;
    }

    // Subtract letters used by the word
    let mut needed: HashMap<char, i32> = HashMap::new();
    for &ch in &word_chars {
        let avail = available.entry(ch).or_insert(0);
        if *avail > 0 {
            *avail -= 1;
        } else {
            // This letter isn't available — needs a dot/blank
            *needed.entry(ch).or_insert(0) += 1;
        }
    }

    // Count how many blanks (dots) we need
    let blanks_needed: i32 = needed.values().sum();
    let blanks_available = dot_count.unwrap_or(0) as i32;

    if blanks_needed > blanks_available {
        return None; // Not enough blanks to cover missing letters
    }

    // Build balance string
    // Omitted letters (still in available with count > 0) → minus
    // Used blanks (letters in needed) → plus
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

    let balance = if omitted.is_empty() && added.is_empty() {
        String::new()
    } else {
        let mut parts = String::new();
        if !omitted.is_empty() {
            parts.push('-');
            for ch in &omitted {
                parts.push(ch.to_ascii_uppercase());
            }
        }
        if !added.is_empty() {
            parts.push('+');
            for ch in &added {
                parts.push(ch.to_ascii_uppercase());
            }
        }
        parts
    };

    Some(balance)
}

/// Search a word list with the given pattern
pub fn search(
    words: &[String],
    pattern: &Pattern,
    min_len: usize,
    max_len: usize,
) -> Vec<MatchResult> {
    let mut results = Vec::new();

    for word in words {
        let word_lower = word.to_ascii_lowercase();
        let word_len = word_lower.chars().filter(|c| c.is_alphabetic()).count();

        // Apply length filter
        if word_len < min_len || word_len > max_len {
            continue;
        }

        match pattern {
            Pattern::Template(template) => {
                if matches_template(&word_lower, template) {
                    results.push(MatchResult {
                        word: word_lower,
                        balance: None,
                    });
                }
            }

            Pattern::Anagram(letters, dots) => {
                if let Some(balance) = matches_anagram(&word_lower, letters, *dots, true) {
                    results.push(MatchResult {
                        word: word_lower,
                        balance: if balance.is_empty() {
                            None
                        } else {
                            Some(balance)
                        },
                    });
                }
            }

            Pattern::TemplateWithAnagram(template, letters, dots) => {
                if matches_template(&word_lower, template) {
                    if let Some(balance) =
                        matches_anagram(&word_lower, letters, *dots, false)
                    {
                        results.push(MatchResult {
                            word: word_lower,
                            balance: if balance.is_empty() {
                                None
                            } else {
                                Some(balance)
                            },
                        });
                    }
                }
            }
        }
    }

    // Sort by word length, then alphabetically
    results.sort_by(|a, b| {
        a.word.len().cmp(&b.word.len()).then(a.word.cmp(&b.word))
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word_list() -> Vec<String> {
        vec![
            "electron", "canter", "nectar", "recant", "trance",
            "aardvark", "elephant", "cat", "act", "arc",
            "drinker", "beside", "bodice", "edible",
            "maharaja", "quick", "quack", "quirk", "quark",
            "escalator",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    #[test]
    fn test_template_basic() {
        let words = word_list();
        let pattern = parse_pattern(".l...r.n").unwrap();
        let results = search(&words, &pattern, 1, 50);
        let matched: Vec<&str> = results.iter().map(|r| r.word.as_str()).collect();
        assert!(matched.contains(&"electron"), "should match electron");
    }

    #[test]
    fn test_template_question_marks() {
        let words = word_list();
        let pattern = parse_pattern("q???k").unwrap();
        let results = search(&words, &pattern, 1, 50);
        let matched: Vec<&str> = results.iter().map(|r| r.word.as_str()).collect();
        assert!(matched.contains(&"quack"));
        assert!(matched.contains(&"quick"));
        assert!(matched.contains(&"quirk"));
        assert!(matched.contains(&"quark"));
    }

    #[test]
    fn test_wildcard() {
        let words = word_list();
        let pattern = parse_pattern("m*ja").unwrap();
        let results = search(&words, &pattern, 1, 50);
        let matched: Vec<&str> = results.iter().map(|r| r.word.as_str()).collect();
        assert!(matched.contains(&"maharaja"));
    }

    #[test]
    fn test_anagram_basic() {
        let words = word_list();
        let pattern = parse_pattern(";acenrt").unwrap();
        let results = search(&words, &pattern, 1, 50);
        let matched: Vec<&str> = results.iter().map(|r| r.word.as_str()).collect();
        assert!(matched.contains(&"canter"));
        assert!(matched.contains(&"nectar"));
        assert!(matched.contains(&"recant"));
        assert!(matched.contains(&"trance"));
        // Should NOT match shorter or longer words
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_anagram_with_balance() {
        let words = word_list();
        // ;eiknrr. — finds drinker with +D balance
        let pattern = parse_pattern(";eiknrr.").unwrap();
        let results = search(&words, &pattern, 1, 50);
        let drinker = results.iter().find(|r| r.word == "drinker");
        assert!(drinker.is_some());
        assert_eq!(drinker.unwrap().balance, Some("+D".to_string()));
    }

    #[test]
    fn test_template_with_anagram() {
        let words = word_list();
        // e........;cats — finds escalator
        let pattern = parse_pattern("e........;cats").unwrap();
        let results = search(&words, &pattern, 1, 50);
        let matched: Vec<&str> = results.iter().map(|r| r.word.as_str()).collect();
        assert!(matched.contains(&"escalator"));
    }

    #[test]
    fn test_sort_by_length() {
        let words = word_list();
        let pattern = parse_pattern(".*").unwrap();
        let results = search(&words, &pattern, 1, 50);
        for i in 1..results.len() {
            assert!(results[i].word.len() >= results[i - 1].word.len());
        }
    }

    #[test]
    fn test_apostrophe_not_counted_in_length() {
        let words = vec!["earmark's".to_string(), "earmarks".to_string()];
        // e........ = 9 alphabetic chars — should NOT match earmark's (7 alpha)
        let pattern = parse_pattern("e........").unwrap();
        let results = search(&words, &pattern, 1, 50);
        let matched: Vec<&str> = results.iter().map(|r| r.word.as_str()).collect();
        assert!(!matched.contains(&"earmark's"));
        assert!(!matched.contains(&"earmarks")); // only 8 chars
    }
}
