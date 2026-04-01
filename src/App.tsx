import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { load, Store } from "@tauri-apps/plugin-store";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import ResultsColumn from "./ResultsColumn";
import WordListDrawer from "./WordListDrawer";

// ── Types ─────────────────────────────────────────────────────────────────────

interface MatchGroup {
  normalized: string;
  variants: string[];
  balance: string | null;
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

// Per-list search state
interface ListResults {
  listId: string;
  listName: string;
  entryCount: number;
  results: MatchGroup[] | null; // null = loading
  isLoading: boolean;
}

// Registry snapshot (minimal — just what App.tsx needs)
interface ListEntry {
  id: string;
  display_name: string;
  word_count: number;
  cache_state: { type: string };
}
interface Registry {
  available: ListEntry[];
  active_ids: string[];
  dedup_enabled: boolean;
}

// ── Store / defaults ──────────────────────────────────────────────────────────

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

// ── Theme ─────────────────────────────────────────────────────────────────────

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
  { feature: "AND",              pattern: "c* & *s",      match: "cats",          note: "must match both"            },
  { feature: "OR",               pattern: "c... | ...r",  match: "cast",          note: "matches either"             },
  { feature: "NOT",              pattern: "c* & !cat*",   match: "cast",          note: "exclude matches"            },
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
      <div className="px-3 pt-2 pb-1"><ReferenceHeader /></div>
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

// ── Context menu ──────────────────────────────────────────────────────────────

function ContextMenuPopup({ x, y, onCopy }: { x: number; y: number; onCopy: () => void }) {
  const CMItem = ({ label, onClick, disabled = false }: { label: string; onClick?: () => void; disabled?: boolean }) => (
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

// ── Main app ──────────────────────────────────────────────────────────────────

export default function App() {
  // ── Search state ──────────────────────────────────────────────────────
  const [pattern, setPattern] = useState("");
  const [explanation, setExplanation] = useState("");
  const [listResults, setListResults] = useState<ListResults[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [buildInProgress, setBuildInProgress] = useState(false);
  const [listsLoading, setListsLoading] = useState(true);
  const [statusMsg, setStatusMsg] = useState("Enter a pattern and press Search");

  // ── Registry ──────────────────────────────────────────────────────────
  const [registry, setRegistry] = useState<Registry>({ available: [], active_ids: [], dedup_enabled: true });

  // ── UI state ──────────────────────────────────────────────────────────
  const [showHistory, setShowHistory] = useState(false);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [selectedWords, setSelectedWords] = useState<Set<string>>(new Set());
  const [contextMenu, setContextMenu] = useState<ContextMenu | null>(null);
  const [drawerOpen, setDrawerOpen] = useState(false);

  // ── Settings ──────────────────────────────────────────────────────────
  const [normalize, setNormalize] = useState(DEFAULTS.normalize);
  const [variantMode, setVariantMode] = useState<VariantMode>(DEFAULTS.variantMode);
  const [viewMode, setViewMode] = useState<ViewMode>(DEFAULTS.viewMode);
  const [minLen, setMinLen] = useState(DEFAULTS.minLen);
  const [maxLen, setMaxLen] = useState(DEFAULTS.maxLen);
  const [referenceMode, setReferenceMode] = useState<ReferenceMode>(DEFAULTS.referenceMode);
  const [showDescription, setShowDescription] = useState(DEFAULTS.showDescription);
  const [showOptions, setShowOptions] = useState(DEFAULTS.showOptions);
  const [appearance, setAppearance] = useState<AppearanceMode>(DEFAULTS.appearance);

  // ── Refs ──────────────────────────────────────────────────────────────
  const storeRef = useRef<Store | null>(null);
  const explainTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const settingsLoaded = useRef(false);
  const historyRef = useRef<HTMLDivElement>(null);

  // All words currently visible (for shift-click range selection)
  const allWords = listResults.flatMap((lr) => (lr.results ?? []).map((r) => r.normalized));

  // ── Theme ────────────────────────────────────────────────────────────────
  useEffect(() => { applyTheme(appearance); }, [appearance]);

  // ── Load settings ─────────────────────────────────────────────────────
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
        store.get<string[]>("word_list_active_ids"),
        store.get<Record<string, string>>("word_list_display_names"),
        store.get<boolean>("dedup_enabled"),
      ]).then(([n, vm, view, min, max, ref_, desc, opts, app_, hist,
                activeIds, displayNames, dedup]) => {
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

        // Restore active list IDs and display names to backend, then load registry.
        const ids = activeIds ?? [];
        const names = displayNames ?? {};

        // Apply persisted active_ids + names to backend, then fetch registry.
        const applyAndFetch = async () => {
          try {
            // First fetch the registry to see what's available.
            const reg = await invoke<Registry>("get_registry");

            // Filter persisted active_ids to only those that are currently Ready.
            const readyIds = ids.filter((id) =>
              reg.available.some((e) => e.id === id && e.cache_state.type === "Ready")
            );

            if (readyIds.length > 0) {
              await invoke("set_active_lists", { ids: readyIds });
            }

            // Re-apply genuine user name overrides only (not auto-derived names).
            // Skip names equal to the id: those were never explicitly set by the user
            // and would override the display name embedded in the .tsc header.
            for (const [id, name] of Object.entries(names)) {
              if (name !== id) {
                try { await invoke("rename_list", { id, name }); } catch { /* ok */ }
              }
            }

            if (dedup !== null && dedup !== undefined) {
              await invoke("set_dedup_enabled", { enabled: dedup });
            }

            const finalReg = await invoke<Registry>("get_registry");
            setRegistry(finalReg);
          } catch (e) {
            console.error("Registry init failed:", e);
          }
        };
        applyAndFetch();
      });
    });
  }, []);

  // ── Persist UI settings ────────────────────────────────────────────────
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
  }, [normalize, variantMode, viewMode, minLen, maxLen, referenceMode, showDescription, showOptions, appearance]);

  useEffect(() => {
    if (!settingsLoaded.current || !storeRef.current) return;
    storeRef.current.set("history", history);
  }, [history]);

  // ── Listen to registry:changed (from Rust when commands change state) ─
  useEffect(() => {
    interface RegistryChangedPayload {
      active_ids: string[];
      display_names: Record<string, string>;
      dedup_enabled: boolean;
    }
    let unlisten: (() => void) | null = null;
    listen<RegistryChangedPayload>("registry:changed", (e) => {
      const payload = e.payload;
      if (storeRef.current) {
        storeRef.current.set("word_list_active_ids", payload.active_ids);
        storeRef.current.set("word_list_display_names", payload.display_names);
        storeRef.current.set("dedup_enabled", payload.dedup_enabled);
      }
      invoke<Registry>("get_registry").then(setRegistry).catch(console.error);
    }).then((u) => { unlisten = u; });
    return () => { if (unlisten) unlisten(); };
  }, []);

  // ── Listen to search events ────────────────────────────────────────────
  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<{ active_ids: string[] }>("search:start", (e) => {
      setIsSearching(true);
      setListResults(
        e.payload.active_ids.map((id) => {
          const entry = registry.available.find((a) => a.id === id);
          return {
            listId: id,
            listName: entry?.display_name ?? id,
            entryCount: entry?.word_count ?? 0,
            results: null,
            isLoading: true,
          };
        })
      );
    }).then((u) => unlisteners.push(u));

    listen<{ list_id: string; list_name: string; results: MatchGroup[]; error: string | null }>(
      "search:list-result",
      (e) => {
        const { list_id, list_name, results } = e.payload;
        setListResults((prev) =>
          prev.map((lr) =>
            lr.listId === list_id
              ? { ...lr, listName: list_name, results, isLoading: false }
              : lr
          )
        );
      }
    ).then((u) => unlisteners.push(u));

    // After dedup: re-apply final results if dedup removed words.
    listen<{ list_id: string; list_name: string; results: MatchGroup[]; error: string | null }>(
      "search:list-result-final",
      (e) => {
        const { list_id, list_name, results } = e.payload;
        setListResults((prev) =>
          prev.map((lr) =>
            lr.listId === list_id
              ? { ...lr, listName: list_name, results, isLoading: false }
              : lr
          )
        );
      }
    ).then((u) => unlisteners.push(u));

    listen("search:complete", () => {
      setIsSearching(false);
    }).then((u) => unlisteners.push(u));

    // Build state affects search availability.
    listen<{ list_id: string }>("build:start", () => setBuildInProgress(true)).then((u) => unlisteners.push(u));
    listen("build:complete", () => setBuildInProgress(false)).then((u) => unlisteners.push(u));
    listen("build:error", () => setBuildInProgress(false)).then((u) => unlisteners.push(u));

    return () => unlisteners.forEach((u) => u());
  }, [registry]);

  // ── Wait for background cache handle loading ────────────────────────────
  // registry:ready fires once when Rust's background task finishes opening mmaps.
  // We also poll handles_ready() as a fallback in case the event fires before
  // this listener is registered (race condition for fast/small word lists).
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen("registry:ready", () => {
      setListsLoading(false);
      invoke<Registry>("get_registry").then(setRegistry).catch(console.error);
    }).then((u) => { unlisten = u; });
    // Fallback: check if handles are already ready (fired before listener registered).
    invoke<boolean>("handles_ready")
      .then((ready) => { if (ready) setListsLoading(false); })
      .catch(() => setListsLoading(false));
    return () => { if (unlisten) unlisten(); };
  }, []);

  // ── Menu events ────────────────────────────────────────────────────────
  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<string>("menu:toggle", (e) => {
      if (e.payload === "description") setShowDescription((v) => !v);
      else if (e.payload === "options") setShowOptions((v) => !v);
    }).then((u) => unlisteners.push(u));

    listen<string>("menu:reference", (e) => setReferenceMode(e.payload as ReferenceMode)).then((u) => unlisteners.push(u));

    listen<string>("menu:appearance", (e) => {
      const mode = e.payload as AppearanceMode;
      setAppearance(mode);
      applyTheme(mode);
    }).then((u) => unlisteners.push(u));

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
    }).then((u) => unlisteners.push(u));

    listen("menu:lists", () => setDrawerOpen(true)).then((u) => unlisteners.push(u));

    return () => unlisteners.forEach((u) => u());
  }, []);

  // ── Close history on outside click ────────────────────────────────────
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (historyRef.current && !historyRef.current.contains(e.target as Node)) {
        setShowHistory(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  // ── Close context menu on click ────────────────────────────────────────
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [contextMenu]);

  // ── Pattern description (debounced, 500ms) ─────────────────────────────
  useEffect(() => {
    setExplanation("");
    if (explainTimerRef.current) clearTimeout(explainTimerRef.current);
    if (!pattern.trim()) return;
    explainTimerRef.current = setTimeout(async () => {
      try {
        const desc = await invoke<string | null>("describe_pattern", { pattern: pattern.trim() });
        setExplanation(desc ?? "");
      } catch { setExplanation(""); }
    }, 500);
    return () => { if (explainTimerRef.current) clearTimeout(explainTimerRef.current); };
  }, [pattern]);

  // ── Search ─────────────────────────────────────────────────────────────
  const doSearch = useCallback(async (patternOverride?: string) => {
    const trimmed = (patternOverride ?? pattern).trim();
    if (!trimmed) return;
    if (patternOverride) setPattern(patternOverride);
    setShowHistory(false);
    setSelectedWords(new Set());
    setStatusMsg("Searching…");

    if (listsLoading) {
      setStatusMsg("Word lists are still loading, please wait…");
      return;
    }

    if (buildInProgress) {
      setStatusMsg("Search unavailable while a word list is being indexed.");
      return;
    }

    try {
      await invoke("search", { pattern: trimmed, minLen, maxLen, normalize });
      // Results arrive via events (search:start, search:list-result, search:complete).
      // Update history after issuing the search.
      setHistory((prev) => {
        const filtered = prev.filter((h) => h.pattern !== trimmed);
        // We'll update match count when search:complete fires with the total.
        return [{ pattern: trimmed, matchCount: 0 }, ...filtered].slice(0, MAX_HISTORY);
      });
    } catch (err) {
      setStatusMsg(`Error: ${err}`);
      setIsSearching(false);
    }
  }, [pattern, minLen, maxLen, normalize, listsLoading, buildInProgress]);

  // Update history match count when search completes.
  useEffect(() => {
    if (isSearching) return;
    const total = listResults.reduce((sum, lr) => sum + (lr.results?.length ?? 0), 0);
    if (total === 0 && listResults.length === 0) return;
    setHistory((prev) => {
      if (prev.length === 0) return prev;
      return [{ ...prev[0], matchCount: total }, ...prev.slice(1)];
    });
    setStatusMsg(
      total === 0
        ? "No matches found"
        : `${total} match${total === 1 ? "" : "es"}${listResults.length > 1 ? ` across ${listResults.length} lists` : ""}`
    );
  }, [isSearching, listResults]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") doSearch();
    if (e.key === "Escape") setShowHistory(false);
  };

  const selectHistory = (entry: HistoryEntry) => {
    setShowHistory(false);
    doSearch(entry.pattern);
  };

  const handleReferenceClick = (p: string) => { doSearch(p); };

  // ── Word selection ─────────────────────────────────────────────────────
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
    setSelectedWords((prev) => { if (!prev.has(word)) return new Set([word]); return prev; });
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, []);

  const handleCopy = useCallback(async () => {
    const text = [...selectedWords].join("\n");
    try { await writeText(text); } catch { await navigator.clipboard.writeText(text); }
    setContextMenu(null);
  }, [selectedWords]);

  // ── Registry helpers ───────────────────────────────────────────────────
  const refreshRegistry = useCallback(() => {
    invoke<Registry>("get_registry").then(setRegistry).catch(console.error);
  }, []);

  const totalMatches = listResults.reduce((sum, lr) => sum + (lr.results?.length ?? 0), 0);
  const activeListCount = registry.active_ids.length;
  const hasMultipleLists = activeListCount > 1;

  // ── Render ─────────────────────────────────────────────────────────────
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
                      <span className="text-xs text-gray-400 ml-4 flex-shrink-0">{entry.matchCount > 0 ? `${entry.matchCount} matches` : ""}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>

          <button
            onClick={() => doSearch()}
            disabled={isSearching || buildInProgress || listsLoading}
            className="px-5 py-2 bg-blue-500 text-white rounded-lg text-sm font-medium hover:bg-blue-600 disabled:opacity-50 transition-colors flex-shrink-0"
          >
            {isSearching ? "…" : "Search"}
          </button>

          {/* Word list button */}
          <button
            onClick={() => setDrawerOpen(true)}
            className={`px-3 py-2 rounded-lg text-sm border transition-colors flex-shrink-0 ${
              activeListCount === 0
                ? "border-orange-300 text-orange-600 dark:text-orange-400 bg-orange-50 dark:bg-orange-900/20 hover:bg-orange-100"
                : "border-gray-300 dark:border-gray-600 text-gray-500 dark:text-gray-400 bg-white dark:bg-gray-800 hover:bg-gray-50 dark:hover:bg-gray-700"
            }`}
            title="Manage Word Lists (⌘⇧L)"
          >
            {activeListCount === 0 ? "⚠ Lists" : `📚 ${activeListCount}`}
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

        {/* Loading banner — shown while background mmap task is running */}
        {listsLoading && (
          <div className="text-xs text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg px-3 py-1.5 mb-1.5">
            Loading word lists…
          </div>
        )}

        {/* Build-in-progress banner */}
        {!listsLoading && buildInProgress && (
          <div className="text-xs text-yellow-700 dark:text-yellow-300 bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800 rounded-lg px-3 py-1.5 mb-1.5">
            Building word list index — search unavailable
          </div>
        )}

        {/* No active lists warning */}
        {!listsLoading && !buildInProgress && activeListCount === 0 && (
          <div className="text-xs text-orange-600 dark:text-orange-400 bg-orange-50 dark:bg-orange-900/20 border border-orange-200 dark:border-orange-800 rounded-lg px-3 py-1.5 mb-1.5">
            No word lists active.{" "}
            <button onClick={() => setDrawerOpen(true)} className="underline hover:no-underline">
              Open Word Lists
            </button>{" "}
            to build and activate a list.
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

      {/* ── RESULTS HEADER (shown when results exist) ── */}
      {listResults.length > 0 && (
        <div className="flex items-center justify-between px-5 py-2 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
          <div className="flex items-baseline gap-2">
            {isSearching ? (
              <span className="text-sm text-gray-400 animate-pulse">Searching…</span>
            ) : (
              <span className="text-sm font-semibold text-gray-800 dark:text-gray-100">
                {totalMatches} {totalMatches === 1 ? "match" : "matches"}
                {hasMultipleLists && <span className="text-xs font-normal text-gray-400 ml-1">across {listResults.length} lists</span>}
              </span>
            )}
          </div>
        </div>
      )}

      {/* ── SCROLLABLE RESULTS ── */}
      <div className="flex-1 overflow-hidden">
        {/* Single list: full-width scrollable pane */}
        {!hasMultipleLists && (
          <div className="h-full overflow-y-auto px-5 py-3 bg-white dark:bg-gray-900">
            {listResults.length === 0 && (
              <p className="text-sm text-gray-400 dark:text-gray-500">{statusMsg}</p>
            )}
            {listResults.length === 1 && (
              <ResultsColumn
                listId={listResults[0].listId}
                listName={listResults[0].listName}
                entryCount={listResults[0].entryCount}
                results={listResults[0].results}
                isLoading={listResults[0].isLoading}
                normalize={normalize}
                variantMode={variantMode}
                viewMode={viewMode}
                selectedWords={selectedWords}
                onWordClick={handleWordClick}
                onWordRightClick={handleWordRightClick}
              />
            )}
          </div>
        )}

        {/* Multiple lists: stacked panes with independent scroll, proportional height */}
        {hasMultipleLists && listResults.length > 0 && (
          <div className="h-full flex flex-col gap-2 px-5 py-3 overflow-hidden bg-white dark:bg-gray-900">
            {(() => {
              return listResults.map((lr) => {
                // Once the full search completes, size panes proportional to match count.
                // Using !isSearching (set false on search:complete) rather than checking
                // per-list loading flags, so the resize happens once at the end — not
                // mid-stream when the last individual list result arrives.
                const count = lr.results?.length ?? 0;
                const grow = !isSearching ? Math.max(count, 1) : 1;
                return (
                  <div
                    key={lr.listId}
                    style={{ flex: `${grow} 1 0%`, minHeight: "120px", overflow: "hidden" }}
                  >
                    <ResultsColumn
                      listId={lr.listId}
                      listName={lr.listName}
                      entryCount={lr.entryCount}
                      results={lr.results}
                      isLoading={lr.isLoading}
                      normalize={normalize}
                      variantMode={variantMode}
                      viewMode={viewMode}
                      selectedWords={selectedWords}
                      onWordClick={handleWordClick}
                      onWordRightClick={handleWordRightClick}
                    />
                  </div>
                );
              });
            })()}
          </div>
        )}

        {/* Multiple lists but no search yet */}
        {hasMultipleLists && listResults.length === 0 && (
          <div className="h-full overflow-y-auto px-5 py-3 bg-white dark:bg-gray-900">
            <p className="text-sm text-gray-400 dark:text-gray-500">{statusMsg}</p>
          </div>
        )}
      </div>

      {/* ── STATUS BAR ── */}
      <div className="flex-shrink-0 px-5 py-1.5 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 flex items-center justify-between">
        <span className="text-xs text-gray-400 dark:text-gray-500">
          {selectedWords.size > 0
            ? `${selectedWords.size} word${selectedWords.size === 1 ? "" : "s"} selected`
            : listResults.length > 0 && !isSearching
            ? `${totalMatches} words`
            : ""}
        </span>
        {activeListCount > 0 && (
          <span className="text-xs text-gray-400 dark:text-gray-500">
            {activeListCount} list{activeListCount === 1 ? "" : "s"} active
          </span>
        )}
      </div>

      {/* ── CONTEXT MENU ── */}
      {contextMenu && (
        <ContextMenuPopup x={contextMenu.x} y={contextMenu.y} onCopy={handleCopy} />
      )}

      {/* ── WORD LIST DRAWER ── */}
      <WordListDrawer
        open={drawerOpen}
        onClose={() => setDrawerOpen(false)}
        onRegistryChanged={refreshRegistry}
      />
    </div>
  );
}
