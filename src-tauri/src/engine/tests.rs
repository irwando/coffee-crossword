// ── Engine tests ──────────────────────────────────────────────────────────────
// All tests are here. Each test imports exactly what it needs.
// Shared helpers (word_list, keys) come from test_utils.

use crate::engine::mod_pub::search_words;
use crate::engine::test_utils::{keys, word_list};

// Helper: run search_words with default min/max/normalize
fn sw(pattern: &str) -> Vec<crate::engine::ast::MatchGroup> {
    search_words(&word_list(), pattern, 1, 50, true)
}

// ── Template ──────────────────────────────────────────────────────────────────

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
    let r = sw("...");
    for result in &r {
        assert_eq!(result.normalized.len(), 3);
    }
}

// ── Wildcard ──────────────────────────────────────────────────────────────────

#[test]
fn test_wildcard_basic() {
    let r = sw("m*ja");
    assert!(keys(&r).contains(&"maharaja"));
}

#[test]
fn test_wildcard_start() {
    let r = sw("e*");
    let k = keys(&r);
    assert!(k.contains(&"electron"));
    assert!(k.contains(&"escalator"));
    assert!(k.contains(&"elephant"));
}

#[test]
fn test_wildcard_end() {
    let r = sw("*t");
    for result in &r {
        assert!(result.normalized.ends_with('t'));
    }
}

// ── Anagram ───────────────────────────────────────────────────────────────────

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
    assert!(k.contains(&"escalator"));
    assert!(k.contains(&"escapists"));
    assert!(r.len() >= 2);
}

// ── Template + anagram ────────────────────────────────────────────────────────

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
        assert_eq!(result.normalized.len(), 9, "wrong length: {}", result.normalized);
    }
}

#[test]
fn test_wildcard_with_anagram() {
    let r = sw("e*;cats");
    let k = keys(&r);
    assert!(k.contains(&"escalator"));
    assert!(k.contains(&"escapists"));
}

// ── Choice lists ──────────────────────────────────────────────────────────────

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
        assert_eq!(result.normalized.len(), 4, "wrong length: {}", result.normalized);
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

// ── Wildcard × choice list ────────────────────────────────────────────────────

#[test]
fn test_wildcard_with_choice_list() {
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
    let r = sw("l[^aeiou]*");
    assert!(!r.is_empty());
    for result in &r {
        assert!(result.normalized.starts_with('l'));
        let second = result.normalized.chars().nth(1).unwrap();
        assert!(!"aeiou".contains(second),
            "second letter should be consonant: {}", result.normalized);
    }
}

// ── Anagram wildcard × template/choice ───────────────────────────────────────

#[test]
fn test_anagram_wildcard_with_template() {
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
    let r = sw(";str[aeiou]*");
    assert!(!r.is_empty());
    for result in &r {
        assert!(result.normalized.contains('s'));
        assert!(result.normalized.contains('t'));
        assert!(result.normalized.contains('r'));
    }
}

// ── Macros ────────────────────────────────────────────────────────────────────

#[test]
fn test_macro_at_vowel_template() {
    let r_macro = sw("@....");
    let r_explicit = sw("[aeiou]....");
    let k_macro: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
    let k_explicit: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
    assert_eq!(k_macro, k_explicit, "@ should expand to [aeiou]");
}

#[test]
fn test_macro_hash_consonant_template() {
    let r_macro = sw("#...");
    let r_explicit = sw("[^aeiou]...");
    let k_macro: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
    let k_explicit: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
    assert_eq!(k_macro, k_explicit, "# should expand to [^aeiou]");
}

#[test]
fn test_macro_in_anagram() {
    let r_macro = sw(";str@");
    let r_explicit = sw(";str[aeiou]");
    let k_macro: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
    let k_explicit: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
    assert_eq!(k_macro, k_explicit);
}

#[test]
fn test_macro_multiple_in_pattern() {
    let r = sw("@#@#");
    for result in &r {
        assert_eq!(result.normalized.len(), 4);
        let cs: Vec<char> = result.normalized.chars().collect();
        assert!("aeiou".contains(cs[0]));
        assert!(!"aeiou".contains(cs[1]));
        assert!("aeiou".contains(cs[2]));
        assert!(!"aeiou".contains(cs[3]));
    }
}

#[test]
fn test_macro_with_wildcard() {
    let r = sw("@*");
    assert!(!r.is_empty());
    for result in &r {
        assert!("aeiou".contains(result.normalized.chars().next().unwrap()),
            "should start with vowel: {}", result.normalized);
    }
}

