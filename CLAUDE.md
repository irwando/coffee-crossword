# CLAUDE.md — Coffee Crossword Project Context

This file is read by Claude at the start of every session. Keep it up to date
as decisions are made. It is the single source of truth for project context.

---

## What this project is

A modern, cross-platform reimplementation of **TEA (The TA Crossword Helper)**,
a Windows word-search tool used for solving crossword puzzles and other word
games. TEA is no longer maintained. This project recreates its functionality
with a modern stack that runs on Mac, browser, iOS, Android, and Windows.

The original TEA help documentation has been preserved as HTML files and lives
in `docs/tea-original-help/`. These are the authoritative reference for what
features to implement and how they should behave.

---

## Owner context

- Product Manager background, not a fluent developer
- Comfortable with architecture and product thinking
- Limited language-specific coding experience (some Python)
- No prior Rust experience — Claude writes all Rust code
- Working with Claude as the primary coding assistant
- Intended to open source when ready

---

## Stack decisions

| Layer | Technology | Reason |
|---|---|---|
| Desktop app | Tauri v2 | Native Mac .app, small binary, Rust backend, reuses web UI |
| UI framework | React + TypeScript + Vite | Familiar ecosystem, works in Tauri and browser |
| Search engine | Rust | C++-class performance, compiles to native and WASM |
| Browser target | WASM build of Rust engine | Same code, no JS fallback |
| Mobile (future) | React Native or Flutter | Deferred — not in scope yet |
| Styling | TailwindCSS v4 | Utility-first, fast iteration |
| CLI | Rust binary + clap v4 | Same engine, no extra runtime |

### Why not Electron
Electron bundles a full Chromium copy (~200MB app). Tauri uses the OS webview
and a Rust backend, resulting in a ~3–10MB app with genuinely native performance.

### Why not Flutter
Flutter ships its own renderer, adding complexity and diverging from web
standards. Tauri lets us ship one web UI everywhere.

---

## Dependencies

### Rust (`src-tauri/Cargo.toml`)

| Crate | Version | Purpose |
|---|---|---|
| `tauri` | 2.10.3 | Desktop app framework |
| `tauri-build` | 2.5.6 | Build tooling |
| `tauri-plugin-store` | 2.4.2 | Settings persistence |
| `tauri-plugin-clipboard-manager` | 2.3.2 | Copy to clipboard |
| `tauri-plugin-log` | 2 | Debug logging (dev only) |
| `serde` | 1.0 | Serialization (derive feature) |
| `serde_json` | 1.0 | JSON output for CLI |
| `clap` | 4.5.61 | CLI argument parsing (derive feature) |
| `log` | 0.4 | Logging facade |
| `memmap2` | 0.9 | Memory-mapped file access for `.tsc` cache files |
| `futures` | 0.3 | `join_all` for parallel list search |

**When adding a new Rust plugin:** add to `Cargo.toml`, register in `lib.rs` with
`.plugin(...)`, add permissions to `src-tauri/capabilities/default.json`.

---

## Repository structure (actual)

```
/
├── CLAUDE.md                  ← this file
├── README.md
├── DICTIONARY_FORMAT.md       ← spec for the open dictionary format
├── tailwind.config.js
├── src/                       ← React frontend (Vite + TypeScript)
│   ├── App.tsx                ← main UI component
│   ├── ResultsColumn.tsx      ← [PLANNED] single-list results column
│   ├── WordListDrawer.tsx     ← [PLANNED] right-side sliding drawer
│   ├── main.tsx
│   └── index.css
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs             ← app state, menu setup, Tauri commands
│   │   ├── cache.rs           ← [PLANNED] .tsc build/read/mmap
│   │   ├── registry.rs        ← [PLANNED] list discovery, state tracking
│   │   ├── dedup.rs           ← [PLANNED] cross-list deduplication
│   │   └── engine/
│   │       ├── mod.rs         ← public API
│   │       ├── ast.rs
│   │       ├── parser.rs
│   │       ├── matcher.rs
│   │       ├── normalize.rs
│   │       ├── grouping.rs
│   │       ├── describe.rs
│   │       ├── tests.rs
│   │       └── test_utils.rs
│   └── bin/
│       └── ccli.rs            ← CLI binary
├── dictionaries/
│   ├── english.txt            ← SCOWL-based word list (~101k words)
│   ├── english.tsc            ← [PLANNED] binary cache (auto-generated)
│   └── wikipedia-en.txt       ← [PLANNED] 6.3M Wikipedia article titles
└── docs/
    └── tea-original-help/     ← original TEA HTML help files (reference only)
```

