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

## Menu checkmark sync
The native menu's `CheckMenuItem`s are constructed in `setup()` with hardcoded
default checked states (Full/Rows/System, Description+Options on) before the
frontend has loaded its persisted settings from `tauri-plugin-store`. Without
an explicit sync, a restored session (e.g. Compact reference, Columns layout)
would show correctly on screen while the View menu still showed the hardcoded
defaults checked.

Fix: `MenuHandles` (in `lib.rs`) holds clones of every checkable menu item,
registered as Tauri app state. The `sync_menu_state` command sets all of them
from values passed by the frontend; `App.tsx` calls it once, right after
restoring persisted settings at startup. `CheckMenuItem::set_checked` does not
happen automatically on click — each `on_menu_event` arm must call it itself
(this is also why `toggle_description`/`toggle_options` originally only
emitted an event without flipping their own checkmark, and why
`reset_layout` originally reset the reference/layout checkmarks but not
appearance/description/options — both fixed alongside the startup sync).

When Word List Layout (Grid/List) and Variants (Show/Hide) moved from inline
Options-row buttons into the View menu, they followed this exact same
pattern: two more `CheckMenuItem` pairs in `MenuHandles`, two more params on
`sync_menu_state`, two more `on_menu_event` arms (each emitting
`menu:word_list_layout`/`menu:variants` for `App.tsx` to pick up), and two
more resets in the `reset_layout` arm. Any future menu-driven setting should
follow the same checklist — it's easy to add the menu item and forget one of
the four spots (build, sync, click handler, reset) and end up with exactly
the kind of stale-checkmark bug this section describes.

