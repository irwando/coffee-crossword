// ── ccli — Coffee Crossword CLI ───────────────────────────────────────────────
// Searches one or more word lists using TEA-style patterns.
// Uses .tsc binary cache for performance; falls back to plain text with --no-cache.
//
// Shell quoting: patterns containing ! must use single quotes:
//   ccli 'c* & !cat*'

use app_lib::cache::{build_cache, cache_validity, open_cache, CacheValidity};
use app_lib::dedup::{deduplicate, ListSearchResult};
use app_lib::engine::{search_cache, search_words, MatchGroup};
use clap::Parser;
use std::io::{self, BufRead};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "ccli", version, about = "Coffee Crossword CLI — search word lists using TEA-style patterns")]
struct Args {
    /// Pattern to search for (omit to read from stdin)
    pattern: Option<String>,

    /// Minimum word length
    #[arg(long, default_value_t = 1)]
    minlen: usize,

    /// Maximum word length
    #[arg(long, default_value_t = 50)]
    maxlen: usize,

    /// Dictionary .txt file(s) to search. Repeatable: --dict a.txt --dict b.txt
    /// If not given, scans the dictionaries/ folder for all Ready lists.
    #[arg(long, action = clap::ArgAction::Append)]
    dict: Vec<PathBuf>,

    /// Strip punctuation before matching (default: true, e.g. --normalize false to disable).
    #[arg(long, default_value_t = true)]
    normalize: bool,

    /// Show anagram balances after each result
    #[arg(long)]
    balances: bool,

    /// Output format: plain, json, tsv
    #[arg(long, default_value = "plain")]
    format: String,

    /// Results only — no summary or header lines
    #[arg(long)]
    quiet: bool,

    /// Describe the pattern without searching
    #[arg(long)]
    describe: bool,

    /// Validate a pattern (exit 0 = valid, exit 1 = invalid)
    #[arg(long)]
    validate: bool,

    /// Show all discovered lists and their cache status, then exit
    #[arg(long)]
    dicts: bool,

    /// Build (or rebuild) the .tsc index for all lists that need it, then exit
    #[arg(long)]
    build_cache: bool,

    /// Force plain text search (skip .tsc cache). Slow for large lists.
    #[arg(long)]
    no_cache: bool,

    /// Show full results per list without deduplication (dedup is on by default)
    #[arg(long)]
    no_dedup: bool,
}

// ── Dictionary discovery ──────────────────────────────────────────────────────

fn find_default_dict_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("dictionaries"))),
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("coffee-crossword")
                .join("dictionaries")
        }),
        std::env::var("CCLI_DICT").ok().map(PathBuf::from),
        Some(PathBuf::from("dictionaries")),
        Some(PathBuf::from("../dictionaries")),
    ];
    candidates.into_iter().flatten().find(|p| p.is_dir())
}

struct DictInfo {
    id: String,
    txt_path: PathBuf,
    tsc_path: PathBuf,
    status: CacheValidity,
    entry_count: usize,
}

fn discover_dicts(dir: &std::path::Path) -> Vec<DictInfo> {
    let mut infos = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return infos; };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("txt"))
        .collect();
    paths.sort();
    for txt in paths {
        let tsc = txt.with_extension("tsc");
        let status = cache_validity(&txt, &tsc);
        let entry_count = if matches!(status, CacheValidity::Ready) {
            open_cache(&tsc).map(|h| h.entry_count).unwrap_or(0)
        } else {
            0
        };
        let id = txt.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        infos.push(DictInfo { id, txt_path: txt, tsc_path: tsc, status, entry_count });
    }
    infos
}

/// Resolve the list of (id, txt_path, tsc_path) to search, from --dict args or folder scan.
fn resolve_dicts(args: &Args) -> Vec<DictInfo> {
    if !args.dict.is_empty() {
        return args.dict.iter().map(|p| {
            let tsc = p.with_extension("tsc");
            let status = cache_validity(p, &tsc);
            let entry_count = if matches!(status, CacheValidity::Ready) {
                open_cache(&tsc).map(|h| h.entry_count).unwrap_or(0)
            } else { 0 };
            DictInfo {
                id: p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string(),
                txt_path: p.clone(),
                tsc_path: tsc,
                status,
                entry_count,
            }
        }).collect();
    }

    if let Some(dir) = find_default_dict_dir() {
        // Only return Ready lists when scanning folder (not explicit --dict).
        discover_dicts(&dir)
            .into_iter()
            .filter(|d| matches!(d.status, CacheValidity::Ready))
            .collect()
    } else {
        eprintln!("Error: no dictionary folder found. Use --dict to specify a file.");
        eprintln!("Searched: next to binary, ~/Library/Application Support/coffee-crossword/, $CCLI_DICT");
        std::process::exit(1);
    }
}