#[test]
fn test_macro_with_letter_variable() {
    let r = sw("@1..1");
    for result in &r {
        assert_eq!(result.normalized.len(), 5);
        assert!("aeiou".contains(result.normalized.chars().next().unwrap()));
        let cs: Vec<char> = result.normalized.chars().collect();
        assert_eq!(cs[1], cs[4]);
    }
}

#[test]
fn test_macro_with_anagram_wildcard() {
    let r = sw(";str@*");
    assert!(!r.is_empty());
    for result in &r {
        assert!(result.normalized.contains('s'));
        assert!(result.normalized.contains('t'));
        assert!(result.normalized.contains('r'));
    }
}

// ── Letter variables ──────────────────────────────────────────────────────────

#[test]
fn test_letter_variable_palindrome_5() {
    let r = sw("12321");
    let k = keys(&r);
    assert!(k.contains(&"level"));
    assert!(k.contains(&"radar"));
    assert!(k.contains(&"civic"));
    for result in &r {
        let cs: Vec<char> = result.normalized.chars().collect();
        assert_eq!(cs[0], cs[4]);
        assert_eq!(cs[1], cs[3]);
    }
}

#[test]
fn test_letter_variable_palindrome_7() {
    let r = sw("1234321");
    let k = keys(&r);
    assert!(k.contains(&"repaper"));
    for result in &r {
        let cs: Vec<char> = result.normalized.chars().collect();
        assert_eq!(cs[0], cs[6]);
        assert_eq!(cs[1], cs[5]);
        assert_eq!(cs[2], cs[4]);
    }
}

#[test]
fn test_letter_variable_same_first_last() {
    let r = sw("1...1");
    assert!(!r.is_empty());
    for result in &r {
        assert_eq!(result.normalized.len(), 5);
        let cs: Vec<char> = result.normalized.chars().collect();
        assert_eq!(cs[0], cs[4]);
    }
}

#[test]
fn test_letter_variable_tautonym() {
    let r = sw("123123");
    let k = keys(&r);
    assert!(k.contains(&"murmur"));
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
    let r = sw("1*1");
    assert!(!r.is_empty());
    for result in &r {
        let cs: Vec<char> = result.normalized.chars().collect();
        assert_eq!(cs[0], *cs.last().unwrap());
    }
}

#[test]
fn test_letter_variable_with_choice_list() {
    let r = sw("[aeiou]1..1");
    for result in &r {
        assert_eq!(result.normalized.len(), 5);
        assert!("aeiou".contains(result.normalized.chars().next().unwrap()));
        let cs: Vec<char> = result.normalized.chars().collect();
        assert_eq!(cs[1], cs[4]);
    }
}

#[test]
fn test_letter_variable_with_macro() {
    let r_macro = sw("@1..1");
    let r_explicit = sw("[aeiou]1..1");
    let k_m: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
    let k_e: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
    assert_eq!(k_m, k_e);
}

