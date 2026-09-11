// ── Cache ─────────────────────────────────────────────────────────────────────
// Builds and reads .tsc binary cache files from .txt word list sources.
//
// The .tsc format stores four parallel string arrays (original, lowercase
// original, normalized, accent-folded-normalized) sorted by normalized length,
// with a length-bucket index for fast skip-to-length access. The file is
// memory-mapped so only accessed pages are loaded by the OS — critical for
// the 125MB Wikipedia list.
//
// Cache invalidation: the header stores the source .txt mtime and a format
// version at build time. On load we compare mtime against the current .txt
// mtime, and version against CURRENT_FORMAT_VERSION; either mismatch →
// NeedsRebuild. Every .tsc built before the format-version field existed has
// those header bytes zero-filled, so it reads back as version 0 and is
// correctly flagged as needing a rebuild with no special-case migration code.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use memmap2::Mmap;
use unicode_normalization::UnicodeNormalization;

// ── Constants ────────────────────────────────────────────────────────────────

const MAGIC: &[u8; 4] = b"TSC1";
const HEADER_SIZE: usize = 832;
const LENGTH_INDEX_SIZE: usize = 1024; // 256 × u32

/// Bumped whenever the binary .tsc layout changes. Stored in the header;
/// any mismatch on load (in either direction) forces a rebuild.
const CURRENT_FORMAT_VERSION: u32 = 2;

// ── Public types ──────────────────────────────────────────────────────────────

/// State of a list's cache relative to its source .txt file.
#[derive(Debug, Clone, PartialEq)]
pub enum CacheValidity {
    /// .tsc exists and matches source mtime.
    Ready,
    /// .tsc exists but .txt has been modified since the cache was built.
    NeedsRebuild,
    /// No .tsc file exists.
    NotBuilt,
}

/// Statistics returned after a successful cache build.
#[derive(Debug, Clone)]
pub struct BuildStats {
    pub entry_count: usize,
    pub elapsed_ms: u64,
}

/// A zero-copy view into one entry in a memory-mapped cache.
#[derive(Debug, Clone)]
pub struct CacheEntry<'a> {
    pub orig: &'a str,
    /// Lowercase of `orig`, punctuation preserved — the `normalize=off` form,
    /// precomputed so that mode is zero-alloc at query time too.
    pub orig_lower: &'a str,
    pub norm: &'a str,
    /// Accent-folded `norm` — the `normalize=on` + fold-accents-on form.
    pub fold: &'a str,
}

/// Handle to an open memory-mapped .tsc file.
/// Safe to share across threads via Arc.
pub struct CacheHandle {
    _mmap: Mmap,
    // Pointers into the mmap — valid for the lifetime of _mmap.
    // We store raw slices derived from the mmap bytes.
    data: *const u8,
    data_len: usize,

    pub entry_count: usize,
    pub display_name: String,
    pub source_updated: String,
    pub source_desc: String,

    // Offsets within the mmap for the four string sections.
    // Retained for future direct-section scanning; not yet read after construction.
    #[allow(dead_code)]
    orig_base: usize,
    #[allow(dead_code)]
    orig_lower_base: usize,
    #[allow(dead_code)]
    norm_base: usize,
    #[allow(dead_code)]
    fold_base: usize,

    // Entry index: quadruples of (orig_offset, orig_lower_offset, norm_offset,
    // fold_offset) as u32. Starts at HEADER_SIZE + LENGTH_INDEX_SIZE.
    entry_index_base: usize,

    // norm_length_offsets[n] = first entry index for normalized length n.
    norm_length_offsets: [u32; 256],
}

// SAFETY: CacheHandle holds a Mmap which is read-only and the raw pointer
// is derived from it. No mutation ever occurs after construction.
unsafe impl Send for CacheHandle {}
unsafe impl Sync for CacheHandle {}

// ── Build ─────────────────────────────────────────────────────────────────────

