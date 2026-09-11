# Word List Management — Full Design

## Overview

Like TEA's explicit Dictionary Builder requirement, Coffee Crossword requires
word lists to be indexed before use. The index (`.tsc` binary cache) is built
explicitly by the user via the List Manager drawer. This ensures predictable
performance even for very large lists (tested target: 6.3M Wikipedia titles,
125MB plain text).

---

## Text file header format

Word list `.txt` files may optionally begin with a YAML front matter block:

```
---
name: Wikipedia English Titles
updated: 2024-11-15
description: All English Wikipedia article titles as of November 2024.
  Approximately 6.3 million entries including multi-word phrases,
  proper nouns, and titles with punctuation.
---
Abd al-Rahman III
bacteria
United States
...
```

Supported header keys:
- `name` — overrides the display name; otherwise filename stem is used
- `updated` — informational only, shown in the List Manager drawer
- `description` — shown as tooltip/detail in the drawer; multi-line via leading-space continuation
- `external_lookup` — URL template for right-click → External lookup (see below)

Rules:
- Header is **optional** — files without `---` work as-is (backward compatible)
- Unknown keys are silently ignored
- Blank lines and `#` comment lines are skipped throughout the file body

### `external_lookup` field

Enables per-list external lookup from the right-click context menu.

```
external_lookup: https://www.collinsdictionary.com/dictionary/english/{term}
```

Validation (enforced at registry scan time):
- Must start with `http://` or `https://`
- Must contain **exactly one** `{term}` token (zero or two+ are rejected)
- Invalid values are silently ignored (field becomes `null`)

When a user right-clicks a word from a list with a valid `external_lookup` URL,
the **External lookup** context menu item becomes active. Clicking it opens an
embedded panel (iframe) with `{term}` replaced by the URL-encoded word. An
**Open in Browser ↗** button opens the URL in the system browser. If the site
blocks iframe embedding, a fallback message with the browser button is shown.

---

## Binary cache format (`.tsc`)

Each `.txt` file gets a `.tsc` cache file in the same `dictionaries/` folder.
`english.txt` → `english.tsc`. The cache is memory-mapped (not loaded into heap).

```
Header block (fixed 832 bytes):
  [0..4]     magic: b"TSC1"
  [4..12]    source_mtime: u64  (unix timestamp of .txt when built)
  [12..16]   entry_count: u32
  [16..20]   data_offset: u32   (byte offset where string data begins)
  [20..276]  display_name: [u8; 256]   (null-padded)
  [276..308] source_updated: [u8; 32]  (null-padded)
  [308..820] source_desc: [u8; 512]    (null-padded)
  [820..824] format_version: u32       (bumped on any layout change — see below)
  [824..832] reserved: [u8; 8]

Length index (1024 bytes):
  norm_length_offsets: [u32; 256]
  (norm_length_offsets[n] = first entry index for normalized length n)
  Entries are sorted by normalized length within the file (secondarily by an
  anagram sort-key, computed at build time for scan locality but not itself
  persisted — see v2 note below).

Entry index (entry_count × 16 bytes):
  Per entry: orig_offset: u32, orig_lower_offset: u32, norm_offset: u32, fold_offset: u32
  (byte offsets into the four string sections below)

String data sections (packed null-terminated strings):
  orig_strings:        verbatim original lines
  orig_lower_strings:  lowercase(orig), punctuation preserved
  norm_strings:        normalized (lowercase letters+digits only)
  fold_strings:        accent-folded norm (e.g. "andré" -> "andre")
```

**Search paths** (all zero-copy borrows from the mmap except one — see below):
- `normalize=on`, fold-accents off (default): iterate `norm_strings`
- `normalize=on`, fold-accents on: iterate `fold_strings`
- `normalize=off`, fold-accents off: iterate `orig_lower_strings`
- `normalize=off`, fold-accents on: iterate `orig_lower_strings`, folding each
  candidate on the fly (the one allocating combination — `normalize=off` is
  already the narrowest of the four modes, so a 5th on-disk field wasn't
  worth it; see `implementation-notes.md`)
- Anagram search: letter-set match (`CharCounts` in `matcher.rs`) against
  whichever of the above forms the current mode selects — there's no
  separate sort-key-based lookup path
- Length filtering: `norm_length_offsets` gives direct jump to right bucket