#[test]
fn test_letter_variable_with_anagram() {
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

// ── Logical operations ────────────────────────────────────────────────────────

#[test]
fn test_logical_and_basic() {
    let r = sw("c* & *s");
    assert!(!r.is_empty());
    for result in &r {
        assert!(result.normalized.starts_with('c'));
        assert!(result.normalized.ends_with('s'));
    }
}

#[test]
fn test_logical_or_basic() {
    let r = sw("c... | ...s");
    assert!(!r.is_empty());
    for result in &r {
        assert_eq!(result.normalized.len(), 4);
        let starts_c = result.normalized.starts_with('c');
        let ends_s = result.normalized.ends_with('s');
        assert!(starts_c || ends_s);
    }
}

#[test]
fn test_logical_not_basic() {
    let r = sw("c* & !cat*");
    assert!(!r.is_empty());
    for result in &r {
        assert!(result.normalized.starts_with('c'));
        assert!(!result.normalized.starts_with("cat"));
    }
}

#[test]
fn test_logical_grouped_or() {
    let r = sw("(c... | ...r)");
    assert!(!r.is_empty());
    for result in &r {
        assert_eq!(result.normalized.len(), 4);
        let starts_c = result.normalized.starts_with('c');
        let ends_r = result.normalized.ends_with('r');
        assert!(starts_c || ends_r);
    }
}

#[test]
fn test_logical_and_with_anagram() {
    let r = sw(";cats & c*");
    assert!(!r.is_empty());
    for result in &r {
        assert!(result.normalized.starts_with('c'));
        assert!(result.normalized.contains('c'));
        assert!(result.normalized.contains('a'));
        assert!(result.normalized.contains('t'));
        assert!(result.normalized.contains('s'));
    }
}

#[test]
fn test_logical_and_with_wildcard() {
    let r = sw("c* & *s");
    for result in &r {
        assert!(result.normalized.starts_with('c'));
        assert!(result.normalized.ends_with('s'));
    }
}

#[test]
fn test_logical_and_with_choice_list() {
    let r = sw("[aeiou]... & *t");
    for result in &r {
        assert_eq!(result.normalized.len(), 4);
        assert!("aeiou".contains(result.normalized.chars().next().unwrap()));
        assert!(result.normalized.ends_with('t'));
    }
}

#[test]
fn test_logical_and_with_macro() {
    let r_macro = sw("@... & *t");
    let r_explicit = sw("[aeiou]... & *t");
    let k_m: Vec<&str> = r_macro.iter().map(|r| r.normalized.as_str()).collect();
    let k_e: Vec<&str> = r_explicit.iter().map(|r| r.normalized.as_str()).collect();
    assert_eq!(k_m, k_e);
}

#[test]
fn test_logical_and_with_letter_variable() {
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
    let r = sw(";cats | ;arts");
    assert!(!r.is_empty());
    for result in &r {
        let is_cats = { let mut c: Vec<char> = result.normalized.chars().collect(); c.sort(); c == vec!['a','c','s','t'] };
        let is_arts = { let mut c: Vec<char> = result.normalized.chars().collect(); c.sort(); c == vec!['a','r','s','t'] };
        assert!(is_cats || is_arts, "should be anagram of cats or arts: {}", result.normalized);
    }
}

#[test]
fn test_logical_not_with_wildcard() {
    let r = sw("c* & !*s");
    for result in &r {
        assert!(result.normalized.starts_with('c'));
        assert!(!result.normalized.ends_with('s'));
    }
}

#[test]
fn test_logical_complex_grouped() {
    let r = sw("(c* | *r) & ....");
    assert!(!r.is_empty());
    for result in &r {
        assert_eq!(result.normalized.len(), 4);
        let starts_c = result.normalized.starts_with('c');
        let ends_r = result.normalized.ends_with('r');
        assert!(starts_c || ends_r);
    }
}

// ── Normalization / deduplication ─────────────────────────────────────────────

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
    for result in &r {
        assert!(result.variants.is_empty());
    }
}

#[test]
fn test_sort_by_length() {
    let r = sw(".*");
    for i in 1..r.len() {
        assert!(r[i].normalized.len() >= r[i-1].normalized.len());
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

#[test]
fn test_validate_pattern_valid() {
    use crate::engine::mod_pub::validate_pattern;
    assert!(validate_pattern(";acenrt").is_ok());
    assert!(validate_pattern(".l...r.n").is_ok());
    assert!(validate_pattern("c* & !cat*").is_ok());
    assert!(validate_pattern("@....").is_ok());
    assert!(validate_pattern("12321").is_ok());
}

#[test]
fn test_validate_pattern_empty() {
    use crate::engine::mod_pub::validate_pattern;
    assert!(validate_pattern("").is_err());
    assert!(validate_pattern("   ").is_err());
}

#[test]
fn test_describe_pattern_template() {
    use crate::engine::mod_pub::describe_pattern;
    let d = describe_pattern(".l...r.n").unwrap();
    assert!(d.contains("8"), "should mention 8 letters: {}", d);
}

#[test]
fn test_describe_pattern_anagram() {
    use crate::engine::mod_pub::describe_pattern;
    let d = describe_pattern(";acenrt").unwrap();
    assert!(d.to_lowercase().contains("anagram"));
    assert!(d.contains("ACENRT") || d.contains("acenrt"));
}

#[test]
fn test_describe_pattern_empty() {
    use crate::engine::mod_pub::describe_pattern;
    assert!(describe_pattern("").is_none());
}

#[test]
fn test_describe_pattern_logical_stub() {
    use crate::engine::mod_pub::describe_pattern;
    let d = describe_pattern("c* & !cat*").unwrap();
    assert_eq!(d, "Complex pattern");
}

#[test]
fn test_describe_pattern_macro() {
    use crate::engine::mod_pub::describe_pattern;
    let d = describe_pattern("@....").unwrap();
    assert!(d.contains("5") || d.contains("vowel"), "should describe macro: {}", d);
}
