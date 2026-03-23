// ── lib.rs ────────────────────────────────────────────────────────────────────
// App state, Tauri commands, native menu construction, and event wiring.
//
// AppState now holds a Registry (all discovered word lists) and a cache of
// open memory-mapped CacheHandles for Ready lists. A build_in_progress flag
// blocks search while any list is being indexed.

pub mod engine;
pub mod cache;
pub mod registry;
pub mod dedup;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    Emitter, Manager, State,
};

use crate::cache::{build_cache, open_cache, CacheHandle};
use crate::dedup::{deduplicate, ListSearchResult};
use crate::engine::{search_cache, MatchGroup};
use crate::registry::{build_registry, update_entry_state, CacheState, Registry};

// ── App state ────────────────────────────────────────────────────────────────

pub struct AppState {
    pub registry: Mutex<Registry>,
    /// Open mmap handles for Ready lists, keyed by list ID.
    pub cache_handles: Mutex<HashMap<String, Arc<CacheHandle>>>,
    /// True while any list is being indexed — search is blocked.
    pub build_in_progress: AtomicBool,
    /// True once the background startup task has finished opening all mmap handles.
    /// Starts false; set true when registry:ready is emitted.
    pub handles_loaded: AtomicBool,
    /// Path to the dictionaries folder (set once at startup).
    pub dict_dir: PathBuf,
}

// ── Serialisable types sent to the frontend ──────────────────────────────────

