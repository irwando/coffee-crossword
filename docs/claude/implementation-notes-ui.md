# Implementation Notes — UI & Interaction

> Part of the implementation-notes split — see also
> `implementation-notes-engine.md` (pattern matching, matcher internals) and
> `implementation-notes-backend.md` (Tauri state, caching, concurrency).

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
