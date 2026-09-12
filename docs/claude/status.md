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
- [x] Word-length prefix (`5:`, `5-:`, `-5:`, `5-8:`)

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
- [x] Draggable divider between stacked panes
- [x] Column layout (side-by-side panes with layout toggle in View → Window Layout)
- [x] Draggable reference panel column in column view (resizable, persisted)
- [x] Cancel button — Search → Cancel during active search; cancels via AtomicBool flag
- [x] Configurable search timeout (default 30s, persisted)
- [x] Normalize=OFF anagram fix — punctuation stripped before letter-set matching
- [x] Dictionary lookup (FreeDictionary API) via right-click → Look up definition
- [x] External lookup — per-list URL template with `{term}` token; embedded iframe panel with Open in Browser
- [x] Incremental streaming results — `search:list-result-partial` events per length bucket; skeleton replaced on first hit
- [x] Result cap (`maxResults`, default 5,000) — configurable via Options menu → Max Results…; prevents IPC backpressure deadlock on negated/broad patterns; shows amber truncation notice when hit
- [x] Batch size cap (`MAX_BATCH_SIZE = 500`) — IPC events never exceed 500 entries; prevents multi-MB single events
- [x] Search-loop allocation avoidance — `Cow<str>`-based candidate matching (`grouping.rs`), fixed-array `MatchContext` (`matcher.rs`), single-pass `GroupBuilder` (`grouping.rs`), fixed-array `CharCounts` for anagram matching (`matcher.rs`); see `implementation-notes-engine.md`
- [x] Virtualized results rendering (`@tanstack/react-virtual`) + throttled streaming accumulation — fixes large-list freeze at high `maxResults`; see `implementation-notes-ui.md`
- [x] Native menu checkmark sync (`sync_menu_state`) — menu checkmarks now match restored session state and actually flip on click for Description/Normalize/Fold Accents
- [x] Accent folding option (`foldAccents`) — "andre" optionally matches "André"; opt-in, default off. `.tsc` format bumped to v2 (adds precomputed `orig_lower`/`fold` fields, drops unused on-disk `sort_key`) with version-based rebuild detection; see `word-lists.md` and `implementation-notes-engine.md`
- [x] Word List Layout (Grid/List) and Variants (Show/Hide) moved from inline Options-row buttons to View menu; "Layout" renamed to "Window Layout" to disambiguate from the new Word List Layout
- [x] Grid view: collapsible length-group headers (matching List view), tighter chip sizing (row height 34px → 28px; fixed CSS Grid `align-items: stretch` making chips look ~1.5x too tall)
- [x] List view selection-highlight fix — `bg-white` was applied unconditionally alongside a conditional `bg-blue-50`, so which one rendered depended on Tailwind's generated stylesheet order, not click state; made mutually exclusive via ternary (matching Grid view's already-correct pattern)
- [x] Word length as a pattern prefix (`5:`, `5-:`, `-5:`, `5-8:`) — replaces the old "Word length" window option; parsed once in `engine::parser::parse_length_prefix` before the rest of the pattern, so `search_words`/`search_cache`/CLI/`search` Tauri command no longer take separate min/max-length params; see `implementation-notes-engine.md`
- [x] Options moved to a native "Options" menu (Normalize submenu: Remove Punctuation/Fold Accents; Max Results…/Timeout… open a dialog) — removes the main-window Options row and the View menu's Options toggle entirely; no matching-behavior change, same `normalize`/`foldAccents` flags as before; see `implementation-notes-ui.md`
- [x] Pop out Pattern Reference to its own window — View → Pattern Reference → Pop Out to Window (or the panel's own "Pop out ↗" button); a separate `WebviewWindow` (`open_reference_window`/`close_reference_window`) shows the Full or Compact table, live-synced to the main window's style; clicking a pattern there runs the search in the main window; closing the window (Dock button, native close, or Cmd+W) docks it back; see `implementation-notes-ui.md`
- [x] File → Open Dictionaries Folder… — native folder picker (`tauri-plugin-dialog`, Rust-side, no capabilities/ACL changes needed) lets the user point the app at any folder of `.txt` word lists; `AppState.dict_dir` is now a `Mutex<PathBuf>` so it can change at runtime; switching rescans metadata immediately then reopens all Ready lists' mmap handles in the background (same pattern as startup); persisted via `dictionariesDir` in the settings store and replayed with `set_dictionaries_dir` at launch; fixes packaged `.app` builds opening with zero word lists since `find_dict_dir()`'s "next to the binary" fallback can't resolve inside a `.app` bundle; see `implementation-notes-backend.md`

### Phase 4 — definitions and lookup
- [ ] Definition window
- [ ] Full text search mode
- [ ] External lookup (web search)
- [ ] Navigation history (back/forward)

### Phase 5 — polish
- [ ] Export results (text file)
- [ ] Print / print preview
- [ ] Sorting options (alphabetical, by length)
- [ ] Filtering (proper nouns, hyphenated, phrases)

---

## UI features implemented

- Native macOS menu bar (File, Edit, View, Options)
- **File menu:** Open Dictionaries Folder… (native picker, switches word list source folder at runtime and persists it), Manage Word Lists…
- **View menu:** Pattern Reference (Full/Compact/Off, Pop Out to Window), Pattern Description toggle, Word List Layout (Grid/List), Variants (Show/Hide), Window Layout (Rows/Columns), Appearance (Light/Dark/System), Reset to Default Layout
- **Options menu:** Normalize submenu (Remove Punctuation, Fold Accents — independent checkboxes), Max Results… (dialog, default 5,000), Timeout… (dialog, default 30s)
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
  - [x] Draggable divider between stacked panes
  - [x] Column layout with draggable dividers and resizable reference column
  - [x] Cancel button + search timeout + normalize=OFF anagram fix
  - [x] Incremental per-bucket result streaming (`search:list-result-partial`)
  - [x] Result cap (`maxResults`) + batch size cap (`MAX_BATCH_SIZE`) — fixes deadlock on negated/broad patterns against large lists
- [ ] Phase 4: Definition window, full text search, external lookup
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
