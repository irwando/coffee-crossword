// ── ResultsColumn ─────────────────────────────────────────────────────────────
// Renders one word list's search results. Shows a skeleton loading state while
// results are arriving, then renders GridView or ListView content.

import { useState } from "react";

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

// ── Grid view (embedded) ──────────────────────────────────────────────────────

function GridView({
  lengths, grouped, normalize, variantMode, selectedWords, onWordClick, onWordRightClick,
}: {
  lengths: number[];
  grouped: Record<number, MatchGroup[]>;
  normalize: boolean;
  variantMode: VariantMode;
  selectedWords: Set<string>;
  onWordClick: (word: string, e: React.MouseEvent) => void;
  onWordRightClick: (word: string, originalWord: string, e: React.MouseEvent) => void;
}) {
  return (
    <>
      {lengths.map((len) => (
        <div key={len} className="mb-3">
          <div className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide mb-1 px-3">
            {len} letter{len === 1 ? "" : "s"} ({grouped[len].length})
          </div>
          <div className="flex flex-wrap gap-1.5 px-3">
            {grouped[len].map((r) => (
              <div
                key={r.normalized}
                onClick={(e) => onWordClick(r.normalized, e)}
                onContextMenu={(e) => onWordRightClick(r.normalized, r.variants[0] ?? r.normalized, e)}
                className={`flex items-baseline gap-1 border rounded px-2.5 py-0.5 cursor-pointer select-none transition-colors ${
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

// ── List view (embedded) ──────────────────────────────────────────────────────

function ListView({
  lengths, grouped, normalize, variantMode, selectedWords, onWordClick, onWordRightClick,
}: {
  lengths: number[];
  grouped: Record<number, MatchGroup[]>;
  normalize: boolean;
  variantMode: VariantMode;
  selectedWords: Set<string>;
  onWordClick: (word: string, e: React.MouseEvent) => void;
  onWordRightClick: (word: string, originalWord: string, e: React.MouseEvent) => void;
}) {
  const [collapsed, setCollapsed] = useState<Record<number, boolean>>({});
  const toggle = (len: number) => setCollapsed((prev) => ({ ...prev, [len]: !prev[len] }));

  return (
    <>
      {lengths.map((len) => {
        const isCollapsed = collapsed[len] ?? false;
        return (
          <div key={len} className="mb-1 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden mx-3">
            <button
              onClick={() => toggle(len)}
              className={`w-full flex items-center justify-between px-3 py-1 text-left transition-colors ${
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
                    onContextMenu={(e) => onWordRightClick(r.normalized, r.variants[0] ?? r.normalized, e)}
                    className={`flex items-baseline justify-between px-3 py-0.5 cursor-pointer select-none transition-colors ${
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
  // Group results by word length.
  const grouped = (results ?? []).reduce<Record<number, MatchGroup[]>>((acc, r) => {
    const len = r.normalized.length;
    if (!acc[len]) acc[len] = [];
    acc[len].push(r);
    return acc;
  }, {});
  const lengths = Object.keys(grouped).map(Number).sort((a, b) => a - b);

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

      {/* Content area — scrolls independently */}
      <div className="flex-1 overflow-y-auto">
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
          <div className="py-2">
            <GridView
              lengths={lengths}
              grouped={grouped}
              normalize={normalize}
              variantMode={variantMode}
              selectedWords={selectedWords}
              onWordClick={onWordClick}
              onWordRightClick={(word, originalWord, e) => onWordRightClick(word, originalWord, listId, e)}
            />
          </div>
        )}

        {!isLoading && results !== null && results.length > 0 && viewMode === "list" && (
          <div className="py-2">
            <ListView
              lengths={lengths}
              grouped={grouped}
              normalize={normalize}
              variantMode={variantMode}
              selectedWords={selectedWords}
              onWordClick={onWordClick}
              onWordRightClick={(word, originalWord, e) => onWordRightClick(word, originalWord, listId, e)}
            />
          </div>
        )}
      </div>
    </div>
  );
}