---

## Architecture overview

```
UI Layer (React / App.tsx)
    ↕  Tauri commands (async IPC via invoke()) + Tauri events (listen())
Rust Backend (lib.rs)
    — app state (registry, cache handles)
    — native menu bar construction
    — menu event → frontend event bridge
    — exposes: search, get_registry, set_active_lists, build_list_cache,
               set_dedup_enabled, rename_list, validate_pattern, describe_pattern
    ↕
Cache Layer (cache.rs)          ← NEW
    — builds .tsc from .txt
    — memory-maps .tsc for zero-heap-allocation word access
    — CacheHandle exposes iter_by_norm_len(), entry_count()
    ↕
Registry (registry.rs)          ← NEW
    — scans dictionaries/ folder at startup
    — tracks per-list CacheState (Ready/NeedsRebuild/NotBuilt/Building/Error)
    — persists active_ids ordering and dedup_enabled via tauri-plugin-store
    ↕
Engine (engine/)                ← mostly unchanged
    — search_cache(cache, pattern, min, max, normalize) → Vec<MatchGroup>  NEW
    — search_words(words, pattern, min, max, normalize) → Vec<MatchGroup>  existing
    — validate_pattern, describe_pattern, normalize
    ↕
Dedup (dedup.rs)                ← NEW
    — deduplicate(results: &mut Vec<ListSearchResult>)
    ↕
CLI (bin/ccli.rs)               ← updated
    — multi-dict, --build-cache, --no-cache, --no-dedup
```

---

## Word List Management — Full Design (PLANNED, not yet implemented)

### Overview

Like TEA's explicit Dictionary Builder requirement, Coffee Crossword requires
word lists to be indexed before use. The index (`.tsc` binary cache) is built
explicitly by the user via the List Manager drawer. This ensures predictable
performance even for very large lists (tested target: 6.3M Wikipedia titles,
125MB plain text).

### Text file header format

Word list `.txt` files may optionally begin with a YAML front matter block:

```
---
name: Wikipedia English Titles
updated: 2024-11-15
description: All English Wikipedia article titles as of November 2024.
  Approximately 6.3 million entries including multi-word phrases,
  proper nouns, and titles with punctuation.
---
Abd al-Rahman III
bacteria
United States
...
```

Rules:
- Header is **optional** — files without `---` work as-is (backward compatible)
- Only `name`, `updated`, `description` keys recognized; others ignored
- `name` overrides the display name; otherwise filename stem is used
- `updated` is informational only, shown in the List Manager drawer
- `description` shown as tooltip/detail in the drawer
- Multi-line `description` uses leading-space continuation lines
- Blank lines and `#` comment lines are skipped throughout the file body

### Binary cache format (`.tsc`)

Each `.txt` file gets a `.tsc` cache file in the same `dictionaries/` folder.
`english.txt` → `english.tsc`. The cache is memory-mapped (not loaded into heap).

```
Header block (fixed 832 bytes):
  [0..4]     magic: b"TSC1"
  [4..12]    source_mtime: u64  (unix timestamp of .txt when built)
  [12..16]   entry_count: u32
  [16..20]   data_offset: u32   (byte offset where string data begins)
  [20..276]  display_name: [u8; 256]   (null-padded)
  [276..308] source_updated: [u8; 32]  (null-padded)
  [308..820] source_desc: [u8; 512]    (null-padded)
  [820..832] reserved: [u8; 12]

Length index (1024 bytes):
  norm_length_offsets: [u32; 256]
  (norm_length_offsets[n] = first entry index for normalized length n)
  Entries are sorted by normalized length within the file.

Entry index (entry_count × 12 bytes):
  Per entry: orig_offset: u32, norm_offset: u32, sort_offset: u32
  (byte offsets into the three string sections below)

String data sections (packed null-terminated strings):
  orig_strings:  verbatim original lines
  norm_strings:  normalized (lowercase letters+digits only)
  sort_strings:  normalized letters sorted A–Z (for anagram lookup)
```

