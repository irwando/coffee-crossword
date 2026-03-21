use app_lib::engine::{describe_pattern, search_words, validate_pattern, MatchGroup};
use clap::Parser;
use std::io::{self, BufRead};
use std::path::PathBuf;

/// Coffee Crossword CLI — search word lists using TEA-style patterns
#[derive(Parser, Debug)]
#[command(name = "ccli", version, about)]
struct Args {
    /// Pattern to search for (omit to read from stdin)
    pattern: Option<String>,

    /// Minimum word length
    #[arg(long, default_value_t = 1)]
    minlen: usize,

    /// Maximum word length
    #[arg(long, default_value_t = 50)]
    maxlen: usize,

    /// Dictionary file to search (defaults to built-in english.txt)
    #[arg(long)]
    dict: Option<PathBuf>,

    /// Strip punctuation before matching, e.g. --normalize false (default: true)
    #[arg(long, default_value_t = true)]
    normalize: bool,

    /// Show anagram balances after each result
    #[arg(long)]
    balances: bool,

    /// Output format: plain, json, tsv
    #[arg(long, default_value = "plain")]
    format: String,

    /// Results only — no summary line
    #[arg(long)]
    quiet: bool,

    /// Describe a pattern without searching
    #[arg(long)]
    describe: bool,

    /// Validate a pattern without searching (exit 0 = valid, exit 1 = invalid)
    #[arg(long)]
    validate: bool,

    /// Show active dictionaries and exit
    #[arg(long)]
    dicts: bool,
}

fn find_default_dict() -> Option<PathBuf> {
    // 1. Next to the binary
    if let Ok(exe) = std::env::current_exe() {
        let candidate = exe.parent()
            .unwrap_or(&exe)
            .join("dictionaries")
            .join("english.txt");
        if candidate.exists() { return Some(candidate); }
    }

    // 2. macOS Application Support
    if let Some(home) = std::env::var_os("HOME") {
        let candidate = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("coffee-crossword")
            .join("dictionaries")
            .join("english.txt");
        if candidate.exists() { return Some(candidate); }
    }

    // 3. CCLI_DICT environment variable
    if let Ok(path) = std::env::var("CCLI_DICT") {
        let candidate = PathBuf::from(path);
        if candidate.exists() { return Some(candidate); }
    }

    // 4. Relative to current directory (useful during development)
    let candidate = PathBuf::from("dictionaries").join("english.txt");
    if candidate.exists() { return Some(candidate); }

    let candidate = PathBuf::from("../dictionaries").join("english.txt");
    if candidate.exists() { return Some(candidate); }

    None
}

fn load_words(path: &PathBuf) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Could not read {:?}: {}", path, e))?;
    let content = String::from_utf8_lossy(&bytes).into_owned();
    Ok(content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

fn display_name(path: &PathBuf) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dictionary")
        .to_string()
}

fn format_results(results: &[MatchGroup], format: &str, show_balances: bool) -> String {
    match format {
        "json" => serde_json::to_string_pretty(results).unwrap_or_default(),
        "tsv" => results
            .iter()
            .map(|r| {
                let balance = r.balance.as_deref().unwrap_or("");
                let variants = r.variants.join(", ");
                format!("{}\t{}\t{}", r.normalized, balance, variants)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => results
            .iter()
            .map(|r| {
                if show_balances {
                    if let Some(b) = &r.balance {
                        return format!("{} {}", r.normalized, b);
                    }
                }
                r.normalized.clone()
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn run_search(
    pattern: &str,
    words: &[String],
    args: &Args,
    _dict_path: &PathBuf,
) -> i32 {
    if args.describe {
        match describe_pattern(pattern) {
            Some(desc) => { println!("{}", desc); return 0; }
            None => { eprintln!("Error: empty or invalid pattern"); return 1; }
        }
    }

    if args.validate {
        match validate_pattern(pattern) {
            Ok(()) => { println!("valid"); return 0; }
            Err(e) => { eprintln!("Error: {}", e); return 1; }
        }
    }

    let results = search_words(words, pattern, args.minlen, args.maxlen, args.normalize);

    let output = format_results(&results, &args.format, args.balances);
    if !output.is_empty() {
        println!("{}", output);
    }

    if !args.quiet && args.format == "plain" {
        let count = results.len();
        println!("{} {}", count, if count == 1 { "match" } else { "matches" });
    }

    0
}

fn main() {
    let args = Args::parse();

    // Resolve dictionary path
    let dict_path = if let Some(ref p) = args.dict {
        p.clone()
    } else {
        match find_default_dict() {
            Some(p) => p,
            None => {
                eprintln!("Error: no dictionary found. Use --dict to specify one.");
                eprintln!("Searched: next to binary, ~/Library/Application Support/coffee-crossword/, $CCLI_DICT");
                std::process::exit(1);
            }
        }
    };

    // --dicts: show active dictionary info and exit
    if args.dicts {
        match load_words(&dict_path) {
            Ok(words) => {
                println!("{:<20} {:<60} ({} words)",
                    display_name(&dict_path),
                    dict_path.display(),
                    words.len());
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Load words
    let words = match load_words(&dict_path) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let exit_code = if let Some(ref pattern) = args.pattern {
        // Pattern from argument
        run_search(pattern, &words, &args, &dict_path)
    } else {
        // Read patterns from stdin, one per line
        let stdin = io::stdin();
        let mut code = 0;
        for line in stdin.lock().lines() {
            match line {
                Ok(pattern) => {
                    let pattern = pattern.trim().to_string();
                    if !pattern.is_empty() {
                        let result = run_search(&pattern, &words, &args, &dict_path);
                        if result != 0 { code = result; }
                    }
                }
                Err(e) => {
                    eprintln!("Error reading stdin: {}", e);
                    code = 1;
                    break;
                }
            }
        }
        code
    };

    std::process::exit(exit_code);
}
