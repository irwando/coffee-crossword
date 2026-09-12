# API Reference — Coffee Crossword

## Engine public API (stable)

```rust
pub fn search_words(words: &[String], pattern: &str,
                    normalize: bool, fold_accents: bool) -> Vec<MatchGroup>
pub fn validate_pattern(pattern: &str) -> Result<(), String>
pub fn describe_pattern(pattern: &str) -> Option<String>
pub fn normalize(word: &str) -> String

// Cache-backed entry point
pub fn search_cache(cache: &CacheHandle, pattern: &str,
                    normalize: bool, fold_accents: bool) -> Vec<MatchGroup>

pub struct MatchGroup {
    pub normalized: String,
    pub variants: Vec<String>,
    pub balance: Option<String>,
}
```

`fold_accents` folds accented letters to their plain equivalents before
matching (`andre` matches `André`) — see `implementation-notes-engine.md` for how it
interacts with the `.tsc` cache format.

Word length is an optional prefix on the pattern string itself, parsed by
`parser::parse_length_prefix` before the rest of the pattern is parsed —
there is no separate `min_len`/`max_len` parameter. Grammar (min word length
is always 1):

| Prefix | Meaning |
|---|---|
| `X:<pattern>` | Exactly X letters |
| `X-:<pattern>` | At least X letters |
| `-X:<pattern>` | At most X letters |
| `X-Y:<pattern>` | Between X and Y letters (order doesn't matter — reversed is swapped) |
| *(none)* | Unbounded, as before |

`describe_pattern` prepends a matching clause ("Exactly 5 letters, …", "At
least 8 letters, …") when a prefix is present.

---

## Tauri commands

| Command | Purpose |
|---|---|
| `search` | Run pattern against all active lists; streams events. Params: `pattern` (word length is an optional prefix, e.g. `"5-8:cat*"`), `normalize`, `foldAccents`, `timeoutSecs`, `maxResults` (0 = unlimited) |
| `cancel_search` | Set the shared cancel flag for the currently-running search |
| `describe_pattern` | Return human-readable pattern description |
| `validate_pattern` | Validate pattern syntax |
| `sync_menu_state` | Set native menu checkmarks (reference/layout/word list layout/variants/appearance/description/normalize/fold accents) from frontend state. Called once at startup after persisted settings load, since the menu is built with hardcoded defaults before that resolves |
| `get_registry` | Return current registry state to UI |
| `set_active_lists` | Replace active_ids list (persisted) |
| `set_dedup_enabled` | Toggle dedup (persisted) |
| `rename_list` | Override display name for a list (persisted) |
| `build_list_cache` | Build/rebuild `.tsc` for one list; streams build events |
| `rescan_registry` | Re-scan the dictionaries folder for new/changed `.txt` files |
| `set_dictionaries_dir` | Switch to a new dictionaries folder (persisted by the frontend, replayed at startup). Params: `path`. Fire-and-forget — completion is signaled by `registry:ready`, same as startup |
| `handles_ready` | Poll whether background mmap handle loading has finished (fallback for the `registry:ready` event) |
| `open_reference_window` | Pop the Pattern Reference panel into its own window (labeled `reference`), or focus it if already open. Params: `appearance`, `style` (`"full"` \| `"compact"`) — embedded in the new window's URL so it can render correctly without needing store access |
| `close_reference_window` | Close the popped-out Pattern Reference window if open (a no-op otherwise) — used by both the in-window Dock button and the View → Pattern Reference → Pop Out to Window checkbox |

---

## Tauri events emitted (Rust → frontend)

| Event | Payload | When |
|---|---|---|
| `search:start` | `{ active_ids: string[] }` | Search begins |
| `search:list-result-partial` | `{ list_id, groups: MatchGroup[] }` | Each length bucket completes during search (capped at 500 entries per event); replaces skeleton on first arrival, appended thereafter |
| `search:list-result` | `{ list_id, list_name, results, truncated, error }` | Each list's full result (replaces partials; `truncated: true` if capped at `maxResults`) |
| `search:list-result-final` | `{ list_id, list_name, results, truncated, error }` | After dedup — final authoritative result per list |
| `search:dedup` | `{ list_id, removed_count }` | After dedup applied |
| `search:complete` | — | All lists done |
| `registry:changed` | `{ active_ids, display_names, dedup_enabled }` | Registry mutated |
| `registry:ready` | — | Background cache handles opened (post-startup, or after a dictionaries folder switch) |
| `dictionaries_dir:loading` | — | A new dictionaries folder was chosen; handles are being reopened in the background |
| `dictionaries_dir:changed` | `path: string` | The new folder's fast metadata rescan finished; frontend persists `path` as `dictionariesDir` |
| `build:start` | `{ list_id }` | Build begins |
| `build:progress` | `{ list_id, percent, phase }` | During build |
| `build:complete` | `{ list_id, entry_count, elapsed_ms }` | Build done |
| `build:error` | `{ list_id, message }` | Build failed |
| `menu:toggle` | `"description"` | Pattern Description toggle |
| `menu:reference` | `"full" \| "compact" \| "off"` | Reference mode change |
| `menu:layout` | `"stacked" \| "columns"` | Window layout change |
| `menu:word_list_layout` | `"grid" \| "list"` | Word list layout change |
| `menu:variants` | `"show" \| "hide"` | Variants show/hide |
| `menu:appearance` | `"light" \| "dark" \| "system"` | Appearance change |
| `menu:normalize` | `"on" \| "off"` | Options → Normalize → Remove Punctuation toggle |
| `menu:fold_accents` | `"on" \| "off"` | Options → Normalize → Fold Accents toggle |
| `menu:open_max_results_dialog` | — | Options → Max Results… clicked |
| `menu:open_timeout_dialog` | — | Options → Timeout… clicked |
| `menu:pop_out_reference` | `"on" \| "off"` | View → Pattern Reference → Pop Out to Window toggled |
| `reference:pattern-clicked` | `pattern: string` | Emitted by the popped-out reference window (frontend `emit`, not a menu event) when a row is clicked; the main window runs the search and focuses itself |
| `reference:style-changed` | `"full" \| "compact"` | Emitted by the main window whenever `referenceMode` changes, so a popped-out window's style stays live-synced |
| `reference_window:closed` | — | The popped-out reference window closed (Dock button, native close, Cmd+W, or the main window quitting) — main window resumes inline rendering |
| `menu:reset_layout` | — | Reset layout |
| `menu:lists` | — | Open word list drawer |

---

## CLI reference (`ccli`)

### Usage

```bash
ccli [OPTIONS] "<pattern>"
```

Word length is an optional prefix on the pattern itself, not a flag: `"5:cat*"`
(exactly 5), `"5-:cat*"` (at least 5), `"-5:cat*"` (at most 5), `"5-8:cat*"`
(5 to 8) — see the shell quoting note below for the `-X:` form.

### Options

| Flag | Default | Description |
|---|---|---|
| `--dict PATH` | (repeatable) | Dictionary file(s); if none given, scans `dictionaries/` folder |
| `--normalize <true\|false>` | true | Strip punctuation before matching (e.g. --normalize false) |
| `--fold-accents <true\|false>` | false | Fold accented letters to plain equivalents, e.g. "andre" matches "André" (e.g. --fold-accents true) |
| `--balances` | off | Show anagram balances after results |
| `--format plain\|json\|tsv` | plain | Output format |
| `--quiet` | off | Results only, no summary line |
| `--describe` | — | Print pattern description, don't search |
| `--validate` | — | Validate pattern, don't search (exit 0/1) |
| `--dicts` | — | Show all discovered lists with status |
| `--build-cache` | — | Build/rebuild index for all lists that need it, then exit |
| `--no-cache` | off | Force plain text path (slow, for debugging) |
| `--no-dedup` | off | Show full results per list (dedup on by default) |
| `--version` | — | Show version |
| `--help` | — | Show usage |

### Multi-list behavior

```bash
# Search all Ready lists in dictionaries/ folder
ccli ";acenrt"

# Search specific lists
ccli --dict english.txt --dict wikipedia-en.txt ";acenrt"

# Build all caches that need it
ccli --build-cache

# Show list status
ccli --dicts
# english         dictionaries/english.tsc        Ready   101,368 words
# wikipedia-en    dictionaries/wikipedia-en.txt   Not built  —

# Force plain text (no cache)
ccli --no-cache --dict english.txt ";acenrt"
```

Plain text output with multiple lists:
```
=== english (101,368 words) ===
canter
nectar
recant
trance
4 matches

=== wikipedia-en (6,278,994 entries) ===
2 matches
```

JSON output: array of `{ list_id, list_name, entry_count, results: [...] }`.

### Shell quoting note
Patterns containing `!` must use single quotes to prevent bash history expansion:
```bash
ccli 'c* & !cat*'
```
Patterns starting with `-` (e.g. a `-X:` max-length prefix) look like a flag
to clap — pass them after `--`:
```bash
ccli -- '-5:cat*'
```

### Default dictionary search order
1. All `.tsc`-ready files in `dictionaries/` folder next to binary
2. `~/Library/Application Support/coffee-crossword/dictionaries/` (macOS)
3. `CCLI_DICT` environment variable (single path)
4. `../dictionaries/` relative to cwd (development)
