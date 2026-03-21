use std::collections::HashMap;

/// A parsed pattern ready for matching
#[derive(Debug)]
pub enum Pattern {
    Template(Vec<TemplateChar>),
    /// letters, dot_count, has_wildcard
    Anagram(Vec<char>, Option<usize>, bool),
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
    /// A choice list [abc] or negated [^abc]
    ChoiceList(Vec<char>, bool), // (letters, negated)
}

/// A group of words that normalize to the same canonical form.
/// When normalize=false, each group will have exactly one variant.
#[derive(Debug, serde::Serialize)]
pub struct MatchGroup {
    pub normalized: String,
    pub variants: Vec<String>,
    pub balance: Option<String>,
}

/// Normalize a word: strip non-letter, non-digit characters and lowercase.
pub fn normalize(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphabetic() || c.is_ascii_digit())
        .flat_map(|c| c.to_lowercase())
        .collect()
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
        let mut has_wildcard = false;

        let anagram_chars: Vec<char> = anagram_part.chars().collect();
        let mut i = 0;
        while i < anagram_chars.len() {
            match anagram_chars[i] {
                '*' => { has_wildcard = true; i += 1; }
                '.' | '?' => { dot_count += 1; i += 1; }
                '[' => {
                    // Choice list in anagram counts as one dot (one unknown letter slot)
                    dot_count += 1;
                    i += 1;
                    while i < anagram_chars.len() && anagram_chars[i] != ']' {
                        i += 1;
                    }
                    if i < anagram_chars.len() { i += 1; } // skip ']'
                }
                c if c.is_alphabetic() => {
                    anagram_letters.push(c.to_ascii_lowercase());
                    i += 1;
                }
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
                i += 1; // skip '['
                let negated = i < chars.len() && chars[i] == '^';
                if negated { i += 1; }
                let mut letters = Vec::new();
                while i < chars.len() && chars[i] != ']' {
                    if chars[i].is_alphabetic() {
                        letters.push(chars[i].to_ascii_lowercase());
                    }
                    i += 1;
                }
                if i < chars.len() { i += 1; } // skip ']'
                result.push(TemplateChar::ChoiceList(letters, negated));
            }
            c => { result.push(TemplateChar::Literal(c.to_ascii_lowercase())); i += 1; }
        }
    }
    result
}

fn template_fixed_len(template: &[TemplateChar]) -> usize {
    template
        .iter()
        .filter(|t| !matches!(t, TemplateChar::Wildcard))
        .count()
}

fn char_matches_template_char(ch: char, t: &TemplateChar) -> bool {
    match t {
        TemplateChar::Literal(c) => *c == ch,
        TemplateChar::Any => true,
        TemplateChar::Wildcard => unreachable!(),
        TemplateChar::ChoiceList(letters, negated) => {
            let contains = letters.contains(&ch);
            if *negated { !contains } else { contains }
        }
    }
}