**Search paths:**
- Template search (`normalize=on`): iterate `norm_strings` in target length bucket
- Template search (`normalize=off`): iterate `orig_strings`
- Anagram search: sort query letters → scan `sort_strings` in target length bucket
- Length filtering: `norm_length_offsets` gives direct jump to right bucket

**Build time estimates:**
- `english.txt` (101k words): ~0.3 seconds
- `wikipedia-en.txt` (6.3M entries, 125MB): ~10–15 seconds

### Cache state machine

Each list in the registry has one of these states:

| State | Condition | UI |
|---|---|---|
| `Ready` | `.tsc` exists, source mtime ≤ cache mtime | Green dot, word count shown |
| `NeedsRebuild` | `.tsc` exists but `.txt` is newer | Yellow dot, "Source updated" |
| `NotBuilt` | `.txt` exists, no `.tsc` | Gray dot, "Index not built" |
| `Building` | Build in progress | Spinner + progress % |
| `Error(msg)` | Build failed | Red dot, error message |

**Only `Ready` lists can be activated for search.**

If a list transitions from `Ready` to `NeedsRebuild` between app starts (user
edited the `.txt` file), it is automatically removed from `active_ids` and the
user must rebuild before it is searchable again.

**Rebuilding:** The user can explicitly trigger a rebuild at any time from the
drawer — both for `NotBuilt` lists (first build) and `NeedsRebuild` lists
(source was updated). The button label changes accordingly: "Build Index" vs
"Rebuild Index". This is the primary mechanism for incorporating updates to a
word list.

**Search disabled during build:** while any list is building, the search input
is disabled with a clear explanation. This avoids partial-state searches and
simplifies error handling.

### Registry persistence

Stored via `tauri-plugin-store` in `settings.json`:
- `"word_list_active_ids"` — `Vec<String>` ordered by priority
- `"word_list_display_names"` — `HashMap<String, String>` user overrides
- `"dedup_enabled"` — `bool` (default: `true`)

IDs are filename stems (`"english"`, `"wikipedia-en"`). Stale IDs (file deleted)
are silently removed from `active_ids` on load.

### Deduplication

When `dedup_enabled = true` (default), words found in multiple active lists
appear only in the highest-priority list that contains them. Lower-priority
results for that word are suppressed. This matches TEA's default behavior.

When `dedup_enabled = false`, each list shows its complete results independently.

### Parallel search

Each active `Ready` list is searched in a separate Tokio task. The `search`
Tauri command:
1. Emits `search:start` with the list of active list IDs (so frontend creates
   skeleton columns immediately)
2. Spawns one task per active list
3. Each task emits `search:list-result { list_id, list_name, results, error }`
   as it completes
4. After all tasks finish, applies deduplication if enabled and emits
   `search:dedup { list_id, removed_count }` per affected list
5. Emits `search:complete`

### Layout

**Single active list:** UI is identical to the current app. No multi-list chrome.

**Multiple active lists — stacked layout (only layout for now; columns deferred):**
- Each list occupies a horizontal pane
- Panes have independent scrollbars (TEA tile-horizontal style)
- Draggable divider between panes to resize
- Equal initial height split
- Each pane has a header showing: list name, match count, loading skeleton while
  results are arriving

**Layout toggle** (only visible with 2+ active lists): in the results header bar.
Currently only "Stacked" is available; "Columns" will be added later.

### List Manager drawer

Right-side sliding drawer, opened via:
- File menu → "Manage Word Lists…" (Cmd+Shift+L)
- Dismisses on outside click or Escape

Drawer contents:
```
┌─────────────────────────────────────┐
│ Word Lists                     [✕]  │
├─────────────────────────────────────┤
│ ACTIVE (in search order)            │
│ ① english      101k words  [↑][↓]  │
│   green dot  "Built"        [Remove]│
│                                     │
│ ② wikipedia-en  —          [↑][↓]  │
│   gray dot  "Index not built"       │
│   [Build Index]             [Remove]│
│                                     │
│ AVAILABLE (not active)              │
│   my-custom    —                    │
│   yellow dot  "Source updated"      │
│   [Rebuild Index]  [Add to Active]  │
│                                     │
│ ─────────────────────────────────── │
│ ☑ Suppress duplicates across lists  │
└─────────────────────────────────────┘
```

