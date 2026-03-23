// ── Engine tests ──────────────────────────────────────────────────────────────
use crate::engine::mod_pub::search_words;
use crate::engine::test_utils::{keys, word_list};

fn sw(pattern: &str) -> Vec<crate::engine::ast::MatchGroup> {
    search_words(&word_list(), pattern, 1, 50, true)
}
fn sw_raw(pattern: &str) -> Vec<crate::engine::ast::MatchGroup> {
    search_words(&word_list(), pattern, 1, 50, false)
}

#[test] fn test_template_basic() { assert!(keys(&sw(".l...r.n")).contains(&"electron")); }
#[test] fn test_template_question_marks() { let r = sw("q???k"); let k = keys(&r); assert!(k.contains(&"quack")); assert!(k.contains(&"quick")); assert!(k.contains(&"quirk")); assert!(k.contains(&"quark")); }
#[test] fn test_template_length_exact() { for r in sw("...") { assert_eq!(r.normalized.len(), 3); } }
#[test] fn test_wildcard_basic() { assert!(keys(&sw("m*ja")).contains(&"maharaja")); }
#[test] fn test_wildcard_start() { let r = sw("e*"); let k = keys(&r); assert!(k.contains(&"electron")); assert!(k.contains(&"escalator")); assert!(k.contains(&"elephant")); }
#[test] fn test_wildcard_end() { for r in sw("*t") { assert!(r.normalized.ends_with('t')); } }
#[test] fn test_anagram_exact() { let r = sw(";acenrt"); let k = keys(&r); assert!(k.contains(&"canter")); assert!(k.contains(&"nectar")); assert!(k.contains(&"recant")); assert!(k.contains(&"trance")); assert_eq!(r.len(), 4); }
#[test] fn test_anagram_with_blank() { let r = sw(";eiknrr."); let d = r.iter().find(|r| r.normalized == "drinker"); assert!(d.is_some()); assert_eq!(d.unwrap().balance, Some("+D".to_string())); }
#[test] fn test_anagram_wildcard() { let r = sw(";cats*"); let k = keys(&r); assert!(k.contains(&"escalator")); assert!(k.contains(&"escapists")); assert!(r.len() >= 2); }
#[test] fn test_template_with_anagram_basic() { assert!(keys(&sw("e........;cats")).contains(&"escalator")); }
#[test] fn test_template_with_anagram_balance() { let r = sw("e........;cats"); let e = r.iter().find(|r| r.normalized == "escapists"); assert!(e.is_some()); assert!(e.unwrap().balance.as_deref().unwrap_or("").starts_with('+')); }
#[test] fn test_template_with_anagram_length_enforced() { for r in sw("e........;cats") { assert_eq!(r.normalized.len(), 9, "wrong length: {}", r.normalized); } }
#[test] fn test_wildcard_with_anagram() { let r = sw("e*;cats"); let k = keys(&r); assert!(k.contains(&"escalator")); assert!(k.contains(&"escapists")); }
#[test] fn test_choice_list_vowel_start() { let r = sw("[aeiou]...."); assert!(!r.is_empty()); for res in &r { assert_eq!(res.normalized.len(), 5); assert!("aeiou".contains(res.normalized.chars().next().unwrap())); } }
#[test] fn test_choice_list_negated() { let r = sw("[^aeiou]..."); assert!(!r.is_empty()); for res in &r { assert_eq!(res.normalized.len(), 4); assert!(!"aeiou".contains(res.normalized.chars().next().unwrap())); } }
#[test] fn test_choice_list_middle() { let r = sw(".[aeiou]."); assert!(!r.is_empty()); for res in &r { assert_eq!(res.normalized.len(), 3); assert!("aeiou".contains(res.normalized.chars().nth(1).unwrap())); } }
#[test] fn test_choice_list_end() { let r = sw("....[ck]"); assert!(!r.is_empty()); for res in &r { assert_eq!(res.normalized.len(), 5); assert!("ck".contains(res.normalized.chars().last().unwrap())); } }
#[test] fn test_choice_list_in_anagram() { let r = sw(";str[aeiou]"); assert!(!r.is_empty()); for res in &r { assert_eq!(res.normalized.len(), 4); } }
#[test] fn test_macro_at_vowel_template() { let rm = sw("@...."); let m = keys(&rm); let re = sw("[aeiou]...."); let e = keys(&re); assert_eq!(m, e); }
#[test] fn test_macro_hash_consonant_template() { let rm = sw("#..."); let m = keys(&rm); let re = sw("[^aeiou]..."); let e = keys(&re); assert_eq!(m, e); }
#[test] fn test_macro_in_anagram() { let rm = sw(";str@"); let m = keys(&rm); let re = sw(";str[aeiou]"); let e = keys(&re); assert_eq!(m, e); }
#[test] fn test_letter_variable_palindrome_5() { let r = sw("12321"); let k = keys(&r); assert!(k.contains(&"level")); assert!(k.contains(&"radar")); assert!(k.contains(&"civic")); for res in &r { let cs: Vec<char> = res.normalized.chars().collect(); assert_eq!(cs[0], cs[4]); assert_eq!(cs[1], cs[3]); } }
#[test] fn test_letter_variable_palindrome_7() { let r = sw("1234321"); assert!(keys(&r).contains(&"repaper")); for res in &r { let cs: Vec<char> = res.normalized.chars().collect(); assert_eq!(cs[0], cs[6]); assert_eq!(cs[1], cs[5]); assert_eq!(cs[2], cs[4]); } }
#[test] fn test_letter_variable_same_first_last() { let r = sw("1...1"); assert!(!r.is_empty()); for res in &r { assert_eq!(res.normalized.len(), 5); let cs: Vec<char> = res.normalized.chars().collect(); assert_eq!(cs[0], cs[4]); } }
#[test] fn test_letter_variable_tautonym() { let r = sw("123123"); assert!(keys(&r).contains(&"murmur")); for res in &r { let cs: Vec<char> = res.normalized.chars().collect(); assert_eq!(res.normalized.len(), 6); assert_eq!(cs[0], cs[3]); assert_eq!(cs[1], cs[4]); assert_eq!(cs[2], cs[5]); } }
#[test] fn test_logical_and_basic() { let r = sw("c* & *s"); assert!(!r.is_empty()); for res in &r { assert!(res.normalized.starts_with('c')); assert!(res.normalized.ends_with('s')); } }
#[test] fn test_logical_or_basic() { let r = sw("c... | ...s"); assert!(!r.is_empty()); for res in &r { assert_eq!(res.normalized.len(), 4); assert!(res.normalized.starts_with('c') || res.normalized.ends_with('s')); } }
#[test] fn test_logical_not_basic() { let r = sw("c* & !cat*"); assert!(!r.is_empty()); for res in &r { assert!(res.normalized.starts_with('c')); assert!(!res.normalized.starts_with("cat")); } }
#[test] fn test_logical_grouped_or() { let r = sw("(c... | ...r)"); assert!(!r.is_empty()); for res in &r { assert_eq!(res.normalized.len(), 4); assert!(res.normalized.starts_with('c') || res.normalized.ends_with('r')); } }
#[test] fn test_deduplication_groups_variants() { let r = sw("e........"); let e = r.iter().find(|r| r.normalized == "escargots"); assert!(e.is_some()); assert_eq!(e.unwrap().variants.len(), 1); assert!(e.unwrap().variants.contains(&"escargot's".to_string())); }
#[test] fn test_sort_by_length() { let r = sw(".*"); for i in 1..r.len() { assert!(r[i].normalized.len() >= r[i-1].normalized.len()); } }
#[test] fn test_subpattern_anagram_in_template() { let r = sw("...(;orange)"); let k = keys(&r); assert!(k.contains(&"patronage"), "patronage should match ...(;orange), got: {:?}", k); for res in &r { assert_eq!(res.normalized.len(), 9); let last6: Vec<char> = res.normalized.chars().skip(3).collect(); let mut s = last6.clone(); s.sort(); let mut o: Vec<char> = "orange".chars().collect(); o.sort(); assert_eq!(s, o); } }
#[test] fn test_subpattern_template_in_anagram() { let r = sw(";rebel(ada)"); assert!(keys(&r).contains(&"readable"), "readable should match ;rebel(ada), got: {:?}", keys(&r)); for res in &r { assert!(res.normalized.contains("ada")); } }
#[test] fn test_subpattern_anagram_in_anagram() { let r = sw(";umber(;lily)"); assert!(keys(&r).contains(&"beryllium"), "beryllium should match ;umber(;lily), got: {:?}", keys(&r)); }
#[test] fn test_punct_hyphenated_4_2_2() { let r = sw_raw("....-..-.."); let k = keys(&r); assert!(k.iter().any(|&w| w == "pick-me-up" || w == "well-to-do"), "got: {:?}", k); }
#[test] fn test_punct_wildcard_with_hyphen() { let r = sw_raw("*-*"); assert!(!r.is_empty()); for res in &r { assert!(res.normalized.contains('-')); } }
#[test] fn test_punct_normalize_on_ignores_punctuation() { let r = sw("e........"); assert!(r.iter().any(|r| r.normalized == "escargots")); }
#[test] fn test_validate_pattern_valid() { use crate::engine::mod_pub::validate_pattern; assert!(validate_pattern(";acenrt").is_ok()); assert!(validate_pattern(".l...r.n").is_ok()); assert!(validate_pattern("c* & !cat*").is_ok()); assert!(validate_pattern("@....").is_ok()); assert!(validate_pattern("12321").is_ok()); }
#[test] fn test_validate_pattern_empty() { use crate::engine::mod_pub::validate_pattern; assert!(validate_pattern("").is_err()); assert!(validate_pattern("   ").is_err()); }
#[test] fn test_describe_pattern_template() { use crate::engine::mod_pub::describe_pattern; let d = describe_pattern(".l...r.n").unwrap(); assert!(d.contains("8"), "should mention 8 letters: {}", d); }
#[test] fn test_describe_pattern_anagram() { use crate::engine::mod_pub::describe_pattern; let d = describe_pattern(";acenrt").unwrap(); assert!(d.to_lowercase().contains("anagram")); assert!(d.contains("ACENRT") || d.contains("acenrt")); }
#[test] fn test_describe_pattern_empty() { use crate::engine::mod_pub::describe_pattern; assert!(describe_pattern("").is_none()); }
#[test] fn test_describe_pattern_logical_and() { use crate::engine::mod_pub::describe_pattern; let d = describe_pattern("c* & *s").unwrap(); assert!(d.to_lowercase().contains("and")); }
#[test] fn test_describe_pattern_logical_or() { use crate::engine::mod_pub::describe_pattern; let d = describe_pattern("c... | ...r").unwrap(); assert!(d.to_lowercase().contains("or")); }
#[test] fn test_describe_pattern_logical_not() { use crate::engine::mod_pub::describe_pattern; let d = describe_pattern("c* & !cat*").unwrap(); assert!(d.to_lowercase().contains("excluding")); }
#[test] fn test_describe_pattern_punctuation() { use crate::engine::mod_pub::describe_pattern; let d = describe_pattern("...-..-..").unwrap(); assert!(d.to_lowercase().contains("punctuation")); }