#[derive(serde::Serialize, Clone)]
pub struct SearchStartPayload {
    pub active_ids: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct SearchListResultPayload {
    pub list_id: String,
    pub list_name: String,
    pub results: Vec<MatchGroup>,
    pub error: Option<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct SearchDedupPayload {
    pub list_id: String,
    pub removed_count: usize,
}

#[derive(serde::Serialize, Clone)]
pub struct BuildProgressPayload {
    pub list_id: String,
    pub percent: u8,
    pub phase: String,
}

#[derive(serde::Serialize, Clone)]
pub struct BuildCompletePayload {
    pub list_id: String,
    pub entry_count: usize,
    pub elapsed_ms: u64,
}

#[derive(serde::Serialize, Clone)]
pub struct BuildErrorPayload {
    pub list_id: String,
    pub message: String,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Find the dictionaries/ folder relative to the binary or source tree.
fn find_dict_dir() -> PathBuf {
    let candidates = vec![
        // Next to the binary (production).
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("dictionaries"))),
        // Relative to Cargo manifest (development).
        Some(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or(&PathBuf::from("."))
                .join("dictionaries"),
        ),
        Some(PathBuf::from("dictionaries")),
        Some(PathBuf::from("../dictionaries")),
    ];

    candidates
        .into_iter()
        .flatten()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("dictionaries"))
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Return the current registry state to the frontend.
#[tauri::command]
fn get_registry(state: State<AppState>) -> Result<Registry, String> {
    let registry = state.registry.lock().map_err(|e| e.to_string())?;
    Ok(registry.clone())
}

/// Replace the active list ordering and persist it.
#[tauri::command]
fn set_active_lists(
    ids: Vec<String>,
    state: State<AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;

    // Validate: only Ready lists may be activated.
    for id in &ids {
        let entry = registry.available.iter().find(|e| &e.id == id)
            .ok_or_else(|| format!("List '{}' not found", id))?;
        if !entry.cache_state.is_ready() {
            return Err(format!("List '{}' must be built before it can be activated", id));
        }
    }

    registry.active_ids = ids;
    persist_registry(&registry, &app);
    Ok(())
}

/// Toggle deduplication and persist.
#[tauri::command]
fn set_dedup_enabled(
    enabled: bool,
    state: State<AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    registry.dedup_enabled = enabled;
    persist_registry(&registry, &app);
    Ok(())
}

/// Override the display name for a list and persist.
#[tauri::command]
fn rename_list(
    id: String,
    name: String,
    state: State<AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    if let Some(entry) = registry.available.iter_mut().find(|e| e.id == id) {
        entry.display_name = name;
        persist_registry(&registry, &app);
        Ok(())
    } else {
        Err(format!("List '{}' not found", id))
    }
}

/// Persist active_ids, display names, and dedup_enabled to the store.
fn persist_registry(registry: &Registry, app: &tauri::AppHandle) {
    // Fire-and-forget: persistence failures are logged but not fatal.
    // We use tauri-plugin-store via JS-side calls, but we can also
    // emit an event for the frontend to handle persistence.
    // For now emit an event; the frontend listener calls store.set().
    let _ = app.emit(
        "registry:changed",
        serde_json::json!({
            "active_ids": registry.active_ids,
            // Only persist names that differ from the id (genuine user overrides).
            // Names equal to the id are auto-derived from the filename and need
            // not be persisted — they would override a tsc-provided display name
            // on restart if saved.
            "display_names": registry.available.iter()
                .filter(|e| e.display_name != e.id)
                .map(|e| (e.id.clone(), e.display_name.clone()))
                .collect::<HashMap<String, String>>(),
            "dedup_enabled": registry.dedup_enabled,
        }),
    );
}

/// Build (or rebuild) the .tsc index for a list.
/// Emits build:start, build:progress, build:complete or build:error events.
/// Blocks search while running via build_in_progress flag.
#[tauri::command]
async fn build_list_cache(
    list_id: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Check not already building.
    if state.build_in_progress.swap(true, Ordering::SeqCst) {
        return Err("A build is already in progress".to_string());
    }

    // Get paths from registry. On any error, clear the in-progress flag first.
    let (txt_path, tsc_path) = {
        let registry = state.registry.lock().map_err(|e| {
            state.build_in_progress.store(false, Ordering::SeqCst);
            e.to_string()
        })?;
        let entry = registry
            .available
            .iter()
            .find(|e| e.id == list_id)
            .ok_or_else(|| {
                state.build_in_progress.store(false, Ordering::SeqCst);
                format!("List '{}' not found", list_id)
            })?;
        (entry.txt_path.clone(), entry.tsc_path.clone())
    };

    // Mark as Building.
    {
        let mut registry = state.registry.lock().map_err(|e| {
            state.build_in_progress.store(false, Ordering::SeqCst);
            e.to_string()
        })?;
        update_entry_state(&mut registry, &list_id, CacheState::Building);
    }

    let _ = app.emit("build:start", serde_json::json!({ "list_id": list_id }));

    // Clone what we need for the closure.
    let lid = list_id.clone();
    let app_clone = app.clone();

    let result = tokio::task::spawn_blocking(move || {
        build_cache(&txt_path, &tsc_path, |percent, phase| {
            let _ = app_clone.emit(
                "build:progress",
                BuildProgressPayload {
                    list_id: lid.clone(),
                    percent,
                    phase: phase.to_string(),
                },
            );
        })
    })
    .await
    .map_err(|e| format!("Build task panicked: {}", e))?;

    // Clear the in-progress flag regardless of outcome.
    state.build_in_progress.store(false, Ordering::SeqCst);

    match result {
        Ok(stats) => {
            // Re-open the new cache handle.
            let tsc_path2 = {
                let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
                update_entry_state(&mut registry, &list_id, CacheState::Ready);
                let entry = registry.available.iter().find(|e| e.id == list_id).unwrap();
                entry.tsc_path.clone()
            };

            if let Ok(handle) = open_cache(&tsc_path2) {
                let mut handles = state.cache_handles.lock().map_err(|e| e.to_string())?;
                handles.insert(list_id.clone(), Arc::new(handle));
            }

            let _ = app.emit(
                "build:complete",
                BuildCompletePayload {
                    list_id,
                    entry_count: stats.entry_count,
                    elapsed_ms: stats.elapsed_ms,
                },
            );
            // Notify frontend to refresh registry display.
            let reg = state.registry.lock().map_err(|e| e.to_string())?;
            persist_registry(&reg, &app);
            Ok(())
        }
        Err(e) => {
            {
                let mut registry = state.registry.lock().map_err(|e2| e2.to_string())?;
                update_entry_state(&mut registry, &list_id, CacheState::Error(e.clone()));
            }
            let _ = app.emit(
                "build:error",
                BuildErrorPayload {
                    list_id,
                    message: e.clone(),
                },
            );
            Err(e)
        }
    }
}

/// Run a pattern search across all active Ready lists.
/// Emits: search:start → (search:list-result per list) → search:dedup → search:complete
/// Returns immediately after spawning; results arrive via events.
#[tauri::command]
async fn search(
    pattern: String,
    min_len: usize,
    max_len: usize,
    normalize: bool,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Block search while startup handle loading is in progress.
    if !state.handles_loaded.load(Ordering::SeqCst) {
        return Err("Word lists are still loading, please wait a moment".to_string());
    }

    // Block search during build.
    if state.build_in_progress.load(Ordering::SeqCst) {
        return Err("Search unavailable while a word list is being indexed".to_string());
    }

    // Collect active list IDs + handles.
    let (active_ids, dedup_enabled, list_handles): (Vec<String>, bool, Vec<(String, String, Arc<CacheHandle>)>) = {
        let registry = state.registry.lock().map_err(|e| e.to_string())?;
        let handles = state.cache_handles.lock().map_err(|e| e.to_string())?;

        let mut list_handles = Vec::new();
        for id in &registry.active_ids {
            if let Some(handle) = handles.get(id) {
                let name = registry
                    .available
                    .iter()
                    .find(|e| &e.id == id)
                    .map(|e| e.display_name.clone())
                    .unwrap_or_else(|| id.clone());
                list_handles.push((id.clone(), name, Arc::clone(handle)));
            }
        }
        (registry.active_ids.clone(), registry.dedup_enabled, list_handles)
    };

    if list_handles.is_empty() {
        let _ = app.emit("search:complete", serde_json::Value::Null);
        return Ok(());
    }

    // Emit search:start so frontend can create skeleton columns immediately.
    let _ = app.emit("search:start", SearchStartPayload {
        active_ids: active_ids.clone(),
    });

    // Spawn parallel search tasks — one per active list.
    let pattern = Arc::new(pattern);
    let mut task_handles = Vec::new();

    for (list_id, list_name, cache_handle) in list_handles {
        let pattern = Arc::clone(&pattern);
        let app_clone = app.clone();
        let lid = list_id.clone();
        let lname = list_name.clone();

        let task = tokio::task::spawn_blocking(move || {
            let results = search_cache(&cache_handle, &pattern, min_len, max_len, normalize);
            let payload = SearchListResultPayload {
                list_id: lid.clone(),
                list_name: lname,
                results,
                error: None,
            };
            let _ = app_clone.emit("search:list-result", payload.clone());
            ListSearchResult {
                list_id: lid,
                list_name: payload.list_name,
                results: payload.results,
                error: None,
            }
        });

        task_handles.push(task);
    }

    // Wait for all tasks in priority order, then apply dedup.
    let app_clone = app.clone();
    tokio::spawn(async move {
        let mut all_results: Vec<ListSearchResult> = Vec::new();

        for task in task_handles {
            match task.await {
                Ok(result) => all_results.push(result),
                Err(e) => {
                    // Task panicked — push error result.
                    all_results.push(ListSearchResult {
                        list_id: "unknown".to_string(),
                        list_name: "Unknown".to_string(),
                        results: vec![],
                        error: Some(format!("Search task failed: {}", e)),
                    });
                }
            }
        }

        // Re-order results to match active_ids priority order.
        all_results.sort_by_key(|r| {
            active_ids.iter().position(|id| id == &r.list_id).unwrap_or(usize::MAX)
        });

        if dedup_enabled {
            let before: Vec<usize> = all_results.iter().map(|r| r.results.len()).collect();
            deduplicate(&mut all_results);
            let after: Vec<usize> = all_results.iter().map(|r| r.results.len()).collect();

            for (i, result) in all_results.iter().enumerate() {
                let removed = before[i].saturating_sub(after[i]);
                if removed > 0 {
                    let _ = app_clone.emit(
                        "search:dedup",
                        SearchDedupPayload {
                            list_id: result.list_id.clone(),
                            removed_count: removed,
                        },
                    );
                }
            }
        }

        // Always emit final results — consistent event regardless of dedup.
        for result in &all_results {
            let _ = app_clone.emit(
                "search:list-result-final",
                SearchListResultPayload {
                    list_id: result.list_id.clone(),
                    list_name: result.list_name.clone(),
                    results: result.results.clone(),
                    error: result.error.clone(),
                },
            );
        }

        let _ = app_clone.emit("search:complete", serde_json::Value::Null);
    });

    Ok(())
}

/// Re-scan the dictionaries folder for new or changed .txt files.
/// Updates the registry and opens cache handles for any newly-Ready lists.
/// Emits registry:changed so the frontend refreshes.
#[tauri::command]
async fn rescan_registry(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let dict_dir = state.dict_dir.clone();
    let new_available = crate::registry::scan_dictionaries(&dict_dir);

    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    let mut handles = state.cache_handles.lock().map_err(|e| e.to_string())?;

    // Keep only active_ids that still exist and are Ready.
    let new_ids: std::collections::HashSet<&str> =
        new_available.iter().map(|e| e.id.as_str()).collect();
    registry.active_ids.retain(|id| {
        new_ids.contains(id.as_str())
            && new_available
                .iter()
                .find(|e| &e.id == id)
                .map(|e| e.cache_state.is_ready())
                .unwrap_or(false)
    });

    registry.available = new_available;

    // Open handles for newly-Ready lists.
    for entry in &registry.available {
        if entry.cache_state.is_ready() && !handles.contains_key(&entry.id) {
            if let Ok(h) = open_cache(&entry.tsc_path) {
                handles.insert(entry.id.clone(), Arc::new(h));
            }
        }
    }
    // Drop handles for lists that no longer exist.
    handles.retain(|id, _| registry.available.iter().any(|e| &e.id == id));

    persist_registry(&registry, &app);
    Ok(())
}

/// Return true once the background startup task has finished opening all cache handles.
#[tauri::command]
fn handles_ready(state: State<AppState>) -> bool {
    state.handles_loaded.load(Ordering::SeqCst)
}

/// Describe a pattern (unchanged).
#[tauri::command]
fn describe_pattern(pattern: &str) -> Option<String> {
    engine::describe_pattern(pattern)
}

/// Validate a pattern (unchanged).
#[tauri::command]
fn validate_pattern(pattern: &str) -> Result<(), String> {
    engine::validate_pattern(pattern)
}

// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // ── Locate dictionaries folder ─────────────────────────────────
            let dict_dir = find_dict_dir();
            eprintln!("Using dictionaries folder: {:?}", dict_dir);

