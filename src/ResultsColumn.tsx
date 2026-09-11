// ── ResultsColumn ─────────────────────────────────────────────────────────────
// Renders one word list's search results. Shows a skeleton loading state while
// results are arriving, then renders GridView or ListView content.
//
// Both views are virtualized with @tanstack/react-virtual: only the rows
// currently scrolled into view are mounted as real DOM nodes. Without this,
// a broad pattern against a large word list (maxResults defaults to 100,000,
// configurable up to 1,000,000) would render one DOM node per matched word
// and freeze the webview.

import { useMemo, useRef, useState, useEffect } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

interface MatchGroup {
  normalized: string;
  variants: string[];
  balance: string | null;
}

type VariantMode = "show" | "hide";
type ViewMode = "grid" | "list";

interface ResultsColumnProps {
  listId: string;
  listName: string;
  entryCount: number;
  results: MatchGroup[] | null; // null = still loading
  isLoading: boolean;
  isStreaming: boolean; // partial results are arriving, search still running
  truncated: boolean;  // results were capped at the maxResults limit
  normalize: boolean;
  variantMode: VariantMode;
  viewMode: ViewMode;
  selectedWords: Set<string>;
  onWordClick: (word: string, e: React.MouseEvent) => void;
  onWordRightClick: (word: string, originalWord: string, listId: string, e: React.MouseEvent) => void;
}

// ── Skeleton ──────────────────────────────────────────────────────────────────

function SkeletonRow() {
  return (
    <div className="flex items-center gap-2 px-3 py-1 border-b border-gray-50 dark:border-gray-800">
      <div className="h-3 bg-gray-200 dark:bg-gray-700 rounded animate-pulse" style={{ width: `${60 + Math.random() * 80}px` }} />
    </div>
  );
}

function SkeletonContent() {
  // Show a length-group header + several rows, repeated twice.
  return (
    <div className="py-2">
      {[3, 6].map((len) => (
        <div key={len} className="mb-3">
          <div className="px-3 py-1 mb-0.5">
            <div className="h-2.5 bg-gray-200 dark:bg-gray-700 rounded animate-pulse w-20" />
          </div>
          {Array.from({ length: len }).map((_, i) => (
            <SkeletonRow key={i} />
          ))}
        </div>
      ))}
    </div>
  );
}

// ── Shared: track a scroll container's content width (for grid columns) ───────

function useContainerWidth(ref: React.RefObject<HTMLDivElement | null>): number {
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.getBoundingClientRect().width);
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) setWidth(entry.contentRect.width);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);

  return width;
}

// ── Grid view (embedded, virtualized) ──────────────────────────────────────────

const GRID_CHIP_WIDTH = 150; // estimated column width, including gap
const GRID_ROW_HEIGHT = 28;
const GRID_HEADER_HEIGHT = 28;

type GridFlatRow =
  | { type: "header"; len: number }
  | { type: "row"; len: number; words: MatchGroup[] };