Build progress replaces the button with a progress bar while building.

---

## CLI reference (`ccli`) — UPDATED DESIGN

### Usage

```bash
ccli [OPTIONS] "<pattern>"
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--minlen N` | 1 | Minimum word length |
| `--maxlen N` | 50 | Maximum word length |
| `--dict PATH` | (repeatable) | Dictionary file(s); if none given, scans `dictionaries/` folder |
| `--normalize <true\|false>` | true | Strip punctuation before matching |
| `--balances` | off | Show anagram balances after results |
| `--format plain\|json\|tsv` | plain | Output format |
| `--quiet` | off | Results only, no summary line |
| `--describe` | — | Print pattern description, don't search |
| `--validate` | — | Validate pattern, don't search (exit 0/1) |
| `--dicts` | — | Show all discovered lists with status |
| `--build-cache` | — | Build/rebuild index for all lists that need it, then exit |
| `--no-cache` | off | Force plain text path (slow, for debugging) |
| `--no-dedup` | off | Show full results per list (dedup on by default) |
| `--version` | — | Show version |
| `--help` | — | Show usage |

### Multi-list behavior

```bash
# Search all Ready lists in dictionaries/ folder
ccli ";acenrt"

# Search specific lists
ccli --dict english.txt --dict wikipedia-en.txt ";acenrt"

# Build all caches that need it
ccli --build-cache

# Show list status
ccli --dicts
# english         dictionaries/english.tsc        Ready   101,368 words
# wikipedia-en    dictionaries/wikipedia-en.txt   Not built  —

# Force plain text (no cache)
ccli --no-cache --dict english.txt ";acenrt"
```

Plain text output with multiple lists:
```
=== english (101,368 words) ===
canter
nectar
recant
trance
4 matches

=== wikipedia-en (6,278,994 entries) ===
2 matches
```

JSON output: array of `{ list_id, list_name, entry_count, results: [...] }`.

### Shell quoting note
Patterns containing `!` must use single quotes to prevent bash history expansion:
```bash
ccli 'c* & !cat*'
```

### Default dictionary search order
1. All `.tsc`-ready files in `dictionaries/` folder next to binary
2. `~/Library/Application Support/coffee-crossword/dictionaries/` (macOS)
3. `CCLI_DICT` environment variable (single path)
4. `../dictionaries/` relative to cwd (development)

---

## Tauri plugins in use

| Plugin | Purpose | Permissions needed |
|---|---|---|
| `tauri-plugin-store` | Settings persistence | `store:allow-load`, `store:allow-set`, `store:allow-get`, `store:allow-save` |
| `tauri-plugin-clipboard-manager` | Copy to clipboard | `clipboard-manager:allow-write-text`, `clipboard-manager:allow-read-text` |
| `tauri-plugin-log` | Debug logging (dev only) | — |

---

## Engine public API (stable)

```rust
// Existing — unchanged
pub fn search_words(words: &[String], pattern: &str,
                    min_len: usize, max_len: usize, normalize: bool) -> Vec<MatchGroup>
pub fn validate_pattern(pattern: &str) -> Result<(), String>
pub fn describe_pattern(pattern: &str) -> Option<String>
pub fn normalize(word: &str) -> String

// New — cache-backed entry point
pub fn search_cache(cache: &CacheHandle, pattern: &str,
                    min_len: usize, max_len: usize, normalize: bool) -> Vec<MatchGroup>

pub struct MatchGroup {
    pub normalized: String,
    pub variants: Vec<String>,
    pub balance: Option<String>,
}
```

---

## Tauri commands (full list after word list work)

| Command | Purpose |
|---|---|
| `search` | Run pattern against all active lists; streams events |
| `describe_pattern` | Return human-readable pattern description |
| `validate_pattern` | Validate pattern syntax |
| `get_registry` | Return current registry state to UI |
| `set_active_lists` | Replace active_ids list (persisted) |
| `set_dedup_enabled` | Toggle dedup (persisted) |
| `rename_list` | Override display name for a list (persisted) |
| `build_list_cache` | Build/rebuild `.tsc` for one list; streams build events |

