// ── Matcher ───────────────────────────────────────────────────────────────────
// Evaluates LogicalExpr and Pattern against individual words.
// MatchContext is private to this module — no other module needs it.
//
// Raw word threading: when a pattern contains Punct or CasedLiteral chars,
// matching must happen against the original word (lowercased only, punctuation
// preserved) rather than the fully normalized form. grouping.rs passes both
// forms to eval_expr; eval_pattern selects which to use based on pattern content.

use crate::engine::ast::{AnagramChar, LogicalExpr, Pattern, SubPattern, TemplateChar};

/// Carries letter variable bindings through template matching.
/// Variables are single digits (0-9, see parser.rs), so a fixed array avoids
/// the heap allocation a HashMap would incur on every backtracking clone.
#[derive(Clone, Copy)]
struct MatchContext {
    variables: [Option<char>; 10],
}

impl MatchContext {
    fn new() -> Self {
        MatchContext { variables: [None; 10] }
    }

    fn bind(&mut self, var: u8, ch: char) -> bool {
        match self.variables[var as usize] {
            Some(existing) => existing == ch,
            None => {
                self.variables[var as usize] = Some(ch);
                true
            }
        }
    }
}

/// Small char -> count multiset used by anagram matching, avoiding a
/// `HashMap` per candidate word on the hot search-loop path. Dictionary
/// words/phrases are almost always ASCII a-z/0-9 (counted here with no heap
/// allocation), but `normalize()` keeps any Unicode letter (`char::is_alphabetic`),
/// which real lists — e.g. Wikipedia titles with accented names — do contain.
/// A hard-coded a-z/0-9-only array would silently miscount those, so any
/// other character spills into a small `Vec` fallback instead (still correct,
/// just not allocation-free — exactly as before for that rare case).
#[derive(Clone)]
struct CharCounts {
    ascii: [i32; 36],
    other: Vec<(char, i32)>,
}

impl Default for CharCounts {
    fn default() -> Self {
        CharCounts { ascii: [0; 36], other: Vec::new() }
    }
}

impl CharCounts {
    fn ascii_slot(ch: char) -> Option<usize> {
        match ch {
            'a'..='z' => Some((ch as u8 - b'a') as usize),
            '0'..='9' => Some(26 + (ch as u8 - b'0') as usize),
            _ => None,
        }
    }

    fn ascii_char(slot: usize) -> char {
        if slot < 26 {
            (b'a' + slot as u8) as char
        } else {
            (b'0' + (slot - 26) as u8) as char
        }
    }

    fn add(&mut self, ch: char, delta: i32) {
        match Self::ascii_slot(ch) {
            Some(i) => self.ascii[i] += delta,
            None => match self.other.iter_mut().find(|(c, _)| *c == ch) {
                Some(entry) => entry.1 += delta,
                None => self.other.push((ch, delta)),
            },
        }
    }

    fn get(&self, ch: char) -> i32 {
        match Self::ascii_slot(ch) {
            Some(i) => self.ascii[i],
            None => self.other.iter().find(|(c, _)| *c == ch).map(|&(_, v)| v).unwrap_or(0),
        }
    }

    /// Sum of all positive counts (mirrors `.values().filter(|v| v > 0).sum()`
    /// on the `HashMap<char, i32>` this replaces).
    fn sum_positive(&self) -> i32 {
        self.ascii.iter().filter(|&&v| v > 0).sum::<i32>()
            + self.other.iter().filter(|(_, v)| *v > 0).map(|&(_, v)| v).sum::<i32>()
    }

    /// Every char with a positive count, repeated that many times, sorted —
    /// mirrors the `flat_map(...).collect(); .sort()` pattern this replaces.
    fn chars_with_repeats_sorted(&self) -> Vec<char> {
        let mut result: Vec<char> = Vec::new();
        for (slot, &count) in self.ascii.iter().enumerate() {
            if count > 0 {
                result.extend(std::iter::repeat(Self::ascii_char(slot)).take(count as usize));
            }
        }
        for &(ch, count) in &self.other {
            if count > 0 {
                result.extend(std::iter::repeat(ch).take(count as usize));
            }
        }
        result.sort();
        result
    }
}

