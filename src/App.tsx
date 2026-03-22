import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { load, Store } from "@tauri-apps/plugin-store";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

interface MatchGroup {
  normalized: string;
  variants: string[];
  balance: string | null;
}

interface SearchResponse {
  results: MatchGroup[];
  dict_name: string;
  dict_count: number;
}

interface HistoryEntry {
  pattern: string;
  matchCount: number;
}

interface ContextMenu {
  x: number;
  y: number;
}

type VariantMode = "show" | "hide";
type ViewMode = "grid" | "list";
type AppearanceMode = "light" | "dark" | "system";
type ReferenceMode = "full" | "compact" | "off";

const STORE_FILE = "settings.json";
const MAX_HISTORY = 100;

const DEFAULTS = {
  normalize: true,
  variantMode: "show" as VariantMode,
  viewMode: "list" as ViewMode,
  minLen: 1,
  maxLen: 50,
  referenceMode: "full" as ReferenceMode,
  showDescription: true,
  showOptions: true,
  appearance: "system" as AppearanceMode,
};

// ── Theme ────────────────────────────────────────────────────────────────────

let systemDarkListener: ((e: MediaQueryListEvent) => void) | null = null;
let systemMQ: MediaQueryList | null = null;

function applyTheme(mode: AppearanceMode) {
  const root = document.documentElement;
  if (systemMQ && systemDarkListener) {
    systemMQ.removeEventListener("change", systemDarkListener);
    systemDarkListener = null;
    systemMQ = null;
  }
  if (mode === "light") {
    root.classList.remove("dark"); root.classList.add("light");
  } else if (mode === "dark") {
    root.classList.remove("light"); root.classList.add("dark");
  } else {
    systemMQ = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = (dark: boolean) => {
      root.classList.toggle("dark", dark);
      root.classList.toggle("light", !dark);
    };
    apply(systemMQ.matches);
    systemDarkListener = (e) => apply(e.matches);
    systemMQ.addEventListener("change", systemDarkListener);
  }
}

// ── Reference panel data ──────────────────────────────────────────────────────
// Single source of truth — Full table and Compact grid both render from this.
// RULE: when adding a new pattern type, add a row here AND update engine.rs describe_pattern.

const REFERENCE_ROWS = [
  { feature: "Template",         pattern: ".l...r.n",    match: "electron",      note: ". or ? = any letter"        },
  { feature: "Anagram",          pattern: ";acenrt",      match: "canter",        note: "; prefix = rearrange"        },
  { feature: "Wildcard",         pattern: "m*ja",         match: "maharaja",      note: "* = zero or more letters"   },
  { feature: "Choice list",      pattern: "[aeiou]....",  match: "ultra",         note: "any one letter from set"    },
  { feature: "Negated choice",   pattern: "[^aeiou]...",  match: "cast",          note: "any letter not in set"      },
  { feature: "Macro",            pattern: "@....",        match: "ultra",         note: "@ = vowel, # = consonant"   },
  { feature: "Anagram blank",    pattern: ";acenrt.",     match: "cantered +ED",  note: ". = one unknown letter"     },
  { feature: "Anagram wildcard", pattern: ";cats*",       match: "escalator",     note: "* = any extra letters"      },
  { feature: "Tmpl + anagram",   pattern: "e.....;cats",  match: "enacts",        note: "combine both styles"        },
  { feature: "Letter variable",  pattern: "12321",        match: "level",         note: "same digit = same letter"   },
  { feature: "Logical AND",      pattern: "c* & *s",      match: "cats",          note: "must match both"            },
  { feature: "Logical OR",       pattern: "c... | ...r",  match: "cast",          note: "matches either"             },
  { feature: "Logical NOT",      pattern: "c* & !cat*",   match: "cast",          note: "exclude matches"            },
  { feature: "Sub-pattern",      pattern: "...(;orange)", match: "patronage",     note: "() switches mode"           },
  { feature: "Punctuation",      pattern: "...-..-....", match: "pick-me-up",    note: "normalize off to use"       },
];

// ── Reference panels ──────────────────────────────────────────────────────────

function ReferenceHeader() {
  return (
    <div className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide px-3 mb-1.5">
      Pattern reference
    </div>
  );
}