## Tauri events emitted (Rust → frontend)

| Event | Payload | When |
|---|---|---|
| `search:start` | `{ active_ids: string[] }` | Search begins |
| `search:list-result` | `ListSearchResult` | Each list completes |
| `search:dedup` | `{ list_id, removed_count }` | After dedup applied |
| `search:complete` | — | All lists done |
| `build:start` | `{ list_id }` | Build begins |
| `build:progress` | `{ list_id, percent, phase }` | During build |
| `build:complete` | `{ list_id, entry_count, elapsed_ms }` | Build done |
| `build:error` | `{ list_id, message }` | Build failed |
| `menu:toggle` | `"description" \| "options"` | Menu toggle |
| `menu:reference` | `"full" \| "compact" \| "off"` | Reference mode change |
| `menu:appearance` | `"light" \| "dark" \| "system"` | Appearance change |
| `menu:reset_layout` | — | Reset layout |
| `menu:lists` | — | Open word list drawer |

---

## Implementation plan — Word List Management

**Status: PLANNED. Not yet started.**

### Implementation order

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
13. `CLAUDE.md` update (mark as complete, update status)
14. All tests passing

### New files to create

- `src-tauri/src/cache.rs`
- `src-tauri/src/registry.rs`
- `src-tauri/src/dedup.rs`
- `src/ResultsColumn.tsx`
- `src/WordListDrawer.tsx`

### Files to modify

- `src-tauri/src/lib.rs` — AppState, commands, menu
- `src-tauri/src/engine/mod.rs` — search_cache
- `src-tauri/src/bin/ccli.rs` — multi-list CLI
- `src-tauri/Cargo.toml` — memmap2, futures
- `src/App.tsx` — multi-list UI
- `CLAUDE.md` — this file

---

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

### Phase 3 — word list management 🔜 Planned (design complete)
- [ ] `.tsc` binary cache format with mmap
- [ ] Text file YAML front matter headers
- [ ] Registry with per-list cache state machine
- [ ] Explicit Build/Rebuild Index in List Manager drawer
- [ ] Multi-list parallel search with streaming results
- [ ] Cross-list deduplication (on by default)
- [ ] Stacked multi-list results UI with draggable divider
- [ ] Right-side sliding Word List drawer
- [ ] CLI: multi-dict, --build-cache, --no-cache, --no-dedup
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

## Coding conventions

- **Rust**: snake_case for functions/variables, PascalCase for types/structs
- **TypeScript/React**: PascalCase for components, camelCase for functions/variables
- **File naming**: kebab-case for all files
- **Tests**: every Rust engine/cache/registry function must have unit tests
- **Error handling**: `Result<T, E>` everywhere; never `.unwrap()` in non-test code
- **Comments**: explain *why*, not *what*
- **Engine stays Tauri-free**: `engine/` must never import any Tauri crates
- **Cache module stays engine-free**: `cache.rs` has no dependency on `engine/`

---

## Standing rules (always follow these)

### Pattern reference maintenance
Every time a new pattern type is implemented, ALL of the following must be updated:
- `REFERENCE_ROWS` array in `App.tsx`
- `describe_pattern` in `engine/describe.rs`
- A test in `engine/tests.rs`
- The feature list in this file

### Test word list rule
The test word list in `test_utils.rs` must contain at least one word matching
every test pattern. Verify before writing a new test; add words if needed.

### Example pattern validation rule
Any example pattern + match pair must be manually verified before including it.

### Test cross-product rule
Each pattern type must be tested standalone and in combination with every other
pattern type at least once.

### Engine public API stability
The four public functions (`search_words`, `validate_pattern`, `describe_pattern`,
`normalize`) and `search_cache` are the stable API surface. Do not change their
signatures without considering all callers.

### File download path
When Claude provides files for download: `~/Downloads/files/`.

---

## Implementation notes

### Normalization
Toggle on by default. On: strip non-letter/non-digit, lowercase, deduplicate variants.
Off: all characters count literally including punctuation and spaces.

