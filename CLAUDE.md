# CLAUDE.md — Coffee Crossword Project Context

This file is read by Claude at the start of every session. It contains rules
and always-needed context. Detailed reference material lives in `docs/claude/`.

## Sub-files index

| File | Contents |
|---|---|
| `docs/claude/architecture.md` | Stack, dependencies, repo structure, arch diagram, Tauri plugins |
| `docs/claude/word-lists.md` | Full word list management design (cache format, registry, UI) |
| `docs/claude/api-reference.md` | Engine API, Tauri commands, Tauri events, CLI reference |
| `docs/claude/implementation-notes.md` | Implementation notes, startup delay fix |
| `docs/claude/status.md` | Feature checklists, UI features, current status, impl plan |
| `docs/claude/decisions.md` | Decisions log, TSD research, known gaps, reference links |
| `docs/claude/testing.md` | Test rules, coverage requirements, how to run tests |

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

## Coding conventions

- **Rust**: snake_case for functions/variables, PascalCase for types/structs
- **TypeScript/React**: PascalCase for components, camelCase for functions/variables
- **File naming**: kebab-case for all files
- **Tests**: every Rust engine/cache/registry function must have unit tests — see `docs/claude/testing.md`
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
- The feature list in `docs/claude/status.md`

### Test rules
See `docs/claude/testing.md` for full detail. Summary:
- Test word list in `test_utils.rs` must cover every test pattern — verify before writing a new test
- Every example pattern + match pair must be manually verified before use
- Each pattern type must be tested standalone and in combination with every other type

### Engine public API stability
The four public functions (`search_words`, `validate_pattern`, `describe_pattern`,
`normalize`) and `search_cache` are the stable API surface. Do not change their
signatures without considering all callers.

### Doc file size and review rule
- **Before coding** any major update (multi-file change or significant refactor): after agreeing on a plan, review `docs/claude/` files that will be affected and update them to reflect the new plan.
- **After fixing** any significant issue: update the relevant `docs/claude/` file.
- **Periodically** check that no single file is growing too large (target: each file under ~200 lines). If a file is growing, propose a split.

### File download path
When Claude provides files for download: `~/Downloads/files/`.

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

# Build all word list caches
cd src-tauri && ./target/debug/ccli --build-cache

# Run CLI directly
cd src-tauri && cargo run --bin ccli -- ";acenrt"

# Single quotes required for patterns with !
cd src-tauri && ./target/debug/ccli 'c* & !cat*'
```

---

## Deferred (carry into next conversation)
- ccli --normalize help text: add "e.g. --normalize false" to description
- App.tsx internal section comment headers (// ── Search state ── etc.)