function ReferenceFull({ onPatternClick }: { onPatternClick: (p: string) => void }) {
  return (
    <div className="mb-2 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
      <div className="px-3 pt-2 pb-1">
        <ReferenceHeader />
      </div>
      <table className="w-full text-xs" style={{ borderCollapse: "collapse" }}>
        <thead>
          <tr className="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
            <th className="text-left px-3 py-1.5 font-medium text-gray-400 dark:text-gray-500 w-1/4">Feature</th>
            <th className="text-left px-3 py-1.5 font-medium text-gray-400 dark:text-gray-500 w-1/4">Pattern</th>
            <th className="text-left px-3 py-1.5 font-medium text-gray-400 dark:text-gray-500 w-1/4">Match</th>
            <th className="text-left px-3 py-1.5 font-medium text-gray-400 dark:text-gray-500 w-1/4">Notes</th>
          </tr>
        </thead>
        <tbody>
          {REFERENCE_ROWS.map((row) => (
            <tr key={row.feature}
              className="border-b border-gray-100 dark:border-gray-700 last:border-0 hover:bg-white dark:hover:bg-gray-700 cursor-pointer"
              onClick={() => onPatternClick(row.pattern)}
            >
              <td className="px-3 py-1.5 text-gray-600 dark:text-gray-300 font-medium">{row.feature}</td>
              <td className="px-3 py-1.5">
                <span className="font-mono text-gray-800 dark:text-gray-200 bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600 rounded px-1.5 py-0.5">
                  {row.pattern}
                </span>
              </td>
              <td className="px-3 py-1.5 font-mono text-gray-500 dark:text-gray-400">{row.match}</td>
              <td className="px-3 py-1.5 text-gray-400 dark:text-gray-500">{row.note}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ReferenceCompact({ onPatternClick }: { onPatternClick: (p: string) => void }) {
  return (
    <div className="mb-2 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2">
      <ReferenceHeader />
      <div className="grid grid-cols-2 gap-x-4 gap-y-0.5">
        {REFERENCE_ROWS.map((row) => (
          <div
            key={row.feature}
            onClick={() => onPatternClick(row.pattern)}
            className="flex items-baseline gap-1 font-mono text-xs overflow-hidden cursor-pointer hover:opacity-70"
          >
            <span className="font-sans text-gray-500 dark:text-gray-400 flex-shrink-0 text-xs">{row.feature}</span>
            <span className="text-gray-300 dark:text-gray-600 flex-shrink-0">(</span>
            <span className="text-gray-800 dark:text-gray-200 flex-shrink-0">{row.pattern}</span>
            <span className="text-gray-400 dark:text-gray-500 flex-shrink-0">→</span>
            <span className="text-gray-500 dark:text-gray-400 truncate">{row.match.split(",")[0].split(" ")[0]}</span>
            <span className="text-gray-300 dark:text-gray-600 flex-shrink-0">)</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Main app ─────────────────────────────────────────────────────────────────

export default function App() {
  const [pattern, setPattern] = useState("");
  const [explanation, setExplanation] = useState("");
  const [results, setResults] = useState<MatchGroup[]>([]);
  const [dictName, setDictName] = useState("");
  const [status, setStatus] = useState("Enter a pattern and press Search");
  const [loading, setLoading] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [selectedWords, setSelectedWords] = useState<Set<string>>(new Set());
  const [contextMenu, setContextMenu] = useState<ContextMenu | null>(null);

  const [normalize, setNormalize] = useState(DEFAULTS.normalize);
  const [variantMode, setVariantMode] = useState<VariantMode>(DEFAULTS.variantMode);
  const [viewMode, setViewMode] = useState<ViewMode>(DEFAULTS.viewMode);
  const [minLen, setMinLen] = useState(DEFAULTS.minLen);
  const [maxLen, setMaxLen] = useState(DEFAULTS.maxLen);
  const [referenceMode, setReferenceMode] = useState<ReferenceMode>(DEFAULTS.referenceMode);
  const [showDescription, setShowDescription] = useState(DEFAULTS.showDescription);
  const [showOptions, setShowOptions] = useState(DEFAULTS.showOptions);
  const [appearance, setAppearance] = useState<AppearanceMode>(DEFAULTS.appearance);

  const storeRef = useRef<Store | null>(null);
  const explainTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const settingsLoaded = useRef(false);
  const historyRef = useRef<HTMLDivElement>(null);

  const allWords = results.map((r) => r.normalized);

  useEffect(() => { applyTheme(appearance); }, [appearance]);

  // Load settings
  useEffect(() => {
    load(STORE_FILE, { autoSave: true, defaults: {} }).then((store) => {
      storeRef.current = store;
      Promise.all([
        store.get<boolean>("normalize"),
        store.get<VariantMode>("variantMode"),
        store.get<ViewMode>("viewMode"),
        store.get<number>("minLen"),
        store.get<number>("maxLen"),
        store.get<ReferenceMode>("referenceMode"),
        store.get<boolean>("showDescription"),
        store.get<boolean>("showOptions"),
        store.get<AppearanceMode>("appearance"),
        store.get<HistoryEntry[]>("history"),
      ]).then(([n, vm, view, min, max, ref_, desc, opts, app_, hist]) => {
        if (n !== null && n !== undefined) setNormalize(n);
        if (vm) setVariantMode(vm);
        if (view) setViewMode(view);
        if (min !== null && min !== undefined) setMinLen(min);
        if (max !== null && max !== undefined) setMaxLen(max);
        if (ref_) setReferenceMode(ref_);
        if (desc !== null && desc !== undefined) setShowDescription(desc);
        if (opts !== null && opts !== undefined) setShowOptions(opts);
        if (app_) { setAppearance(app_); applyTheme(app_); }
        if (hist) setHistory(hist);
        settingsLoaded.current = true;
      });
    });
  }, []);

  // Persist settings
  useEffect(() => {
    if (!settingsLoaded.current || !storeRef.current) return;
    const s = storeRef.current;
    s.set("normalize", normalize);
    s.set("variantMode", variantMode);
    s.set("viewMode", viewMode);
    s.set("minLen", minLen);
    s.set("maxLen", maxLen);
    s.set("referenceMode", referenceMode);
    s.set("showDescription", showDescription);
    s.set("showOptions", showOptions);
    s.set("appearance", appearance);
  }, [normalize, variantMode, viewMode, minLen, maxLen,
      referenceMode, showDescription, showOptions, appearance]);

  // Persist history
  useEffect(() => {
    if (!settingsLoaded.current || !storeRef.current) return;
    storeRef.current.set("history", history);
  }, [history]);

  // Menu events
  useEffect(() => {
    const unlisten: Array<() => void> = [];

    listen<string>("menu:toggle", (event) => {
      const panel = event.payload;
      if (panel === "description") setShowDescription((v) => !v);
      else if (panel === "options") setShowOptions((v) => !v);
    }).then((u) => unlisten.push(u));

    listen<string>("menu:reference", (event) => {
      setReferenceMode(event.payload as ReferenceMode);
    }).then((u) => unlisten.push(u));

    listen<string>("menu:appearance", (event) => {
      const mode = event.payload as AppearanceMode;
      setAppearance(mode);
      applyTheme(mode);
    }).then((u) => unlisten.push(u));

    listen<string>("menu:reset_layout", () => {
      setReferenceMode(DEFAULTS.referenceMode);
      setShowDescription(DEFAULTS.showDescription);
      setShowOptions(DEFAULTS.showOptions);
      setNormalize(DEFAULTS.normalize);
      setVariantMode(DEFAULTS.variantMode);
      setViewMode(DEFAULTS.viewMode);
      setMinLen(DEFAULTS.minLen);
      setMaxLen(DEFAULTS.maxLen);
      setAppearance(DEFAULTS.appearance);
      applyTheme(DEFAULTS.appearance);
    }).then((u) => unlisten.push(u));

    return () => unlisten.forEach((u) => u());
  }, []);

  // Close history on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (historyRef.current && !historyRef.current.contains(e.target as Node)) {
        setShowHistory(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  // Close context menu on click
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [contextMenu]);

  // Debounced pattern description — 500ms, calls Rust describe_pattern
  useEffect(() => {
    setExplanation("");
    if (explainTimerRef.current) clearTimeout(explainTimerRef.current);
    if (!pattern.trim()) return;
    explainTimerRef.current = setTimeout(async () => {
      try {
        const desc = await invoke<string | null>("describe_pattern", { pattern: pattern.trim() });
        setExplanation(desc ?? "");
      } catch {
        setExplanation("");
      }
    }, 500);
    return () => { if (explainTimerRef.current) clearTimeout(explainTimerRef.current); };
  }, [pattern]);

  const doSearch = useCallback(async (patternOverride?: string) => {
    const trimmed = (patternOverride ?? pattern).trim();
    if (!trimmed) return;
    if (patternOverride) setPattern(patternOverride);
    setLoading(true);
    setStatus("Searching...");
    setShowHistory(false);
    setSelectedWords(new Set());
    try {
      const response = await invoke<SearchResponse>("search", {
        pattern: trimmed, minLen, maxLen, normalize,
      });
      setResults(response.results);
      setDictName(response.dict_name);
      const matchCount = response.results.length;
      setStatus(matchCount === 0 ? "No matches found" : `${matchCount} matches`);
      setHistory((prev) => {
        const filtered = prev.filter((h) => h.pattern !== trimmed);
        return [{ pattern: trimmed, matchCount }, ...filtered].slice(0, MAX_HISTORY);
      });
    } catch (err) {
      setStatus(`Error: ${err}`);
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, [pattern, minLen, maxLen, normalize]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") doSearch();
    if (e.key === "Escape") setShowHistory(false);
  };

  // Selecting from history runs the search immediately
  const selectHistory = (entry: HistoryEntry) => {
    setShowHistory(false);
    doSearch(entry.pattern);
  };

  // Clicking a reference pattern runs search immediately
  const handleReferenceClick = (p: string) => {
    doSearch(p);
  };

  const handleWordClick = useCallback((word: string, e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu(null);
    if (e.metaKey || e.ctrlKey) {
      setSelectedWords((prev) => {
        const next = new Set(prev);
        if (next.has(word)) next.delete(word); else next.add(word);
        return next;
      });
    } else if (e.shiftKey && selectedWords.size > 0) {
      const lastSelected = [...selectedWords].pop()!;
      const fromIdx = allWords.indexOf(lastSelected);
      const toIdx = allWords.indexOf(word);
      if (fromIdx !== -1 && toIdx !== -1) {
        const [start, end] = fromIdx < toIdx ? [fromIdx, toIdx] : [toIdx, fromIdx];
        setSelectedWords(new Set(allWords.slice(start, end + 1)));
      }
    } else {
      setSelectedWords(new Set([word]));
    }
  }, [allWords, selectedWords]);

  const handleWordRightClick = useCallback((word: string, e: React.MouseEvent) => {
    e.preventDefault();
    setSelectedWords((prev) => {
      if (!prev.has(word)) return new Set([word]);
      return prev;
    });
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, []);

  const handleCopy = useCallback(async () => {
    const text = [...selectedWords].join("\n");
    try {
      await writeText(text);
    } catch {
      await navigator.clipboard.writeText(text);
    }
    setContextMenu(null);
  }, [selectedWords]);

  const grouped = results.reduce<Record<number, MatchGroup[]>>((acc, r) => {
    const len = r.normalized.length;
    if (!acc[len]) acc[len] = [];
    acc[len].push(r);
    return acc;
  }, {});
  const lengths = Object.keys(grouped).map(Number).sort((a, b) => a - b);

  return (
    <div
      style={{ display: "flex", flexDirection: "column", height: "100vh", overflow: "hidden" }}
      className="bg-white dark:bg-gray-900"
      onClick={() => setContextMenu(null)}
    >
      {/* ── STATIC HEADER ── */}
      <div className="border-b border-gray-200 dark:border-gray-700 px-5 pt-3 pb-0 flex-shrink-0 bg-white dark:bg-gray-900">

        {referenceMode === "full" && <ReferenceFull onPatternClick={handleReferenceClick} />}
        {referenceMode === "compact" && <ReferenceCompact onPatternClick={handleReferenceClick} />}

        {/* Search row */}
        <div className="flex gap-2 mb-1.5" ref={historyRef}>
          <div className="relative flex-1">
            <input
              type="text"
              value={pattern}
              onChange={(e) => setPattern(e.target.value)}
              onKeyDown={handleKeyDown}
              onFocus={() => history.length > 0 && setShowHistory(true)}
              placeholder="e.g.  .l...r.n  or  ;acenrt  or  m*ja"
              className="w-full pl-3 pr-8 py-2 border border-gray-300 dark:border-gray-600 rounded-lg font-mono text-sm focus:outline-none focus:ring-2 focus:ring-blue-400 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500"
              autoCorrect="off" autoCapitalize="off" autoComplete="off" spellCheck={false} autoFocus
            />
            {history.length > 0 && (
              <button
                onClick={() => setShowHistory(!showHistory)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-xs px-1"
                tabIndex={-1}
              >▾</button>
            )}

            {showHistory && history.length > 0 && (
              <div className="absolute top-full left-0 right-0 mt-1 z-50 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg overflow-hidden">
                <div className="flex items-center justify-between px-3 py-1.5 bg-gray-50 dark:bg-gray-750 border-b border-gray-100 dark:border-gray-700">
                  <span className="text-xs font-medium text-gray-400 uppercase tracking-wide">Recent</span>
                  <button
                    onClick={() => { setHistory([]); setShowHistory(false); }}
                    className="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                  >Clear</button>
                </div>
                <div className="max-h-56 overflow-y-auto">
                  {history.map((entry) => (
                    <button
                      key={entry.pattern}
                      onClick={() => selectHistory(entry)}
                      className="w-full flex items-center justify-between px-3 py-2 hover:bg-gray-50 dark:hover:bg-gray-700 text-left border-b border-gray-50 dark:border-gray-700 last:border-0"
                    >
                      <span className="font-mono text-sm text-gray-800 dark:text-gray-200">{entry.pattern}</span>
                      <span className="text-xs text-gray-400 ml-4 flex-shrink-0">{entry.matchCount} matches</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>

          <button
            onClick={() => doSearch()}
            disabled={loading}
            className="px-5 py-2 bg-blue-500 text-white rounded-lg text-sm font-medium hover:bg-blue-600 disabled:opacity-50 transition-colors flex-shrink-0"
          >
            {loading ? "..." : "Search"}
          </button>
        </div>

        {/* Pattern description — always shown, powered by Rust describe_pattern */}
        {showDescription && (
          <div className="text-xs text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-gray-800 rounded-lg px-3 py-1.5 mb-1.5 leading-relaxed">
            {explanation || (
              <span className="text-gray-400 dark:text-gray-500 italic">Enter a pattern to see a description</span>
            )}
          </div>
        )}

        {/* Options */}
        {showOptions && (
          <div className="flex flex-wrap items-center gap-4 py-2">
            <label className="flex items-center gap-2 cursor-pointer select-none">
              <div
                onClick={() => setNormalize(!normalize)}
                className={`w-8 h-4 rounded-full transition-colors relative cursor-pointer ${normalize ? "bg-blue-500" : "bg-gray-300 dark:bg-gray-600"}`}
              >
                <div className={`absolute top-0.5 w-3 h-3 bg-white rounded-full shadow transition-transform ${normalize ? "translate-x-4" : "translate-x-0.5"}`} />
              </div>
              <span className="text-xs text-gray-600 dark:text-gray-400">Normalize</span>
            </label>

            {normalize && (
              <div className="flex items-center gap-2">
                <span className="text-xs text-gray-400">Variants:</span>
                {(["show", "hide"] as VariantMode[]).map((mode) => (
                  <button
                    key={mode}
                    onClick={() => setVariantMode(mode)}
                    className={`px-2 py-0.5 rounded text-xs border transition-colors ${
                      variantMode === mode
                        ? "bg-blue-500 text-white border-blue-500"
                        : "bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 border-gray-300 dark:border-gray-600"
                    }`}
                  >{mode === "show" ? "Show" : "Hide"}</button>
                ))}
              </div>
            )}

            <div className="flex items-center gap-2 ml-auto">
              <span className="text-xs text-gray-400">View:</span>
              {(["grid", "list"] as ViewMode[]).map((v) => (
                <button
                  key={v}
                  onClick={() => setViewMode(v)}
                  className={`px-2 py-0.5 rounded text-xs border transition-colors ${
                    viewMode === v
                      ? "bg-blue-500 text-white border-blue-500"
                      : "bg-white dark:bg-gray-800 text-gray-500 dark:text-gray-400 border-gray-300 dark:border-gray-600"
                  }`}
                >{v === "grid" ? "Grid" : "List"}</button>
              ))}
            </div>
          </div>
        )}

        {/* Word length filter */}
        <div className="flex items-center gap-2 pb-2.5 text-xs text-gray-400">
          <span>Word length:</span>
          <input
            type="number" value={minLen} min={1} max={maxLen}
            onChange={(e) => setMinLen(Math.max(1, Number(e.target.value)))}
            className="w-12 px-1.5 py-0.5 border border-gray-300 dark:border-gray-600 rounded text-center text-xs text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-800"
          />
          <span>to</span>
          <input
            type="number" value={maxLen} min={minLen} max={100}
            onChange={(e) => setMaxLen(Math.max(minLen, Number(e.target.value)))}
            className="w-12 px-1.5 py-0.5 border border-gray-300 dark:border-gray-600 rounded text-center text-xs text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-800"
          />
          <span>letters</span>
        </div>
      </div>

      {/* ── RESULTS HEADER ── */}
      {results.length > 0 && (
        <div className="flex items-center justify-between px-5 py-2 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
          <div className="flex items-baseline gap-2">
            <span className="text-sm font-semibold text-gray-800 dark:text-gray-100">{results.length} matches</span>
            {dictName && <span className="text-xs text-gray-400">from {dictName}</span>}
          </div>
        </div>
      )}

      {/* ── SCROLLABLE RESULTS ── */}
      <div className="flex-1 overflow-y-auto px-5 py-3 bg-white dark:bg-gray-900">
        {results.length === 0 && (
          <p className="text-sm text-gray-400 dark:text-gray-500">{status}</p>
        )}
        {results.length > 0 && viewMode === "grid" && (
          <GridView lengths={lengths} grouped={grouped} normalize={normalize} variantMode={variantMode}
            selectedWords={selectedWords} onWordClick={handleWordClick} onWordRightClick={handleWordRightClick} />
        )}
        {results.length > 0 && viewMode === "list" && (
          <ListView lengths={lengths} grouped={grouped} normalize={normalize} variantMode={variantMode}
            selectedWords={selectedWords} onWordClick={handleWordClick} onWordRightClick={handleWordRightClick} />
        )}
      </div>

      {/* ── STATUS BAR ── */}
      <div className="flex-shrink-0 px-5 py-1.5 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 flex items-center">
        <span className="text-xs text-gray-400 dark:text-gray-500">
          {selectedWords.size > 0
            ? `${selectedWords.size} word${selectedWords.size === 1 ? "" : "s"} selected`
            : results.length > 0 ? `${results.length} words` : ""}
        </span>
      </div>

      {/* ── CONTEXT MENU ── */}
      {contextMenu && (
        <ContextMenuPopup
          x={contextMenu.x}
          y={contextMenu.y}
          onCopy={handleCopy}
        />
      )}
    </div>
  );
}

// ── Context menu ──────────────────────────────────────────────────────────────

function ContextMenuPopup({ x, y, onCopy }: {
  x: number; y: number; onCopy: () => void;
}) {
  const CMItem = ({ label, onClick, disabled = false }: {
    label: string; onClick?: () => void; disabled?: boolean;
  }) => (
    <button
      onClick={disabled ? undefined : onClick}
      className={`w-full text-left px-4 py-1.5 text-sm transition-colors ${
        disabled
          ? "context-menu-disabled cursor-default"
          : "text-gray-700 dark:text-gray-200 hover:bg-blue-500 hover:text-white cursor-pointer"
      }`}
    >{label}</button>
  );

  return (
    <div
      style={{ position: "fixed", left: x, top: y, zIndex: 1000 }}
      className="bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-xl overflow-hidden min-w-48"
      onMouseDown={(e) => e.stopPropagation()}
    >
      <CMItem label="Copy" onClick={onCopy} />
      <div className="border-t border-gray-100 dark:border-gray-700 my-0.5" />
      <CMItem label="Look up definition" disabled />
      <CMItem label="Open in external dictionary" disabled />
      <div className="border-t border-gray-100 dark:border-gray-700 my-0.5" />
      <CMItem label="Copy to word list" disabled />
    </div>
  );
}

// ── Grid view ─────────────────────────────────────────────────────────────────

interface ViewProps {
  lengths: number[];
  grouped: Record<number, MatchGroup[]>;
  normalize: boolean;
  variantMode: VariantMode;
  selectedWords: Set<string>;
  onWordClick: (word: string, e: React.MouseEvent) => void;
  onWordRightClick: (word: string, e: React.MouseEvent) => void;
}

function GridView({ lengths, grouped, normalize, variantMode, selectedWords, onWordClick, onWordRightClick }: ViewProps) {
  return (
    <>
      {lengths.map((len) => (
        <div key={len} className="mb-5">
          <div className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide mb-2">
            {len} letter{len === 1 ? "" : "s"} ({grouped[len].length})
          </div>
          <div className="flex flex-wrap gap-2">
            {grouped[len].map((r) => (
              <div
                key={r.normalized}
                onClick={(e) => onWordClick(r.normalized, e)}
                onContextMenu={(e) => onWordRightClick(r.normalized, e)}
                className={`flex items-baseline gap-1 border rounded px-3 py-1.5 cursor-pointer select-none transition-colors ${
                  selectedWords.has(r.normalized)
                    ? "bg-blue-50 dark:bg-blue-900 border-blue-300 dark:border-blue-700"
                    : "bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-500"
                }`}
              >
                <span className="font-mono text-sm text-gray-800 dark:text-gray-200">{r.normalized}</span>
                {normalize && variantMode === "show" && r.variants.length > 0 && (
                  <span className="text-xs text-gray-400">({r.variants.join(", ")})</span>
                )}
                {r.balance && <span className="font-mono text-xs text-blue-500">{r.balance}</span>}
              </div>
            ))}
          </div>
        </div>
      ))}
    </>
  );
}

// ── List view ─────────────────────────────────────────────────────────────────

function ListView({ lengths, grouped, normalize, variantMode, selectedWords, onWordClick, onWordRightClick }: ViewProps) {
  const [collapsed, setCollapsed] = useState<Record<number, boolean>>({});
  const toggle = (len: number) => setCollapsed((prev) => ({ ...prev, [len]: !prev[len] }));

  return (
    <>
      {lengths.map((len) => {
        const isCollapsed = collapsed[len] ?? false;
        return (
          <div key={len} className="mb-2 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
            <button
              onClick={() => toggle(len)}
              className={`w-full flex items-center justify-between px-3 py-2 text-left transition-colors ${
                isCollapsed ? "bg-gray-50 dark:bg-gray-800" : "bg-gray-100 dark:bg-gray-700"
              }`}
            >
              <div className="flex items-center gap-2">
                <span
                  className={`text-xs transition-transform ${!isCollapsed ? "text-gray-500 dark:text-gray-300" : "text-gray-400 dark:text-gray-500"}`}
                  style={{ display: "inline-block", transform: isCollapsed ? "rotate(-90deg)" : "rotate(0deg)" }}
                >▾</span>
                <span className={`text-xs font-semibold ${isCollapsed ? "text-gray-500 dark:text-gray-400" : "text-gray-700 dark:text-gray-200"}`}>
                  {len} letter{len === 1 ? "" : "s"}
                </span>
              </div>
              <span className="text-xs text-gray-400 dark:text-gray-400 bg-gray-50 dark:bg-gray-600 px-2 py-0.5 rounded-full border border-gray-200 dark:border-gray-500">
                {grouped[len].length} match{grouped[len].length === 1 ? "" : "es"}
              </span>
            </button>

            {!isCollapsed && (
              <div className="divide-y divide-gray-50 dark:divide-gray-800 bg-white dark:bg-gray-900">
                {grouped[len].map((r) => (
                  <div
                    key={r.normalized}
                    onClick={(e) => onWordClick(r.normalized, e)}
                    onContextMenu={(e) => onWordRightClick(r.normalized, e)}
                    className={`flex items-baseline justify-between px-3 py-1.5 cursor-pointer select-none transition-colors ${
                      selectedWords.has(r.normalized)
                        ? "bg-blue-50 dark:bg-blue-900"
                        : "hover:bg-gray-50 dark:hover:bg-gray-800"
                    }`}
                  >
                    <span className="font-mono text-sm text-gray-800 dark:text-gray-200">{r.normalized}</span>
                    <div className="flex items-baseline gap-2">
                      {normalize && variantMode === "show" && r.variants.length > 0 && (
                        <span className="text-xs text-gray-400">({r.variants.join(", ")})</span>
                      )}
                      {r.balance && <span className="font-mono text-xs text-blue-500">{r.balance}</span>}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}