/// Parse the optional YAML front-matter header from a .txt word list.
/// Returns (name, updated, description, first_word_line_index).
fn parse_header(lines: &[&str]) -> (Option<String>, Option<String>, Option<String>, usize) {
    if lines.is_empty() || lines[0].trim() != "---" {
        return (None, None, None, 0);
    }

    let mut name = None;
    let mut updated = None;
    let mut description_lines: Vec<String> = Vec::new();
    let mut in_description = false;
    let mut end_line = 1;

    for (i, line) in lines[1..].iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == "---" {
            end_line = i + 2; // +1 for slice offset, +1 to point past the closing ---
            break;
        }

        // Continuation line for description (starts with whitespace)
        if in_description && (line.starts_with(' ') || line.starts_with('\t')) {
            description_lines.push(trimmed.to_string());
            continue;
        }

        in_description = false;

        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(val.trim().to_string());
        } else if let Some(val) = trimmed.strip_prefix("updated:") {
            updated = Some(val.trim().to_string());
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

    (name, updated, description, end_line)
}

/// Normalize a word: lowercase, keep only letters and ASCII digits.
pub fn normalize_word(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphabetic() || c.is_ascii_digit())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Fold accented letters to their plain equivalents (e.g. "café" -> "cafe")
/// via Unicode NFD decomposition + stripping combining marks (U+0300-U+036F).
/// Length-preserving for the common case (one precomposed accented letter ->
/// one base letter). Duplicated from `engine::normalize::fold_accents` — see
/// that function's doc comment for why (`cache.rs` must stay engine-free).
fn fold_accents(s: &str) -> String {
    s.nfd().filter(|c| !('\u{0300}'..='\u{036f}').contains(c)).collect()
}

/// Compute the sort key: normalized letters sorted A–Z (for anagram lookup).
fn sort_key(norm: &str) -> String {
    let mut chars: Vec<char> = norm.chars().collect();
    chars.sort_unstable();
    chars.into_iter().collect()
}

/// Get the Unix mtime of a file in seconds, or 0 on error.
fn file_mtime(path: &Path) -> u64 {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Write a null-padded fixed-length byte field into a buffer.
fn write_fixed(buf: &mut Vec<u8>, s: &str, max_len: usize) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(max_len - 1); // leave room for null
    buf.extend_from_slice(&bytes[..len]);
    buf.extend(std::iter::repeat(0u8).take(max_len - len));
}

