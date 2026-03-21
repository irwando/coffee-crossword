# CLAUDE.md — WordSeeker Project Context

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
| Styling | TailwindCSS | Utility-first, fast iteration |

### Why not Electron
Electron bundles a full Chromium copy (~200MB app). Tauri uses the OS webview
and a Rust backend, resulting in a ~3–10MB app with genuinely native performance.

### Why not Flutter
Flutter ships its own renderer, adding complexity and diverging from web
standards. Tauri lets us ship one web UI everywhere.

---

## Repository structure (planned)

```
/
├── CLAUDE.md                  ← this file
├── README.md                  ← human-facing project description
├── DICTIONARY_FORMAT.md       ← spec for the open dictionary format
├── src/                       ← React frontend (Vite + TypeScript)
│   ├── components/
│   ├── hooks/
│   └── App.tsx
├── src-tauri/                 ← Rust backend (Tauri)
│   ├── src/
│   │   ├── main.rs
│   │   └── engine/            ← pattern matching core
│   │       ├── mod.rs
│   │       ├── template.rs
│   │       ├── anagram.rs
│   │       ├── wildcard.rs
│   │       ├── choice_list.rs
│   │       ├── letter_variable.rs
│   │       ├── logical_ops.rs
│   │       └── dictionary.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── dictionaries/              ← word list files in open format
│   ├── core-english.tsv
│   └── ...
├── tools/                     ← utility scripts (dict converters, etc.)
└── docs/
    └── tea-original-help/     ← original TEA HTML help files (reference)
```

---

## Architecture overview

```
UI Layer (React)
    ↕  Tauri commands (async IPC)
Logic Layer (TypeScript)
    — pattern parser
    — result manager (filter, sort, anagram balances)
    — definition lookup
    — settings
    ↕  FFI
Engine Layer (Rust)
    — pattern matching core
    — dictionary index (memory-mapped)
    ↕
Data Layer
    — open dictionary format (.tsv + gzip)
    — user word list imports
    — optional TSD import/conversion
```

---

## Dictionary format (replacing TSD)

TEA used a proprietary binary `.TSD` format. We are replacing it with an open
format. Spec lives in `DICTIONARY_FORMAT.md` (to be written).

**Working assumptions:**
- Sorted UTF-8 text, one entry per line
- Tab-separated fields: `word\tannotation\tdefinition_id`
- Gzipped for distribution
- Memory-mapped at runtime for fast random access
- Build-time converter from plain text word lists (SCOWL, ENABLE, WordNet)
- Optional one-time migration path from original TSD binary files

**Open source word lists to use:**
- SCOWL (Spell Checker Oriented Word Lists) — tiered by commonness, ideal
- ENABLE — commonly used in word game software
- WordNet — for definitions

---

## TEA feature set (implementation priority order)

### Phase 1 — core search (MVP)
- [ ] Template matching: `.` and `?` as match-all characters
- [ ] Anagram search: `;` prefix
- [ ] Wildcards: `*` for zero or more letters
- [ ] Multiple dictionaries, priority ordering
- [ ] Results list with length sorting

### Phase 2 — power features
- [ ] Choice lists: `[aeiou]`, `[^aeiou]`
- [ ] Macros: `@` (vowel) and `#` (consonant), user-configurable
- [ ] Letter variables: digits `0–9` for positional matching
- [ ] Templates combined with anagrams: `e....;cats`
- [ ] Anagram balances: show `+D` and `-JX` after results
- [ ] Logical operations: `&`, `|`, `!`

### Phase 3 — definitions and lookup
- [ ] Definition window
- [ ] Full text search mode
- [ ] External lookup (web search)
- [ ] Navigation history (back/forward)

### Phase 4 — polish
- [ ] Sub-patterns: `()`
- [ ] Punctuation matching
- [ ] Export results (text file, copy to clipboard)
- [ ] Print / print preview
- [ ] Sorting options (alphabetical, by length)
- [ ] Filtering (proper nouns, hyphenated, phrases)

---

## Key TEA concepts Claude must understand

These are domain terms used throughout the codebase and in conversation:

