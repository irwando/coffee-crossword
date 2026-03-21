# CLAUDE.md — Coffee Crossword Project Context

This file is read by Claude at the start of every session. Keep it up to date
as decisions are made. It is the single source of truth for project context.

---

## What this project is

A modern, cross-platform reimplementation of **TEA (The Electronic Alveary)**,
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
Accepts tradeoffs: tighter Rust coupling, smaller community than Electron, more
complex debugging surface. These are acceptable given the product's requirements.

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

### JavaScript (`package.json`)

| Package | Purpose |
|---|---|
| `@tauri-apps/api` | Tauri JS bridge |
| `@tauri-apps/plugin-store` | Settings persistence |
| `@tauri-apps/plugin-clipboard-manager` | Copy to clipboard |
| `@tailwindcss/vite` | Tailwind v4 Vite plugin |
| `react` + `react-dom` | UI framework |
| `typescript` | Type checking |
| `vite` | Build tool |

**When adding a new Rust plugin:** add to `Cargo.toml`, register in `lib.rs` with
`.plugin(...)`, add permissions to `src-tauri/capabilities/default.json`.

---

## Repository structure (actual)

```
/
├── CLAUDE.md                  ← this file
├── README.md                  ← human-facing project description (to be written)
├── DICTIONARY_FORMAT.md       ← spec for the open dictionary format (to be written)
├── tailwind.config.js         ← Tailwind dark mode config (darkMode: 'class')
├── src/                       ← React frontend (Vite + TypeScript)
│   ├── App.tsx                ← main UI component (all UI here for now; split at Phase 3)
│   ├── main.tsx               ← entry point (StrictMode removed — see decisions)
│   └── index.css              ← Tailwind import + manual dark mode overrides
├── src-tauri/                 ← Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs            ← Tauri entry point
│   │   ├── lib.rs             ← app state, menu setup, Tauri commands
│   │   ├── engine.rs          ← all pattern matching logic + tests (split planned before Phase 3)
│   │   └── bin/
│   │       └── ccli.rs        ← CLI binary
│   ├── capabilities/
│   │   └── default.json       ← plugin permissions (update when adding plugins)
│   ├── Cargo.toml             ← default-run = "app" required due to multiple bin targets
│   └── tauri.conf.json
├── dictionaries/
│   └── english.txt            ← SCOWL-based word list (~101k words)
├── tools/                     ← utility scripts (dict converters, etc.)
└── docs/
    └── tea-original-help/     ← original TEA HTML help files (reference only)
```

---

## Architecture overview

```
UI Layer (React / App.tsx)
    ↕  Tauri commands (async IPC via invoke())
Rust Backend (lib.rs)
    — app state (word list, dict name)
    — native menu bar construction
    — menu event → frontend event bridge
    — exposes: search_words, validate_pattern, describe_pattern as Tauri commands
    ↕
Engine (engine.rs)  ← pure Rust, no Tauri dependencies
    — pattern parsing (parse_logical → LogicalExpr tree)
    — pattern matching (template, anagram, wildcard, choice list, macros,
                        letter variables, logical ops)
    — result grouping, deduplication, variant handling
    — anagram balance calculation
    — PUBLIC API (for CLI, Python, WASM):
        search_words(words, pattern_str, min, max, normalize) → Vec<MatchGroup>
        validate_pattern(pattern: &str) → Result<(), String>
        describe_pattern(pattern: &str) → Option<String>
        normalize(word: &str) → String
    ↕
CLI (bin/ccli.rs)              ← calls engine public API directly
Data Layer
    — dictionaries/english.txt (plain UTF-8, one word per line)
    — settings.json (persisted via tauri-plugin-store)
```

### Engine public API (stable — all callers depend on this)

```rust
pub fn search_words(words: &[String], pattern: &str, min_len: usize,
                    max_len: usize, normalize: bool) -> Vec<MatchGroup>
pub fn validate_pattern(pattern: &str) -> Result<(), String>
pub fn describe_pattern(pattern: &str) -> Option<String>
pub fn normalize(word: &str) -> String

pub struct MatchGroup {
    pub normalized: String,
    pub variants: Vec<String>,
    pub balance: Option<String>,
}
```