/// Build a .tsc cache file from a .txt word list source.
/// Progress callback: (percent: u8, phase: &str)
pub fn build_cache(
    txt_path: &Path,
    tsc_path: &Path,
    mut progress: impl FnMut(u8, &str),
) -> Result<BuildStats, String> {
    let start = SystemTime::now();

    progress(0, "reading");

    let raw_bytes = fs::read(txt_path)
        .map_err(|e| format!("Cannot read {:?}: {}", txt_path, e))?;
    let raw = String::from_utf8_lossy(&raw_bytes);

    let all_lines: Vec<&str> = raw.lines().collect();

    progress(5, "reading");

    // Parse optional header.
    let (header_name, header_updated, header_desc, word_start) = parse_header(&all_lines);

    let display_name = header_name.unwrap_or_else(|| {
        txt_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("dictionary")
            .to_string()
    });
    let source_updated = header_updated.unwrap_or_default();
    let source_desc = header_desc.unwrap_or_default();

    progress(8, "indexing");

    // Collect entries: skip blank lines and # comments.
    // `sort` is only used to order entries within a length bucket below (for
    // cache-friendly anagram scanning) — it's never persisted to the file.
    struct Entry {
        orig: String,
        orig_lower: String,
        norm: String,
        fold: String,
        sort: String,
    }

    let total_lines = all_lines.len().saturating_sub(word_start);
    let mut entries: Vec<Entry> = Vec::with_capacity(total_lines);

    for (i, line) in all_lines[word_start..].iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Strip inline annotation/definition markers (+ | :) — store only headword.
        // The cache stores the bare word/phrase; annotations are a future feature.
        let headword = trimmed
            .split_once('+')
            .or_else(|| trimmed.split_once('|'))
            .or_else(|| trimmed.split_once(':'))
            .map(|(w, _)| w.trim())
            .unwrap_or(trimmed);

        if headword.is_empty() {
            continue;
        }

        let norm = normalize_word(headword);
        if norm.is_empty() {
            continue;
        }
        let sort = sort_key(&norm);
        let orig_lower = headword.to_lowercase();
        let fold = fold_accents(&norm);

        entries.push(Entry {
            orig: headword.to_string(),
            orig_lower,
            norm,
            fold,
            sort,
        });

        // Emit progress at 1% intervals during indexing phase (5%–60%).
        if i % 50_000 == 0 && total_lines > 0 {
            let pct = 8 + (i * 52 / total_lines.max(1)) as u8;
            progress(pct, "indexing");
        }
    }

    progress(60, "sorting");

    // Sort by normalized length (primary), then sort key (secondary).
    // This enables length-bucket access and cache-friendly anagram scanning.
    entries.sort_unstable_by(|a, b| {
        a.norm
            .len()
            .cmp(&b.norm.len())
            .then_with(|| a.sort.cmp(&b.sort))
    });

    progress(70, "writing");

    let entry_count = entries.len();

    // Build length-offset index: norm_length_offsets[n] = first index with norm.len() == n.
    let mut norm_length_offsets = [u32::MAX; 256];
    for (i, e) in entries.iter().enumerate() {
        let len = e.norm.len().min(255);
        if norm_length_offsets[len] == u32::MAX {
            norm_length_offsets[len] = i as u32;
        }
    }
    // Fill gaps: if a length bucket is empty, point it past the end.
    let mut fill = entry_count as u32;
    for offset in norm_length_offsets.iter_mut().rev() {
        if *offset == u32::MAX {
            *offset = fill;
        } else {
            fill = *offset;
        }
    }

    // Serialize string sections and build per-entry offsets.
    let mut orig_strings: Vec<u8> = Vec::new();
    let mut orig_lower_strings: Vec<u8> = Vec::new();
    let mut norm_strings: Vec<u8> = Vec::new();
    let mut fold_strings: Vec<u8> = Vec::new();

    // entry_offsets: (orig_offset, orig_lower_offset, norm_offset, fold_offset) per entry
    let mut entry_offsets: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(entry_count);

    for (i, e) in entries.iter().enumerate() {
        let orig_off = orig_strings.len() as u32;
        orig_strings.extend_from_slice(e.orig.as_bytes());
        orig_strings.push(0);

        let orig_lower_off = orig_lower_strings.len() as u32;
        orig_lower_strings.extend_from_slice(e.orig_lower.as_bytes());
        orig_lower_strings.push(0);

        let norm_off = norm_strings.len() as u32;
        norm_strings.extend_from_slice(e.norm.as_bytes());
        norm_strings.push(0);

        let fold_off = fold_strings.len() as u32;
        fold_strings.extend_from_slice(e.fold.as_bytes());
        fold_strings.push(0);

        entry_offsets.push((orig_off, orig_lower_off, norm_off, fold_off));

        if i % 200_000 == 0 && entry_count > 0 {
            let pct = 70 + (i * 25 / entry_count.max(1)) as u8;
            progress(pct, "writing");
        }
    }

    // data_offset: byte position of string data section (after header + length index + entry index)
    let entry_index_size = entry_count * 16; // 4 × u32 per entry
    let data_offset =
        (HEADER_SIZE + LENGTH_INDEX_SIZE + entry_index_size) as u32;

    // String sections are laid out: orig_strings | orig_lower_strings | norm_strings | fold_strings.
    // Base offsets are relative to data_offset.
    // Rebuild entry_offsets to be absolute file offsets.
    let orig_section_start = data_offset;
    let orig_lower_section_start = orig_section_start + orig_strings.len() as u32;
    let norm_section_start = orig_lower_section_start + orig_lower_strings.len() as u32;
    let fold_section_start = norm_section_start + norm_strings.len() as u32;

    // ── Assemble file ────────────────────────────────────────────────────────

    let mut file_buf: Vec<u8> = Vec::with_capacity(
        HEADER_SIZE + LENGTH_INDEX_SIZE + entry_index_size
            + orig_strings.len()
            + orig_lower_strings.len()
            + norm_strings.len()
            + fold_strings.len(),
    );

    // Header (832 bytes)
    file_buf.extend_from_slice(MAGIC);                          // [0..4]   magic
    file_buf.extend_from_slice(&file_mtime(txt_path).to_le_bytes()); // [4..12]  source_mtime
    file_buf.extend_from_slice(&(entry_count as u32).to_le_bytes()); // [12..16] entry_count
    file_buf.extend_from_slice(&data_offset.to_le_bytes());    // [16..20] data_offset
    write_fixed(&mut file_buf, &display_name, 256);             // [20..276]
    write_fixed(&mut file_buf, &source_updated, 32);            // [276..308]
    write_fixed(&mut file_buf, &source_desc, 512);              // [308..820]
    file_buf.extend_from_slice(&CURRENT_FORMAT_VERSION.to_le_bytes()); // [820..824] format_version
    file_buf.extend_from_slice(&[0u8; 8]);                      // [824..832] reserved

    debug_assert_eq!(file_buf.len(), HEADER_SIZE);

    // Length index (1024 bytes = 256 × u32 LE)
    for &off in &norm_length_offsets {
        file_buf.extend_from_slice(&off.to_le_bytes());
    }

    debug_assert_eq!(file_buf.len(), HEADER_SIZE + LENGTH_INDEX_SIZE);

    // Entry index (entry_count × 16 bytes)
    for &(orig_off, orig_lower_off, norm_off, fold_off) in &entry_offsets {
        // Offsets stored as absolute file positions.
        let abs_orig = orig_section_start + orig_off;
        let abs_orig_lower = orig_lower_section_start + orig_lower_off;
        let abs_norm = norm_section_start + norm_off;
        let abs_fold = fold_section_start + fold_off;
        file_buf.extend_from_slice(&abs_orig.to_le_bytes());
        file_buf.extend_from_slice(&abs_orig_lower.to_le_bytes());
        file_buf.extend_from_slice(&abs_norm.to_le_bytes());
        file_buf.extend_from_slice(&abs_fold.to_le_bytes());
    }

    // String data
    file_buf.extend_from_slice(&orig_strings);
    file_buf.extend_from_slice(&orig_lower_strings);
    file_buf.extend_from_slice(&norm_strings);
    file_buf.extend_from_slice(&fold_strings);

    progress(97, "writing");

    // Write atomically: write to .tmp then rename.
    let tmp_path = tsc_path.with_extension("tsc.tmp");
    fs::write(&tmp_path, &file_buf)
        .map_err(|e| format!("Cannot write {:?}: {}", tmp_path, e))?;
    fs::rename(&tmp_path, tsc_path)
        .map_err(|e| format!("Cannot rename cache file: {}", e))?;

    progress(100, "done");

    let elapsed_ms = SystemTime::now()
        .duration_since(start)
        .unwrap_or_default()
        .as_millis() as u64;

    Ok(BuildStats { entry_count, elapsed_ms })
}

