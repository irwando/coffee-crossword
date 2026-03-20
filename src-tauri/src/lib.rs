mod engine;

use std::sync::Mutex;
use tauri::State;

/// Global dictionary state — loaded once at startup
pub struct Dictionary(Mutex<Vec<String>>);

/// The search command exposed to the React frontend
#[tauri::command]
fn search(
    pattern: &str,
    min_len: usize,
    max_len: usize,
    normalize: bool,
    dictionary: State<Dictionary>,
) -> Result<Vec<engine::MatchGroup>, String> {
    let words = dictionary.0.lock().map_err(|e| e.to_string())?;

    let parsed = engine::parse_pattern(pattern)
        .ok_or_else(|| "Empty pattern".to_string())?;

    let results = engine::search(&words, &parsed, min_len, max_len, normalize);
    Ok(results)
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

    eprintln!("Loading dictionary from: {:?}", dict_path);

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
        .manage(Dictionary(Mutex::new(words)))
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![search])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