Everything else in `engine.rs` is private. `LogicalExpr`, `Pattern`, `TemplateChar`,
`MatchContext` are all internal implementation details.

---

## Planned: engine.rs module split (before Phase 3)

`engine.rs` is currently a single file (~900 lines). Before adding Phase 3 features,
split it into a module directory:

```
engine/
├── mod.rs        ← public API (search_words, validate_pattern, describe_pattern, normalize)
├── ast.rs        ← LogicalExpr, Pattern, TemplateChar types
├── parser.rs     ← parse_logical, parse_pattern, parse_template, expand_macros
├── matcher.rs    ← matches_template, matches_anagram_*, eval_expr, MatchContext
├── normalize.rs  ← normalize(), matching_form()
├── grouping.rs   ← RawMatch, grouping/dedup logic, MatchGroup construction
├── describe.rs   ← describe_pattern, describe_simple, helper functions
├── tests.rs      ← all tests, explicit imports
└── test_utils.rs ← shared test helpers (word_list, keys); only compiled in test builds
```

This split is committed — do it before starting Phase 3 work.

---

## CLI reference (`ccli`)

### Usage

```bash
ccli [OPTIONS] "<pattern>"
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--minlen N` | 1 | Minimum word length |
| `--maxlen N` | 50 | Maximum word length |
| `--dict PATH` | auto | Dictionary file |
| `--normalize <true\|false>` | true | Strip punctuation before matching |
| `--balances` | off | Show anagram balances after results |
| `--format plain\|json\|tsv` | plain | Output format |
| `--quiet` | off | Results only, no summary line |
| `--describe` | — | Print pattern description, don't search |
| `--validate` | — | Validate pattern, don't search (exit 0/1) |
| `--dicts` | — | Show active dictionaries, don't search |
| `--version` | — | Show version |
| `--help` | — | Show usage |

### Important: shell quoting
Patterns containing `!` must use single quotes to prevent bash history expansion:
```bash
ccli 'c* & !cat*'    # correct
ccli "c* & !cat*"    # WRONG — shell expands !cat*
```

### Default dictionary search order
1. Next to the binary (`./dictionaries/english.txt`)
2. `~/Library/Application Support/coffee-crossword/dictionaries/english.txt` (macOS)
3. `CCLI_DICT` environment variable
4. `../dictionaries/english.txt` (relative to cwd, useful during development)

### Example outputs (all verified correct)

```bash
$ ccli ";acenrt"
canter
nectar
recant
trance
4 matches

$ ccli --balances ";eiknrr."
drinker +D
1 match

$ ccli --quiet ";acenrt"
canter
nectar
recant
trance

$ ccli --describe ";acenrt"
Anagrams of "ACENRT"

$ ccli --validate "[aeiou]..."
valid

$ ccli --validate "[aeiou"
Error: Invalid pattern

$ ccli --format json ";acenrt"
[
  {"normalized":"canter","variants":[],"balance":null},
  {"normalized":"nectar","variants":[],"balance":null},
  {"normalized":"recant","variants":[],"balance":null},
  {"normalized":"trance","variants":[],"balance":null}
]

$ ccli --dicts
english   /path/to/dictionaries/english.txt   (101368 words)
```

---

## Tauri plugins in use

| Plugin | Purpose | Permissions needed |
|---|---|---|
| `tauri-plugin-store` | Settings persistence | `store:allow-load`, `store:allow-set`, `store:allow-get`, `store:allow-save` |
| `tauri-plugin-clipboard-manager` | Copy to clipboard | `clipboard-manager:allow-write-text`, `clipboard-manager:allow-read-text` |
| `tauri-plugin-log` | Debug logging (dev only) | — |

---

## Dictionary / word list system

### Current (MVP)
- Single list loaded at startup from `dictionaries/english.txt`
- Display name = filename without extension (e.g. "english")
- Shown in results header as "X matches from english"

### Future: list management
- Each list has a file path, a file name, and a user-editable display name
- Lists have a priority order (higher priority = searched first)
- List management UI to be designed later

### Future: multi-list display modes

**Merged mode (default):**
- All lists searched together, results combined into one set
- Deduplication applied across all lists
- Header shows "english + 2 more" format
- Priority order determines which list "owns" a result for dedup purposes

