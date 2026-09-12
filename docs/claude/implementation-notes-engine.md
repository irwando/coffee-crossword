# Implementation Notes — Engine & Pattern Matching

> Part of the implementation-notes split — see also
> `implementation-notes-ui.md` (menus, rendering, React state) and
> `implementation-notes-backend.md` (Tauri state, caching, concurrency).

## Macro expansion
Pre-processing step: `@` → `[aeiou]`, `#` → `[^aeiou]` before any other parsing.

## Letter variable matching
`MatchContext` struct tracks digit→letter bindings. Non-exclusive by default.

## Normalize=OFF anagram matching
Anagram matching is a letter-set operation — punctuation (apostrophes, hyphens)
must not count as letters. In `matcher.rs`, `matches_anagram_exact` strips
non-alphabetic/non-digit characters from the word before building `word_chars`,
regardless of normalize mode. Without this, `canter's` had 8 chars and failed
the length check against a 7-letter anagram set.

## Search-loop allocation avoidance
Two hot-path allocation sources were removed (both fire per *scanned candidate*,
not just per match, so they scale with dictionary size, not result count):

- `grouping.rs`'s `candidate_word()` returns `Cow<str>`: `normalize=true`
  borrows `entry.norm` directly (already lowercased/stripped at cache-build
  time — re-lowercasing it was pure waste), `normalize=false` allocates once.
  Only converted to an owned `String` (`into_owned()`) on an actual match.
- `matcher.rs`'s `MatchContext` stores letter-variable bindings (digits 0-9
  only, see `parser.rs`) in a `Copy` `[Option<char>; 10]` array instead of a
  `HashMap<u8, char>`, so backtracking clones (`*ctx` instead of `ctx.clone()`)
  are a stack copy instead of a heap allocation.

Also fixed, same class of issue:
- `search_cache_streaming` previously built `MatchGroup`s once per batch and
  again over the full result set at the end, cloning each `RawMatch` twice to
  keep two parallel accumulations. `GroupBuilder` (`grouping.rs`) now groups
  incrementally in one pass; `drain_batch()` returns a streaming snapshot,
  `finish()` returns the final sorted result with no re-grouping needed.
- Anagram matching (`matches_anagram_exact`/`matches_anagram_within`/
  `matches_subpattern_anagram`) used a `HashMap<char, i32>` per candidate.
  `CharCounts` (`matcher.rs`) replaces it with a fixed `[i32; 36]` array for
  a-z/0-9 (the common case, zero-alloc) with a small `Vec` fallback for any
  other Unicode character — a hard-coded ASCII-only array would silently
  miscount accented letters, which real lists (Wikipedia titles) do contain.

## Accent folding (fold-accents option)
`normalize()` only lowercases and strips punctuation — `é` and `e` are
different characters to it, so `andre` never matched `André`. Fold-accents is
a separate, opt-in toggle (default off) that additionally strips Unicode
combining diacritical marks (NFD decomposition + filter U+0300–U+036F).

Applied at two points, both mirroring existing patterns rather than touching
the matcher:
- **The pattern string** is folded once before parsing (`fold_pattern()` in
  `engine/mod.rs`) — the same pre-processing-step pattern already used for
  macro expansion. `parser.rs`/`ast.rs`/`matcher.rs` have no idea fold-accents
  exists; they just match whatever characters the (possibly folded) pattern
  and word have.
- **The candidate word** — precomputed at cache-build time as a 4th `.tsc`
  field (`fold`) for the zero-alloc `normalize=on` path, since folding is
  applied per *scanned candidate* on every search rather than once. See
  `word-lists.md`'s "Binary cache format" section for the full field layout
  and format-version/rebuild-detection design. `normalize=off` + fold-accents
  on is the one allocating combination (folds each candidate on the fly
  rather than reading a precomputed field) — `normalize=off` is already the
  narrowest of the four modes, so a 5th on-disk field wasn't worth it.

## Word-length prefix (replaces the "Word length" window option)
Word length moved from a `minLen`/`maxLen` UI option into the pattern text
itself: `5:cat*` (exactly 5), `5-:cat*` (at least 5), `-5:cat*` (at most 5),
`5-8:cat*` (5 to 8) — min word length is always 1. `engine::parser::parse_length_prefix`
strips this prefix off the front of the raw pattern string (before macro
expansion / accent folding / logical parsing) and returns `(min_len, max_len,
remainder)`, defaulting to `(1, usize::MAX)` when no prefix is present.

Because of this, `search_words`, `search_cache`, and the internal
`search_cache_cancellable_streaming` no longer take `min_len`/`max_len`
parameters at all — the only place length is specified is the pattern string.
`grouping.rs`'s internal search loops are unchanged (still parameterized by
min/max length; `usize::MAX` is naturally capped by the existing
`max_len.min(255)` cache-bucket logic, so an absurd `9999-:` prefix just
yields an empty range rather than a panic). `describe_pattern` and
`validate_pattern` strip the same prefix so `5:` alone (no pattern after the
colon) is correctly treated as empty/invalid, and `describe_pattern` prepends
a clause ("Exactly 5 letters, …") when a prefix is present.

A pattern starting with `-` (the `-X:` form) looks like a CLI flag to clap —
`ccli`'s call sites need `ccli -- '-5:cat*'`; not an issue for the frontend's
plain text input.