// ── Open / read ───────────────────────────────────────────────────────────────

/// Check whether the .tsc for a given .txt is valid without opening the mmap.
pub fn cache_validity(txt_path: &Path, tsc_path: &Path) -> CacheValidity {
    if !tsc_path.exists() {
        return CacheValidity::NotBuilt;
    }

    // Read only the first 824 bytes (magic + source_mtime + ... + format
    // version). Avoids reading the entire file — critical for large caches
    // (e.g. 428 MB Wikipedia .tsc).
    use std::io::Read;
    let mut header_bytes = [0u8; 824];
    let ok = fs::File::open(tsc_path)
        .ok()
        .and_then(|mut f| f.read_exact(&mut header_bytes).ok())
        .is_some();
    if !ok {
        return CacheValidity::NotBuilt;
    }

    if &header_bytes[0..4] != MAGIC {
        return CacheValidity::NotBuilt;
    }

    let stored_version = u32::from_le_bytes(header_bytes[820..824].try_into().unwrap());
    if stored_version != CURRENT_FORMAT_VERSION {
        return CacheValidity::NeedsRebuild;
    }

    let stored_mtime = u64::from_le_bytes(header_bytes[4..12].try_into().unwrap_or([0u8; 8]));
    let current_mtime = file_mtime(txt_path);

    if current_mtime > stored_mtime {
        CacheValidity::NeedsRebuild
    } else {
        CacheValidity::Ready
    }
}