// ── Output formatting ─────────────────────────────────────────────────────────

fn format_single(results: &[MatchGroup], format: &str, show_balances: bool) -> String {
    match format {
        "json" => serde_json::to_string_pretty(results).unwrap_or_default(),
        "tsv" => results.iter().map(|r| {
            let balance = r.balance.as_deref().unwrap_or("");
            let variants = r.variants.join(", ");
            format!("{}\t{}\t{}", r.normalized, balance, variants)
        }).collect::<Vec<_>>().join("\n"),
        _ => results.iter().map(|r| {
            if show_balances {
                if let Some(b) = &r.balance { return format!("{} {}", r.normalized, b); }
            }
            r.normalized.clone()
        }).collect::<Vec<_>>().join("\n"),
    }
}

// ── Search ────────────────────────────────────────────────────────────────────

fn search_one(dict: &DictInfo, pattern: &str, args: &Args) -> ListSearchResult {
    if !args.no_cache && matches!(dict.status, CacheValidity::Ready) {
        match open_cache(&dict.tsc_path) {
            Ok(handle) => {
                let results = search_cache(&handle, pattern, args.minlen, args.maxlen, args.normalize);
                return ListSearchResult {
                    list_id: dict.id.clone(),
                    list_name: handle.display_name.clone(),
                    results,
                    truncated: false,
                    error: None,
                };
            }
            Err(e) => {
                eprintln!("Warning: could not open cache for {}: {}. Falling back to text.", dict.id, e);
            }
        }
    }

    // Plain text fallback.
    match load_words(&dict.txt_path) {
        Ok(words) => {
            let results = search_words(&words, pattern, args.minlen, args.maxlen, args.normalize);
            ListSearchResult {
                list_id: dict.id.clone(),
                list_name: dict.id.clone(),
                results,
                truncated: false,
                error: None,
            }
        }
        Err(e) => ListSearchResult {
            list_id: dict.id.clone(),
            list_name: dict.id.clone(),
            results: vec![],
            truncated: false,
            error: Some(e),
        }
    }
}

fn load_words(path: &PathBuf) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {:?}: {}", path, e))?;
    let content = String::from_utf8_lossy(&bytes).into_owned();
    Ok(content.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect())
}