Per-list normalize override: **not supported** — one global setting applies to all
lists. Users are responsible for choosing the appropriate normalize setting when
searching mixed lists (e.g. Wikipedia titles work better with normalize=off).

### Variant display modes (normalize=on only)
- **Show**: canonical word with variants in parentheses
- **Hide**: canonical word only

### Pattern input
`autoCorrect`, `autoCapitalize`, `spellCheck` all disabled — critical, macOS
autocorrect converts `...` to `…` which breaks patterns.

### Macro expansion
Pre-processing step: `@` → `[aeiou]`, `#` → `[^aeiou]` before any other parsing.

### Letter variable matching
`MatchContext` struct tracks digit→letter bindings. Non-exclusive by default.

### Dark mode
Manual `.dark .class` CSS overrides in `index.css`. `dark`/`light` toggled on
`document.documentElement` by `applyTheme()`. System mode uses `MediaQueryList`.

### React StrictMode
Removed — temporary constraint. Impedance mismatch between React's sync effect
lifecycle and Tauri's async `listen()` API causes double-registration in dev mode.

### Menu architecture
Native menu built in `lib.rs`. Events emitted Rust→frontend via `Emitter::emit`.
Frontend listens with `@tauri-apps/api/event` `listen()`.

### Multiple binary targets
`default-run = "app"` required in `Cargo.toml`. Engine module must be `pub mod`.

### mmap and cache access
`CacheHandle` wraps a `memmap2::Mmap`. The mmap is `Send + Sync` via
`Arc<CacheHandle>`. Each search task gets an `Arc` clone — zero copy.
Cache handles are stored in `AppState.cache_handles: Mutex<HashMap<String, Arc<CacheHandle>>>`.
A handle is opened once when a list becomes Ready and kept until app exit.

### Build concurrency
`AppState.build_in_progress: AtomicBool` is set `true` when any build starts
and `false` when it completes or errors. The `search` command checks this flag
and returns an error immediately if true. The UI shows a "Building index —
search unavailable" message in this state.

---

## Build and run commands

```bash
# Install dependencies
npm install

# Run in development (opens Tauri window)
npm run tauri dev

# Build for production
npm run tauri build

# Run Rust tests only
cd src-tauri && cargo test

# Run frontend only (browser, no Tauri)
npm run dev

# Build the CLI
cd src-tauri && cargo build --bin ccli

# Build all word list caches (once CLI is updated)
cd src-tauri && ./target/debug/ccli --build-cache

# Run CLI directly
cd src-tauri && cargo run --bin ccli -- ";acenrt"

# Single quotes required for patterns with !
cd src-tauri && ./target/debug/ccli 'c* & !cat*'
```

---

## Current status

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
- [ ] **Phase 3: Word list management (design complete, implementation not started)**
  - [ ] `cache.rs` — `.tsc` build/read/mmap
  - [ ] `registry.rs` — list discovery and state tracking
  - [ ] `dedup.rs` — cross-list deduplication
  - [ ] `engine/mod.rs` — `search_cache` entry point
  - [ ] `lib.rs` — new AppState, new commands, build_in_progress
  - [ ] `ccli.rs` — multi-dict, --build-cache, --no-cache, --no-dedup
  - [ ] `ResultsColumn.tsx`
  - [ ] `WordListDrawer.tsx`
  - [ ] `App.tsx` — multi-list UI, streaming events
  - [ ] Menu wiring
  - [ ] Tests for all new modules
- [ ] Phase 4: Definition window, full text search, external lookup, column layout
- [ ] Phase 5: Export, print, sorting, filtering

---

## Decisions log

> Record significant decisions here with brief rationale. Never delete entries.

