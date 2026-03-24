# Implementation Notes — Coffee Crossword

## Normalization
Toggle on by default. On: strip non-letter/non-digit, lowercase, deduplicate variants.
Off: all characters count literally including punctuation and spaces.

Per-list normalize override: **not supported** — one global setting applies to all
lists. Users are responsible for choosing the appropriate normalize setting when
searching mixed lists (e.g. Wikipedia titles work better with normalize=off).

## Variant display modes (normalize=on only)
- **Show**: canonical word with variants in parentheses
- **Hide**: canonical word only

## Pattern input
`autoCorrect`, `autoCapitalize`, `spellCheck` all disabled — critical, macOS
autocorrect converts `...` to `…` which breaks patterns.

## Macro expansion
Pre-processing step: `@` → `[aeiou]`, `#` → `[^aeiou]` before any other parsing.

## Letter variable matching
`MatchContext` struct tracks digit→letter bindings. Non-exclusive by default.

## Dark mode
Manual `.dark .class` CSS overrides in `index.css`. `dark`/`light` toggled on
`document.documentElement` by `applyTheme()`. System mode uses `MediaQueryList`.

## React StrictMode
Removed — temporary constraint. Impedance mismatch between React's sync effect
lifecycle and Tauri's async `listen()` API causes double-registration in dev mode.

## Menu architecture
Native menu built in `lib.rs`. Events emitted Rust→frontend via `Emitter::emit`.
Frontend listens with `@tauri-apps/api/event` `listen()`.

## Multiple binary targets
`default-run = "app"` required in `Cargo.toml`. Engine module must be `pub mod`.

## mmap and cache access
`CacheHandle` wraps a `memmap2::Mmap`. The mmap is `Send + Sync` via
`Arc<CacheHandle>`. Each search task gets an `Arc` clone — zero copy.
Cache handles are stored in `AppState.cache_handles: Mutex<HashMap<String, Arc<CacheHandle>>>`.
A handle is opened once when a list becomes Ready and kept until app exit.

## Build concurrency
`AppState.build_in_progress: AtomicBool` is set `true` when any build starts
and `false` when it completes or errors. The `search` command checks this flag
and returns an error immediately if true. The UI shows a "Building index —
search unavailable" message in this state.

---

## Startup delay fix — planned but not yet implemented

**Root cause:** `open_ready_handles()` is called synchronously inside Tauri's `setup()` closure in `lib.rs`. The window does not appear until `setup()` returns. For large `.tsc` files (e.g. 428 MB wikipedia list), `Mmap::map()` on macOS takes several seconds even though mmap is supposed to be lazy — APFS does non-trivial work at mapping time for large files.

**Startup sequence (for reference):**
1. Rust `setup()` runs synchronously — window blocked until it returns
   - `find_dict_dir()` — scans filesystem
   - `build_registry()` / `scan_dictionaries()` — reads 12 bytes per .tsc to check validity
   - **`open_ready_handles()`** ← the bottleneck; calls `Mmap::map()` per Ready list
   - `app.manage()`, native menu construction
2. Window appears; frontend mounts
3. `load(STORE_FILE)` + 13 `store.get()` calls — async IPC
4. `get_registry` → `set_active_lists` → `rename_list` → `set_dedup_enabled` → `get_registry` — async IPC chain

**Planned fix:**
- In `setup()`, start with an empty `cache_handles` (skip `open_ready_handles()`)
- After `app.manage()`, spawn a background task (`tauri::async_runtime::spawn`) that:
  1. Opens all Ready cache handles
  2. Inserts them into `AppState.cache_handles`
  3. Emits a `registry:ready` event so the frontend knows search is available
- Frontend shows a subtle "Loading word lists…" indicator until `registry:ready` fires

**Files to change:** `src-tauri/src/lib.rs` (setup function), `src/App.tsx` (listen for `registry:ready`)