/// Returns true if the template contains any Punct or CasedLiteral chars,
/// meaning we must match against the raw (unpunctuated) word form.
fn template_needs_raw(template: &[TemplateChar]) -> bool {
    template.iter().any(|t| matches!(t, TemplateChar::Punct(_) | TemplateChar::CasedLiteral(_)))
}

/// Evaluate a logical expression against a word.
/// raw_word: lowercased only, punctuation preserved.
/// norm_word: fully normalized (letters/digits only, lowercased).
/// pub(crate) — called from grouping.rs.
pub(crate) fn eval_expr(raw_word: &str, norm_word: &str, word_len: usize, expr: &LogicalExpr) -> Option<String> {
    match expr {
        LogicalExpr::Single(pattern) => eval_pattern(raw_word, norm_word, word_len, pattern),
        LogicalExpr::And(left, right) => {
            eval_expr(raw_word, norm_word, word_len, left)?;
            eval_expr(raw_word, norm_word, word_len, right)
        }
        LogicalExpr::Or(left, right) => {
            eval_expr(raw_word, norm_word, word_len, left)
                .or_else(|| eval_expr(raw_word, norm_word, word_len, right))
        }
        LogicalExpr::Not(inner) => {
            if eval_expr(raw_word, norm_word, word_len, inner).is_some() {
                None
            } else {
                Some(String::new())
            }
        }
    }
}

fn eval_pattern(raw_word: &str, norm_word: &str, _word_len: usize, pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Template(template) => {
            // Use raw word if pattern contains punctuation or cased literals
            let word = if template_needs_raw(template) { raw_word } else { norm_word };
            if matches_template(word, template) {
                Some(String::new())
            } else {
                None
            }
        }
        Pattern::Anagram(anagram_chars, dots, has_wildcard) => {
            // Anagram matching always uses normalized word
            matches_anagram_exact(norm_word, anagram_chars, *dots, *has_wildcard)
        }
        Pattern::TemplateWithAnagram(template, anagram_chars, dots) => {
            let word = if template_needs_raw(template) { raw_word } else { norm_word };
            let has_wildcard = template.iter().any(|t| matches!(t, TemplateChar::Wildcard));
            let length_ok = if has_wildcard {
                true
            } else {
                word.chars().count() == template_fixed_len(template)
            };
            if length_ok && matches_template(word, template) {
                let wlen = word.chars().count();
                let free_positions = if has_wildcard {
                    let fixed_letters: usize = anagram_chars.iter().map(|ac| anagram_char_len(ac)).sum();
                    wlen.saturating_sub(fixed_letters)
                } else {
                    template
                        .iter()
                        .filter(|t| !matches!(t,
                            TemplateChar::Literal(_) |
                            TemplateChar::Punct(_) |
                            TemplateChar::CasedLiteral(_) |
                            TemplateChar::SubPattern(_)
                        ))
                        .count()
                };
                let effective_dots = Some(free_positions + dots.unwrap_or(0));
                matches_anagram_within(norm_word, anagram_chars, effective_dots)
            } else {
                None
            }
        }
    }
}