| Date | Decision | Rationale |
|---|---|---|
| 2026-03 | Use Tauri over Electron | Performance, binary size, native feel |
| 2026-03 | Use Rust for engine | Speed parity with C++ TEA, compiles to WASM |
| 2026-03 | Replace TSD with open format | TSD is proprietary binary; open format enables tooling |
| 2026-03 | Start with Mac + browser, defer mobile | Reduces scope |
| 2026-03 | React over Flutter for UI | Simpler stack for PM-led development |
| 2026-03 | Remove StrictMode | Temporary: async Tauri listen() + React double-invoke race condition |
| 2026-03 | Manual dark mode CSS overrides | Tailwind v4 dark: variant not generating correctly |
| 2026-03 | Apple neutral grays for dark mode | Blue-tinted grays looked wrong |
| 2026-03 | Pattern Reference: Full/Compact/Off | Power users want minimal UI; new users need guidance |
| 2026-03 | History and reference clicks run search immediately | More efficient UX |
| 2026-03 | Variant logic in Rust engine, not UI | All future clients get deduplication for free |
| 2026-03 | REFERENCE_ROWS as single source of truth | Avoids drift between Full and Compact panels |
| 2026-03 | describe_pattern in Rust | CLI, WASM, Python can all describe patterns |
| 2026-03 | Engine public API: stable surface | Keeps CLI, Python, WASM integration minimal |
| 2026-03 | default-run = "app" in Cargo.toml | Multiple [[bin]] targets require explicit default |
| 2026-03 | pub mod engine in lib.rs | CLI binary needs cross-crate access |
| 2026-03 | Sub-pattern () as TemplateChar::SubPattern | Spans multiple chars; needs special AST handling |
| 2026-03 | Punctuation matching uses normalize toggle | No new toggle; normalize=off preserves punctuation |
| 2026-03 | Word list folder: fixed `dictionaries/` scanned at startup | Simplest model; restart to pick up new files |
| 2026-03 | List Manager UI: right-side sliding drawer | Non-blocking; user sees column layout update in real time |
| 2026-03 | Stacked layout: independent scrolling panes, draggable divider | Matches TEA tile-horizontal behavior |
| 2026-03 | Column layout: deferred | Simplify initial multi-list implementation; add later |
| 2026-03 | Dedup default: on | Matches TEA default behavior |
| 2026-03 | Layout toggle: only visible with 2+ active lists | Avoids clutter for single-list users |
| 2026-03 | Single-list UI: identical to today | No regressions for the common case |
| 2026-03 | Explicit cache build required (like TEA's Dictionary Builder) | Ensures predictable performance; user knows list state |
| 2026-03 | Binary cache format: .tsc in same dictionaries/ folder | Simple, visible, easy to delete; no hidden files |
| 2026-03 | Cache invalidation: .txt mtime vs .tsc header mtime | Simple and reliable |
| 2026-03 | Rebuild triggered explicitly by user | User controls when index is updated after editing source |
| 2026-03 | mmap for cache access | 125MB plain text list would require ~200MB heap; mmap avoids this |
| 2026-03 | Search disabled during build | Simplest correct behavior; avoids partial-state searches |
| 2026-03 | Per-list normalize override: not supported | One global setting; user responsibility for mixed lists |
| 2026-03 | Wikipedia list normalize: user's responsibility | Wikipedia titles (phrases, mixed case) suit normalize=off |
| 2026-03 | CLI --build-cache command | Same cache benefits available from command line |
| 2026-03 | CLI dedup: on by default, --no-dedup to disable | Consistent with app behavior |
| 2026-03 | Lists not assumed to be in alphabetical order | Wikipedia list (6.3M entries) is not sorted |

---

## TSD format research (background reference)

### TSD1 format — fully cracked
- Magic: `TSD1`, XOR-encoded with `0xBD` from offset `~0x14F0`
- Source format: `word+ annotation`, `word|definition`, `word:variant|def`

### TSD0 format — deferred
- DAWG-compressed; reverse engineering deferred; using SCOWL + custom .tsc instead

---

## Known word list gaps

- Possessive forms not in SCOWL
- TEA's dictionaries were refined over many years — parity is long-term
- Wikipedia list (6.3M) planned but not yet added to project

---

## Reference material

- Original TEA help files: `docs/tea-original-help/*.htm`
- TEA home page (archived): http://www.crosswordman.com/tea.html
- SCOWL word lists: http://wordlist.aspell.net/
- Tauri docs: https://tauri.app/
- memmap2 crate: https://docs.rs/memmap2/latest/memmap2/
- Rust book: https://doc.rust-lang.org/book/

## Deferred (carry into next conversation)
- ccli --normalize help text: add "e.g. --normalize false" to description
- App.tsx internal section comment headers (// ── Search state ── etc.)
