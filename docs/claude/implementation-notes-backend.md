# Implementation Notes — Backend, Caching & Concurrency

> Part of the implementation-notes split — see also
> `implementation-notes-ui.md` (menus, rendering, React state) and
> `implementation-notes-engine.md` (pattern matching, matcher internals).

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
`search_cache_cancellable_streaming` (pub(crate)) is the Tauri variant.

## Incremental streaming results

The Tauri `search` command emits results incrementally via `search:list-result-partial`
events as the search progresses, rather than waiting for each list to complete.

**How it works (Rust side):**
- `search_cache_cancellable_streaming` in `engine/mod.rs` accepts an `on_batch: impl Fn(Vec<MatchGroup>)` callback and `max_results: usize`.
- `search_cache_streaming` in `engine/grouping.rs` processes one length bucket at a time. After each bucket, `on_batch` is called with that bucket's grouped matches.
- Two flush boundaries: every `MAX_BATCH_SIZE = 500` matches (prevents multi-MB IPC events) and at each bucket boundary (natural length-group boundary).
- The function returns `(Vec<MatchGroup>, bool)` — the complete result and a `truncated` flag.
- `max_results = 0` means unlimited (used by CLI and non-streaming `search_cache`).

**Event sequence per search:**
```
search:start
  → (per list, in parallel):
      search:list-result-partial  (first bucket → skeleton replaced)
      search:list-result-partial  (subsequent buckets → appended)
      ...
      search:list-result          (full result; truncated=true if cap hit)
  → (after all lists finish):
      search:list-result-final    (post-dedup; preserves truncated flag)
search:complete
```

**Frontend state (`ListResults`):**
- `isLoading: true` → skeleton shown
- `isStreaming: true` → partial results visible, `…` indicator in header
- `isStreaming: false` → search complete for this list
- `truncated: true` → amber banner: "Showing first N results — refine your pattern or increase the limit in Options."

## Result cap and IPC backpressure fix

**Problem:** Negated or broad patterns (e.g. `[^aeiou]...` against a 6M-entry list)
can match millions of entries. Without a cap, this causes a deadlock:
1. Large bucket emits a multi-MB IPC event → frontend blocked rendering
2. IPC channel fills up (frontend can't consume messages)
3. Rust blocking thread stuck inside `emit()` waiting for channel to drain
4. Cancel flag never reached (thread is in IPC, not search loop)
5. UI permanently frozen — no recovery without force-quitting

**Fix:** `max_results` (default 5,000, configurable via Options menu → Max Results…) stops the
search loop once the total match count reaches the cap. Combined with
`MAX_BATCH_SIZE = 500`, each IPC event is at most ~500 entries regardless of
pattern type, and the total result set is bounded. The `truncated` flag flows
through the payload chain so the UI can surface a notice when the cap is hit.

The CLI is unaffected: `search_cache` (non-streaming, uncapped) is its entry
point. Only the Tauri `search` command uses the streaming + capped path.

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

## Open Dictionaries Folder — implemented

**Motivation:** `find_dict_dir()`'s "next to the binary" fallback resolves
inside `Contents/MacOS/` for a packaged `.app`, which never contains a real
dictionaries folder — a packaged build opened with zero word lists. Rather
than add a second hard-coded fallback, the user picks the folder directly via
File → Open Dictionaries Folder…, using `tauri-plugin-dialog`'s Rust API
(`DialogExt`) so no round-trip to the frontend or webview ACL entry is
needed — the picker is triggered from the menu-event handler in Rust, not
invoked from the webview, so `capabilities/default.json` is untouched.

**Implementation:**
- `AppState.dict_dir: Mutex<PathBuf>` (was a plain `PathBuf` set once at
  startup) — read/write via the lock everywhere it's used
- `do_rescan()` — `rescan_registry`'s old body, extracted to a plain sync fn
  taking `&Path`/`&AppState`/`&AppHandle` so both `rescan_registry` (same
  directory) and `switch_dictionaries_dir` (new directory) can call it
- `spawn_handle_loading()` — the startup background-mmap-open task, extracted
  so a directory switch can reuse it: switching means *every* Ready entry
  needs a fresh handle (unlike a same-directory rescan, where only new
  entries typically need one), so this must stay off the main thread for the
  same reason startup does (see "Startup delay fix" above)
- `switch_dictionaries_dir()` — locks in the new dir, sets `handles_loaded =
  false`, emits `dictionaries_dir:loading`, calls `do_rescan` (fast,
  synchronous, metadata only), emits `dictionaries_dir:changed` with the new
  path, then calls `spawn_handle_loading`
- `set_dictionaries_dir` command — frontend-invokable wrapper around
  `switch_dictionaries_dir`, used to replay a persisted folder at launch
  (same "frontend owns persistence, replays via a command at startup"
  pattern as `active_ids`/`dedup_enabled`)
- Frontend: `dictionaries_dir:loading` re-shows the `listsLoading` banner;
  `dictionaries_dir:changed` persists the new path to the settings store as
  `dictionariesDir`; at startup, if a path was persisted, `applyAndFetch()`
  calls `set_dictionaries_dir` before its first `get_registry` fetch
- Search button disabled and "Loading word lists…" shown while `listsLoading`
  is true, same as the startup path.