**Separate mode:**
- Each list shows its own results column (stacked or side-by-side, user choice)
- Results are prioritized — a word found in list 1 is suppressed from list 2,
  list 2 suppresses from list 3, etc. (matches TEA behavior exactly)
- Each column shows its own list name and match count

---

## TEA feature set (implementation status)

### Phase 1 — core search ✅ Complete
- [x] Template matching: `.` and `?` as match-all characters
- [x] Anagram search: `;` prefix
- [x] Wildcards: `*` for zero or more letters (in template and anagram parts)
- [x] Anagram wildcard: `;cats*` finds words containing all of CATS plus any extras
- [x] Templates combined with anagrams: `e.....;cats` → enacts
- [x] Anagram balances: show `+D` and `-JX` after results
- [x] Results list with length sorting and grouping

### Phase 2 — power features ✅ Complete
- [x] Choice lists: `[aeiou]`, `[^aeiou]` in templates and anagram parts
- [x] Macros: `@` (vowel) and `#` (consonant)
- [x] Letter variables: digits `0–9` for positional matching
- [x] Logical operations: `&`, `|`, `!` with grouping via `()`

### Pre-Phase 3 — planned refactoring
- [ ] Split `engine.rs` into module directory (see planned split above)
- [ ] README written for open source release

### Phase 3 — definitions and lookup
- [ ] Definition window
- [ ] Full text search mode
- [ ] External lookup (web search) — placeholder in right-click menu
- [ ] Navigation history (back/forward)

### Phase 4 — polish
- [ ] Sub-patterns: `()` — type-switching inside patterns (different from logical grouping)
- [ ] Punctuation matching
- [ ] Export results (text file) — copy to clipboard already done
- [ ] Print / print preview
- [ ] Sorting options (alphabetical, by length)
- [ ] Filtering (proper nouns, hyphenated, phrases)
- [ ] Multiple dictionary support (UI and engine)

---

## Key TEA concepts Claude must understand

- **Template**: positional pattern, e.g. `.l...r.n` matches `electron`
- **Anagram**: letter-set search, prefixed with `;`, e.g. `;acenrt` → `canter`
- **Wildcard**: `*` = zero or more letters, e.g. `m*ja` → `maharaja`
- **Anagram wildcard**: `*` in anagram part = any number of extra letters, e.g. `;cats*`
- **Choice list**: `[abc]` = any one of those letters; `[^abc]` = any letter except those
- **Macro**: `@` expands to `[aeiou]`, `#` expands to `[^aeiou]` — pre-processing step
- **Letter variable**: digits 0–9 match the same letter consistently, e.g. `1234321` finds palindromes
- **Anagram balance**: leftover or added letters shown as `-JX` or `+D`
- **Template + anagram**: semicolon separates template from anagram, e.g. `e.....;cats`
- **Logical ops**: `&` (AND), `|` (OR), `!` (NOT) combine patterns
- **Logical grouping**: `()` groups logical expressions for precedence
- **Sub-pattern**: `()` type-switching feature (Phase 4) — different from logical grouping
- **Full text search**: searches word *definitions* rather than word forms (not yet implemented)
- **Headword**: the main entry word in a dictionary result
- **Annotation**: short note appended to a result entry (shown inline)
- **Dictionary definition**: longer explanation shown in a separate definition window
- **TSD file**: original TEA binary dictionary format — not using this format

---

## UI features implemented

- Native macOS menu bar (File, Edit, View)
- **View menu:**
  - Pattern Reference submenu: Full (table) / Compact (inline) / Off — radio style, persisted
  - Pattern Description toggle — persisted
  - Options toggle — persisted
  - Appearance submenu: Light / Dark / System — radio style, persisted
  - Reset to Default Layout
- Dark mode: Apple-style neutral grays (`#1c1c1e` / `#2c2c2e` / `#3a3a3c`)
- Pattern history: 100 entries, persisted, most recent first; selecting runs search immediately
- Clicking a pattern in the reference panel runs search immediately
- Word selection in list and grid view: click, Cmd+click, Shift+click
- Right-click context menu: Copy (enabled), Look up definition / External dictionary / Copy to word list (disabled placeholders)
- Status bar: shows selection count when words selected
- Settings persistence via `tauri-plugin-store`
- Scrollable results with fixed header
- Pattern description box: always visible, 500ms debounce, powered by Rust `describe_pattern`

