import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface MatchResult {
  word: string;
  balance: string | null;
}

export default function App() {
  const [pattern, setPattern] = useState("");
  const [results, setResults] = useState<MatchResult[]>([]);
  const [status, setStatus] = useState("Enter a pattern and press Search");
  const [loading, setLoading] = useState(false);
  const [minLen, setMinLen] = useState(1);
  const [maxLen, setMaxLen] = useState(50);

  const doSearch = useCallback(async () => {
    const trimmed = pattern.trim();
    if (!trimmed) return;

    setLoading(true);
    setStatus("Searching...");

    try {
      const matches = await invoke<MatchResult[]>("search", {
        pattern: trimmed,
        minLen,
        maxLen,
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
  }, [pattern, minLen, maxLen]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") doSearch();
  };

  // Group results by word length
  const grouped = results.reduce<Record<number, MatchResult[]>>((acc, r) => {
    const len = r.word.length;
    if (!acc[len]) acc[len] = [];
    acc[len].push(r);
    return acc;
  }, {});
  const lengths = Object.keys(grouped)
    .map(Number)
    .sort((a, b) => a - b);

  return (
    <div className="min-h-screen bg-gray-50 p-6">
      <div className="max-w-2xl mx-auto">

        {/* Header */}
        <h1 className="text-2xl font-semibold text-gray-800 mb-1">
          Coffee Crossword
        </h1>
        <p className="text-sm text-gray-500 mb-6">
          Word search helper — 101k words
        </p>

        {/* Search bar */}
        <div className="flex gap-2 mb-3">
          <input
            type="text"
            value={pattern}
            onChange={(e) => setPattern(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="e.g.  .l...r.n  or  ;acenrt  or  m*ja"
            className="flex-1 px-4 py-2 border border-gray-300 rounded-lg font-mono text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
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

        {/* Length filters */}
        <div className="flex items-center gap-4 mb-6 text-sm text-gray-600">
          <span>Length:</span>
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
                <div
                  key={r.word}
                  className="flex items-baseline gap-1 bg-white border border-gray-200 rounded px-3 py-1.5"
                >
                  <span className="font-mono text-sm text-gray-800">{r.word}</span>
                  {r.balance && (
                    <span className="font-mono text-xs text-blue-500">{r.balance}</span>
                  )}
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