**Format version (v2):** `sort_key` used to be a fifth persisted field, but
nothing ever read it after build (it only orders entries within a length
bucket at build time) — so v2 drops it from disk and uses that slot for the
new `fold` field instead. Every `.tsc` built before `format_version` existed
has those header bytes zero-filled, so it reads back as version 0 and is
automatically treated as `NeedsRebuild` — no explicit migration code needed.
Bump `CURRENT_FORMAT_VERSION` in `cache.rs` for any future layout change.

**Build time estimates:**
- `english.txt` (101k words): ~0.3 seconds
- `wikipedia-en.txt` (6.3M entries, 125MB): ~5–15 seconds

---

## Cache state machine

Each list in the registry has one of these states:

| State | Condition | UI |
|---|---|---|
| `Ready` | `.tsc` exists, source mtime ≤ cache mtime | Green dot, word count shown |
| `NeedsRebuild` | `.tsc` exists but `.txt` is newer, or `.tsc`'s format_version doesn't match `CURRENT_FORMAT_VERSION` | Yellow dot, "Source updated" |
| `NotBuilt` | `.txt` exists, no `.tsc` | Gray dot, "Index not built" |
| `Building` | Build in progress | Spinner + progress % |
| `Error(msg)` | Build failed | Red dot, error message |

**Only `Ready` lists can be activated for search.**

If a list transitions from `Ready` to `NeedsRebuild` between app starts (user
edited the `.txt` file), it is automatically removed from `active_ids` and the
user must rebuild before it is searchable again.

**Rebuilding:** The user can explicitly trigger a rebuild at any time from the
drawer — both for `NotBuilt` lists (first build) and `NeedsRebuild` lists
(source was updated). The button label changes accordingly: "Build Index" vs
"Rebuild Index". This is the primary mechanism for incorporating updates to a
word list.

**Search disabled during build:** while any list is building, the search input
is disabled with a clear explanation. This avoids partial-state searches and
simplifies error handling.

---

## Registry persistence

Stored via `tauri-plugin-store` in `settings.json`:
- `"word_list_active_ids"` — `Vec<String>` ordered by priority
- `"word_list_display_names"` — `HashMap<String, String>` user overrides
- `"dedup_enabled"` — `bool` (default: `true`)

IDs are filename stems (`"english"`, `"wikipedia-en"`). Stale IDs (file deleted)
are silently removed from `active_ids` on load.

---

## Deduplication

When `dedup_enabled = true` (default), words found in multiple active lists
appear only in the highest-priority list that contains them. Lower-priority
results for that word are suppressed. This matches TEA's default behavior.

When `dedup_enabled = false`, each list shows its complete results independently.

---

## Parallel search

Each active `Ready` list is searched in a separate Tokio task. The `search`
Tauri command:
1. Emits `search:start` with the list of active list IDs (so frontend creates
   skeleton columns immediately)
2. Spawns one task per active list
3. Each task emits `search:list-result { list_id, list_name, results, error }`
   as it completes
4. After all tasks finish, applies deduplication if enabled and emits
   `search:dedup { list_id, removed_count }` per affected list
5. Emits `search:complete`

---

## Layout

**Single active list:** UI is identical to the current app. No multi-list chrome.

**Multiple active lists — stacked layout (only layout for now; columns deferred):**
- Each list occupies a horizontal pane
- Panes have independent scrollbars (TEA tile-horizontal style)
- Draggable divider between panes to resize (deferred)
- Equal initial height split
- Each pane has a header showing: list name, match count, loading skeleton while
  results are arriving

**Layout toggle** (only visible with 2+ active lists): in the results header bar.
Currently only "Stacked" is available; "Columns" will be added later.

---

## List Manager drawer

Right-side sliding drawer, opened via:
- File menu → "Manage Word Lists…" (Cmd+Shift+L)
- Dismisses on outside click or Escape

Drawer contents:
```
┌─────────────────────────────────────┐
│ Word Lists                     [✕]  │
├─────────────────────────────────────┤
│ ACTIVE (in search order)            │
│ ① english      101k words  [↑][↓]  │
│   green dot  "Built"        [Remove]│
│                                     │
│ ② wikipedia-en  —          [↑][↓]  │
│   gray dot  "Index not built"       │
│   [Build Index]             [Remove]│
│                                     │
│ AVAILABLE (not active)              │
│   my-custom    —                    │
│   yellow dot  "Source updated"      │
│   [Rebuild Index]  [Add to Active]  │
│                                     │
│ ─────────────────────────────────── │
│ ☑ Suppress duplicates across lists  │
└─────────────────────────────────────┘
```

Build progress replaces the button with a progress bar while building.