/// Open a .tsc file and return a memory-mapped CacheHandle.
pub fn open_cache(tsc_path: &Path) -> Result<CacheHandle, String> {
    let file = fs::File::open(tsc_path)
        .map_err(|e| format!("Cannot open cache {:?}: {}", tsc_path, e))?;

    let mmap = unsafe {
        Mmap::map(&file).map_err(|e| format!("Cannot mmap {:?}: {}", tsc_path, e))?
    };

    let len = mmap.len();
    if len < HEADER_SIZE + LENGTH_INDEX_SIZE {
        return Err(format!("Cache file {:?} is too small", tsc_path));
    }

    let bytes = &mmap[..];

    if &bytes[0..4] != MAGIC {
        return Err(format!("Cache file {:?} has invalid magic bytes", tsc_path));
    }

    let entry_count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let data_offset = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;

    let display_name = read_fixed_str(&bytes[20..276]);
    let source_updated = read_fixed_str(&bytes[276..308]);
    let source_desc = read_fixed_str(&bytes[308..820]);

    // Parse length index.
    let mut norm_length_offsets = [0u32; 256];
    let li_start = HEADER_SIZE;
    for i in 0..256 {
        let off = li_start + i * 4;
        norm_length_offsets[i] = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
    }

    let entry_index_base = HEADER_SIZE + LENGTH_INDEX_SIZE;
    let entry_index_end = entry_index_base + entry_count * 16;

    if len < entry_index_end {
        return Err(format!("Cache file {:?} truncated at entry index", tsc_path));
    }

    // The four string sections start at data_offset and are laid out:
    //   orig_strings | orig_lower_strings | norm_strings | fold_strings
    // We derive base offsets by reading the first entry's offsets.
    // (If entry_count == 0 these are unused, set to data_offset.)
    let (orig_base, orig_lower_base, norm_base, fold_base) = if entry_count > 0 {
        let e0 = entry_index_base;
        let orig = u32::from_le_bytes(bytes[e0..e0 + 4].try_into().unwrap()) as usize;
        let orig_lower = u32::from_le_bytes(bytes[e0 + 4..e0 + 8].try_into().unwrap()) as usize;
        let norm = u32::from_le_bytes(bytes[e0 + 8..e0 + 12].try_into().unwrap()) as usize;
        let fold = u32::from_le_bytes(bytes[e0 + 12..e0 + 16].try_into().unwrap()) as usize;
        (orig, orig_lower, norm, fold)
    } else {
        (data_offset, data_offset, data_offset, data_offset)
    };

    let data_ptr = mmap.as_ptr();
    let data_len = mmap.len();

    Ok(CacheHandle {
        _mmap: mmap,
        data: data_ptr,
        data_len,
        entry_count,
        display_name,
        source_updated,
        source_desc,
        orig_base,
        orig_lower_base,
        norm_base,
        fold_base,
        entry_index_base,
        norm_length_offsets,
    })
}

