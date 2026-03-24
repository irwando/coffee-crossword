# Decisions Log — Coffee Crossword

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
| 2026-03 | Split CLAUDE.md into docs/claude/ sub-files | Main file was 800+ lines; split by topic for efficiency |

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