The same checklist applied again when Normalize/Fold Accents moved from the
main-window Options row into a new top-level "Options" menu (`normalize_item`/
`fold_accents_item` in `MenuHandles`, `menu:normalize`/`menu:fold_accents`
events). This is a pure UI relocation — the underlying `normalize`/
`foldAccents` booleans and search behavior are unchanged; only their controls
moved. The `toggle_options` `CheckMenuItem` (View menu's old "Options"
show/hide toggle) was removed entirely rather than migrated, since the
inline Options row it toggled no longer exists — there's nothing left to
show or hide. `Max Results…`/`Timeout…` are plain (non-checkbox) `MenuItem`s
that emit `menu:open_max_results_dialog`/`menu:open_timeout_dialog`; `App.tsx`
opens a small modal (`NumberPromptDialog.tsx`, styled after
`DictionaryPanel.tsx`'s backdrop pattern) pre-filled with the current value —
Tauri's menu API and dialog plugin have no native numeric-input prompt, so a
webview modal was the only cross-platform option.

**Gotcha: never negate `is_checked()` in a click handler.** Four independent
`CheckMenuItem` handlers (`toggle_description`, `normalize_toggle`,
`fold_accents_toggle`, `pop_out_reference`) computed their new state as
`let next = !item.is_checked().unwrap_or(default);`. On macOS the native
menu item already flips its own visual checkmark as part of default click
handling *before* `on_menu_event` fires — so `is_checked()` inside the
handler already returns the post-click state. Negating it undoes the OS's
own toggle, and since every subsequent click repeats the same
undo, the checkbox gets stuck: `toggle_description` always showed checked
(masked because its frontend listener does a local `(v) => !v` flip,
independent of the menu's `next`, so the *action* still worked — only the
checkmark was wrong), while `pop_out_reference` emitted an explicit "on"/"off"
payload computed from the broken `next`, so the *action* itself got stuck
too (confirmed via temporary `eprintln!` diagnostics: `next` kept resolving
back to its previous value click after click). Fix: read `is_checked()`
directly, with no negation — the OS has already applied the new state; the
handler only needs to notify the frontend and (redundantly but harmlessly)
call `set_checked()` with that same value. This bug does **not** affect the
radio-group handlers (`ref_full`/`ref_compact`/`ref_off`, layout, word list
layout, variants, appearance) since those force absolute states on all
group members unconditionally rather than negating current state.

## Pop out Pattern Reference to its own window
The Pattern Reference panel can be popped into its own OS window (View →
Pattern Reference → Pop Out to Window, or a "Pop out ↗" button on the panel
itself) and docked back by closing it (its own "Dock ⇲" button, native
close, or Cmd+W). `REFERENCE_ROWS`/`ReferenceHeader`/`ReferenceFull`/
`ReferenceCompact` were extracted from `App.tsx` into `PatternReference.tsx`
(and `applyTheme`/`AppearanceMode` into `theme.ts`) so both the main window
and the new standalone window (`ReferenceWindow.tsx`, selected in `main.tsx`
via the `?window=reference` URL query param) can share them.

- `open_reference_window`/`close_reference_window` (`lib.rs`) create/focus or
  close a `WebviewWindow` labeled `reference`. `appearance` and `style`
  (full/compact) are embedded directly in the window's URL rather than read
  from the store, so the new window renders correctly on first paint with no
  extra IPC. Both the in-panel button and the View-menu checkbox call
  `open_reference_window` (the button directly; the menu via
  `menu:pop_out_reference` → `App.tsx`), so the checkbox is checked from
  *inside* the command itself — otherwise popping out via the button would
  leave the menu checkbox showing unchecked (a real bug caught during manual
  testing).
- `capabilities/default.json` needed `"reference"` added to `"windows"`, and
  `"core:window:allow-set-focus"` added to `"permissions"` — `core:default`'s
  `core:window` permission set does **not** include `allow-set-focus` or
  `allow-close`. The Dock button therefore calls the `close_reference_window`
  *command* rather than `getCurrentWindow().close()` from JS — custom
  `#[tauri::command]`s aren't ACL-gated, so this sidesteps needing
  `allow-close` at all. `setFocus()` (used to bring the main window forward
  after a pattern click in the popped-out window) does need the explicit
  permission since it has no command-based equivalent already built.
- Clicking a pattern in the popped-out window emits an app-wide
  `reference:pattern-clicked` event (frontend `emit`, not a Rust-mediated
  menu event) since that window has no access to the main window's search
  state; the main window's `handleReferenceClickRef` (kept current via a
  ref, since the listener is registered in a mount-once effect and
  `doSearch`'s identity changes with its own deps) runs the search.
  Full/Compact style stays live-synced the same way: the main window emits
  `reference:style-changed` whenever `referenceMode` changes, and the
  reference window listens for it — the initial URL-param value only
  matters for first paint.
- Quit safety: `open_reference_window` also registers a `Destroyed` handler
  on the *main* window that closes `reference` if still open, so quitting
  via the main window never leaves an orphaned reference-only process
  running in the background.

## List view selection highlight (Tailwind conditional-class cascade)
`ListView`'s row background was `bg-white ... ${selected ? "bg-blue-50" : "hover:bg-gray-50"}`
— `bg-white` unconditional, `bg-blue-50` only added when selected. Both are
plain background-color utilities of equal CSS specificity, so which one wins
depends on their order in Tailwind's *generated stylesheet*, not on the order
they appear in the `className` string — class attribute order has no effect
on the cascade. This made the selection highlight unreliable, while the
underlying `selectedWords` state (and thus copy) was correct the whole time.
`GridView` never had this bug because its selected/unselected backgrounds
were already fully mutually exclusive via one ternary. Fix: same pattern in
`ListView` — never pair an unconditional background utility with a
conditionally-added one; make them one ternary with no shared base.

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

## Search-loop allocation avoidance
Two hot-path allocation sources were removed (both fire per *scanned candidate*,
not just per match, so they scale with dictionary size, not result count):

- `grouping.rs`'s `candidate_word()` returns `Cow<str>`: `normalize=true`
  borrows `entry.norm` directly (already lowercased/stripped at cache-build
  time — re-lowercasing it was pure waste), `normalize=false` allocates once.
  Only converted to an owned `String` (`into_owned()`) on an actual match.
- `matcher.rs`'s `MatchContext` stores letter-variable bindings (digits 0-9
  only, see `parser.rs`) in a `Copy` `[Option<char>; 10]` array instead of a
  `HashMap<u8, char>`, so backtracking clones (`*ctx` instead of `ctx.clone()`)
  are a stack copy instead of a heap allocation.

Also fixed, same class of issue:
- `search_cache_streaming` previously built `MatchGroup`s once per batch and
  again over the full result set at the end, cloning each `RawMatch` twice to
  keep two parallel accumulations. `GroupBuilder` (`grouping.rs`) now groups
  incrementally in one pass; `drain_batch()` returns a streaming snapshot,
  `finish()` returns the final sorted result with no re-grouping needed.
