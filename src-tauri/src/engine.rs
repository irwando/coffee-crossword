use std::collections::HashMap;

/// A parsed pattern ready for matching
#[derive(Debug)]
pub enum Pattern {
    /// Pure template: e.g. ".l...r.n"
    Template(Vec<TemplateChar>),
    /// Pure anagram: e.g. ";acenrt"
    Anagram(Vec<char>, Option<usize>),
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

/// A group of words that normalize to the same canonical form.
/// When normalize=false, each group will have exactly one variant.
#[derive(Debug, serde::Serialize)]
pub struct MatchGroup {
    /// The canonical normalized lowercase form, e.g. "escargots"
    pub normalized: String,
    /// All original forms from the dictionary that map to this group,
    /// e.g. ["escargot's", "Escargots"]. Empty when normalize=false.
    pub variants: Vec<String>,
    /// Anagram balance string, e.g. "+D" or "-JX"
    pub balance: Option<String>,
}

/// Normalize a word: strip non-letter, non-digit characters and lowercase.
/// Unicode letters and digits are kept; apostrophes, hyphens, spaces etc. removed.
pub fn normalize(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphabetic() || c.is_ascii_digit())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Effective length for matching and display purposes.
pub fn effective_len(word: &str, normalize_mode: bool) -> usize {
    if normalize_mode {
        normalize(word).chars().count()
    } else {
        word.chars().count()
    }
}

/// The form used for pattern matching: lowercased, optionally normalized.
fn matching_form(word: &str, normalize_mode: bool) -> String {
    if normalize_mode {
        normalize(word)
    } else {
        word.to_ascii_lowercase()
    }
}

/// Parse a raw pattern string into a Pattern enum
pub fn parse_pattern(input: &str) -> Option<Pattern> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    if let Some(semi_pos) = input.find(';') {
        let template_part = &input[..semi_pos];
        let anagram_part = &input[semi_pos + 1..];

        let mut anagram_letters: Vec<char> = Vec::new();
        let mut dot_count = 0usize;

        for ch in anagram_part.chars() {
            match ch {
                '.' | '?' | '*' => dot_count += 1,
                c if c.is_alphabetic() => anagram_letters.push(c.to_ascii_lowercase()),
                _ => {}
            }
        }

        let dots = if dot_count > 0 { Some(dot_count) } else { None };

        if template_part.is_empty() {
            return Some(Pattern::Anagram(anagram_letters, dots));
        } else {
            let template = parse_template(template_part);
            return Some(Pattern::TemplateWithAnagram(template, anagram_letters, dots));
        }
    }

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

/// Number of non-wildcard positions in a template (defines required word length)
fn template_fixed_len(template: &[TemplateChar]) -> usize {
    template
        .iter()
        .filter(|t| !matches!(t, TemplateChar::Wildcard))
        .count()
}

/// Check if a word matches a template pattern.
/// word should already be in matching form (lowercased, normalized if needed).
fn matches_template(word: &str, template: &[TemplateChar]) -> bool {
    let word_chars: Vec<char> = word.chars().collect();
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
            for i in 0..=word.len() {
                if matches_template_wildcard(&word[i..], &template[1..]) {
                    return true;
                }
            }
            false
        }
    }
}