fn anagram_char_len(ac: &AnagramChar) -> usize {
    match ac {
        AnagramChar::Letter(_) => 1,
        AnagramChar::Blank => 1,
        AnagramChar::ChoiceList(_, _) => 1,
        AnagramChar::SubPattern(SubPattern::Template(tmpl)) => template_fixed_len(tmpl),
        AnagramChar::SubPattern(SubPattern::Anagram(letters)) => letters.len(),
        AnagramChar::SubPattern(SubPattern::AnagramInAnagram(letters)) => letters.len(),
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

fn char_matches(ch: char, t: &TemplateChar, ctx: &mut MatchContext) -> bool {
    match t {
        TemplateChar::Literal(c) => *c == ch,
        TemplateChar::Any => ch.is_alphabetic() || ch.is_ascii_digit(),
        TemplateChar::Wildcard => unreachable!(),
        TemplateChar::SubPattern(_) => unreachable!("SubPattern spans multiple chars, handled separately"),
        TemplateChar::ChoiceList(letters, negated) => {
            let contains = letters.contains(&ch);
            if *negated { !contains } else { contains }
        }
        TemplateChar::Variable(v) => ctx.bind(*v, ch),
        TemplateChar::Punct(c) => *c == ch,
        TemplateChar::CasedLiteral(c) => *c == ch, // caller passes raw char preserving case
    }
}

fn matches_subpattern_anagram(word_slice: &[char], letters: &[char]) -> bool {
    if word_slice.len() != letters.len() {
        return false;
    }
    let mut available = CharCounts::default();
    for &ch in letters {
        available.add(ch, 1);
    }
    for &ch in word_slice {
        if available.get(ch) <= 0 {
            return false;
        }
        available.add(ch, -1);
    }
    true
}

fn matches_subpattern_template(word_slice: &[char], template: &[TemplateChar]) -> bool {
    let mut ctx = MatchContext::new();
    matches_template_slice(word_slice, template, &mut ctx)
}

fn matches_template_slice(word: &[char], template: &[TemplateChar], ctx: &mut MatchContext) -> bool {
    if template.is_empty() {
        return word.is_empty();
    }
    match &template[0] {
        TemplateChar::Wildcard => {
            for i in 0..=word.len() {
                let mut ctx_clone = *ctx;
                if matches_template_slice(&word[i..], &template[1..], &mut ctx_clone) {
                    *ctx = ctx_clone;
                    return true;
                }
            }
            false
        }
        TemplateChar::SubPattern(sp) => {
            let sp_len = match sp {
                SubPattern::Anagram(letters) => letters.len(),
                SubPattern::Template(tmpl) => template_fixed_len(tmpl),
                SubPattern::AnagramInAnagram(letters) => letters.len(),
            };
            if word.len() < sp_len {
                return false;
            }
            let matches = match sp {
                SubPattern::Anagram(letters) => matches_subpattern_anagram(&word[..sp_len], letters),
                SubPattern::Template(tmpl) => matches_subpattern_template(&word[..sp_len], tmpl),
                SubPattern::AnagramInAnagram(letters) => matches_subpattern_anagram(&word[..sp_len], letters),
            };
            if matches {
                matches_template_slice(&word[sp_len..], &template[1..], ctx)
            } else {
                false
            }
        }
        t => {
            if word.is_empty() {
                return false;
            }
            let mut ctx_clone = *ctx;
            if char_matches(word[0], t, &mut ctx_clone) {
                if matches_template_slice(&word[1..], &template[1..], &mut ctx_clone) {
                    *ctx = ctx_clone;
                    return true;
                }
            }
            false
        }
    }
}

fn matches_template(word: &str, template: &[TemplateChar]) -> bool {
    let word_chars: Vec<char> = word.chars().collect();
    let has_wildcard = template.iter().any(|t| matches!(t, TemplateChar::Wildcard));
    let mut ctx = MatchContext::new();

    if !has_wildcard {
        if word_chars.len() != template_fixed_len(template) {
            return false;
        }
        return matches_template_slice(&word_chars, template, &mut ctx);
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
                let mut ctx_clone = *ctx;
                if matches_template_wildcard(&word[i..], &template[1..], &mut ctx_clone) {
                    *ctx = ctx_clone;
                    return true;
                }
            }
            false
        }
        TemplateChar::SubPattern(sp) => {
            let sp_len = match sp {
                SubPattern::Anagram(letters) => letters.len(),
                SubPattern::Template(tmpl) => template_fixed_len(tmpl),
                SubPattern::AnagramInAnagram(letters) => letters.len(),
            };
            if word.len() < sp_len {
                return false;
            }
            let matches = match sp {
                SubPattern::Anagram(letters) => matches_subpattern_anagram(&word[..sp_len], letters),
                SubPattern::Template(tmpl) => matches_subpattern_template(&word[..sp_len], tmpl),
                SubPattern::AnagramInAnagram(letters) => matches_subpattern_anagram(&word[..sp_len], letters),
            };
            if matches {
                matches_template_wildcard(&word[sp_len..], &template[1..], ctx)
            } else {
                false
            }
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

fn decompose_anagram_chars(
    anagram_chars: &[AnagramChar],
) -> (Vec<char>, Vec<Vec<TemplateChar>>, Vec<Vec<char>>, usize, Vec<(Vec<char>, bool)>) {
    let mut plain_letters: Vec<char> = Vec::new();
    let mut template_subs: Vec<Vec<TemplateChar>> = Vec::new();
    let mut anagram_subs: Vec<Vec<char>> = Vec::new();
    let mut blank_count = 0usize;
    let mut choice_slots: Vec<(Vec<char>, bool)> = Vec::new();

    for ac in anagram_chars {
        match ac {
            AnagramChar::Letter(c) => plain_letters.push(*c),
            AnagramChar::Blank => blank_count += 1,
            AnagramChar::ChoiceList(letters, negated) => {
                choice_slots.push((letters.clone(), *negated));
            }
            AnagramChar::SubPattern(SubPattern::Template(tmpl)) => {
                template_subs.push(tmpl.clone());
            }
            AnagramChar::SubPattern(SubPattern::Anagram(letters)) => {
                anagram_subs.push(letters.clone());
            }
            AnagramChar::SubPattern(SubPattern::AnagramInAnagram(letters)) => {
                anagram_subs.push(letters.clone());
            }
        }
    }

    (plain_letters, template_subs, anagram_subs, blank_count, choice_slots)
}

fn matches_anagram_exact(
    word: &str,
    anagram_chars: &[AnagramChar],
    dot_count: Option<usize>,
    has_wildcard: bool,
) -> Option<String> {
    // Strip punctuation before building the character set: anagram matching is a
    // letter-set operation, so apostrophes and other punctuation in raw words
    // (e.g. "canter's" with normalize=off) must not count as letters.
    let word_letters: String = word.chars().filter(|c| c.is_alphabetic() || c.is_ascii_digit()).collect();
    let word_chars: Vec<char> = word_letters.chars().collect();

    let (plain_letters, template_subs, anagram_subs, _blank_count, choice_slots) =
        decompose_anagram_chars(anagram_chars);

    let sub_len: usize = template_subs.iter().map(|t| template_fixed_len(t)).sum::<usize>()
        + anagram_subs.iter().map(|a| a.len()).sum::<usize>();
    let total_required = plain_letters.len() + sub_len + choice_slots.len();

    if !has_wildcard {
        let expected_len = total_required + dot_count.unwrap_or(0);
        if word_chars.len() != expected_len {
            return None;
        }
    }

    let mut used: Vec<bool> = vec![false; word_chars.len()];

    for tmpl in &template_subs {
        let seq_len = template_fixed_len(tmpl);
        let mut found = false;
        'outer: for start in 0..=word_chars.len().saturating_sub(seq_len) {
            if (start..start + seq_len).any(|i| used[i]) {
                continue;
            }
            if matches_subpattern_template(&word_chars[start..start + seq_len], tmpl) {
                for i in start..start + seq_len {
                    used[i] = true;
                }
                found = true;
                break 'outer;
            }
        }
        if !found {
            return None;
        }
    }

    for sub_letters in &anagram_subs {
        let mut remaining = sub_letters.clone();
        let mut sub_used: Vec<usize> = Vec::new();
        for (i, &wc) in word_chars.iter().enumerate() {
            if used[i] { continue; }
            if let Some(pos) = remaining.iter().position(|&c| c == wc) {
                remaining.remove(pos);
                sub_used.push(i);
                if remaining.is_empty() { break; }
            }
        }
        if !remaining.is_empty() { return None; }
        for i in sub_used { used[i] = true; }
    }

    let unused_chars: Vec<char> = word_chars
        .iter()
        .enumerate()
        .filter(|(i, _)| !used[*i])
        .map(|(_, &c)| c)
        .collect();

    let mut remaining_unused = unused_chars.clone();
    for (choice_letters, negated) in &choice_slots {
        let pos = remaining_unused.iter().position(|&c| {
            let contains = choice_letters.contains(&c);
            if *negated { !contains } else { contains }
        });
        match pos {
            Some(p) => { remaining_unused.remove(p); }
            None => return None,
        }
    }

    let mut available = CharCounts::default();
    for &ch in &plain_letters {
        available.add(ch, 1);
    }

    let mut needed = CharCounts::default();
    for &ch in &remaining_unused {
        if available.get(ch) > 0 {
            available.add(ch, -1);
        } else {
            needed.add(ch, 1);
        }
    }

    let missing_required = available.sum_positive();
    if missing_required > 0 { return None; }

    let extra_count = needed.sum_positive();
    if !has_wildcard {
        let blanks_available = (dot_count.unwrap_or(0) as i32 - choice_slots.len() as i32).max(0);
        if extra_count > blanks_available { return None; }
    }

    let omitted = available.chars_with_repeats_sorted();
    let added = needed.chars_with_repeats_sorted();

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
    anagram_chars: &[AnagramChar],
    dot_count: Option<usize>,
) -> Option<String> {
    let word_chars: Vec<char> = word.chars().collect();

    let (plain_letters, template_subs, anagram_subs, _blank_count, choice_slots) =
        decompose_anagram_chars(anagram_chars);

    let mut used: Vec<bool> = vec![false; word_chars.len()];

    for tmpl in &template_subs {
        let seq_len = template_fixed_len(tmpl);
        let mut found = false;
        'outer: for start in 0..=word_chars.len().saturating_sub(seq_len) {
            if (start..start + seq_len).any(|i| used[i]) { continue; }
            if matches_subpattern_template(&word_chars[start..start + seq_len], tmpl) {
                for i in start..start + seq_len { used[i] = true; }
                found = true;
                break 'outer;
            }
        }
        if !found { return None; }
    }

    for sub_letters in &anagram_subs {
        let mut remaining = sub_letters.clone();
        let mut sub_used: Vec<usize> = Vec::new();
        for (i, &wc) in word_chars.iter().enumerate() {
            if used[i] { continue; }
            if let Some(pos) = remaining.iter().position(|&c| c == wc) {
                remaining.remove(pos);
                sub_used.push(i);
                if remaining.is_empty() { break; }
            }
        }
        if !remaining.is_empty() { return None; }
        for i in sub_used { used[i] = true; }
    }

    let unused_chars: Vec<char> = word_chars
        .iter()
        .enumerate()
        .filter(|(i, _)| !used[*i])
        .map(|(_, &c)| c)
        .collect();

    let mut remaining_unused = unused_chars.clone();
    for (choice_letters, negated) in &choice_slots {
        let pos = remaining_unused.iter().position(|&c| {
            let contains = choice_letters.contains(&c);
            if *negated { !contains } else { contains }
        });
        match pos {
            Some(p) => { remaining_unused.remove(p); }
            None => return None,
        }
    }

    let mut available = CharCounts::default();
    for &ch in &plain_letters {
        available.add(ch, 1);
    }

    let mut extra: Vec<char> = Vec::new();
    for &ch in &remaining_unused {
        if available.get(ch) > 0 {
            available.add(ch, -1);
        } else {
            extra.push(ch);
        }
    }

    if available.sum_positive() > 0 { return None; }

    let effective_dots = dot_count.unwrap_or(0).saturating_sub(choice_slots.len());
    if extra.len() > effective_dots { return None; }

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