fn matches_template(word: &str, template: &[TemplateChar]) -> bool {
    let word_chars: Vec<char> = word.chars().collect();
    let has_wildcard = template.iter().any(|t| matches!(t, TemplateChar::Wildcard));

    if !has_wildcard {
        if word_chars.len() != template.len() {
            return false;
        }
        return template.iter().zip(word_chars.iter()).all(|(t, w)| {
            char_matches_template_char(*w, t)
        });
    }

    matches_template_wildcard(&word_chars, template)
}

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
        TemplateChar::ChoiceList(letters, negated) => {
            if word.is_empty() { return false; }
            let contains = letters.contains(&word[0]);
            let matches = if *negated { !contains } else { contains };
            matches && matches_template_wildcard(&word[1..], &template[1..])
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

fn matches_anagram_exact(
    word: &str,
    letters: &[char],
    dot_count: Option<usize>,
    has_wildcard: bool,
) -> Option<String> {
    let word_chars: Vec<char> = word.chars().collect();

    // With wildcard, skip length check — any number of extra letters allowed
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

    // Check all required letters were found in the word
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

struct RawMatch {
    original: String,
    normalized_key: String,
    balance: Option<String>,
}

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

            Pattern::Anagram(letters, dots, has_wildcard) => {
                matches_anagram_exact(&matched_form, letters, *dots, *has_wildcard)
            }

            Pattern::TemplateWithAnagram(template, letters, dots) => {
                let has_wildcard =
                    template.iter().any(|t| matches!(t, TemplateChar::Wildcard));
                let length_ok = if has_wildcard {
                    true
                } else {
                    word_len == template_fixed_len(template)
                };
                if length_ok && matches_template(&matched_form, template) {
                    let free_positions = if has_wildcard {
                        word_len.saturating_sub(letters.len())
                    } else {
                        template
                            .iter()
                            .filter(|t| !matches!(t, TemplateChar::Literal(_)))
                            .count()
                    };
                    let effective_dots = Some(free_positions + dots.unwrap_or(0));
                    matches_anagram_within(&matched_form, letters, effective_dots)
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

    let mut group_order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, MatchGroup> = HashMap::new();

    for raw_match in raw {
        let key = raw_match.normalized_key.clone();

        if let Some(group) = groups.get_mut(&key) {
            let original_lower = raw_match.original.to_ascii_lowercase();
            if original_lower != key {
                group.variants.push(raw_match.original);
            }
        } else {
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
            "catch-22", "escapists", "ultra",
            // extras for choice list tests
            "arts", "rest", "rust", "sort", "star", "stir",
            "llama", "lynch", "lymph", "lyric",
            "yoga", "zinc",
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
    fn test_template_with_anagram_finds_escalator() {
        let words = word_list();
        let pattern = parse_pattern("e........;cats").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let keys: Vec<&str> = results.iter().map(|r| r.normalized.as_str()).collect();
        assert!(keys.contains(&"escalator"));
    }

    #[test]
    fn test_template_with_anagram_balance() {
        let words = word_list();
        let pattern = parse_pattern("e........;cats").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let escapists = results.iter().find(|r| r.normalized == "escapists");
        assert!(escapists.is_some(), "should find escapists");
        let balance = escapists.unwrap().balance.as_deref().unwrap_or("");
        assert!(balance.starts_with('+'), "balance should show extra letters: {}", balance);
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
    fn test_wildcard_with_anagram() {
        let words = word_list();
        let pattern = parse_pattern("e*;cats").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let keys: Vec<&str> = results.iter().map(|r| r.normalized.as_str()).collect();
        assert!(keys.contains(&"escalator"));
        assert!(keys.contains(&"escapists"));
    }

    #[test]
    fn test_anagram_wildcard_unlimited() {
        let words = word_list();
        let pattern = parse_pattern(";cats*").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let keys: Vec<&str> = results.iter().map(|r| r.normalized.as_str()).collect();
        assert!(keys.contains(&"escalator"), "should find escalator");
        assert!(keys.contains(&"escapists"), "should find escapists");
        assert!(results.len() >= 2, "expected multiple matches, got {}", results.len());
    }

    #[test]
    fn test_deduplication_groups_variants() {
        let words = word_list();
        let pattern = parse_pattern("e........").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        let escargots = results.iter().find(|r| r.normalized == "escargots");
        assert!(escargots.is_some());
        let group = escargots.unwrap();
        assert_eq!(group.variants.len(), 1,
            "expected 1 variant, got {:?}", group.variants);
        assert!(group.variants.contains(&"escargot's".to_string()));
    }

    #[test]
    fn test_normalize_off_no_grouping() {
        let words = word_list();
        let pattern = parse_pattern("e........").unwrap();
        let results = search(&words, &pattern, 1, 50, false);
        for r in &results {
            assert!(r.variants.is_empty());
        }
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

    // ── Choice list tests ─────────────────────────────────────────────────

    #[test]
    fn test_choice_list_vowel_start() {
        let words = word_list();
        // 5-letter words starting with a vowel
        let pattern = parse_pattern("[aeiou]....").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        assert!(!results.is_empty(), "should find vowel-starting 5-letter words");
        for r in &results {
            assert_eq!(r.normalized.len(), 5, "wrong length: {}", r.normalized);
            assert!("aeiou".contains(r.normalized.chars().next().unwrap()),
                "should start with vowel: {}", r.normalized);
        }
    }

    #[test]
    fn test_choice_list_negated_consonant_start() {
        let words = word_list();
        // 4-letter words starting with a consonant
        let pattern = parse_pattern("[^aeiou]...").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        assert!(!results.is_empty(), "should find consonant-starting 4-letter words");
        for r in &results {
            assert_eq!(r.normalized.len(), 4, "wrong length: {}", r.normalized);
            assert!(!"aeiou".contains(r.normalized.chars().next().unwrap()),
                "should start with consonant: {}", r.normalized);
        }
    }

    #[test]
    fn test_choice_list_middle_position() {
        let words = word_list();
        // 3-letter words with a vowel in the middle
        let pattern = parse_pattern(".[aeiou].").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        assert!(!results.is_empty(), "should find 3-letter words with vowel in middle");
        for r in &results {
            assert_eq!(r.normalized.len(), 3, "wrong length: {}", r.normalized);
            assert!("aeiou".contains(r.normalized.chars().nth(1).unwrap()),
                "middle should be vowel: {}", r.normalized);
        }
    }

    #[test]
    fn test_choice_list_end_position() {
        let words = word_list();
        // words ending with x, y, or z (yoga ends in 'a', zinc ends in 'c' — use 'c' or 'k')
        // quack, quick, quirk, quark all end in 'k'
        let pattern = parse_pattern("....[ck]").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        assert!(!results.is_empty(), "should find 5-letter words ending in c or k");
        for r in &results {
            assert_eq!(r.normalized.len(), 5, "wrong length: {}", r.normalized);
            let last = r.normalized.chars().last().unwrap();
            assert!("ck".contains(last), "should end in c or k: {}", r.normalized);
        }
    }

    #[test]
    fn test_choice_list_in_anagram() {
        let words = word_list();
        // anagram of str + any vowel — 4-letter words containing s, t, r and a vowel
        let pattern = parse_pattern(";str[aeiou]").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        assert!(!results.is_empty(), "should find anagram results with choice list");
        // all results should be 4 letters (3 required + 1 from choice list)
        for r in &results {
            assert_eq!(r.normalized.len(), 4,
                "wrong length for anagram+choice: {}", r.normalized);
        }
    }

    #[test]
    fn test_choice_list_with_template_and_anagram() {
        let words = word_list();
        // 8-letter words starting with 'e' or 'a', containing letters 'r', 'c', 't'
        let pattern = parse_pattern("[ea]......;rct").unwrap();
        let results = search(&words, &pattern, 1, 50, true);
        for r in &results {
            assert_eq!(r.normalized.len(), 8, "wrong length: {}", r.normalized);
            let first = r.normalized.chars().next().unwrap();
            assert!("ea".contains(first), "should start with e or a: {}", r.normalized);
            assert!(r.normalized.contains('r'), "should contain r: {}", r.normalized);
            assert!(r.normalized.contains('c'), "should contain c: {}", r.normalized);
            assert!(r.normalized.contains('t'), "should contain t: {}", r.normalized);
        }
    }
}