/// Pure anagram match: word must use exactly the given letters + dots.
/// Returns Some(balance) on match, None if no match.
fn matches_anagram_exact(
    word: &str,
    letters: &[char],
    dot_count: Option<usize>,
) -> Option<String> {
    let word_chars: Vec<char> = word.chars().collect();

    let expected_len = letters.len() + dot_count.unwrap_or(0);
    if word_chars.len() != expected_len {
        return None;
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

    let blanks_needed: i32 = needed.values().sum();
    let blanks_available = dot_count.unwrap_or(0) as i32;
    if blanks_needed > blanks_available {
        return None;
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

/// Template+anagram match: checks the word contains all required anagram letters.
/// Length is enforced by the caller. Returns Some(balance) on match.
fn matches_anagram_within(
    word: &str,
    letters: &[char],
    dot_count: Option<usize>,
) -> Option<String> {
    let word_chars: Vec<char> = word.chars().collect();

    let mut word_counts: HashMap<char, i32> = HashMap::new();
    for &ch in &word_chars {
        *word_counts.entry(ch).or_insert(0) += 1;
    }

    let mut required: HashMap<char, i32> = HashMap::new();
    for &ch in letters {
        *required.entry(ch).or_insert(0) += 1;
    }

    let mut dots_used = 0i32;
    for (&ch, &req) in &required {
        let have = *word_counts.get(&ch).unwrap_or(&0);
        if have < req {
            dots_used += req - have;
        }
    }

    if dots_used > dot_count.unwrap_or(0) as i32 {
        return None;
    }

    Some(String::new())
}

/// Internal match result before grouping
struct RawMatch {
    original: String,
    normalized_key: String,
    balance: Option<String>,
}

/// Search a word list and return grouped, deduplicated results.
///
/// normalize_mode=true:  words are normalized before matching; results are
///                       grouped by their canonical form with variants collected.
/// normalize_mode=false: words matched as-is; each result is its own group.
pub fn search(
    words: &[String],
    pattern: &Pattern,
    min_len: usize,
    max_len: usize,
    normalize_mode: bool,
) -> Vec<MatchGroup> {
    let mut raw: Vec<RawMatch> = Vec::new();

    for word in words {
        let matched_form = matching_form(word, normalize_mode);
        let word_len = matched_form.chars().count();

        if word_len < min_len || word_len > max_len {
            continue;
        }

        let balance = match pattern {
            Pattern::Template(template) => {
                if matches_template(&matched_form, template) {
                    Some(String::new())
                } else {
                    None
                }
            }

            Pattern::Anagram(letters, dots) => {
                matches_anagram_exact(&matched_form, letters, *dots)
            }

            Pattern::TemplateWithAnagram(template, letters, dots) => {
                let has_wildcard = template.iter().any(|t| matches!(t, TemplateChar::Wildcard));
                let length_ok = if has_wildcard {
                    true // template matching handles length for wildcard patterns
                } else {
                    word_len == template_fixed_len(template)
                };
                if length_ok && matches_template(&matched_form, template) {
                    matches_anagram_within(&matched_form, letters, *dots)
                } else {
                    None
            }
}
        };

        if let Some(balance_str) = balance {
            raw.push(RawMatch {
                original: word.clone(),
                normalized_key: matched_form,
                balance: if balance_str.is_empty() {
                    None
                } else {
                    Some(balance_str)
                },
            });
        }
    }

    // Group by normalized key
    // Use an IndexMap-style approach: preserve insertion order with a Vec of keys
    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, MatchGroup> = HashMap::new();

    for raw_match in raw {
        let key = raw_match.normalized_key.clone();

        if let Some(group) = groups.get_mut(&key) {
            // Already have this key — add as variant if different from canonical
            let original_lower = raw_match.original.to_ascii_lowercase();
            if original_lower != key {
                group.variants.push(raw_match.original);
            }
        } else {
            // New key — create group
            group_order.push(key.clone());
            let original_lower = raw_match.original.to_ascii_lowercase();
            let variants = if original_lower != key {
                vec![raw_match.original]
            } else {
                vec![]
            };
            groups.insert(
                key.clone(),
                MatchGroup {
                    normalized: key,
                    variants,
                    balance: raw_match.balance,
                },
            );
        }
    }

    // Collect in insertion order, then sort by length then alphabetically
    let mut result: Vec<MatchGroup> = group_order
        .into_iter()
        .filter_map(|k| groups.remove(&k))
        .collect();

    result.sort_by(|a, b| {
        a.normalized
            .len()
            .cmp(&b.normalized.len())
            .then(a.normalized.cmp(&b.normalized))
    });

    result
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
            "escalator", "explorer's", "Escargots", "escargots", "escargot's",
            "catch-22",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    #[test]
    fn test_template_basic() {
        let words = word_list();
        let pattern = parse_pattern(".l...r.n").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let keys: Vec<&str> = results.iter().map(|r| r.normalized.as_str()).collect();
        assert!(keys.contains(&"electron"));
    }

    #[test]
    fn test_template_question_marks() {
        let words = word_list();
        let pattern = parse_pattern("q???k").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let keys: Vec<&str> = results.iter().map(|r| r.normalized.as_str()).collect();
        assert!(keys.contains(&"quack"));
        assert!(keys.contains(&"quick"));
        assert!(keys.contains(&"quirk"));
        assert!(keys.contains(&"quark"));
    }

    #[test]
    fn test_wildcard() {
        let words = word_list();
        let pattern = parse_pattern("m*ja").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let keys: Vec<&str> = results.iter().map(|r| r.normalized.as_str()).collect();
        assert!(keys.contains(&"maharaja"));
    }

    #[test]
    fn test_anagram_exact_match() {
        let words = word_list();
        let pattern = parse_pattern(";acenrt").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let keys: Vec<&str> = results.iter().map(|r| r.normalized.as_str()).collect();
        assert!(keys.contains(&"canter"));
        assert!(keys.contains(&"nectar"));
        assert!(keys.contains(&"recant"));
        assert!(keys.contains(&"trance"));
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_anagram_with_balance() {
        let words = word_list();
        let pattern = parse_pattern(";eiknrr.").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let drinker = results.iter().find(|r| r.normalized == "drinker");
        assert!(drinker.is_some());
        assert_eq!(drinker.unwrap().balance, Some("+D".to_string()));
    }

    #[test]
    fn test_template_with_anagram() {
        let words = word_list();
        let pattern = parse_pattern("e........;cats").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let keys: Vec<&str> = results.iter().map(|r| r.normalized.as_str()).collect();
        assert!(keys.contains(&"escalator"));
        // explorer's doesn't contain c,a,t,s — should not match
        assert!(!keys.contains(&"explorers"));
    }

    #[test]
    fn test_template_with_anagram_no_wrong_length() {
        let words = word_list();
        let pattern = parse_pattern("e........;cats").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        for r in &results {
            assert_eq!(r.normalized.len(), 9,
                "expected 9 letters, got {} for '{}'", r.normalized.len(), r.normalized);
        }
    }

    #[test]
    fn test_deduplication_groups_variants() {
        let words = word_list();
        // Search for 9-letter words starting with e — should group escargots variants
        let pattern = parse_pattern("e........").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let escargots = results.iter().find(|r| r.normalized == "escargots");
        assert!(escargots.is_some(), "should find escargots group");
        let group = escargots.unwrap();
        // Should have collected the variant forms
        assert!(!group.variants.is_empty(), "escargots group should have variants");
        // The three variants are: Escargots, escargot's (escargots itself is the key)
        assert_eq!(group.variants.len(), 1,
            "expected 1 variant (escargot's), got {:?}", group.variants);
        assert!(group.variants.contains(&"escargot's".to_string()));
    }

    #[test]
    fn test_normalize_off_no_grouping() {
        let words = word_list();
        let pattern = parse_pattern("e........").unwrap();
        let results = search(&words, &pattern, 1, 50, false);
        // With normalize off, explorer's is 10 chars — filtered out
        // escargots, Escargots are 9 chars but apostrophe version is 10
        for r in &results {
            assert!(r.variants.is_empty(), "no grouping when normalize=false");
        }
    }

    #[test]
    fn test_hyphenated_word_normalize() {
        let words = word_list();
        let pattern = parse_pattern("c......").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let catch22 = results.iter().find(|r| r.normalized == "catch22");
        assert!(catch22.is_some());
        assert!(!catch22.unwrap().variants.is_empty());
    }

    #[test]
    fn test_sort_by_length() {
        let words = word_list();
        let pattern = parse_pattern(".*").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        for i in 1..results.len() {
            assert!(results[i].normalized.len() >= results[i - 1].normalized.len());
        }
    }
}