- Anagram matching (`matches_anagram_exact`/`matches_anagram_within`/
  `matches_subpattern_anagram`) used a `HashMap<char, i32>` per candidate.
  `CharCounts` (`matcher.rs`) replaces it with a fixed `[i32; 36]` array for
  a-z/0-9 (the common case, zero-alloc) with a small `Vec` fallback for any
  other Unicode character — a hard-coded ASCII-only array would silently
  miscount accented letters, which real lists (Wikipedia titles) do contain.

## Accent folding (fold-accents option)
`normalize()` only lowercases and strips punctuation — `é` and `e` are
different characters to it, so `andre` never matched `André`. Fold-accents is
a separate, opt-in toggle (default off) that additionally strips Unicode
combining diacritical marks (NFD decomposition + filter U+0300–U+036F).

Applied at two points, both mirroring existing patterns rather than touching
the matcher:
- **The pattern string** is folded once before parsing (`fold_pattern()` in
  `engine/mod.rs`) — the same pre-processing-step pattern already used for
  macro expansion. `parser.rs`/`ast.rs`/`matcher.rs` have no idea fold-accents
  exists; they just match whatever characters the (possibly folded) pattern
  and word have.
- **The candidate word** — precomputed at cache-build time as a 4th `.tsc`
  field (`fold`) for the zero-alloc `normalize=on` path, since folding is
  applied per *scanned candidate* on every search rather than once. See
  `word-lists.md`'s "Binary cache format" section for the full field layout
  and format-version/rebuild-detection design.

## Results rendering performance
`ResultsColumn.tsx`'s `GridView`/`ListView` are virtualized with
`@tanstack/react-virtual` — only visible rows are mounted as DOM nodes.
Necessary because `maxResults` is configurable up to 1,000,000; without
virtualization a broad pattern would render one DOM node per match and freeze
the webview. `GridView` uses fixed-column CSS grid rows (column count from a
`ResizeObserver`-measured container width) rather than `flex-wrap`, since
virtualization needs deterministic row membership; long variant text
truncates with a `title` tooltip instead of wrapping.

`App.tsx`'s `search:list-result-partial` handler buffers incoming batches in
a ref and flushes to state at most once per animation frame (`requestAnimationFrame`),
instead of the previous `[...lr.results, ...groups]` spread on every batch
(O(n) copy per batch, ~hundreds of batches for a large streamed search).
`search:list-result`/`-final` clear the buffer for their list so a
still-pending flush can't race in after the authoritative replace.

## Normalize=OFF anagram matching
Anagram matching is a letter-set operation — punctuation (apostrophes, hyphens)
must not count as letters. In `matcher.rs`, `matches_anagram_exact` strips
non-alphabetic/non-digit characters from the word before building `word_chars`,
regardless of normalize mode. Without this, `canter's` had 8 chars and failed
the length check against a 7-letter anagram set.

## Word-length prefix (replaces the "Word length" window option)
Word length moved from a `minLen`/`maxLen` UI option into the pattern text
itself: `5:cat*` (exactly 5), `5-:cat*` (at least 5), `-5:cat*` (at most 5),
`5-8:cat*` (5 to 8) — min word length is always 1. `engine::parser::parse_length_prefix`
strips this prefix off the front of the raw pattern string (before macro
expansion / accent folding / logical parsing) and returns `(min_len, max_len,
remainder)`, defaulting to `(1, usize::MAX)` when no prefix is present.

Because of this, `search_words`, `search_cache`, and the internal
`search_cache_cancellable_streaming` no longer take `min_len`/`max_len`
parameters at all — the only place length is specified is the pattern string.
`grouping.rs`'s internal search loops are unchanged (still parameterized by
min/max length; `usize::MAX` is naturally capped by the existing
`max_len.min(255)` cache-bucket logic, so an absurd `9999-:` prefix just
yields an empty range rather than a panic). `describe_pattern` and
`validate_pattern` strip the same prefix so `5:` alone (no pattern after the
colon) is correctly treated as empty/invalid, and `describe_pattern` prepends
a clause ("Exactly 5 letters, …") when a prefix is present.

A pattern starting with `-` (the `-X:` form) looks like a CLI flag to clap —
`ccli`'s call sites need `ccli -- '-5:cat*'`; not an issue for the frontend's
plain text input.

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

---

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
