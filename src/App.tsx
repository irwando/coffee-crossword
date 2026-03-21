import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { load, Store } from "@tauri-apps/plugin-store";
import { listen } from "@tauri-apps/api/event";

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

type VariantMode = "show" | "hide";
type ViewMode = "grid" | "list";
type AppearanceMode = "light" | "dark" | "system";

const STORE_FILE = "settings.json";
const MAX_HISTORY = 100;

const DEFAULTS = {
  normalize: true,
  variantMode: "show" as VariantMode,
  viewMode: "list" as ViewMode,
  minLen: 1,
  maxLen: 50,
  showReference: true,
  showDescription: true,
  showOptions: true,
  appearance: "system" as AppearanceMode,
};

// ── Theme management ─────────────────────────────────────────────────────────

let systemDarkListener: ((e: MediaQueryListEvent) => void) | null = null;
let systemMQ: MediaQueryList | null = null;

function applyTheme(mode: AppearanceMode) {
  const root = document.documentElement;
  // Clean up any previous system listener
  if (systemMQ && systemDarkListener) {
    systemMQ.removeEventListener("change", systemDarkListener);
    systemDarkListener = null;
    systemMQ = null;
  }
  if (mode === "light") {
    root.classList.remove("dark");
    root.classList.add("light");
  } else if (mode === "dark") {
    root.classList.remove("light");
    root.classList.add("dark");
  } else {
    // System — match current preference and track changes
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

// ── Pattern explainer ────────────────────────────────────────────────────────

function explainTemplate(tmpl: string): string {
  const hasWild = tmpl.includes("*");
  const first = tmpl[0];
  const last = tmpl[tmpl.length - 1];
  const firstIsLetter = first && /[a-zA-Z]/.test(first);
  const lastIsLetter = last && /[a-zA-Z]/.test(last);

  if (hasWild) {
    let desc = "Words";
    if (firstIsLetter && lastIsLetter && first.toLowerCase() !== last.toLowerCase()) {
      desc += ` starting with "${first.toUpperCase()}" and ending with "${last.toUpperCase()}"`;
    } else if (firstIsLetter) {
      desc += ` starting with "${first.toUpperCase()}"`;
    } else if (lastIsLetter) {
      desc += ` ending with "${last.toUpperCase()}"`;
    } else {
      desc += " of any length";
    }
    return desc;
  }

  const len = tmpl.length;
  let desc = `${len}-letter words`;
  if (firstIsLetter && lastIsLetter && len > 1 && first.toLowerCase() !== last.toLowerCase()) {
    desc += ` starting with "${first.toUpperCase()}" and ending with "${last.toUpperCase()}"`;
  } else if (firstIsLetter) {
    desc += ` starting with "${first.toUpperCase()}"`;
  } else if (lastIsLetter) {
    desc += ` ending with "${last.toUpperCase()}"`;
  }
  return desc;
}

function explainPattern(raw: string): string {
  const val = raw.trim();
  if (!val) return "";
  const semiPos = val.indexOf(";");
  if (semiPos === -1) return explainTemplate(val);

  const tmpl = val.slice(0, semiPos);
  const anagramPart = val.slice(semiPos + 1);
  const letters = anagramPart.replace(/[^a-zA-Z]/g, "").toUpperCase();
  const dots = (anagramPart.match(/[.?]/g) || []).length;
  const hasWild = anagramPart.includes("*");

  if (!tmpl) {
    let s = letters ? `Anagrams of "${letters}"` : "Anagram search";
    if (dots === 1) s += " using 1 additional letter";
    else if (dots > 1) s += ` using ${dots} additional letters`;
    if (hasWild) s += " (any number of extra letters)";
    return s;
  }

  let s = explainTemplate(tmpl);
  if (letters) s += `, containing the letters "${letters}"`;
  if (dots === 1) s += " plus 1 free letter";
  else if (dots > 1) s += ` plus ${dots} free letters`;
  return s;
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

  const [normalize, setNormalize] = useState(DEFAULTS.normalize);
  const [variantMode, setVariantMode] = useState<VariantMode>(DEFAULTS.variantMode);
  const [viewMode, setViewMode] = useState<ViewMode>(DEFAULTS.viewMode);
  const [minLen, setMinLen] = useState(DEFAULTS.minLen);
  const [maxLen, setMaxLen] = useState(DEFAULTS.maxLen);
  const [showReference, setShowReference] = useState(DEFAULTS.showReference);
  const [showDescription, setShowDescription] = useState(DEFAULTS.showDescription);
  const [showOptions, setShowOptions] = useState(DEFAULTS.showOptions);
  const [appearance, setAppearance] = useState<AppearanceMode>(DEFAULTS.appearance);

  const storeRef = useRef<Store | null>(null);
  const explainTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const settingsLoaded = useRef(false);
  const historyRef = useRef<HTMLDivElement>(null);

  // Apply theme whenever appearance changes
  useEffect(() => { applyTheme(appearance); }, [appearance]);

  // Load all settings on startup
  useEffect(() => {
    load(STORE_FILE, { autoSave: true }).then((store) => {
      storeRef.current = store;
      Promise.all([
        store.get<boolean>("normalize"),
        store.get<VariantMode>("variantMode"),
        store.get<ViewMode>("viewMode"),
        store.get<number>("minLen"),
        store.get<number>("maxLen"),
        store.get<boolean>("showReference"),
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
        if (ref_ !== null && ref_ !== undefined) setShowReference(ref_);
        if (desc !== null && desc !== undefined) setShowDescription(desc);
        if (opts !== null && opts !== undefined) setShowOptions(opts);
        if (app_) { setAppearance(app_); applyTheme(app_); }
        if (hist) setHistory(hist);
        settingsLoaded.current = true;
      });
    });
  }, []);

  // Persist settings on change
  useEffect(() => {
    if (!settingsLoaded.current || !storeRef.current) return;
    const s = storeRef.current;
    s.set("normalize", normalize);
    s.set("variantMode", variantMode);
    s.set("viewMode", viewMode);
    s.set("minLen", minLen);
    s.set("maxLen", maxLen);
    s.set("showReference", showReference);
    s.set("showDescription", showDescription);
    s.set("showOptions", showOptions);
    s.set("appearance", appearance);
  }, [normalize, variantMode, viewMode, minLen, maxLen,
      showReference, showDescription, showOptions, appearance]);

  // Persist history
  useEffect(() => {
    if (!settingsLoaded.current || !storeRef.current) return;
    storeRef.current.set("history", history);
  }, [history]);

  // Listen for native menu events 
  useEffect(() => {
    const unlisten: Array<() => void> = [];

    listen<string>("menu:toggle", (event) => {
      console.log("menu:toggle received", event.payload);
      const panel = event.payload;
      if (panel === "reference") setShowReference((v) => !v);
      else if (panel === "description") setShowDescription((v) => !v);
      else if (panel === "options") setShowOptions((v) => !v);
    }).then((u) => unlisten.push(u));

    listen<string>("menu:appearance", (event) => {
      const mode = event.payload as AppearanceMode;
      setAppearance(mode);
      applyTheme(mode);
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

  // Debounced pattern explanation
  useEffect(() => {
    setExplanation("");
    if (explainTimerRef.current) clearTimeout(explainTimerRef.current);
    if (!pattern.trim()) return;
    explainTimerRef.current = setTimeout(() => {
      setExplanation(explainPattern(pattern));
    }, 2000);
    return () => { if (explainTimerRef.current) clearTimeout(explainTimerRef.current); };
  }, [pattern]);

  const doSearch = useCallback(async () => {
    const trimmed = pattern.trim();
    if (!trimmed) return;
    setLoading(true);
    setStatus("Searching...");
    setShowHistory(false);
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

  const selectHistory = (entry: HistoryEntry) => {
    setPattern(entry.pattern);
    setResults([]);
    setStatus("Enter a pattern and press Search");
    setShowHistory(false);
  };

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
    >
      {/* ── STATIC HEADER ── */}
      <div className="border-b border-gray-200 dark:border-gray-700 px-5 pt-3 pb-0 flex-shrink-0 bg-white dark:bg-gray-900">

        {/* Pattern reference — above search box */}
        {showReference && (
          <div className="mb-2 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg px-4 py-2 font-mono text-xs text-gray-500 dark:text-gray-400">
            <div className="grid grid-cols-2 gap-x-6 gap-y-0.5">
              <span><span className="text-gray-700 dark:text-gray-300">.l...r.n</span> → electron (template)</span>
              <span><span className="text-gray-700 dark:text-gray-300">;acenrt</span> → canter, trance… (anagram)</span>
              <span><span className="text-gray-700 dark:text-gray-300">m*ja</span> → maharaja (wildcard)</span>
              <span><span className="text-gray-700 dark:text-gray-300">q???k</span> → quick, quack… (? = any)</span>
              <span><span className="text-gray-700 dark:text-gray-300">;acenrt.</span> → anagram + 1 blank</span>
              <span><span className="text-gray-700 dark:text-gray-300">e....;cats</span> → template + anagram</span>
            </div>
          </div>
        )}

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
              autoCorrect="off"
              autoCapitalize="off"
              autoComplete="off"
              spellCheck={false}
              autoFocus
            />
            {history.length > 0 && (
              <button
                onClick={() => setShowHistory(!showHistory)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-xs px-1"
                tabIndex={-1}
              >▾</button>
            )}

            {/* History dropdown */}
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
            onClick={doSearch}
            disabled={loading}
            className="px-5 py-2 bg-blue-500 text-white rounded-lg text-sm font-medium hover:bg-blue-600 disabled:opacity-50 transition-colors flex-shrink-0"
          >
            {loading ? "..." : "Search"}
          </button>
        </div>

        {/* Pattern description */}
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
                  >
                    {mode === "show" ? "Show" : "Hide"}
                  </button>
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
                >
                  {v === "grid" ? "Grid" : "List"}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Word length filter */}
        <div className="flex items-center gap-2 pb-2.5 text-xs text-gray-400">
          <span>Word length:</span>
          <input
            type="number"
            value={minLen}
            min={1}
            max={maxLen}
            onChange={(e) => setMinLen(Math.max(1, Number(e.target.value)))}
            className="w-12 px-1.5 py-0.5 border border-gray-300 dark:border-gray-600 rounded text-center text-xs text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-800"
          />
          <span>to</span>
          <input
            type="number"
            value={maxLen}
            min={minLen}
            max={100}
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
            <span className="text-sm font-semibold text-gray-800 dark:text-gray-100">
              {results.length} matches
            </span>
            {dictName && (
              <span className="text-xs text-gray-400">from {dictName}</span>
            )}
          </div>
        </div>
      )}

      {/* ── SCROLLABLE RESULTS ── */}
      <div className="flex-1 overflow-y-auto px-5 py-3 bg-white dark:bg-gray-900">
        {results.length === 0 && (
          <p className="text-sm text-gray-400 dark:text-gray-500">{status}</p>
        )}
        {results.length > 0 && viewMode === "grid" && (
          <GridView lengths={lengths} grouped={grouped} normalize={normalize} variantMode={variantMode} />
        )}
        {results.length > 0 && viewMode === "list" && (
          <ListView lengths={lengths} grouped={grouped} normalize={normalize} variantMode={variantMode} />
        )}
      </div>
    </div>
  );
}

// ── Grid view ─────────────────────────────────────────────────────────────────

interface ViewProps {
  lengths: number[];
  grouped: Record<number, MatchGroup[]>;
  normalize: boolean;
  variantMode: VariantMode;
}

function GridView({ lengths, grouped, normalize, variantMode }: ViewProps) {
  return (
    <>
      {lengths.map((len) => (
        <div key={len} className="mb-5">
          <div className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide mb-2">
            {len} letter{len === 1 ? "" : "s"} ({grouped[len].length})
          </div>
          <div className="flex flex-wrap gap-2">
            {grouped[len].map((r) => (
              <WordChip
                key={r.normalized}
                group={r}
                showVariants={normalize && variantMode === "show"}
              />
            ))}
          </div>
        </div>
      ))}
    </>
  );
}

// ── List view ─────────────────────────────────────────────────────────────────

function ListView({ lengths, grouped, normalize, variantMode }: ViewProps) {
  const [collapsed, setCollapsed] = useState<Record<number, boolean>>({});
  const toggle = (len: number) =>
    setCollapsed((prev) => ({ ...prev, [len]: !prev[len] }));

  return (
    <>
      {lengths.map((len) => {
        const isCollapsed = collapsed[len] ?? false;
        return (
          <div key={len} className="mb-2 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
            <button
              onClick={() => toggle(len)}
              className={`w-full flex items-center justify-between px-3 py-2 text-left transition-colors ${
                isCollapsed
                  ? "bg-gray-50 dark:bg-gray-800"
                  : "bg-gray-100 dark:bg-gray-700"
              }`}
            >
              <div className="flex items-center gap-2">
                <span
                  className={`text-xs transition-transform ${!isCollapsed ? "text-gray-500 dark:text-gray-300" : "text-gray-400 dark:text-gray-500"}`}
                  style={{
                    display: "inline-block",
                    transform: isCollapsed ? "rotate(-90deg)" : "rotate(0deg)",
                  }}
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
                    className="flex items-baseline justify-between px-3 py-1.5 hover:bg-gray-50 dark:hover:bg-gray-800"
                  >
                    <span className="font-mono text-sm text-gray-800 dark:text-gray-200">{r.normalized}</span>
                    <div className="flex items-baseline gap-2">
                      {normalize && variantMode === "show" && r.variants.length > 0 && (
                        <span className="text-xs text-gray-400">({r.variants.join(", ")})</span>
                      )}
                      {r.balance && (
                        <span className="font-mono text-xs text-blue-500">{r.balance}</span>
                      )}
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

// ── WordChip ──────────────────────────────────────────────────────────────────

function WordChip({ group, showVariants }: { group: MatchGroup; showVariants: boolean }) {
  return (
    <div className={`flex items-baseline gap-1 bg-white dark:bg-gray-800 border rounded px-3 py-1.5 ${
      showVariants && group.variants.length > 0
        ? "border-blue-200 dark:border-blue-800"
        : "border-gray-200 dark:border-gray-700"
    }`}>
      <span className="font-mono text-sm text-gray-800 dark:text-gray-200">{group.normalized}</span>
      {showVariants && group.variants.length > 0 && (
        <span className="text-xs text-gray-400">({group.variants.join(", ")})</span>
      )}
      {group.balance && (
        <span className="font-mono text-xs text-blue-500">{group.balance}</span>
      )}
    </div>
  );
}
