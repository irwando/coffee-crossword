// ── WordListDrawer ─────────────────────────────────────────────────────────────
// Right-side sliding drawer for managing word lists.
// Shows all discovered .txt files, lets the user activate/deactivate and
// reorder them, build/rebuild indices, and toggle deduplication.

import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ── Types ─────────────────────────────────────────────────────────────────────

interface CacheStateReady { type: "Ready" }
interface CacheStateNeedsRebuild { type: "NeedsRebuild" }
interface CacheStateNotBuilt { type: "NotBuilt" }
interface CacheStateBuilding { type: "Building" }
interface CacheStateError { type: "Error"; message: string }
type CacheState = CacheStateReady | CacheStateNeedsRebuild | CacheStateNotBuilt | CacheStateBuilding | CacheStateError;

interface ListEntry {
  id: string;
  display_name: string;
  txt_path: string;
  tsc_path: string;
  word_count: number;
  source_updated: string;
  source_desc: string;
  cache_state: CacheState;
}

interface Registry {
  available: ListEntry[];
  active_ids: string[];
  dedup_enabled: boolean;
}

interface BuildProgressPayload {
  list_id: string;
  percent: number;
  phase: string;
}

interface BuildCompletePayload {
  list_id: string;
  entry_count: number;
  elapsed_ms: number;
}

interface BuildErrorPayload {
  list_id: string;
  message: string;
}

interface WordListDrawerProps {
  open: boolean;
  onClose: () => void;
  onRegistryChanged: () => void;
}

// ── Status dot ────────────────────────────────────────────────────────────────

function StatusDot({ state }: { state: CacheState }) {
  switch (state.type) {
    case "Ready":
      return <span className="w-2 h-2 rounded-full bg-green-500 flex-shrink-0" title="Index ready" />;
    case "NeedsRebuild":
      return <span className="w-2 h-2 rounded-full bg-yellow-400 flex-shrink-0" title="Source updated — rebuild needed" />;
    case "NotBuilt":
      return <span className="w-2 h-2 rounded-full bg-gray-400 flex-shrink-0" title="Index not built" />;
    case "Building":
      return (
        <span className="w-2 h-2 rounded-full bg-blue-400 flex-shrink-0 animate-pulse" title="Building…" />
      );
    case "Error":
      return <span className="w-2 h-2 rounded-full bg-red-500 flex-shrink-0" title={state.message} />;
  }
}

function stateLabel(state: CacheState): string {
  switch (state.type) {
    case "Ready": return "Ready";
    case "NeedsRebuild": return "Source updated";
    case "NotBuilt": return "Index not built";
    case "Building": return "Building…";
    case "Error": return `Error: ${state.message}`;
  }
}

// ── Main component ────────────────────────────────────────────────────────────

