# Project Status — Coffee Crossword

## TEA feature set (implementation status)

### Phase 1 — core search ✅ Complete
- [x] Template matching (`.` `?` match-all, `*` wildcard)
- [x] Anagram search (`;` prefix, exact and with blanks)
- [x] Anagram wildcard (`*` in anagram part)
- [x] Template + anagram combined patterns
- [x] Anagram balances (`+D`, `-JX`)
- [x] Results list with length sorting and grouping

### Phase 2 — power features ✅ Complete
- [x] Choice lists (`[aeiou]`, `[^aeiou]`)
- [x] Macros (`@` = vowel, `#` = consonant)
- [x] Letter variables (digits 0–9)
- [x] Logical operations (`&`, `|`, `!`) with grouping `()`
- [x] Sub-patterns `()` — type-switching inside patterns
- [x] Punctuation matching

### Phase 3 — word list management ✅ Complete (initial)
- [x] `.tsc` binary cache format with mmap
- [x] Text file YAML front matter headers
- [x] Registry with per-list cache state machine
- [x] Explicit Build/Rebuild Index in List Manager drawer
- [x] Multi-list parallel search with streaming results
- [x] Cross-list deduplication (on by default)
- [x] Stacked multi-list results UI (independent scrolling panes)
- [x] Right-side sliding Word List drawer
- [x] CLI: multi-dict, --build-cache, --no-cache, --no-dedup
- [ ] Draggable divider between stacked panes (deferred)
- [ ] Column layout (deferred to Phase 4)

### Phase 4 — definitions and lookup
- [ ] Definition window
- [ ] Full text search mode
- [ ] External lookup (web search)
- [ ] Navigation history (back/forward)
- [ ] Column layout for multi-list results

### Phase 5 — polish
- [ ] Export results (text file)
- [ ] Print / print preview
- [ ] Sorting options (alphabetical, by length)
- [ ] Filtering (proper nouns, hyphenated, phrases)

---

## UI features implemented

- Native macOS menu bar (File, Edit, View)
- **View menu:** Pattern Reference (Full/Compact/Off), Pattern Description toggle, Options toggle, Appearance (Light/Dark/System), Reset to Default Layout
- Dark mode: Apple-style neutral grays (`#1c1c1e` / `#2c2c2e` / `#3a3a3c`)
- Pattern history: 100 entries, persisted, runs search on selection
- Reference panel pattern clicks run search immediately
- Word selection: click, Cmd+click, Shift+click
- Right-click context menu: Copy (enabled), others disabled placeholders
- Status bar: selection count
- Settings persistence via `tauri-plugin-store`
- Scrollable results with fixed header
- Pattern description: 500ms debounce, Rust `describe_pattern`

---

## Current status checklist

- [x] Architecture designed, stack selected
- [x] Prerequisites installed (Node, Rust, Xcode tools)
- [x] GitHub repo: https://github.com/irwando/coffee-crossword
- [x] Tauri scaffold verified building
- [x] TailwindCSS v4 installed
- [x] Word list loaded (SCOWL-based, ~101k words, `dictionaries/english.txt`)
- [x] Template matching
- [x] Anagram search (exact, blanks, wildcard)
- [x] Template + anagram combined
- [x] Anagram balances
- [x] Choice lists and negated choice lists
- [x] Macros (`@`, `#`)
- [x] Letter variables
- [x] Logical operations (`&`, `|`, `!`)
- [x] Sub-patterns `()`
- [x] Punctuation matching
- [x] Normalization + variant grouping
- [x] Native macOS menu bar
- [x] Pattern Reference panel (Full/Compact/Off)
- [x] Pattern description (Rust `describe_pattern`)
- [x] Dark mode
- [x] Pattern history
- [x] Word selection + right-click context menu
- [x] Settings persistence
- [x] CLI binary (`ccli`)
- [x] Public engine API
- [x] engine/ module split
- [x] README and DICTIONARY_FORMAT.md written
- [x] **Phase 3: Word list management — initial implementation complete, tests passing**
  - [x] `cache.rs` — `.tsc` build/read/mmap
  - [x] `registry.rs` — list discovery and state tracking
  - [x] `dedup.rs` — cross-list deduplication
  - [x] `engine/mod.rs` — `search_cache` entry point
  - [x] `lib.rs` — new AppState, new commands, build_in_progress
  - [x] `ccli.rs` — multi-dict, --build-cache, --no-cache, --no-dedup
  - [x] `ResultsColumn.tsx`
  - [x] `WordListDrawer.tsx`
  - [x] `App.tsx` — multi-list UI, streaming events
  - [x] Menu wiring ("Manage Word Lists…" + Cmd+Shift+L)
  - [x] Tests for all new modules
  - [ ] Draggable divider between stacked panes (deferred)
- [ ] Phase 4: Definition window, full text search, external lookup, column layout
- [ ] Phase 5: Export, print, sorting, filtering

---

## Implementation plan — Word List Management

**Status: Complete. All tests passing.**

### Implementation order (completed)

1. `cache.rs` — build `.tsc` from `.txt`; mmap wrapper; `CacheHandle`; tests
2. `registry.rs` — scan, load, save, cache state checks; `CacheState` enum; tests
3. `dedup.rs` — deduplication logic; tests
4. Update `engine/mod.rs` — add `search_cache` entry point
5. Update `AppState` in `lib.rs` — replace `words`/`dict_name` with registry + cache handles; `build_in_progress: AtomicBool`
6. New Tauri commands — `get_registry`, `set_active_lists`, `set_dedup_enabled`, `build_list_cache`, `rename_list`
7. Updated `search` command — streaming events, `search_cache` backed, parallel Tokio tasks
8. Update `ccli.rs` — multi-dict, `--build-cache`, `--no-cache`, `--no-dedup`, updated `--dicts`
9. `ResultsColumn.tsx` — column with skeleton loading state
10. `WordListDrawer.tsx` — drawer with per-list state, build/rebuild buttons, progress bars
11. Update `App.tsx` — streaming search events, stacked multi-list render, drawer wiring, search-disabled-during-build state
12. Menu wiring in `lib.rs` — "Manage Word Lists…" + Cmd+Shift+L
13. All tests passing