---

## Coding conventions

- **Rust**: snake_case for functions and variables, PascalCase for types/structs
- **TypeScript/React**: PascalCase for components, camelCase for functions/variables
- **File naming**: kebab-case for all files
- **Tests**: every Rust engine function must have unit tests; use `#[cfg(test)]` modules
- **Error handling**: Rust functions return `Result<T, E>`; never use `.unwrap()` in non-test code
- **Comments**: explain *why*, not *what*; the code explains what
- **Engine stays Tauri-free**: `engine.rs` must never import any Tauri crates — it is shared
  by the CLI, future WASM, and future Python bindings
- **App.tsx internal structure**: use comment section headers (`// ── Search state ──` etc.)
  to keep the file navigable until the Phase 3 split

---

## Standing rules (always follow these)

### Pattern reference maintenance
Every time a new pattern type is implemented, ALL of the following must be updated together:
- `REFERENCE_ROWS` array in `App.tsx` — both Full table and Compact grid render from this
- `describe_pattern` function in `engine.rs` — must correctly describe the new pattern
- A test in `engine.rs` covering the new pattern
- The feature list in this file

### Test word list rule
The test word list in `engine.rs` must contain at least one word that matches
every test pattern. Before writing a new test, verify the word list has a
qualifying word and add one explicitly if not. Never write a test that depends
on a word not in the test word list.

### Example pattern validation rule
Any time an example pattern + match pair is written (in `REFERENCE_ROWS`, CLI help,
tests, documentation, or anywhere else), manually verify that the match word actually
satisfies the pattern before including it. Check:
- Correct length (count the dots/positions carefully)
- All template positions match the example word
- All required anagram letters are present in the word
- Any wildcards/choice lists/macros/letter variables are satisfied

Never include an unverified example — it wastes debugging time and confuses users.

### Test cross-product rule
Each pattern type must be tested:
1. Standalone
2. In combination with every other pattern type at least once

Before adding a new pattern type, audit existing tests for gaps and add missing
combinations. Consider property-based testing (proptest crate) when the combination
space becomes too large to enumerate manually.

### Engine public API stability
The four public functions (`search_words`, `validate_pattern`, `describe_pattern`,
`normalize`) and `MatchGroup` struct are the stable API surface. The CLI, Tauri
commands, and future Python/WASM callers all depend on these. Do not change their
signatures without considering all callers.

### File download path
When Claude provides files for download and the user downloads them as a zip,
assume files are located at `~/Downloads/files/`. Always write copy commands
using this path.

---

## Implementation notes

### Normalization
A user-facing toggle, **on by default**, controls how words are matched and measured.

**On (default):**
- Strip all non-letter, non-digit characters before matching and length calculation
- Unicode letters count; digits count (`catch-22` → `catch22` = 7 chars)
- Results are deduplicated and grouped by canonical normalized form
- Variants (original forms that differ from canonical) shown based on variant mode setting

**Off:**
- All characters count literally including apostrophes, hyphens, spaces
- No deduplication — each dictionary entry shown separately

### Variant display modes (only active when normalize is ON)
Two-way toggle in the UI:
- **Show** — canonical word with variants in parentheses: `escargots (escargot's)`
- **Hide** — canonical word only, no variants shown

Variant logic lives in the Rust engine (not UI) so all future clients get it for free:
- Grouping key = `normalize(word).to_lowercase()`
- A word is a variant only if its lowercase form differs from the key

### Pattern input
The search input box has `autoCorrect`, `autoCapitalize`, and `spellCheck` disabled.
This is critical — macOS autocorrect converts `...` to `…` which breaks patterns.

### Macro expansion
Macros are expanded as a pre-processing step at the top of `parse_logical`, before
any other parsing: `@` → `[aeiou]`, `#` → `[^aeiou]`. This makes macros work
transparently in templates, anagrams, and choice list positions.

### Letter variable matching
Letter variables (digits 0–9) are tracked via a `MatchContext` struct passed through
the template matching functions. Each digit maps to at most one letter. Non-exclusive
mode (default): different digits can map to the same letter. Exclusive mode (future
setting): each digit must map to a different letter.

