// ── Registry ──────────────────────────────────────────────────────────────────
// Scans the dictionaries/ folder at startup, tracks per-list cache state,
// and persists user choices (active list ordering, dedup setting).
//
// IDs are filename stems ("english", "wikipedia-en"). Stale IDs (file deleted)
// are silently removed from active_ids on load.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cache::{cache_validity, open_cache, CacheValidity};

// ── Types ─────────────────────────────────────────────────────────────────────

/// Runtime state of a list's binary cache.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum CacheState {
    /// .tsc exists and mtime matches source.
    Ready,
    /// .tsc exists but source .txt is newer — must rebuild before use.
    NeedsRebuild,
    /// No .tsc exists — must build before use.
    NotBuilt,
    /// Build is currently running.
    Building,
    /// Last build attempt failed.
    Error(String),
}

impl CacheState {
    pub fn is_ready(&self) -> bool {
        matches!(self, CacheState::Ready)
    }
}

/// Metadata and state for a single word list.
#[derive(Debug, Clone, Serialize)]
pub struct ListEntry {
    /// Stable ID derived from filename stem ("english", "wikipedia-en").
    pub id: String,
    /// Display name — from header if present, else filename stem.
    /// May be overridden by the user.
    pub display_name: String,
    /// Absolute path to the .txt source file.
    pub txt_path: PathBuf,
    /// Absolute path to the .tsc cache file (may not exist yet).
    pub tsc_path: PathBuf,
    /// Number of entries in the cache (0 if not yet built).
    pub word_count: usize,
    /// Updated field from file header (informational).
    pub source_updated: String,
    /// Description from file header.
    pub source_desc: String,
    /// Current cache state.
    pub cache_state: CacheState,
    /// Validated external lookup URL template from file header.
    /// Contains exactly one `{term}` token. None if absent or invalid.
    pub external_lookup: Option<String>,
}

/// The full registry: all discovered lists + user's active selection.
#[derive(Debug, Clone, Serialize)]
pub struct Registry {
    /// All .txt files found in the dictionaries folder.
    pub available: Vec<ListEntry>,
    /// IDs of active lists in priority order (highest priority first).
    pub active_ids: Vec<String>,
    /// Whether deduplication is enabled across lists.
    pub dedup_enabled: bool,
}


// ── Persistence keys (used by the frontend via tauri-plugin-store) ────────────
// Defined here as the single source of truth for key names.

#[allow(dead_code)] pub const KEY_ACTIVE_IDS: &str = "word_list_active_ids";
#[allow(dead_code)] pub const KEY_DISPLAY_NAMES: &str = "word_list_display_names";
#[allow(dead_code)] pub const KEY_DEDUP: &str = "dedup_enabled";

// ── Scanning ─────────────────────────────────────────────────────────────────

/// Scan a directory for all .txt files and return basic ListEntry records.
/// Does not load word data — only reads file metadata and parses headers.
pub fn scan_dictionaries(dir: &Path) -> Vec<ListEntry> {
    let mut entries = Vec::new();

    let read_dir = match std::fs::read_dir(dir) {
        Ok(d) => d,
        Err(_) => return entries,
    };

    let mut paths: Vec<PathBuf> = read_dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("txt"))
        .collect();

    // Sort for deterministic ordering (though active_ids controls search priority).
    paths.sort();

    for txt_path in paths {
        let id = txt_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let tsc_path = txt_path.with_extension("tsc");

        // Always read the .txt header for external_lookup (not stored in .tsc).
        let (txt_name, txt_updated, txt_desc, external_lookup) = read_txt_header(&txt_path);

        // Try to read display name / metadata from the cache if it exists,
        // otherwise fall back to the filename stem.
        let (display_name, source_updated, source_desc, word_count) =
            if tsc_path.exists() {
                match open_cache(&tsc_path) {
                    Ok(h) => {
                        let name = if h.display_name.is_empty() {
                            id.clone()
                        } else {
                            h.display_name.clone()
                        };
                        (name, h.source_updated.clone(), h.source_desc.clone(), h.entry_count)
                    }
                    Err(_) => (id.clone(), String::new(), String::new(), 0),
                }
            } else {
                let name = txt_name.unwrap_or_else(|| id.clone());
                (name, txt_updated.unwrap_or_default(), txt_desc.unwrap_or_default(), 0)
            };

        let cache_state = match cache_validity(&txt_path, &tsc_path) {
            CacheValidity::Ready => CacheState::Ready,
            CacheValidity::NeedsRebuild => CacheState::NeedsRebuild,
            CacheValidity::NotBuilt => CacheState::NotBuilt,
        };

        entries.push(ListEntry {
            id,
            display_name,
            txt_path,
            tsc_path,
            word_count,
            source_updated,
            source_desc,
            cache_state,
            external_lookup,
        });
    }

    entries
}

