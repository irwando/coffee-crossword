# Coffee Crossword

A modern, cross-platform reimplementation of [TEA (The TA Crossword Helper)](http://www.crosswordman.com/tea.html) by Ross Beresford (Crossword Man), a word-search tool for solving crossword puzzles and other word games. TEA is no longer maintained. Coffee Crossword recreates its functionality with a modern stack that runs on macOS, in the browser, and (in future) on iOS, Android, and Windows.

---

## Features

### Pattern language

Coffee Crossword supports the full TEA pattern language:

| Pattern | Syntax | Example | Matches |
|---|---|---|---|
| Template | `.` or `?` per letter | `.l...r.n` | `electron` |
| Wildcard | `*` for any run of letters | `m*ja` | `maharaja` |
| Anagram | `;` prefix | `;acenrt` | `canter`, `nectar`, `recant`, `trance` |
| Anagram wildcard | `*` in anagram part | `;cats*` | any word using C, A, T, S plus extras |
| Template + anagram | semicolon separator | `e.....;cats` | `enacts` |
| Choice list | `[letters]` | `[aeiou]...` | vowel-initial 4-letter words |
| Negated choice | `[^letters]` | `[^aeiou]...` | consonant-initial 4-letter words |
| Vowel macro | `@` | `@...` | vowel-initial 4-letter words |
| Consonant macro | `#` | `#...` | consonant-initial 4-letter words |
| Letter variable | digit `0–9` | `1234321` | palindromes |
| AND | `&` | `c* & *ing` | words starting with C and ending in -ing |
| OR | `\|` | `cat \| dog` | either word |
| NOT | `!` | `c* & !cat*` | words starting with C, excluding those starting with CAT |
| Grouping | `()` | `(c* \| d*) & *ing` | starts with C or D, ends in -ing |

### Anagram balances

When searching with a blank tile (`.` in the anagram part), results show the letter used:

```
;eiknrr.  →  drinker +D
```

A `+` means the word uses that extra letter; a `-` means the word is missing that letter from your set.

### App features

- Native macOS menu bar
- Light / Dark / System appearance, persisted
- Pattern Reference panel (Full table, Compact, or Off)
- Pattern description shown as you type
- Search history (100 entries, persisted)
- Word selection with click, Cmd+click, Shift+click
- Copy selected words to clipboard
- Normalization: strips punctuation before matching (toggleable)
- Variant grouping: `escargots` and `escargot's` shown as one result

---

## Installation

> Pre-built releases are not yet available. Build from source using the instructions below.

### Prerequisites

- [Node.js](https://nodejs.org/) 18 or later
- [Rust](https://rustup.rs/) (stable toolchain)
- On macOS: Xcode Command Line Tools (`xcode-select --install`)

### Build

```bash
git clone https://github.com/irwando/coffee-crossword.git
cd coffee-crossword
npm install
npm run tauri build
```

The built app will be at `src-tauri/target/release/bundle/macos/Coffee Crossword.app`.

### Run in development

```bash
npm run tauri dev
```

### Run in browser (no Tauri)

```bash
npm run dev
```

---

## CLI — `ccli`

A command-line version of the search engine is included.

### Build

```bash
cd src-tauri
cargo build --bin ccli
```

### Usage

```bash
ccli [OPTIONS] "<pattern>"
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--minlen N` | 1 | Minimum word length |
| `--maxlen N` | 50 | Maximum word length |
| `--dict PATH` | auto | Path to dictionary file |
| `--normalize <true\|false>` | true | Strip punctuation before matching |
| `--balances` | off | Show anagram balances after results |
| `--format plain\|json\|tsv` | plain | Output format |
| `--quiet` | off | Results only, no summary line |
| `--describe` | — | Print pattern description, don't search |
| `--validate` | — | Validate pattern syntax (exit 0 = valid) |
| `--dicts` | — | Show active dictionary, don't search |
| `--version` | — | Show version |
| `--help` | — | Show usage |

### Examples

```bash
# Anagram search
ccli ";acenrt"
# canter / nectar / recant / trance

# Template search
ccli ".l...r.n"
# electron

# Wildcard
ccli "m*ja"
# maharaja

# Logical AND (single quotes required when pattern contains !)
ccli 'c* & !cat*'

# Anagram balance
ccli --balances ";eiknrr."
# drinker +D

# JSON output
ccli --format json ";acenrt"

# Validate a pattern
ccli --validate "[aeiou]..."   # prints: valid
ccli --validate "[aeiou"       # prints: Error: Invalid pattern
```

> **Shell quoting note:** patterns containing `!` must use single quotes to prevent bash history expansion.

---

## Architecture

```
UI (React + TypeScript + Vite)
    ↕  Tauri IPC (invoke)
Rust backend (Tauri v2)
    ↕
Engine (pure Rust — no Tauri dependency)
    — pattern parsing, matching, grouping, normalization
    — public API: search_words / validate_pattern / describe_pattern / normalize
    ↕
CLI (ccli)          ← calls engine directly
WASM (future)       ← same engine, compiled to WebAssembly
```

The engine is intentionally decoupled from Tauri so it can be used from the CLI, compiled to WASM for the browser, and eventually wrapped for Python.

---

## Dictionary format

Dictionaries are plain UTF-8 text files, one word per line. See [DICTIONARY_FORMAT.md](DICTIONARY_FORMAT.md) for the full spec, including support for annotations, definitions, and variants.

The bundled word list is derived from [SCOWL](http://wordlist.aspell.net/) (~101,000 words).

---

## Project status

| Phase | Status |
|---|---|
| Phase 1 — core search (template, anagram, wildcard) | ✅ Complete |
| Phase 2 — power features (choice lists, macros, letter variables, logical ops) | ✅ Complete |
| Phase 3 — definitions and lookup | 🔜 Planned |
| Phase 4 — polish (sorting, filtering, export, multiple dictionaries) | 🔜 Planned |

---

## Contributing

This project is in active early development. Contributions, bug reports, and dictionary improvements are welcome. Please open an issue before submitting large changes.

---

## License

To be determined prior to first release.

---

## Acknowledgements

Coffee Crossword is a spiritual successor to TEA (The TA Crossword Helper) by Ross Beresford (Crossword Man). TEA ran on Windows and was widely used by competitive crossword solvers for many years. This project aims to preserve that functionality for modern platforms.
