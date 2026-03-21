mod engine;

use std::sync::Mutex;
use tauri::{
    menu::{CheckMenuItem, Menu, PredefinedMenuItem, Submenu},
    Emitter, Manager, State,
};

/// Global app state — loaded once at startup
pub struct AppState {
    pub words: Mutex<Vec<String>>,
    pub dict_name: String,
}

/// Response shape returned to the frontend
#[derive(serde::Serialize)]
pub struct SearchResponse {
    pub results: Vec<engine::MatchGroup>,
    pub dict_name: String,
    pub dict_count: usize,
}

/// The search command exposed to the React frontend
#[tauri::command]
fn search(
    pattern: &str,
    min_len: usize,
    max_len: usize,
    normalize: bool,
    state: State<AppState>,
) -> Result<SearchResponse, String> {
    let words = state.words.lock().map_err(|e| e.to_string())?;

    let parsed = engine::parse_pattern(pattern)
        .ok_or_else(|| "Empty pattern".to_string())?;

    let results = engine::search(&words, &parsed, min_len, max_len, normalize);

    Ok(SearchResponse {
        dict_name: state.dict_name.clone(),
        dict_count: words.len(),
        results,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let candidates = vec![
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("dictionaries")
            .join("english.txt"),
        std::path::PathBuf::from("dictionaries").join("english.txt"),
        std::path::PathBuf::from("../dictionaries").join("english.txt"),
    ];

    let dict_path = candidates
        .into_iter()
        .find(|p| p.exists())
        .expect("Could not find dictionaries/english.txt");

    let dict_name = dict_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dictionary")
        .to_string();

    eprintln!("Loading dictionary '{}' from: {:?}", dict_name, dict_path);

    let bytes = std::fs::read(&dict_path)
        .expect("Found dictionary file but could not read it");
    let content = String::from_utf8_lossy(&bytes).into_owned();

    let words: Vec<String> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    eprintln!("Loaded {} words", words.len());

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState {
            words: Mutex::new(words),
            dict_name,
        })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // ── Build native menu bar ──────────────────────────────────────

            // File menu
            let file_menu = Submenu::with_items(
                app,
                "File",
                true,
                &[&PredefinedMenuItem::quit(app, None)?],
            )?;

            // Edit menu
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

            // View menu — panel toggles
            let toggle_reference = CheckMenuItem::with_id(
                app, "toggle_reference", "Pattern Reference", true, true, None::<&str>,
            )?;
            let toggle_description = CheckMenuItem::with_id(
                app, "toggle_description", "Pattern Description", true, true, None::<&str>,
            )?;
            let toggle_options = CheckMenuItem::with_id(
                app, "toggle_options", "Options", true, true, None::<&str>,
            )?;

            // Appearance submenu — radio-style (only one active at a time)
            let appearance_light = CheckMenuItem::with_id(
                app, "appearance_light", "Light", true, false, None::<&str>,
            )?;
            let appearance_dark = CheckMenuItem::with_id(
                app, "appearance_dark", "Dark", true, false, None::<&str>,
            )?;
            let appearance_system = CheckMenuItem::with_id(
                app, "appearance_system", "System", true, true, None::<&str>,
            )?;

            let appearance_menu = Submenu::with_items(
                app,
                "Appearance",
                true,
                &[&appearance_light, &appearance_dark, &appearance_system],
            )?;

            let view_menu = Submenu::with_items(
                app,
                "View",
                true,
                &[
                    &toggle_reference,
                    &toggle_description,
                    &toggle_options,
                    &PredefinedMenuItem::separator(app)?,
                    &appearance_menu,
                ],
            )?;

            let menu = Menu::with_items(
                app,
                &[&file_menu, &edit_menu, &view_menu],
            )?;

            app.set_menu(menu)?;

            // ── Handle menu events ─────────────────────────────────────────
            // Emit events to the main window so React can listen to them.
            // We clone the appearance items so we can update their checked
            // state from within the handler (radio-style mutual exclusion).
            let al = appearance_light.clone();
            let ad = appearance_dark.clone();
            let as_ = appearance_system.clone();

            app.on_menu_event(move |app, event| {
                let window = app.get_webview_window("main");

                let emit = |event_name: &str, payload: &str| {
                    if let Some(ref w) = window {
                        let _ = Emitter::emit(w, event_name, payload.to_string());
                    }
                };

                match event.id().as_ref() {
                    "toggle_reference" => emit("menu:toggle", "reference"),
                    "toggle_description" => emit("menu:toggle", "description"),
                    "toggle_options" => emit("menu:toggle", "options"),

                    "appearance_light" => {
                        let _ = al.set_checked(true);
                        let _ = ad.set_checked(false);
                        let _ = as_.set_checked(false);
                        emit("menu:appearance", "light");
                    }
                    "appearance_dark" => {
                        let _ = al.set_checked(false);
                        let _ = ad.set_checked(true);
                        let _ = as_.set_checked(false);
                        emit("menu:appearance", "dark");
                    }
                    "appearance_system" => {
                        let _ = al.set_checked(false);
                        let _ = ad.set_checked(false);
                        let _ = as_.set_checked(true);
                        emit("menu:appearance", "system");
                    }
                    _ => {}
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![search])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