export default function WordListDrawer({ open, onClose, onRegistryChanged }: WordListDrawerProps) {
  const [registry, setRegistry] = useState<Registry | null>(null);
  const [buildProgress, setBuildProgress] = useState<Record<string, { percent: number; phase: string }>>({});
  const backdropRef = useRef<HTMLDivElement>(null);

  // Load registry on open.
  useEffect(() => {
    if (!open) return;
    invoke<Registry>("get_registry").then(setRegistry).catch(console.error);
  }, [open]);

  // Listen for build events.
  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<BuildProgressPayload>("build:progress", (e) => {
      setBuildProgress((prev) => ({
        ...prev,
        [e.payload.list_id]: { percent: e.payload.percent, phase: e.payload.phase },
      }));
    }).then((u) => unlisteners.push(u));

    listen<BuildCompletePayload>("build:complete", (e) => {
      setBuildProgress((prev) => { const n = { ...prev }; delete n[e.payload.list_id]; return n; });
      // Refresh registry to reflect new word count and Ready state.
      invoke<Registry>("get_registry").then((r) => { setRegistry(r); onRegistryChanged(); }).catch(console.error);
    }).then((u) => unlisteners.push(u));

    listen<BuildErrorPayload>("build:error", (e) => {
      setBuildProgress((prev) => { const n = { ...prev }; delete n[e.payload.list_id]; return n; });
      invoke<Registry>("get_registry").then(setRegistry).catch(console.error);
    }).then((u) => unlisteners.push(u));

    listen("registry:changed", () => {
      invoke<Registry>("get_registry").then((r) => { setRegistry(r); onRegistryChanged(); }).catch(console.error);
    }).then((u) => unlisteners.push(u));

    return () => unlisteners.forEach((u) => u());
  }, [onRegistryChanged]);

  // Close on Escape.
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!registry) {
    return (
      <>
        {open && <div className="fixed inset-0 z-40 bg-black/20" onClick={onClose} />}
        <div className={`fixed top-0 right-0 h-full w-80 z-50 bg-white dark:bg-gray-900 shadow-2xl border-l border-gray-200 dark:border-gray-700 transition-transform duration-200 ${open ? "translate-x-0" : "translate-x-full"}`}>
          <div className="p-4 text-sm text-gray-400">Loading…</div>
        </div>
      </>
    );
  }

  const { available, active_ids, dedup_enabled } = registry;
  const isAnyBuilding = available.some((e) => e.cache_state.type === "Building");

  // Active entries in order.
  const activeEntries = active_ids
    .map((id) => available.find((e) => e.id === id))
    .filter((e): e is ListEntry => e !== undefined);

  // Available-but-not-active entries.
  const inactiveEntries = available.filter((e) => !active_ids.includes(e.id));

  // ── Actions ────────────────────────────────────────────────────────────

  const buildList = (id: string) => {
    // Optimistically mark as Building in local state.
    setRegistry((prev) => {
      if (!prev) return prev;
      return {
        ...prev,
        available: prev.available.map((e) =>
          e.id === id ? { ...e, cache_state: { type: "Building" } } : e
        ),
      };
    });
    invoke("build_list_cache", { listId: id }).catch((err) => {
      console.error("Build failed:", err);
    });
  };

  const activateList = (id: string) => {
    const newIds = [...active_ids, id];
    invoke("set_active_lists", { ids: newIds })
      .then(() => invoke<Registry>("get_registry").then((r) => { setRegistry(r); onRegistryChanged(); }))
      .catch(console.error);
  };

  const removeFromActive = (id: string) => {
    const newIds = active_ids.filter((x) => x !== id);
    invoke("set_active_lists", { ids: newIds })
      .then(() => invoke<Registry>("get_registry").then((r) => { setRegistry(r); onRegistryChanged(); }))
      .catch(console.error);
  };

  const moveUp = (id: string) => {
    const idx = active_ids.indexOf(id);
    if (idx <= 0) return;
    const newIds = [...active_ids];
    [newIds[idx - 1], newIds[idx]] = [newIds[idx], newIds[idx - 1]];
    invoke("set_active_lists", { ids: newIds })
      .then(() => invoke<Registry>("get_registry").then((r) => { setRegistry(r); onRegistryChanged(); }))
      .catch(console.error);
  };

  const moveDown = (id: string) => {
    const idx = active_ids.indexOf(id);
    if (idx < 0 || idx >= active_ids.length - 1) return;
    const newIds = [...active_ids];
    [newIds[idx], newIds[idx + 1]] = [newIds[idx + 1], newIds[idx]];
    invoke("set_active_lists", { ids: newIds })
      .then(() => invoke<Registry>("get_registry").then((r) => { setRegistry(r); onRegistryChanged(); }))
      .catch(console.error);
  };

  const toggleDedup = () => {
    invoke("set_dedup_enabled", { enabled: !dedup_enabled })
      .then(() => invoke<Registry>("get_registry").then((r) => { setRegistry(r); onRegistryChanged(); }))
      .catch(console.error);
  };

  // ── Render ─────────────────────────────────────────────────────────────

  const ListRow = ({ entry, isActive, rank }: { entry: ListEntry; isActive: boolean; rank?: number }) => {
    const progress = buildProgress[entry.id];
    const isBuilding = entry.cache_state.type === "Building";
    const canBuild = entry.cache_state.type === "NotBuilt" || entry.cache_state.type === "NeedsRebuild" || entry.cache_state.type === "Error";
    const canActivate = !isActive && entry.cache_state.type === "Ready";

    return (
      <div className="px-3 py-2.5 border-b border-gray-100 dark:border-gray-800 last:border-0">
        <div className="flex items-start gap-2">
          <div className="mt-1 flex-shrink-0">
            <StatusDot state={entry.cache_state} />
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-1.5">
              {rank !== undefined && (
                <span className="text-xs text-gray-400 dark:text-gray-500 flex-shrink-0">{rank}.</span>
              )}
              <span className="text-sm font-medium text-gray-800 dark:text-gray-200 truncate">
                {entry.display_name}
              </span>
              {entry.cache_state.type === "Ready" && entry.word_count > 0 && (
                <span className="text-xs text-gray-400 flex-shrink-0">
                  {entry.word_count.toLocaleString()}
                </span>
              )}
            </div>
            <div className="text-xs text-gray-400 dark:text-gray-500 mt-0.5">
              {stateLabel(entry.cache_state)}
              {entry.source_updated && ` · Updated ${entry.source_updated}`}
            </div>
            {entry.source_desc && (
              <div className="text-xs text-gray-400 dark:text-gray-500 mt-0.5 truncate" title={entry.source_desc}>
                {entry.source_desc}
              </div>
            )}

            {/* Build progress bar */}
            {isBuilding && progress && (
              <div className="mt-2">
                <div className="flex items-center justify-between text-xs text-gray-400 mb-1">
                  <span className="capitalize">{progress.phase}…</span>
                  <span>{progress.percent}%</span>
                </div>
                <div className="h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-blue-500 transition-all duration-200 rounded-full"
                    style={{ width: `${progress.percent}%` }}
                  />
                </div>
              </div>
            )}

            {/* Action buttons */}
            <div className="flex items-center gap-2 mt-1.5 flex-wrap">
              {canBuild && !isAnyBuilding && (
                <button
                  onClick={() => buildList(entry.id)}
                  className="text-xs px-2 py-0.5 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
                >
                  {entry.cache_state.type === "NeedsRebuild" ? "Rebuild Index" : "Build Index"}
                </button>
              )}
              {canBuild && isAnyBuilding && (
                <span className="text-xs text-gray-400 italic">Build in progress…</span>
              )}
              {canActivate && (
                <button
                  onClick={() => activateList(entry.id)}
                  className="text-xs px-2 py-0.5 bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors border border-gray-200 dark:border-gray-600"
                >
                  Add to Active
                </button>
              )}
              {isActive && (
                <>
                  <button
                    onClick={() => moveUp(entry.id)}
                    disabled={active_ids[0] === entry.id}
                    className="text-xs px-1.5 py-0.5 bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600 disabled:opacity-40 disabled:cursor-not-allowed border border-gray-200 dark:border-gray-600"
                    title="Higher priority"
                  >↑</button>
                  <button
                    onClick={() => moveDown(entry.id)}
                    disabled={active_ids[active_ids.length - 1] === entry.id}
                    className="text-xs px-1.5 py-0.5 bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 rounded hover:bg-gray-200 dark:hover:bg-gray-600 disabled:opacity-40 disabled:cursor-not-allowed border border-gray-200 dark:border-gray-600"
                    title="Lower priority"
                  >↓</button>
                  <button
                    onClick={() => removeFromActive(entry.id)}
                    className="text-xs px-2 py-0.5 text-red-500 hover:text-red-600 dark:hover:text-red-400 transition-colors"
                  >Remove</button>
                </>
              )}
            </div>
          </div>
        </div>
      </div>
    );
  };

  return (
    <>
      {/* Backdrop */}
      {open && (
        <div
          ref={backdropRef}
          className="fixed inset-0 z-40 bg-black/20"
          onClick={onClose}
        />
      )}

      {/* Drawer */}
      <div
        className={`fixed top-0 right-0 h-full w-80 z-50 flex flex-col bg-white dark:bg-gray-900 shadow-2xl border-l border-gray-200 dark:border-gray-700 transition-transform duration-200 ease-in-out ${
          open ? "translate-x-0" : "translate-x-full"
        }`}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
          <h2 className="text-sm font-semibold text-gray-800 dark:text-gray-100">Word Lists</h2>
          <div className="flex items-center gap-2">
            <button
              onClick={() => {
                invoke<Registry>("rescan_registry")
                  .then(() => invoke<Registry>("get_registry").then((r) => { setRegistry(r); onRegistryChanged(); }))
                  .catch(console.error);
              }}
              className="text-xs px-2 py-0.5 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 border border-gray-200 dark:border-gray-600 rounded transition-colors"
              title="Re-scan dictionaries folder for new word lists"
            >↺ Rescan</button>
            <button
              onClick={onClose}
              className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-lg leading-none"
            >✕</button>
          </div>
        </div>

        {/* Search-disabled banner */}
        {isAnyBuilding && (
          <div className="px-4 py-2 bg-yellow-50 dark:bg-yellow-900/20 border-b border-yellow-200 dark:border-yellow-800 flex-shrink-0">
            <p className="text-xs text-yellow-700 dark:text-yellow-300">
              Building index — search unavailable
            </p>
          </div>
        )}

        {/* Scrollable list body */}
        <div className="flex-1 overflow-y-auto">
          {/* Active lists */}
          {activeEntries.length > 0 && (
            <div>
              <div className="px-3 py-2 text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wide bg-gray-50 dark:bg-gray-800 border-b border-gray-100 dark:border-gray-700">
                Active — search order
              </div>
              {activeEntries.map((entry, i) => (
                <ListRow key={entry.id} entry={entry} isActive rank={i + 1} />
              ))}
            </div>
          )}

          {activeEntries.length === 0 && (
            <div className="px-3 py-3 text-xs text-gray-400 dark:text-gray-500 italic">
              No active lists. Add a Ready list below to start searching.
            </div>
          )}

          {/* Available (inactive) lists */}
          {inactiveEntries.length > 0 && (
            <div>
              <div className="px-3 py-2 text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wide bg-gray-50 dark:bg-gray-800 border-t border-b border-gray-100 dark:border-gray-700">
                Available
              </div>
              {inactiveEntries.map((entry) => (
                <ListRow key={entry.id} entry={entry} isActive={false} />
              ))}
            </div>
          )}

          {available.length === 0 && (
            <div className="px-4 py-6 text-sm text-gray-400 dark:text-gray-500 text-center">
              <p>No word lists found.</p>
              <p className="mt-1 text-xs">Add .txt files to your dictionaries/ folder and restart.</p>
            </div>
          )}
        </div>

        {/* Footer — dedup toggle */}
        <div className="flex-shrink-0 border-t border-gray-200 dark:border-gray-700 px-4 py-3">
          <label className="flex items-center gap-3 cursor-pointer select-none">
            <div
              onClick={toggleDedup}
              className={`w-8 h-4 rounded-full transition-colors relative cursor-pointer ${
                dedup_enabled ? "bg-blue-500" : "bg-gray-300 dark:bg-gray-600"
              }`}
            >
              <div className={`absolute top-0.5 w-3 h-3 bg-white rounded-full shadow transition-transform ${
                dedup_enabled ? "translate-x-4" : "translate-x-0.5"
              }`} />
            </div>
            <div>
              <div className="text-xs font-medium text-gray-700 dark:text-gray-300">Suppress duplicates</div>
              <div className="text-xs text-gray-400 dark:text-gray-500">
                {dedup_enabled ? "Words shown in highest-priority list only" : "All results shown per list"}
              </div>
            </div>
          </label>
        </div>
      </div>
    </>
  );
}