function GridView({
  lengths, grouped, normalize, variantMode, selectedWords, onWordClick, onWordRightClick, scrollContainerRef,
}: {
  lengths: number[];
  grouped: Record<number, MatchGroup[]>;
  normalize: boolean;
  variantMode: VariantMode;
  selectedWords: Set<string>;
  onWordClick: (word: string, e: React.MouseEvent) => void;
  onWordRightClick: (word: string, originalWord: string, e: React.MouseEvent) => void;
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
}) {
  const [collapsed, setCollapsed] = useState<Record<number, boolean>>({});
  const toggle = (len: number) => setCollapsed((prev) => ({ ...prev, [len]: !prev[len] }));

  const containerWidth = useContainerWidth(scrollContainerRef);
  const columns = Math.max(1, Math.floor((containerWidth - 24) / GRID_CHIP_WIDTH));

  const flatRows = useMemo(() => {
    const rows: GridFlatRow[] = [];
    for (const len of lengths) {
      rows.push({ type: "header", len });
      if (!(collapsed[len] ?? false)) {
        const words = grouped[len];
        for (let i = 0; i < words.length; i += columns) {
          rows.push({ type: "row", len, words: words.slice(i, i + columns) });
        }
      }
    }
    return rows;
  }, [lengths, grouped, columns, collapsed]);

  const virtualizer = useVirtualizer({
    count: flatRows.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: (index) => (flatRows[index]?.type === "header" ? GRID_HEADER_HEIGHT : GRID_ROW_HEIGHT),
    overscan: 8,
  });

  return (
    <div
      style={{ position: "relative", height: virtualizer.getTotalSize() }}
      className="mx-3 my-2 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden"
    >
      {virtualizer.getVirtualItems().map((vRow) => {
        const row = flatRows[vRow.index];
        if (!row) return null;
        const style: React.CSSProperties = {
          position: "absolute", top: 0, left: 0, right: 0,
          transform: `translateY(${vRow.start}px)`, height: vRow.size,
        };

        if (row.type === "header") {
          const isCollapsed = collapsed[row.len] ?? false;
          return (
            <button
              key={vRow.key}
              style={style}
              onClick={() => toggle(row.len)}
              className={`w-full flex items-center justify-between px-3 text-left transition-colors ${
                isCollapsed ? "bg-gray-50 dark:bg-gray-800" : "bg-gray-100 dark:bg-gray-700"
              }`}
            >
              <div className="flex items-center gap-2">
                <span
                  className={`text-xs transition-transform ${!isCollapsed ? "text-gray-500 dark:text-gray-300" : "text-gray-400 dark:text-gray-500"}`}
                  style={{ display: "inline-block", transform: isCollapsed ? "rotate(-90deg)" : "rotate(0deg)" }}
                >▾</span>
                <span className={`text-xs font-semibold ${isCollapsed ? "text-gray-500 dark:text-gray-400" : "text-gray-700 dark:text-gray-200"}`}>
                  {row.len} letter{row.len === 1 ? "" : "s"}
                </span>
              </div>
              <span className="text-xs text-gray-400 dark:text-gray-400 bg-gray-50 dark:bg-gray-600 px-2 py-0.5 rounded-full border border-gray-200 dark:border-gray-500">
                {grouped[row.len].length} match{grouped[row.len].length === 1 ? "" : "es"}
              </span>
            </button>
          );
        }

        return (
          <div
            key={vRow.key}
            style={{ ...style, display: "grid", gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`, alignItems: "center", gap: "4px", padding: "2px 4px" }}
          >
            {row.words.map((r) => (
              <div
                key={r.normalized}
                onClick={(e) => onWordClick(r.normalized, e)}
                onContextMenu={(e) => onWordRightClick(r.normalized, r.variants[0] ?? r.normalized, e)}
                title={r.variants.length > 0 ? `${r.normalized} (${r.variants.join(", ")})` : r.normalized}
                className={`flex items-baseline gap-1 border rounded px-2 py-px cursor-pointer select-none transition-colors overflow-hidden ${
                  selectedWords.has(r.normalized)
                    ? "bg-blue-50 dark:bg-blue-900 border-blue-300 dark:border-blue-700"
                    : "bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-500"
                }`}
              >
                <span className="font-mono text-sm text-gray-800 dark:text-gray-200 truncate">{r.normalized}</span>
                {normalize && variantMode === "show" && r.variants.length > 0 && (
                  <span className="text-xs text-gray-400 truncate">({r.variants.join(", ")})</span>
                )}
                {r.balance && <span className="font-mono text-xs text-blue-500 flex-shrink-0">{r.balance}</span>}
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}

// ── List view (embedded, virtualized) ──────────────────────────────────────────

const LIST_ROW_HEIGHT = 26;
const LIST_HEADER_HEIGHT = 28;

type ListFlatRow =
  | { type: "header"; len: number }
  | { type: "word"; len: number; group: MatchGroup };

function ListView({
  lengths, grouped, normalize, variantMode, selectedWords, onWordClick, onWordRightClick, scrollContainerRef,
}: {
  lengths: number[];
  grouped: Record<number, MatchGroup[]>;
  normalize: boolean;
  variantMode: VariantMode;
  selectedWords: Set<string>;
  onWordClick: (word: string, e: React.MouseEvent) => void;
  onWordRightClick: (word: string, originalWord: string, e: React.MouseEvent) => void;
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
}) {
  const [collapsed, setCollapsed] = useState<Record<number, boolean>>({});
  const toggle = (len: number) => setCollapsed((prev) => ({ ...prev, [len]: !prev[len] }));

  const flatRows = useMemo(() => {
    const rows: ListFlatRow[] = [];
    for (const len of lengths) {
      rows.push({ type: "header", len });
      if (!(collapsed[len] ?? false)) {
        for (const group of grouped[len]) {
          rows.push({ type: "word", len, group });
        }
      }
    }
    return rows;
  }, [lengths, grouped, collapsed]);

  const virtualizer = useVirtualizer({
    count: flatRows.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: (index) => (flatRows[index]?.type === "header" ? LIST_HEADER_HEIGHT : LIST_ROW_HEIGHT),
    overscan: 12,
  });

  return (
    <div
      style={{ position: "relative", height: virtualizer.getTotalSize() }}
      className="mx-3 my-2 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden"
    >
      {virtualizer.getVirtualItems().map((vRow) => {
        const row = flatRows[vRow.index];
        if (!row) return null;
        const style: React.CSSProperties = {
          position: "absolute", top: 0, left: 0, right: 0,
          transform: `translateY(${vRow.start}px)`, height: vRow.size,
        };

        if (row.type === "header") {
          const isCollapsed = collapsed[row.len] ?? false;
          return (
            <button
              key={vRow.key}
              style={style}
              onClick={() => toggle(row.len)}
              className={`w-full flex items-center justify-between px-3 text-left transition-colors ${
                isCollapsed ? "bg-gray-50 dark:bg-gray-800" : "bg-gray-100 dark:bg-gray-700"
              }`}
            >
              <div className="flex items-center gap-2">
                <span
                  className={`text-xs transition-transform ${!isCollapsed ? "text-gray-500 dark:text-gray-300" : "text-gray-400 dark:text-gray-500"}`}
                  style={{ display: "inline-block", transform: isCollapsed ? "rotate(-90deg)" : "rotate(0deg)" }}
                >▾</span>
                <span className={`text-xs font-semibold ${isCollapsed ? "text-gray-500 dark:text-gray-400" : "text-gray-700 dark:text-gray-200"}`}>
                  {row.len} letter{row.len === 1 ? "" : "s"}
                </span>
              </div>
              <span className="text-xs text-gray-400 dark:text-gray-400 bg-gray-50 dark:bg-gray-600 px-2 py-0.5 rounded-full border border-gray-200 dark:border-gray-500">
                {grouped[row.len].length} match{grouped[row.len].length === 1 ? "" : "es"}
              </span>
            </button>
          );
        }

        const r = row.group;
        return (
          <div
            key={vRow.key}
            style={style}
            onClick={(e) => onWordClick(r.normalized, e)}
            onContextMenu={(e) => onWordRightClick(r.normalized, r.variants[0] ?? r.normalized, e)}
            className={`flex items-baseline justify-between px-3 cursor-pointer select-none transition-colors border-b border-gray-50 dark:border-gray-800 ${
              selectedWords.has(r.normalized)
                ? "bg-blue-50 dark:bg-blue-900"
                : "bg-white dark:bg-gray-900 hover:bg-gray-50 dark:hover:bg-gray-800"
            }`}
          >
            <span className="font-mono text-sm text-gray-800 dark:text-gray-200 truncate">{r.normalized}</span>
            <div className="flex items-baseline gap-2 flex-shrink-0">
              {normalize && variantMode === "show" && r.variants.length > 0 && (
                <span className="text-xs text-gray-400 truncate">({r.variants.join(", ")})</span>
              )}
              {r.balance && <span className="font-mono text-xs text-blue-500">{r.balance}</span>}
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ── ResultsColumn ─────────────────────────────────────────────────────────────

export default function ResultsColumn({
  listId,
  listName,
  entryCount,
  results,
  isLoading,
  isStreaming,
  truncated,
  normalize,
  variantMode,
  viewMode,
  selectedWords,
  onWordClick,
  onWordRightClick,
}: ResultsColumnProps) {
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Group results by word length. Memoized on `results` so the (potentially
  // large) grouping pass — and the flattened row lists derived from it in
  // GridView/ListView — only rerun when the result set actually changes,
  // not on every unrelated parent render.
  const grouped = useMemo(
    () =>
      (results ?? []).reduce<Record<number, MatchGroup[]>>((acc, r) => {
        const len = r.normalized.length;
        if (!acc[len]) acc[len] = [];
        acc[len].push(r);
        return acc;
      }, {}),
    [results]
  );
  const lengths = useMemo(() => Object.keys(grouped).map(Number).sort((a, b) => a - b), [grouped]);

  const matchCount = results?.length ?? 0;

  return (
    <div className="flex flex-col h-full border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden bg-white dark:bg-gray-900">
      {/* Column header */}
      <div className="flex items-center justify-between px-3 py-1 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex-shrink-0">
        <div className="flex items-baseline gap-2 min-w-0">
          <span className="text-sm font-semibold text-gray-800 dark:text-gray-100 truncate">{listName}</span>
          {entryCount > 0 && (
            <span className="text-xs text-gray-400 flex-shrink-0">{entryCount.toLocaleString()} words</span>
          )}
        </div>
        <div className="flex-shrink-0 ml-2">
          {isLoading ? (
            <span className="text-xs text-gray-400 animate-pulse">Searching…</span>
          ) : results !== null ? (
            <span className="text-xs font-medium text-gray-600 dark:text-gray-400">
              {matchCount} {matchCount === 1 ? "match" : "matches"}
              {isStreaming && <span className="animate-pulse ml-1">…</span>}
            </span>
          ) : null}
        </div>
      </div>

      {/* Content area — scrolls independently; also the virtualizer's scroll element */}
      <div ref={scrollContainerRef} className="flex-1 overflow-y-auto">
        {isLoading && <SkeletonContent />}

        {!isLoading && results !== null && results.length === 0 && (
          <p className="text-sm text-gray-400 dark:text-gray-500 px-3 py-3">No matches found</p>
        )}

        {!isLoading && truncated && (
          <div className="mx-3 mt-2 mb-1 px-3 py-1.5 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded text-xs text-amber-700 dark:text-amber-400">
            Showing first {matchCount.toLocaleString()} results — refine your pattern or increase the limit in Options.
          </div>
        )}

        {!isLoading && results !== null && results.length > 0 && viewMode === "grid" && (
          <GridView
            lengths={lengths}
            grouped={grouped}
            normalize={normalize}
            variantMode={variantMode}
            selectedWords={selectedWords}
            onWordClick={onWordClick}
            onWordRightClick={(word, originalWord, e) => onWordRightClick(word, originalWord, listId, e)}
            scrollContainerRef={scrollContainerRef}
          />
        )}

        {!isLoading && results !== null && results.length > 0 && viewMode === "list" && (
          <ListView
            lengths={lengths}
            grouped={grouped}
            normalize={normalize}
            variantMode={variantMode}
            selectedWords={selectedWords}
            onWordClick={onWordClick}
            onWordRightClick={(word, originalWord, e) => onWordRightClick(word, originalWord, listId, e)}
            scrollContainerRef={scrollContainerRef}
          />
        )}
      </div>
    </div>
  );
}