### LogicalExpr internal structure
`LogicalExpr` is internal — not part of the public API:
```rust
enum LogicalExpr {
    Single(Pattern),
    And(Box<LogicalExpr>, Box<LogicalExpr>),
    Or(Box<LogicalExpr>, Box<LogicalExpr>),
    Not(Box<LogicalExpr>),
}
```
`search_words` parses the input string into a `LogicalExpr` tree internally.
Callers never see this type.

### TemplateWithAnagram length handling
When the template part contains a wildcard (e.g. `e*;cats`), skip the fixed-length
check. Only enforce `word_len == template_fixed_len` when there are no wildcards
in the template part.

### Dark mode implementation
Tailwind v4's `dark:` variant classes are not being generated correctly with
`@tailwindcss/vite`. Workaround: manual `.dark .class` overrides in `index.css`.
The `dark` or `light` class is toggled on `document.documentElement` by `applyTheme()`.
System mode uses a `MediaQueryList` listener to track `prefers-color-scheme` in real time.

### React StrictMode
StrictMode has been removed from `main.tsx`. This is a **temporary constraint**, not
a casual decision. The root cause is an impedance mismatch: React's StrictMode
double-invokes effects synchronously (`setup → cleanup → setup`), but Tauri's
`listen()` API is async — the unlisten function is only available after a Promise
resolves. This creates race conditions in dev-only double-invocation that cause menu
event listeners to register twice, making each menu click fire twice and cancel
toggles. Revisit when Tauri's async event API improves or a clean workaround emerges.

### Menu architecture
The native menu bar is built in `lib.rs` using Tauri's menu API. Menu events are
emitted from Rust to the frontend webview using `Emitter::emit(window, ...)`.
The React frontend listens with `listen()` from `@tauri-apps/api/event`.
Appearance and Pattern Reference use radio-style mutual exclusion — manually
unchecking others when one is selected, since Tauri has no native radio menu item.
Window label is `"main"` (Tauri default when no label specified in `tauri.conf.json`).

### describe_pattern for logical expressions
For simple patterns, `describe_pattern` returns a human-readable description.
For complex logical expressions (containing `&`, `|`, `!`), it returns `"Complex pattern"`.
Full description of logical expressions is deferred — the stub is intentional.

### Multiple binary targets
`Cargo.toml` has both `app` (Tauri) and `ccli` as binary targets. This requires
`default-run = "app"` in `[package]` to avoid ambiguity when Tauri runs `cargo run`.
The engine module must be `pub mod engine` (not `mod engine`) in `lib.rs` so the
CLI binary can import from it.

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

# Run CLI directly with cargo
cd src-tauri && cargo run --bin ccli -- ";acenrt"
cargo run --bin ccli -- --help

# Note: patterns with ! require single quotes in bash
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
- [x] Template matching (`.` `?` match-all, `*` wildcard)
- [x] Anagram search (`;` prefix, exact and with blanks)
- [x] Anagram wildcard (`*` in anagram part = unlimited extras)
- [x] Template + anagram combined patterns
- [x] Anagram balances (`+D`, `-JX`)
- [x] Choice lists (`[aeiou]`, `[^aeiou]`)
- [x] Macros (`@` = vowel, `#` = consonant)
- [x] Letter variables (digits 0–9, palindromes, tautonyms)
- [x] Logical operations (`&`, `|`, `!`) with grouping `()`
- [x] Normalization + variant grouping in Rust engine
- [x] Native macOS menu bar (File, Edit, View)
- [x] Pattern Reference panel: Full / Compact / Off modes
- [x] Pattern description (500ms debounce, Rust `describe_pattern`)
- [x] Dark mode (Apple-style neutral grays, persisted)
- [x] Pattern history (100 entries, persisted, runs search on selection)
- [x] Clicking reference panel pattern runs search immediately
- [x] Word selection (click, Cmd+click, Shift+click)
- [x] Right-click context menu with Copy
- [x] Status bar (selection count)
- [x] Settings persistence
- [x] CLI binary (`ccli`) with full option set
- [x] Public engine API (`search_words`, `validate_pattern`, `describe_pattern`, `normalize`)
- [ ] engine.rs module split (before Phase 3)
- [ ] README written
- [ ] DICTIONARY_FORMAT.md written
- [ ] Phase 3: Definition window, full text search, external lookup
- [ ] Phase 4: Sub-patterns, punctuation matching, sorting, filtering