- **Template**: positional pattern, e.g. `.l...r.n` matches `electron`
- **Anagram**: letter-set search, prefixed with `;`, e.g. `;acenrt` → `canter`
- **Wildcard**: `*` = zero or more letters, e.g. `m*ja` → `maharaja`
- **Choice list**: `[abc]` = any one of those letters; `[^abc]` = any letter except those
- **Macro**: `@` expands to `[aeiou]`, `#` expands to `[^aeiou]` by default
- **Letter variable**: digits 0–9 match the same letter consistently, e.g. `1234321` finds palindromes
- **Anagram balance**: when an anagram search has leftover or added letters, shown as `-JX` or `+D`
- **Template + anagram**: semicolon separates the template part from the anagram part, e.g. `e....;cats`
- **Logical ops**: `&` (AND), `|` (OR), `!` (NOT) combine patterns
- **Sub-pattern**: `()` groups an inner pattern of opposite type inside an outer pattern
- **Full text search**: searches word *definitions* rather than word forms
- **Headword**: the main entry word in a dictionary result
- **Annotation**: short note appended to a result entry (shown inline)
- **Dictionary definition**: longer explanation shown in a separate definition window
- **TSD file**: original TEA binary dictionary format — we are not using this format

---

## Coding conventions

- **Rust**: snake_case for functions and variables, PascalCase for types/structs
- **TypeScript/React**: PascalCase for components, camelCase for functions/variables
- **File naming**: kebab-case for all files
- **Tests**: every Rust engine function must have unit tests; use `#[cfg(test)]` modules
- **Error handling**: Rust functions return `Result<T, E>`; never use `.unwrap()` in non-test code
- **Comments**: explain *why*, not *what*; the code explains what

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
```

---

## Current status

> Update this section as the project progresses.

- [x] Architecture designed
- [x] Stack selected (Tauri + React + Rust)
- [x] Prerequisites installed (Node, Rust, Xcode tools)
- [x] GitHub repo created (https://github.com/irwando/coffee-crossword)
- [x] Tauri scaffold created and verified building
- [x] TailwindCSS installed
- [ ] DICTIONARY_FORMAT.md written
- [x] First word list converted to open format
- [x] Normalization mode implemented (strips punctuation for matching)
- [x] Variant grouping implemented in Rust engine (deduplication)
- [x] Variant display modes (show all / hover / hidden)
- [x] Pattern input autocorrect disabled
- [x] Template matching implemented in Rust
- [X] Template matching wired to React UI
---

## UI Features Implemented
- Native macOS menu bar (File, Edit, View)
- View menu: panel toggles (Pattern Reference, Description, Options), Appearance submenu (Light/Dark/System), Reset to Default Layout (pending wire-up in Rust)
- Dark mode with Apple-style neutral grays
- Pattern history (100 entries, persisted)
- Word selection in list and grid view (click, Cmd+click, Shift+click)
- Right-click context menu (Copy enabled, others placeholders)
- Status bar showing selection count
- Settings persistence via tauri-plugin-store
- Scrollable results with fixed header

---

## Dictionary / word list system

### Current (MVP)
- Single list loaded at startup from `dictionaries/english.txt`
- Display name = filename without extension (e.g. "english")

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
## Decisions log

> Record significant decisions here with brief rationale. Never delete entries.

| Date | Decision | Rationale |
|---|---|---|
| 2026-03 | Use Tauri over Electron | Performance, binary size, native feel |
| 2026-03 | Use Rust for engine | Speed parity with original C++ TEA, compiles to WASM |
| 2026-03 | Replace TSD with open format | TSD is proprietary binary; open format enables tooling and contribution |
| 2026-03 | Start with Mac + browser, defer mobile | Reduces scope; mobile can reuse engine later |
| 2026-03 | React over Flutter for UI | Simpler stack for PM-led development; web skills transfer |
| 2026-03 | Remove StrictModeDouble-invoked effects broke Tauri menu event listeners |
| 2026-03 | Manual dark mode CSS overridesTailwind v4 dark: variant not generating correctly with @tailwindcss/vite |
| 2026-03 | Apple neutral grays for dark modeBlue-tinted grays looked wrong; #1c1c1e/#2c2c2e/#3a3a3c match macOS |

---
## Bugs Fixed

- Anagram wildcard (* in anagram part) now correctly requires all specified letters and allows unlimited extras
- Template+anagram free position calculation fixed
- StrictMode removed (caused double event listener registration)
---

## Normalization setting

A user-facing toggle, **on by default**, controls how words are matched and measured.

**On (default):**
- Strip all non-letter, non-digit characters before matching and length calculation
- Unicode letters count; digits count (catch-22 → catch22 = 7 chars)
- Results are deduplicated and grouped by canonical normalized form
- Variants (original forms that differ from canonical) shown based on variant mode

**Off:**
- All characters count literally including apostrophes, hyphens, spaces
- No deduplication — each dictionary entry shown separately

## Variant display modes (only active when normalize is ON)

Three-way toggle in the UI:
- **Show all** — canonical word with variants in parentheses: `escargots (escargot's)`
- **On hover** — canonical word only; variants appear in tooltip on mouse hover
- **Hidden** — canonical word only, no variants shown

Variant logic (in Rust engine, not UI — so all future clients get it for free):
- Grouping key = `normalize(word).to_lowercase()`
- A word is a variant only if its lowercase form differs from the key
- e.g. `Escargots` lowercases to `escargots` = key → NOT a variant
- e.g. `escargot's` normalizes to `escargots` but original differs → IS a variant

## Pattern input

The search input box has autocorrect/autocapitalize/spellcheck disabled.
This is critical — macOS autocorrect converts `...` to `…` breaking patterns.

## TemplateWithAnagram length handling

When pattern has a wildcard (e.g. `e*;cats`), skip the fixed-length check and
let `matches_template` handle length. Only enforce `word_len == template_fixed_len`
when there are no wildcards in the template part.
---

## TSD file format (reverse engineered)

The original TEA binary `.TSD` format has been reverse engineered from the
example files. Key findings:

- **Magic header**: first 4 bytes are `TSD1` (ASCII)
- **Binary index**: first ~0x14F0 bytes contain a structured index (offsets,
  word lengths, flags) — not yet fully decoded
- **Text encoding**: all text content (headwords, annotations, definitions)
  is XOR-encoded with `0xBD`
- **Decoded content**: maps exactly to the source `.txt` format

### Source .txt format (the format we use natively)

Lines starting with `|` are comments/title (ignored)
Words with annotations: `abandon+ n.`  (+ separates headword from annotation)
Words with definitions: `defined|plain text definition`
Spelling variants: `agonise:agonize|shared definition`
Aliases/inflections: `bear+ n.+ n.;bears`
Escaped metacharacters: `C\+\+` for C++
RTF definitions: `rich|{\rtf1 \b rich\b0 ...}`

### Strategy
- Use `.txt` format as our native input — clean and fully understood
- Write a TSD text extractor later to recover words from full TEA
  dictionaries (XOR decode with 0xBD, parse the text section from ~0x14F0)

### TSD0 format (Core English and other main dictionaries)
- Different from TSD1 — uses a compressed DAWG (Directed Acyclic Word Graph)
- Words are NOT stored as plain strings — no simple XOR decode possible
- Requires proper reverse engineering — deferred to later
- File header contains 24 word-length bucket entries (3-byte offsets + flag byte)
- The large 0x80-filled regions are likely the DAWG node tables

### Strategy update
- TSD1 files (example dicts): fully cracked, XOR 0xBD on text section from ~0x14F0
- TSD0 files (main dictionaries): deferred — use SCOWL word lists instead for now

---
## Known word list gaps (vs original TEA)

- Possessive forms (e.g. `canter's`) not included in SCOWL word lists
- TEA's dictionaries were refined over many years — parity is a long-term goal
- Revisit when we tackle the TSD0 reverse engineering or find a better word list
---
## Reference material

- Original TEA help files: `docs/tea-original-help/*.htm`
- TEA home page (archived): http://www.crosswordman.com/tea.html
- SCOWL word lists: http://wordlist.aspell.net/
- WordNet: https://wordnet.princeton.edu/
- Tauri docs: https://tauri.app/
- Rust book: https://doc.rust-lang.org/book- /
