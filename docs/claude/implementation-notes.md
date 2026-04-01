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

## Search cancellation
`AppState.search_cancel: Mutex<Arc<AtomicBool>>` holds the cancel flag for the
current search. On each new search: a fresh `Arc<AtomicBool>` is created and
stored; the old flag is set to `true` to cancel any still-running prior search.
The engine checks the flag every 8192 entries (`entry_count & 0x1FFF == 0`) and
returns empty results when set. `cancel_search` Tauri command sets the flag from
the frontend Cancel button. A `tokio::time::timeout_at` deadline wraps each
per-list task; on timeout the cancel flag is set and remaining tasks get error
results without being awaited. The public `search_cache` API is unchanged;
`search_cache_cancellable` (pub(crate)) is the cancellable variant.

## Normalize=OFF anagram matching
Anagram matching is a letter-set operation — punctuation (apostrophes, hyphens)
must not count as letters. In `matcher.rs`, `matches_anagram_exact` strips
non-alphabetic/non-digit characters from the word before building `word_chars`,
regardless of normalize mode. Without this, `canter's` had 8 chars and failed
the length check against a 7-letter anagram set.

---

## Startup delay fix — implemented

**Root cause:** On macOS/APFS, `Mmap::map()` on large `.tsc` files (e.g. 428 MB wikipedia list) takes several seconds even though mmap is supposed to be lazy. Opening handles synchronously inside Tauri's `setup()` blocked the window from appearing.

**Startup sequence:**
1. Rust `setup()` runs synchronously — window appears immediately after it returns
   - `find_dict_dir()` — scans filesystem
   - `build_registry()` / `scan_dictionaries()` — reads 12 bytes per .tsc to check validity
   - `app.manage()` with empty `cache_handles`, `handles_loaded = false`
   - native menu construction
   - background task spawned (`tauri::async_runtime::spawn`)
2. Window appears; frontend mounts
3. `load(STORE_FILE)` + store reads — async IPC
4. `get_registry` → `set_active_lists` → `rename_list` → `set_dedup_enabled` → `get_registry` — async IPC chain
5. Background task completes: mmaps opened, `handles_loaded = true`, `registry:ready` emitted

**Implementation:**
- `AppState.handles_loaded: AtomicBool` — false until background task finishes
- `search` command returns an error immediately if `handles_loaded` is false
- `handles_ready` Tauri command lets the frontend poll as a fallback
- Frontend `listsLoading` state starts `true`; set `false` on `registry:ready` event or `handles_ready()` poll
- Search button disabled and "Loading word lists…" shown while `listsLoading`