fn run_pattern(pattern: &str, dicts: &[DictInfo], args: &Args) -> i32 {
    if args.describe {
        match app_lib::engine::describe_pattern(pattern) {
            Some(desc) => { println!("{}", desc); return 0; }
            None => { eprintln!("Error: empty or invalid pattern"); return 1; }
        }
    }

    if args.validate {
        match app_lib::engine::validate_pattern(pattern) {
            Ok(()) => { println!("valid"); return 0; }
            Err(e) => { eprintln!("Error: {}", e); return 1; }
        }
    }

    // Search all dicts.
    let mut all_results: Vec<ListSearchResult> = dicts.iter()
        .map(|d| search_one(d, pattern, args))
        .collect();

    if !args.no_dedup && all_results.len() > 1 {
        deduplicate(&mut all_results);
    }

    let multi = all_results.len() > 1;

    match args.format.as_str() {
        "json" => {
            let json_results: Vec<serde_json::Value> = all_results.iter().map(|lr| {
                serde_json::json!({
                    "list_id": lr.list_id,
                    "list_name": lr.list_name,
                    "results": lr.results,
                    "error": lr.error,
                })
            }).collect();
            println!("{}", serde_json::to_string_pretty(&json_results).unwrap_or_default());
        }
        _ => {
            for lr in &all_results {
                if let Some(ref e) = lr.error {
                    eprintln!("Error searching {}: {}", lr.list_id, e);
                    continue;
                }
                if multi && !args.quiet {
                    println!("\n=== {} ({} entries) ===", lr.list_name, dicts.iter().find(|d| d.id == lr.list_id).map(|d| d.entry_count).unwrap_or(0));
                }
                let out = format_single(&lr.results, &args.format, args.balances);
                if !out.is_empty() { println!("{}", out); }
                if !args.quiet {
                    let count = lr.results.len();
                    println!("{} {}", count, if count == 1 { "match" } else { "matches" });
                }
            }
        }
    }

    0
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args = Args::parse();

    // ── --dicts: show list status ─────────────────────────────────────────
    if args.dicts {
        let dir = find_default_dict_dir();
        let dicts = if !args.dict.is_empty() {
            args.dict.iter().map(|p| {
                let tsc = p.with_extension("tsc");
                let status = cache_validity(p, &tsc);
                let entry_count = if matches!(status, CacheValidity::Ready) {
                    open_cache(&tsc).map(|h| h.entry_count).unwrap_or(0)
                } else { 0 };
                DictInfo {
                    id: p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string(),
                    txt_path: p.clone(), tsc_path: tsc, status, entry_count,
                }
            }).collect::<Vec<_>>()
        } else {
            dir.as_deref().map(|d| discover_dicts(d)).unwrap_or_default()
        };

        if dicts.is_empty() {
            println!("No word lists found.");
        }
        for d in &dicts {
            let status_str = match &d.status {
                CacheValidity::Ready => format!("Ready  ({} words)", d.entry_count),
                CacheValidity::NeedsRebuild => "Needs rebuild".to_string(),
                CacheValidity::NotBuilt => "Not built — run --build-cache".to_string(),
            };
            println!("{:<20} {:<60} {}", d.id, d.txt_path.display(), status_str);
        }
        return;
    }

    // ── --build-cache: build indices ──────────────────────────────────────
    if args.build_cache {
        let dicts = if !args.dict.is_empty() {
            args.dict.iter().map(|p| {
                let tsc = p.with_extension("tsc");
                let status = cache_validity(p, &tsc);
                DictInfo {
                    id: p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string(),
                    txt_path: p.clone(), tsc_path: tsc, status, entry_count: 0,
                }
            }).collect::<Vec<_>>()
        } else {
            find_default_dict_dir()
                .as_deref()
                .map(discover_dicts)
                .unwrap_or_else(|| {
                    eprintln!("Error: no dictionaries folder found.");
                    std::process::exit(1);
                })
        };

        let mut any_error = false;
        for d in &dicts {
            match &d.status {
                CacheValidity::Ready => {
                    println!("{}: already up to date ({} words)", d.id, d.entry_count);
                    continue;
                }
                _ => {}
            }
            print!("Building index for {}...", d.id);
            let _ = std::io::Write::flush(&mut std::io::stdout());
            match build_cache(&d.txt_path, &d.tsc_path, |_, _| {}) {
                Ok(stats) => println!(" done ({} entries, {}ms)", stats.entry_count, stats.elapsed_ms),
                Err(e) => { println!(" ERROR: {}", e); any_error = true; }
            }
        }
        if any_error { std::process::exit(1); }
        return;
    }

    // ── Resolve dicts to search ───────────────────────────────────────────
    let dicts = resolve_dicts(&args);

    if dicts.is_empty() {
        eprintln!("Error: no Ready word lists found. Run --build-cache first, or use --dict with --no-cache.");
        std::process::exit(1);
    }

    // ── Run search ────────────────────────────────────────────────────────
    let exit_code = if let Some(ref pattern) = args.pattern {
        run_pattern(pattern, &dicts, &args)
    } else {
        // Read patterns from stdin, one per line.
        let stdin = io::stdin();
        let mut code = 0;
        for line in stdin.lock().lines() {
            match line {
                Ok(pattern) => {
                    let pattern = pattern.trim().to_string();
                    if !pattern.is_empty() {
                        let r = run_pattern(&pattern, &dicts, &args);
                        if r != 0 { code = r; }
                    }
                }
                Err(e) => { eprintln!("Error reading stdin: {}", e); code = 1; break; }
            }
        }
        code
    };

    std::process::exit(exit_code);
}
