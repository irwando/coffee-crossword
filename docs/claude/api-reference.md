# API Reference — Coffee Crossword

## Engine public API (stable)

```rust
// Existing — unchanged
pub fn search_words(words: &[String], pattern: &str,
                    min_len: usize, max_len: usize, normalize: bool) -> Vec<MatchGroup>
pub fn validate_pattern(pattern: &str) -> Result<(), String>
pub fn describe_pattern(pattern: &str) -> Option<String>
pub fn normalize(word: &str) -> String

// Cache-backed entry point
pub fn search_cache(cache: &CacheHandle, pattern: &str,
                    min_len: usize, max_len: usize, normalize: bool) -> Vec<MatchGroup>

pub struct MatchGroup {
    pub normalized: String,
    pub variants: Vec<String>,
    pub balance: Option<String>,
}
```

---

## Tauri commands

| Command | Purpose |
|---|---|
| `search` | Run pattern against all active lists; streams events. Params: `pattern`, `minLen`, `maxLen`, `normalize`, `timeoutSecs`, `maxResults` (0 = unlimited) |
| `describe_pattern` | Return human-readable pattern description |
| `validate_pattern` | Validate pattern syntax |
| `get_registry` | Return current registry state to UI |
| `set_active_lists` | Replace active_ids list (persisted) |
| `set_dedup_enabled` | Toggle dedup (persisted) |
| `rename_list` | Override display name for a list (persisted) |
| `build_list_cache` | Build/rebuild `.tsc` for one list; streams build events |

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
| `registry:ready` | — | Background cache handles opened (post-startup) |
| `build:start` | `{ list_id }` | Build begins |
| `build:progress` | `{ list_id, percent, phase }` | During build |
| `build:complete` | `{ list_id, entry_count, elapsed_ms }` | Build done |
| `build:error` | `{ list_id, message }` | Build failed |
| `menu:toggle` | `"description" \| "options"` | Menu toggle |
| `menu:reference` | `"full" \| "compact" \| "off"` | Reference mode change |
| `menu:appearance` | `"light" \| "dark" \| "system"` | Appearance change |
| `menu:reset_layout` | — | Reset layout |
| `menu:lists` | — | Open word list drawer |

---

## CLI reference (`ccli`)

### Usage

```bash
ccli [OPTIONS] "<pattern>"
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--minlen N` | 1 | Minimum word length |
| `--maxlen N` | 50 | Maximum word length |
| `--dict PATH` | (repeatable) | Dictionary file(s); if none given, scans `dictionaries/` folder |
| `--normalize <true\|false>` | true | Strip punctuation before matching (e.g. --normalize false) |
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

### Default dictionary search order
1. All `.tsc`-ready files in `dictionaries/` folder next to binary
2. `~/Library/Application Support/coffee-crossword/dictionaries/` (macOS)
3. `CCLI_DICT` environment variable (single path)
4. `../dictionaries/` relative to cwd (development)