            // ── Load persisted settings ────────────────────────────────────
            // We read from tauri-plugin-store synchronously during setup.
            // The store is available as a Tauri resource after the plugin is registered.
            let active_ids: Vec<String> = Vec::new(); // frontend will restore from store
            let display_names: HashMap<String, String> = HashMap::new();
            let dedup_enabled = true;

            // ── Build registry ────────────────────────────────────────────
            let registry = build_registry(&dict_dir, active_ids, display_names, dedup_enabled);
            eprintln!(
                "Registry: {} lists available, {} active",
                registry.available.len(),
                registry.active_ids.len()
            );

            // ── Register app state ─────────────────────────────────────────
            // Cache handles are opened in the background after setup() returns,
            // so the window appears immediately instead of waiting for Mmap::map()
            // on potentially large files (e.g. 428 MB wikipedia list).
            app.manage(AppState {
                registry: Mutex::new(registry),
                cache_handles: Mutex::new(HashMap::new()),
                build_in_progress: AtomicBool::new(false),
                handles_loaded: AtomicBool::new(false),
                dict_dir,
            });

            // ── Open cache handles in background ───────────────────────────
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();

                    // Collect paths without holding the lock during blocking I/O.
                    let ready_paths: Vec<(String, PathBuf)> = {
                        let registry = state.registry.lock().unwrap_or_else(|e| e.into_inner());
                        registry.available.iter()
                            .filter(|e| e.cache_state.is_ready())
                            .map(|e| (e.id.clone(), e.tsc_path.clone()))
                            .collect()
                    };

