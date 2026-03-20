import { useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface MatchGroup {
  normalized: string;
  variants: string[];
  balance: string | null;
}

type VariantMode = "all" | "hover" | "none";

export default function App() {
  const [pattern, setPattern] = useState("");
  const [results, setResults] = useState<MatchGroup[]>([]);
  const [status, setStatus] = useState("Enter a pattern and press Search");
  const [loading, setLoading] = useState(false);
  const [minLen, setMinLen] = useState(1);
  const [maxLen, setMaxLen] = useState(50);
  const [normalize, setNormalize] = useState(true);
  const [variantMode, setVariantMode] = useState<VariantMode>("all");

  const doSearch = useCallback(async () => {
    const trimmed = pattern.trim();
    if (!trimmed) return;

    setLoading(true);
    setStatus("Searching...");

    try {
      const matches = await invoke<MatchGroup[]>("search", {
        pattern: trimmed,
        minLen,
        maxLen,
        normalize,
      });
      setResults(matches);
      setStatus(
        matches.length === 0
          ? "No matches found"
          : `${matches.length} match${matches.length === 1 ? "" : "es"}`
      );
    } catch (err) {
      setStatus(`Error: ${err}`);
      setResults([]);
    } finally {
      setLoading(false);
    }
  }, [pattern, minLen, maxLen, normalize]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") doSearch();
  };

  // Group results by word length
  const grouped = results.reduce<Record<number, MatchGroup[]>>((acc, r) => {
    const len = r.normalized.length;
    if (!acc[len]) acc[len] = [];
    acc[len].push(r);
    return acc;
  }, {});
  const lengths = Object.keys(grouped).map(Number).sort((a, b) => a - b);

  return (
    <div className="min-h-screen bg-gray-50 p-6">
      <div className="max-w-2xl mx-auto">

        {/* Header */}
        <h1 className="text-2xl font-semibold text-gray-800 mb-1">
          Coffee Crossword
        </h1>
        <p className="text-sm text-gray-500 mb-6">Word search helper — 101k words</p>

        {/* Search bar */}
        <div className="flex gap-2 mb-3">
          <input
            type="text"
            value={pattern}
            onChange={(e) => setPattern(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="e.g.  .l...r.n  or  ;acenrt  or  m*ja"
            className="flex-1 px-4 py-2 border border-gray-300 rounded-lg font-mono text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
            autoCorrect="off"
            autoCapitalize="off"
            autoComplete="off"
            spellCheck={false}
            autoFocus
          />
          <button
            onClick={doSearch}
            disabled={loading}
            className="px-5 py-2 bg-blue-500 text-white rounded-lg text-sm font-medium hover:bg-blue-600 disabled:opacity-50 transition-colors"
          >
            {loading ? "..." : "Search"}
          </button>
        </div>

        {/* Options row */}
        <div className="flex flex-wrap items-center gap-4 mb-4 text-sm text-gray-600">
          {/* Normalize toggle */}
          <label className="flex items-center gap-2 cursor-pointer select-none">
            <div
              onClick={() => setNormalize(!normalize)}
              className={`w-9 h-5 rounded-full transition-colors relative cursor-pointer ${
                normalize ? "bg-blue-500" : "bg-gray-300"
              }`}
            >
              <div
                className={`absolute top-0.5 w-4 h-4 bg-white rounded-full shadow transition-transform ${
                  normalize ? "translate-x-4" : "translate-x-0.5"
                }`}
              />
            </div>
            <span>Normalize</span>
          </label>

          {/* Variant display — only shown when normalize is on */}
          {normalize && (
            <div className="flex items-center gap-2">
              <span className="text-gray-400 text-xs">Variants:</span>
              {(["all", "hover", "none"] as VariantMode[]).map((mode) => (
                <button
                  key={mode}
                  onClick={() => setVariantMode(mode)}
                  className={`px-2 py-0.5 rounded text-xs border transition-colors ${
                    variantMode === mode
                      ? "bg-blue-500 text-white border-blue-500"
                      : "bg-white text-gray-500 border-gray-300 hover:border-gray-400"
                  }`}
                >
                  {mode === "all" ? "Show all" : mode === "hover" ? "On hover" : "Hidden"}
                </button>
              ))}
            </div>
          )}

          {/* Length filters */}
          <div className="flex items-center gap-3 ml-auto">
            <label className="flex items-center gap-1">
              Min
              <input
                type="number"
                value={minLen}
                min={1}
                max={50}
                onChange={(e) => setMinLen(Number(e.target.value))}
                className="w-14 px-2 py-1 border border-gray-300 rounded text-center"
              />
            </label>
            <label className="flex items-center gap-1">
              Max
              <input
                type="number"
                value={maxLen}
                min={1}
                max={50}
                onChange={(e) => setMaxLen(Number(e.target.value))}
                className="w-14 px-2 py-1 border border-gray-300 rounded text-center"
              />
            </label>
          </div>
        </div>

        {/* Pattern cheat sheet */}
        <div className="bg-white border border-gray-200 rounded-lg p-4 mb-6 text-xs text-gray-500 font-mono">
          <div className="grid grid-cols-2 gap-x-6 gap-y-1">
            <span><span className="text-gray-700">.l...r.n</span> → electron (template)</span>
            <span><span className="text-gray-700">;acenrt</span> → canter, trance… (anagram)</span>
            <span><span className="text-gray-700">m*ja</span> → maharaja (wildcard)</span>
            <span><span className="text-gray-700">q???k</span> → quick, quack… (? = any letter)</span>
            <span><span className="text-gray-700">;acenrt.</span> → anagram + 1 blank</span>
            <span><span className="text-gray-700">e....;cats</span> → template + anagram</span>
          </div>
        </div>

        {/* Status */}
        <p className="text-sm text-gray-500 mb-4">{status}</p>

        {/* Results grouped by length */}
        {lengths.map((len) => (
          <div key={len} className="mb-6">
            <div className="text-xs font-medium text-gray-400 uppercase tracking-wide mb-2">
              {len} letter{len === 1 ? "" : "s"} ({grouped[len].length})
            </div>
            <div className="flex flex-wrap gap-2">
              {grouped[len].map((r) => (
                <WordChip
                  key={r.normalized}
                  group={r}
                  variantMode={normalize ? variantMode : "none"}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ─── WordChip ────────────────────────────────────────────────────────────────

interface WordChipProps {
  group: MatchGroup;
  variantMode: VariantMode;
}

function WordChip({ group, variantMode }: WordChipProps) {
  const [hovered, setHovered] = useState(false);
  const hasVariants = group.variants.length > 0;

  return (
    <div
      className="relative"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div className={`flex items-baseline gap-1 bg-white border rounded px-3 py-1.5 transition-colors ${
        hasVariants && variantMode !== "none"
          ? "border-blue-200 cursor-default"
          : "border-gray-200"
      }`}>
        {/* Canonical normalized word */}
        <span className="font-mono text-sm text-gray-800">{group.normalized}</span>

        {/* Variants shown inline */}
        {hasVariants && variantMode === "all" && (
          <span className="text-xs text-gray-400">
            ({group.variants.join(", ")})
          </span>
        )}

        {/* Balance */}
        {group.balance && (
          <span className="font-mono text-xs text-blue-500">{group.balance}</span>
        )}
      </div>

      {/* Hover tooltip for variants */}
      {hasVariants && variantMode === "hover" && hovered && (
        <div className="absolute bottom-full left-0 mb-1 z-10 bg-gray-800 text-white text-xs rounded px-2 py-1 whitespace-nowrap shadow-lg">
          {group.variants.join(", ")}
        </div>
      )}
    </div>
  );
}