fn read_fixed_str(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

// ── CacheHandle access ────────────────────────────────────────────────────────

impl CacheHandle {
    /// Read the four string pointers for entry at index `i`.
    fn entry_offsets(&self, i: usize) -> (usize, usize, usize, usize) {
        let base = self.entry_index_base + i * 16;
        let bytes = unsafe { std::slice::from_raw_parts(self.data, self.data_len) };
        let orig = u32::from_le_bytes(bytes[base..base + 4].try_into().unwrap()) as usize;
        let orig_lower = u32::from_le_bytes(bytes[base + 4..base + 8].try_into().unwrap()) as usize;
        let norm = u32::from_le_bytes(bytes[base + 8..base + 12].try_into().unwrap()) as usize;
        let fold = u32::from_le_bytes(bytes[base + 12..base + 16].try_into().unwrap()) as usize;
        (orig, orig_lower, norm, fold)
    }

    /// Read a null-terminated string from the mmap at byte offset `off`.
    fn read_str(&self, off: usize) -> &str {
        let bytes = unsafe { std::slice::from_raw_parts(self.data, self.data_len) };
        let start = off;
        let end = bytes[start..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| start + p)
            .unwrap_or(self.data_len);
        std::str::from_utf8(&bytes[start..end]).unwrap_or("")
    }

    /// Get a single entry by index.
    pub fn get_entry(&self, i: usize) -> CacheEntry<'_> {
        let (orig_off, orig_lower_off, norm_off, fold_off) = self.entry_offsets(i);
        CacheEntry {
            orig: self.read_str(orig_off),
            orig_lower: self.read_str(orig_lower_off),
            norm: self.read_str(norm_off),
            fold: self.read_str(fold_off),
        }
    }

    /// Return the index range [start, end) for entries with normalized length `n`.
    pub fn length_bucket(&self, n: usize) -> (usize, usize) {
        if n > 255 {
            return (0, 0);
        }
        let start = self.norm_length_offsets[n] as usize;
        // End is the start of the next non-empty bucket, or entry_count.
        let end = ((n + 1)..=255)
            .find_map(|k| {
                let off = self.norm_length_offsets[k] as usize;
                if off > start {
                    Some(off)
                } else {
                    None
                }
            })
            .unwrap_or(self.entry_count);
        (start.min(self.entry_count), end.min(self.entry_count))
    }

    /// Iterate entries of exactly normalized length `n`.
    pub fn iter_by_norm_len(&self, n: usize) -> impl Iterator<Item = CacheEntry<'_>> {
        let (start, end) = self.length_bucket(n);
        (start..end).map(move |i| self.get_entry(i))
    }

    /// Iterate ALL entries regardless of length.
    pub fn iter_all(&self) -> impl Iterator<Item = CacheEntry<'_>> {
        (0..self.entry_count).map(move |i| self.get_entry(i))
    }

    /// Derive the .tsc path from a .txt path (same folder, .tsc extension).
    pub fn tsc_path_for(txt_path: &Path) -> PathBuf {
        txt_path.with_extension("tsc")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn temp_txt(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let p = dir.path().join(name);
        fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn test_build_plain_list() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "canter\nnectar\nrecant\ntrance\n");
        let tsc = txt.with_extension("tsc");
        let stats = build_cache(&txt, &tsc, |_, _| {}).unwrap();
        assert_eq!(stats.entry_count, 4);
        assert!(tsc.exists());
    }

    #[test]
    fn test_build_skips_blank_and_comments() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(
            &dir,
            "words.txt",
            "# comment\n\ncanter\n  \nnectar\n# another\nrecant\n",
        );
        let tsc = txt.with_extension("tsc");
        let stats = build_cache(&txt, &tsc, |_, _| {}).unwrap();
        assert_eq!(stats.entry_count, 3);
    }

    #[test]
    fn test_build_with_full_header() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(
            &dir,
            "words.txt",
            "---\nname: Test List\nupdated: 2024-01-01\ndescription: A test list.\n---\ncanter\nnectar\n",
        );
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        assert_eq!(handle.display_name, "Test List");
        assert_eq!(handle.source_updated, "2024-01-01");
        assert_eq!(handle.source_desc, "A test list.");
        assert_eq!(handle.entry_count, 2);
    }

    #[test]
    fn test_build_with_partial_header() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "---\nname: Partial\n---\ncanter\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        assert_eq!(handle.display_name, "Partial");
        assert!(handle.source_updated.is_empty());
        assert!(handle.source_desc.is_empty());
    }

    #[test]
    fn test_build_no_header_uses_filename_stem() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "english.txt", "cat\ndog\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        assert_eq!(handle.display_name, "english");
    }

    #[test]
    fn test_build_multiline_description() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(
            &dir,
            "words.txt",
            "---\ndescription: First line.\n  Second line.\n  Third line.\n---\ncat\n",
        );
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        assert!(handle.source_desc.contains("First line"));
        assert!(handle.source_desc.contains("Second line"));
    }

    #[test]
    fn test_entries_sorted_by_norm_length() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "elephant\ncat\naardvark\ndo\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        let lengths: Vec<usize> = (0..handle.entry_count)
            .map(|i| handle.get_entry(i).norm.len())
            .collect();
        for w in lengths.windows(2) {
            assert!(w[0] <= w[1], "entries not sorted by length: {:?}", w);
        }
    }

    #[test]
    fn test_length_bucket_access() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "cat\ndog\nelephant\nnectar\ncanter\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();

        let three: Vec<&str> = handle.iter_by_norm_len(3).map(|e| e.norm).collect();
        assert_eq!(three.len(), 2);
        for w in &three {
            assert_eq!(w.len(), 3);
        }

        let six: Vec<&str> = handle.iter_by_norm_len(6).map(|e| e.norm).collect();
        assert_eq!(six.len(), 2);
        for w in &six {
            assert_eq!(w.len(), 6);
        }
    }

    #[test]
    fn test_orig_lower_preserves_punctuation() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "Pick-Me-Up\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        let entry = handle.get_entry(0);
        assert_eq!(entry.orig_lower, "pick-me-up");
        assert_eq!(entry.norm, "pickmeup");
    }

    #[test]
    fn test_fold_strips_accents() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "André\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        let entry = handle.get_entry(0);
        assert_eq!(entry.norm, "andré");
        assert_eq!(entry.fold, "andre");
    }

    #[test]
    fn test_phrase_normalization() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "Abd al-Rahman III\ndead end\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();

        // Find the Wikipedia-style entry
        let abd = (0..handle.entry_count)
            .map(|i| handle.get_entry(i))
            .find(|e| e.orig == "Abd al-Rahman III");
        assert!(abd.is_some());
        let abd = abd.unwrap();
        assert_eq!(abd.norm, "abdalrahmaniii");

        let dead = (0..handle.entry_count)
            .map(|i| handle.get_entry(i))
            .find(|e| e.orig == "dead end");
        assert!(dead.is_some());
        assert_eq!(dead.unwrap().norm, "deadend");
    }

    #[test]
    fn test_cache_validity_not_built() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "cat\n");
        let tsc = txt.with_extension("tsc");
        assert_eq!(cache_validity(&txt, &tsc), CacheValidity::NotBuilt);
    }

    #[test]
    fn test_cache_validity_ready() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "cat\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        assert_eq!(cache_validity(&txt, &tsc), CacheValidity::Ready);
    }

    #[test]
    fn test_cache_validity_needs_rebuild() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "cat\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();

        // Manually corrupt stored mtime to simulate txt being newer.
        let mut bytes = fs::read(&tsc).unwrap();
        // Stored mtime at bytes [4..12] — set to 0 so txt is always newer.
        bytes[4..12].copy_from_slice(&0u64.to_le_bytes());
        fs::write(&tsc, &bytes).unwrap();

        assert_eq!(cache_validity(&txt, &tsc), CacheValidity::NeedsRebuild);
    }

    #[test]
    fn test_cache_validity_needs_rebuild_on_stale_format_version() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "cat\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();

        // A .tsc built before the format_version field existed has zeros
        // there — simulate that (and any other future format bump) by
        // writing a version that doesn't match CURRENT_FORMAT_VERSION.
        let mut bytes = fs::read(&tsc).unwrap();
        bytes[820..824].copy_from_slice(&0u32.to_le_bytes());
        fs::write(&tsc, &bytes).unwrap();

        assert_eq!(cache_validity(&txt, &tsc), CacheValidity::NeedsRebuild);
    }

    #[test]
    fn test_open_invalid_magic() {
        let dir = TempDir::new().unwrap();
        let tsc = dir.path().join("bad.tsc");
        fs::write(&tsc, b"XXXX\x00\x00\x00\x00\x00\x00\x00\x00").unwrap();
        assert!(open_cache(&tsc).is_err());
    }

    #[test]
    fn test_roundtrip_entry_count() {
        let dir = TempDir::new().unwrap();
        let words: Vec<&str> = (0..500).map(|_| "cat").collect();
        let content = words.join("\n");
        let txt = temp_txt(&dir, "words.txt", &content);
        let tsc = txt.with_extension("tsc");
        let stats = build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        // Duplicates normalize to same key — count may differ from input lines,
        // but the cache stores all entries (dedup is the engine's job).
        assert_eq!(handle.entry_count, stats.entry_count);
    }

    #[test]
    fn test_iter_all() {
        let dir = TempDir::new().unwrap();
        let txt = temp_txt(&dir, "words.txt", "cat\ndog\nelephant\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        let entries: Vec<_> = handle.iter_all().collect();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_annotation_stripped() {
        let dir = TempDir::new().unwrap();
        // The + and | separators should strip the annotation, leaving just the headword.
        let txt = temp_txt(&dir, "words.txt", "OK+ also okay\ncanter|a verb meaning to gallop\n");
        let tsc = txt.with_extension("tsc");
        build_cache(&txt, &tsc, |_, _| {}).unwrap();
        let handle = open_cache(&tsc).unwrap();
        let origs: Vec<&str> = handle.iter_all().map(|e| e.orig).collect();
        assert!(origs.contains(&"OK"), "expected OK, got {:?}", origs);
        assert!(origs.contains(&"canter"), "expected canter, got {:?}", origs);
    }
}