                    // Run Mmap::map() on a blocking thread — can be slow for large files on macOS.
                    let new_handles: HashMap<String, Arc<CacheHandle>> =
                        tokio::task::spawn_blocking(move || {
                            let mut h = HashMap::new();
                            for (id, path) in ready_paths {
                                match open_cache(&path) {
                                    Ok(handle) => { h.insert(id, Arc::new(handle)); }
                                    Err(e) => eprintln!("Warning: could not open cache for {}: {}", id, e),
                                }
                            }
                            h
                        })
                        .await
                        .unwrap_or_default();

                    eprintln!("Opened {} cache handle(s) in background.", new_handles.len());
                    {
                        let mut handles = state.cache_handles.lock().unwrap_or_else(|e| e.into_inner());
                        *handles = new_handles;
                    }
                    state.handles_loaded.store(true, Ordering::SeqCst);
                    let _ = app_handle.emit("registry:ready", serde_json::Value::Null);
                });
            }

            // ── Native menu ────────────────────────────────────────────────
            let file_menu = Submenu::with_items(
                app,
                "File",
                true,
                &[
                    &MenuItem::with_id(
                        app,
                        "manage_lists",
                        "Manage Word Lists…",
                        true,
                        Some("CmdOrCtrl+Shift+L"),
                    )?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, None)?,
                ],
            )?;

            let edit_menu = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?;

            let toggle_description = CheckMenuItem::with_id(
                app, "toggle_description", "Pattern Description", true, true, None::<&str>,
            )?;
            let toggle_options = CheckMenuItem::with_id(
                app, "toggle_options", "Options", true, true, None::<&str>,
            )?;

            let ref_full = CheckMenuItem::with_id(app, "ref_full", "Full", true, true, None::<&str>)?;
            let ref_compact = CheckMenuItem::with_id(app, "ref_compact", "Compact", true, false, None::<&str>)?;
            let ref_off = CheckMenuItem::with_id(app, "ref_off", "Off", true, false, None::<&str>)?;
            let reference_submenu = Submenu::with_items(
                app, "Pattern Reference", true, &[&ref_full, &ref_compact, &ref_off],
            )?;

            let appearance_light = CheckMenuItem::with_id(app, "appearance_light", "Light", true, false, None::<&str>)?;
            let appearance_dark = CheckMenuItem::with_id(app, "appearance_dark", "Dark", true, false, None::<&str>)?;
            let appearance_system = CheckMenuItem::with_id(app, "appearance_system", "System", true, true, None::<&str>)?;
            let appearance_menu = Submenu::with_items(
                app, "Appearance", true, &[&appearance_light, &appearance_dark, &appearance_system],
            )?;

            let reset_layout = MenuItem::with_id(app, "reset_layout", "Reset to Default Layout", true, None::<&str>)?;

            let view_menu = Submenu::with_items(
                app,
                "View",
                true,
                &[
                    &reference_submenu,
                    &toggle_description,
                    &toggle_options,
                    &PredefinedMenuItem::separator(app)?,
                    &appearance_menu,
                    &PredefinedMenuItem::separator(app)?,
                    &reset_layout,
                ],
            )?;

            let menu = Menu::with_items(app, &[&file_menu, &edit_menu, &view_menu])?;
            app.set_menu(menu)?;

            // ── Menu event handler ─────────────────────────────────────────
            let al = appearance_light.clone();
            let ad = appearance_dark.clone();
            let as_ = appearance_system.clone();
            let rf = ref_full.clone();
            let rc = ref_compact.clone();
            let ro = ref_off.clone();

            app.on_menu_event(move |app, event| {
                let window = app.get_webview_window("main");
                let emit = |name: &str, payload: &str| {
                    if let Some(ref w) = window {
                        let _ = Emitter::emit(w, name, payload.to_string());
                    }
                };
                match event.id().as_ref() {
                    "manage_lists"       => emit("menu:lists", ""),
                    "toggle_description" => emit("menu:toggle", "description"),
                    "toggle_options"     => emit("menu:toggle", "options"),
                    "reset_layout" => {
                        let _ = rf.set_checked(true);
                        let _ = rc.set_checked(false);
                        let _ = ro.set_checked(false);
                        emit("menu:reset_layout", "");
                    }
                    "ref_full" => { let _ = rf.set_checked(true); let _ = rc.set_checked(false); let _ = ro.set_checked(false); emit("menu:reference", "full"); }
                    "ref_compact" => { let _ = rf.set_checked(false); let _ = rc.set_checked(true); let _ = ro.set_checked(false); emit("menu:reference", "compact"); }
                    "ref_off" => { let _ = rf.set_checked(false); let _ = rc.set_checked(false); let _ = ro.set_checked(true); emit("menu:reference", "off"); }
                    "appearance_light" => { let _ = al.set_checked(true); let _ = ad.set_checked(false); let _ = as_.set_checked(false); emit("menu:appearance", "light"); }
                    "appearance_dark" => { let _ = al.set_checked(false); let _ = ad.set_checked(true); let _ = as_.set_checked(false); emit("menu:appearance", "dark"); }
                    "appearance_system" => { let _ = al.set_checked(false); let _ = ad.set_checked(false); let _ = as_.set_checked(true); emit("menu:appearance", "system"); }
                    _ => {}
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            describe_pattern,
            validate_pattern,
            get_registry,
            set_active_lists,
            set_dedup_enabled,
            rename_list,
            build_list_cache,
            rescan_registry,
            handles_ready,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