/// Validate an external lookup URL template.
/// Returns `Some(url)` if the URL starts with http(s):// and contains
/// exactly one `{term}` token, `None` otherwise.
fn validate_external_lookup(url: &str) -> Option<String> {
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }
    let count = url.matches("{term}").count();
    if count != 1 {
        return None;
    }
    Some(url.to_string())
}

/// Read only the header fields from a .txt file without processing word lines.
/// Returns (name, updated, description, external_lookup).
fn read_txt_header(path: &Path) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (None, None, None, None),
    };

    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() || lines[0].trim() != "---" {
        return (None, None, None, None);
    }

    let mut name = None;
    let mut updated = None;
    let mut description_lines: Vec<String> = Vec::new();
    let mut external_lookup_raw: Option<String> = None;
    let mut in_description = false;

    for line in &lines[1..] {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if in_description && (line.starts_with(' ') || line.starts_with('\t')) {
            description_lines.push(trimmed.to_string());
            continue;
        }
        in_description = false;

        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("updated:") {
            updated = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("external_lookup:") {
            external_lookup_raw = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("description:") {
            let first = val.trim().to_string();
            if !first.is_empty() {
                description_lines.push(first);
            }
            in_description = true;
        }
        // Unknown keys are silently ignored.
    }

    let description = if description_lines.is_empty() {
        None
    } else {
        Some(description_lines.join(" "))
    };

    let external_lookup = external_lookup_raw.and_then(|u| validate_external_lookup(&u));

    (name, updated, description, external_lookup)
}

// ── Load / save ───────────────────────────────────────────────────────────────

/// Serializable settings loaded from / saved to tauri-plugin-store.
/// Defined here for documentation; actual persistence is via the frontend.
#[allow(dead_code)]
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct PersistedSettings {
    pub active_ids: Vec<String>,
    pub display_names: HashMap<String, String>,
    pub dedup_enabled: Option<bool>,
}

/// Build a Registry by scanning the folder and merging with persisted settings.
/// stale IDs (file no longer present) are removed from active_ids silently.
/// New files appear in `available` but not `active_ids`.
pub fn build_registry(
    dict_dir: &Path,
    active_ids: Vec<String>,
    display_name_overrides: HashMap<String, String>,
    dedup_enabled: bool,
) -> Registry {
    let mut available = scan_dictionaries(dict_dir);

    // Apply user-overridden display names.
    for entry in &mut available {
        if let Some(name) = display_name_overrides.get(&entry.id) {
            entry.display_name = name.clone();
        }
    }

    let available_ids: std::collections::HashSet<&str> =
        available.iter().map(|e| e.id.as_str()).collect();

    // Remove stale IDs whose source files have been deleted.
    // Also remove IDs whose cache is no longer Ready (NeedsRebuild / NotBuilt)
    // so they can't accidentally be searched with stale data.
    let active_ids: Vec<String> = active_ids
        .into_iter()
        .filter(|id| {
            if !available_ids.contains(id.as_str()) {
                return false; // file deleted
            }
            // Keep only Ready lists in active_ids on load.
            let entry = available.iter().find(|e| &e.id == id);
            entry.map(|e| e.cache_state.is_ready()).unwrap_or(false)
        })
        .collect();

    Registry {
        available,
        active_ids,
        dedup_enabled,
    }
}

/// Update a ListEntry's cache state after a build completes or fails.
pub fn update_entry_state(registry: &mut Registry, list_id: &str, new_state: CacheState) {
    if let Some(entry) = registry.available.iter_mut().find(|e| e.id == list_id) {
        // If build succeeded, re-open the cache to get accurate metadata.
        if matches!(new_state, CacheState::Ready) {
            if let Ok(handle) = open_cache(&entry.tsc_path) {
                entry.word_count = handle.entry_count;
                if !handle.display_name.is_empty() {
                    // Only update if user hasn't overridden it.
                    entry.display_name = handle.display_name.clone();
                }
                entry.source_updated = handle.source_updated.clone();
                entry.source_desc = handle.source_desc.clone();
            }
        }
        entry.cache_state = new_state;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::build_cache;
    use std::fs;
    use tempfile::TempDir;

    fn make_txt(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let p = dir.path().join(name);
        fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn test_scan_finds_txt_files() {
        let dir = TempDir::new().unwrap();
        make_txt(&dir, "english.txt", "cat\ndog\n");
        make_txt(&dir, "scrabble.txt", "qi\nza\n");
        // This should be ignored.
        fs::write(dir.path().join("notes.md"), "ignore me").unwrap();

        let entries = scan_dictionaries(dir.path());
        assert_eq!(entries.len(), 2);
        let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"english"));
        assert!(ids.contains(&"scrabble"));
    }

    #[test]
    fn test_scan_ignores_tsc_files() {
        let dir = TempDir::new().unwrap();
        make_txt(&dir, "english.txt", "cat\n");
        // .tsc files should not appear as separate entries.
        fs::write(dir.path().join("english.tsc"), b"TSC1").unwrap();

        let entries = scan_dictionaries(dir.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "english");
    }

    #[test]
    fn test_scan_reads_header_name() {
        let dir = TempDir::new().unwrap();
        make_txt(
            &dir,
            "words.txt",
            "---\nname: My Custom List\n---\ncat\n",
        );
        let entries = scan_dictionaries(dir.path());
        assert_eq!(entries[0].display_name, "My Custom List");
    }

    #[test]
    fn test_scan_falls_back_to_filename() {
        let dir = TempDir::new().unwrap();
        make_txt(&dir, "english.txt", "cat\n");
        let entries = scan_dictionaries(dir.path());
        assert_eq!(entries[0].display_name, "english");
    }

    #[test]
    fn test_scan_state_not_built() {
        let dir = TempDir::new().unwrap();
        make_txt(&dir, "words.txt", "cat\n");
        let entries = scan_dictionaries(dir.path());
        assert_eq!(entries[0].cache_state, CacheState::NotBuilt);
    }

    #[test]
    fn test_scan_state_ready_after_build() {
        let dir = TempDir::new().unwrap();
        let txt = make_txt(&dir, "words.txt", "cat\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();

        let entries = scan_dictionaries(dir.path());
        assert_eq!(entries[0].cache_state, CacheState::Ready);
        assert_eq!(entries[0].word_count, 1);
    }

    #[test]
    fn test_build_registry_removes_stale_active_ids() {
        let dir = TempDir::new().unwrap();
        let txt = make_txt(&dir, "english.txt", "cat\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();

        // "missing-list" is in active_ids but has no file.
        let registry = build_registry(
            dir.path(),
            vec!["english".to_string(), "missing-list".to_string()],
            HashMap::new(),
            true,
        );

        assert!(!registry.active_ids.contains(&"missing-list".to_string()));
        assert!(registry.active_ids.contains(&"english".to_string()));
    }

    #[test]
    fn test_build_registry_new_file_not_auto_activated() {
        let dir = TempDir::new().unwrap();
        let txt = make_txt(&dir, "english.txt", "cat\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).ok();
        // Build with no active_ids.
        make_txt(&dir, "new-list.txt", "dog\n");
        let registry = build_registry(dir.path(), vec![], HashMap::new(), true);

        // new-list appears in available but not in active_ids.
        assert!(registry.available.iter().any(|e| e.id == "new-list"));
        assert!(!registry.active_ids.contains(&"new-list".to_string()));
    }

    #[test]
    fn test_build_registry_not_built_removed_from_active() {
        let dir = TempDir::new().unwrap();
        make_txt(&dir, "words.txt", "cat\n");
        // words.txt exists but no .tsc — should not be in active_ids even if listed.
        let registry = build_registry(
            dir.path(),
            vec!["words".to_string()],
            HashMap::new(),
            true,
        );
        assert!(!registry.active_ids.contains(&"words".to_string()));
    }

    #[test]
    fn test_build_registry_display_name_override() {
        let dir = TempDir::new().unwrap();
        let txt = make_txt(&dir, "english.txt", "cat\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();

        let mut overrides = HashMap::new();
        overrides.insert("english".to_string(), "My English".to_string());

        let registry = build_registry(dir.path(), vec![], overrides, true);
        let entry = registry.available.iter().find(|e| e.id == "english").unwrap();
        assert_eq!(entry.display_name, "My English");
    }

    #[test]
    fn test_update_entry_state_ready() {
        let dir = TempDir::new().unwrap();
        let txt = make_txt(&dir, "english.txt", "cat\ndog\nelephant\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();

        let mut registry = build_registry(dir.path(), vec![], HashMap::new(), true);
        // Manually set to Building first.
        update_entry_state(&mut registry, "english", CacheState::Building);
        assert_eq!(
            registry.available[0].cache_state,
            CacheState::Building
        );

        // Now mark as Ready — should re-read word count.
        update_entry_state(&mut registry, "english", CacheState::Ready);
        assert_eq!(registry.available[0].cache_state, CacheState::Ready);
        assert_eq!(registry.available[0].word_count, 3);
    }

    #[test]
    fn test_dedup_default_true() {
        let dir = TempDir::new().unwrap();
        let registry = build_registry(dir.path(), vec![], HashMap::new(), true);
        assert!(registry.dedup_enabled);
    }

    #[test]
    fn test_external_lookup_valid() {
        let dir = TempDir::new().unwrap();
        make_txt(
            &dir,
            "words.txt",
            "---\nname: Test\nexternal_lookup: https://example.com/define/{term}\n---\ncat\n",
        );
        let entries = scan_dictionaries(dir.path());
        assert_eq!(
            entries[0].external_lookup.as_deref(),
            Some("https://example.com/define/{term}")
        );
    }

    #[test]
    fn test_external_lookup_no_token_rejected() {
        let dir = TempDir::new().unwrap();
        make_txt(
            &dir,
            "words.txt",
            "---\nexternal_lookup: https://example.com/define/\n---\ncat\n",
        );
        let entries = scan_dictionaries(dir.path());
        assert!(entries[0].external_lookup.is_none());
    }

    #[test]
    fn test_external_lookup_two_tokens_rejected() {
        let dir = TempDir::new().unwrap();
        make_txt(
            &dir,
            "words.txt",
            "---\nexternal_lookup: https://example.com/{term}/{term}\n---\ncat\n",
        );
        let entries = scan_dictionaries(dir.path());
        assert!(entries[0].external_lookup.is_none());
    }

    #[test]
    fn test_external_lookup_not_http_rejected() {
        let dir = TempDir::new().unwrap();
        make_txt(
            &dir,
            "words.txt",
            "---\nexternal_lookup: ftp://example.com/{term}\n---\ncat\n",
        );
        let entries = scan_dictionaries(dir.path());
        assert!(entries[0].external_lookup.is_none());
    }

    #[test]
    fn test_external_lookup_absent_is_none() {
        let dir = TempDir::new().unwrap();
        make_txt(&dir, "words.txt", "---\nname: No Lookup\n---\ncat\n");
        let entries = scan_dictionaries(dir.path());
        assert!(entries[0].external_lookup.is_none());
    }
}
