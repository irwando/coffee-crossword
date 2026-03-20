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

## Current status

- [x] Architecture designed
- [x] Stack selected (Tauri + React + Rust)
- [x] Prerequisites installed (Node, Rust, Xcode tools)
- [x] GitHub repo created (https://github.com/irwando/coffee-crossword)
- [x] Tauri scaffold created and verified building
- [x] TailwindCSS installed
- [ ] DICTIONARY_FORMAT.md written
- [ ] First word list converted to open format
- [ ] Template matching implemented in Rust
- [ ] Template matching wired to React UI

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
## Reference material

- Original TEA help files: `docs/tea-original-help/*.htm`
- TEA home page (archived): http://www.crosswordman.com/tea.html
- SCOWL word lists: http://wordlist.aspell.net/
- WordNet: https://wordnet.princeton.edu/
- Tauri docs: https://tauri.app/
- Rust book: https://doc.rust-lang.org/book/