---

## Decisions log

> Record significant decisions here with brief rationale. Never delete entries.

| Date | Decision | Rationale |
|---|---|---|
| 2026-03 | Use Tauri over Electron | Performance, binary size, native feel; accepts: tighter Rust coupling, smaller community, more complex debugging |
| 2026-03 | Use Rust for engine | Speed parity with original C++ TEA, compiles to WASM |
| 2026-03 | Replace TSD with open format | TSD is proprietary binary; open format enables tooling and contribution |
| 2026-03 | Start with Mac + browser, defer mobile | Reduces scope; mobile can reuse engine later |
| 2026-03 | React over Flutter for UI | Simpler stack for PM-led development; web skills transfer |
| 2026-03 | Remove StrictMode | Temporary constraint: impedance mismatch between React's sync effect lifecycle and Tauri's async listen() API causes race conditions in dev double-invocation; revisit when Tauri API improves |
| 2026-03 | Manual dark mode CSS overrides | Tailwind v4 dark: variant not generating correctly with @tailwindcss/vite |
| 2026-03 | Apple neutral grays for dark mode | Blue-tinted grays looked wrong; #1c1c1e/#2c2c2e/#3a3a3c match macOS |
| 2026-03 | Pattern Reference: Full/Compact/Off | Power users want minimal UI; new users need guidance — give them a choice |
| 2026-03 | History and reference clicks run search immediately | More efficient UX — one click instead of two |
| 2026-03 | Variant logic in Rust engine, not UI | All future clients (browser, mobile) get deduplication for free |
| 2026-03 | REFERENCE_ROWS as single source of truth | Both Full and Compact reference panels render from one array — avoids drift |
| 2026-03 | describe_pattern moved to Rust | CLI, WASM, Python can all describe patterns without reimplementing the logic |
| 2026-03 | Engine public API: 4 functions only | Keeps CLI, Python, WASM integration surface minimal and stable |
| 2026-03 | describe_pattern stubs complex logical expressions | Full logical description deferred; stub avoids misleading output |
| 2026-03 | CLI dict search order: binary-relative, then macOS app support, then env var | Sensible defaults for both dev and installed usage |
| 2026-03 | default-run = "app" in Cargo.toml | Adding [[bin]] for ccli creates ambiguity; default-run resolves it |
| 2026-03 | pub mod engine in lib.rs | CLI binary needs to import from engine; private mod prevents cross-binary access |
| 2026-03 | Delay App.tsx component split until Phase 3 | No meaningful architectural seam exists yet; definition window is the natural boundary |
| 2026-03 | Vec<MatchGroup> return type (not streaming) | Current scale (~100k words, hundreds of results) doesn't warrant streaming complexity; design toward it |

---

## TSD format research (background reference)

### TSD1 format (example dictionary files) — fully cracked
- Magic header: first 4 bytes are `TSD1` (ASCII)
- Text content XOR-encoded with `0xBD`, starting at ~offset `0x14F0`
- Source `.txt` format: `word+ annotation`, `word|definition`, `word:variant|def`

### TSD0 format (Core English and main dictionaries) — deferred
- Uses a DAWG (Directed Acyclic Word Graph) — not a simple XOR encoding
- Requires proper reverse engineering — deferred, using SCOWL instead for now

### Strategy
- Use plain UTF-8 word lists as our native format
- TSD0 extraction deferred indefinitely — SCOWL covers our needs

---

## Known word list gaps

- Possessive forms (e.g. `canter's`) not included in SCOWL
- TEA's dictionaries were refined over many years — parity is a long-term goal
- Revisit when tackling TSD0 reverse engineering or finding a better word list

---

## Reference material

- Original TEA help files: `docs/tea-original-help/*.htm`
- TEA home page (archived): http://www.crosswordman.com/tea.html
- SCOWL word lists: http://wordlist.aspell.net/
- WordNet: https://wordnet.princeton.edu/
- Tauri docs: https://tauri.app/
- Rust book: https://doc.rust-lang.org/book/
- Clap docs: https://docs.rs/clap/latest/clap/
- Proptest (property-based testing): https://docs.rs/proptest/latest/proptest/