// ── New: cache-backed search produces same results as plain search ─────────────
#[test]
fn test_search_cache_matches_search_words() {
    use crate::cache::{build_cache, open_cache};
    use crate::engine::search_cache;
    use tempfile::TempDir;
    use std::fs;

    let dir = TempDir::new().unwrap();
    let txt = dir.path().join("words.txt");
    let words = word_list();
    fs::write(&txt, words.join("\n")).unwrap();
    let tsc = txt.with_extension("tsc");
    build_cache(&txt, &tsc, |_, _| {}).unwrap();
    let handle = open_cache(&tsc).unwrap();

    for pattern in &[";acenrt", ".l...r.n", "c* & *s", "m*ja", "e*"] {
        let from_words = search_words(&words, pattern, 1, 50, true);
        let from_cache = search_cache(&handle, pattern, 1, 50, true);

        let mut wk: Vec<&str> = from_words.iter().map(|r| r.normalized.as_str()).collect();
        let mut ck: Vec<&str> = from_cache.iter().map(|r| r.normalized.as_str()).collect();
        wk.sort();
        ck.sort();
        assert_eq!(wk, ck, "pattern {:?}: search_words vs search_cache differ", pattern);
    }
}

// ── New: parallel calls return independent results ────────────────────────────
#[test]
fn test_concurrent_search_words_independent() {
    use std::thread;

    let words1: Vec<String> = vec!["canter".into(), "nectar".into(), "recant".into()];
    let words2: Vec<String> = vec!["maharaja".into(), "elephant".into()];

    let w1 = words1.clone();
    let w2 = words2.clone();

    let t1 = thread::spawn(move || search_words(&w1, ";acenrt", 1, 50, true));
    let t2 = thread::spawn(move || search_words(&w2, "m*ja", 1, 50, true));

    let r1 = t1.join().unwrap();
    let r2 = t2.join().unwrap();

    assert!(keys(&r1).contains(&"canter"));
    assert!(keys(&r2).contains(&"maharaja"));
    // Results are independent — no cross-contamination.
    assert!(!keys(&r1).contains(&"maharaja"));
    assert!(!keys(&r2).contains(&"canter"));
}
