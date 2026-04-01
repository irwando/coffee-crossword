// ── DictionaryPanel ───────────────────────────────────────────────────────────
// Fetches and displays a word definition from the Free Dictionary API.
// Rendered as a centered modal overlay; closes on ✕ click or Escape key.

import { useState, useEffect } from "react";

// ── API types ─────────────────────────────────────────────────────────────────

interface Phonetic {
  text?: string;
  audio?: string;
}

interface Definition {
  definition: string;
  example?: string;
  synonyms: string[];
  antonyms: string[];
}

interface Meaning {
  partOfSpeech: string;
  definitions: Definition[];
  synonyms: string[];
  antonyms: string[];
}

interface Entry {
  word: string;
  phonetics: Phonetic[];
  meanings: Meaning[];
  sourceUrls?: string[];
}

interface ApiError {
  title: string;
  message: string;
  resolution?: string;
}

// ── Fetch logic ───────────────────────────────────────────────────────────────

async function fetchDefinition(word: string): Promise<Entry[]> {
  const url = `https://api.dictionaryapi.dev/api/v2/entries/en/${encodeURIComponent(word)}`;
  const res = await fetch(url);
  if (!res.ok) {
    const err: ApiError = await res.json().catch(() => ({ title: "Error", message: `HTTP ${res.status}` }));
    throw new Error(err.message || err.title);
  }
  return res.json();
}

// ── Sub-components ────────────────────────────────────────────────────────────

function PhoneticBadge({ phonetics }: { phonetics: Phonetic[] }) {
  // Pick the first phonetic with text; also find first with audio.
  const withText = phonetics.find((p) => p.text);
  const withAudio = phonetics.find((p) => p.audio);

  const playAudio = () => {
    if (withAudio?.audio) new Audio(withAudio.audio).play().catch(() => {});
  };

  if (!withText && !withAudio) return null;
  return (
    <div className="flex items-center gap-2 mt-0.5">
      {withText && (
        <span className="text-sm text-gray-500 dark:text-gray-400 font-mono">
          {withText.text}
        </span>
      )}
      {withAudio && (
        <button
          onClick={playAudio}
          title="Play pronunciation"
          className="text-blue-400 hover:text-blue-500 dark:hover:text-blue-300 text-base leading-none"
        >
          🔊
        </button>
      )}
    </div>
  );
}

function MeaningBlock({ meaning }: { meaning: Meaning }) {
  // Collect synonyms from both the meaning level and individual definitions.
  const allSynonyms = Array.from(
    new Set([
      ...meaning.synonyms,
      ...meaning.definitions.flatMap((d) => d.synonyms),
    ])
  ).slice(0, 8);

  const allAntonyms = Array.from(
    new Set([
      ...meaning.antonyms,
      ...meaning.definitions.flatMap((d) => d.antonyms),
    ])
  ).slice(0, 8);

  return (
    <div className="mb-4">
      <div className="text-xs font-semibold uppercase tracking-wide text-blue-500 dark:text-blue-400 mb-1.5">
        {meaning.partOfSpeech}
      </div>
      <ol className="space-y-2 list-decimal list-inside marker:text-gray-400 marker:text-xs">
        {meaning.definitions.map((def, i) => (
          <li key={i} className="text-sm text-gray-800 dark:text-gray-200 pl-1">
            <span>{def.definition}</span>
            {def.example && (
              <div className="mt-0.5 ml-4 text-xs text-gray-500 dark:text-gray-400 italic">
                "{def.example}"
              </div>
            )}
          </li>
        ))}
      </ol>
      {allSynonyms.length > 0 && (
        <div className="mt-2 text-xs text-gray-500 dark:text-gray-400">
          <span className="font-medium text-gray-600 dark:text-gray-300">Synonyms: </span>
          {allSynonyms.join(", ")}
        </div>
      )}
      {allAntonyms.length > 0 && (
        <div className="mt-1 text-xs text-gray-500 dark:text-gray-400">
          <span className="font-medium text-gray-600 dark:text-gray-300">Antonyms: </span>
          {allAntonyms.join(", ")}
        </div>
      )}
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function DictionaryPanel({
  word,
  onClose,
}: {
  word: string;
  onClose: () => void;
}) {
  const [entries, setEntries] = useState<Entry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setEntries(null);
    setError(null);
    setLoading(true);
    fetchDefinition(word)
      .then((data) => { setEntries(data); setLoading(false); })
      .catch((e) => { setError(e.message); setLoading(false); });
  }, [word]);

  // Close on Escape.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    /* Backdrop */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 dark:bg-black/60"
      onMouseDown={onClose}
    >
      {/* Panel — stop propagation so clicks inside don't close */}
      <div
        className="relative bg-white dark:bg-gray-900 rounded-xl shadow-2xl border border-gray-200 dark:border-gray-700 w-full max-w-lg mx-4 flex flex-col"
        style={{ maxHeight: "72vh" }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-start justify-between px-5 pt-4 pb-3 border-b border-gray-100 dark:border-gray-800 flex-shrink-0">
          <div>
            <h2 className="text-lg font-bold text-gray-900 dark:text-gray-100 leading-tight">
              📖 {word}
            </h2>
            {entries && entries[0] && (
              <PhoneticBadge phonetics={entries[0].phonetics} />
            )}
          </div>
          <button
            onClick={onClose}
            className="ml-4 mt-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-xl leading-none flex-shrink-0"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        {/* Body */}
        <div className="overflow-y-auto px-5 py-4 flex-1">
          {loading && (
            <div className="text-sm text-gray-400 animate-pulse py-4 text-center">
              Looking up "{word}"…
            </div>
          )}

          {error && (
            <div className="text-sm text-gray-500 dark:text-gray-400 py-4 text-center">
              <div className="text-2xl mb-2">🤷</div>
              <div>No definition found for <span className="font-mono font-semibold">"{word}"</span>.</div>
              <div className="text-xs mt-1 text-gray-400">{error}</div>
            </div>
          )}

          {entries && entries.map((entry, ei) => (
            <div key={ei}>
              {/* Show additional entry words (rare: e.g. "go" has multiple) */}
              {ei > 0 && (
                <div className="text-xs font-semibold text-gray-400 uppercase tracking-wide mt-2 mb-3 border-t border-gray-100 dark:border-gray-800 pt-3">
                  {entry.word}
                </div>
              )}
              {entry.meanings.map((meaning, mi) => (
                <MeaningBlock key={mi} meaning={meaning} />
              ))}
            </div>
          ))}
        </div>

        {/* Footer */}
        {entries && (
          <div className="px-5 py-2 border-t border-gray-100 dark:border-gray-800 flex-shrink-0">
            <span className="text-xs text-gray-400">
              Source:{" "}
              <a
                href="https://freedictionaryapi.com"
                target="_blank"
                rel="noreferrer"
                className="underline hover:text-gray-600 dark:hover:text-gray-300"
              >
                Free Dictionary
              </a>
              {" "}(CC BY-SA 4.0)
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
