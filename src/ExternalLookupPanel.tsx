// ── ExternalLookupPanel ───────────────────────────────────────────────────────
// Displays an external lookup URL in an embedded iframe.
// The URL template (from the word list header) has {term} replaced with the
// selected word. Falls back gracefully if the site blocks iframe embedding.

import { useState, useEffect } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

interface Props {
  word: string;
  urlTemplate: string;   // validated URL with exactly one {term} token
  listName: string;
  onClose: () => void;
}

export default function ExternalLookupPanel({ word, urlTemplate, listName, onClose }: Props) {
  const resolvedUrl = urlTemplate.replace("{term}", encodeURIComponent(word));
  const [frameBlocked, setFrameBlocked] = useState(false);
  const [frameLoaded, setFrameLoaded] = useState(false);

  // Close on Escape.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const handleOpenInBrowser = async () => {
    await openUrl(resolvedUrl);
  };

  // The iframe load event fires even when the page returns an error response,
  // so we use a short timeout heuristic: if the frame hasn't loaded after 6s
  // it's likely blocked by X-Frame-Options.
  useEffect(() => {
    setFrameBlocked(false);
    setFrameLoaded(false);
    const t = setTimeout(() => {
      setFrameBlocked((loaded) => !loaded as unknown as boolean);
    }, 6000);
    return () => clearTimeout(t);
  }, [resolvedUrl]);

  return (
    /* Backdrop */
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 dark:bg-black/60"
      onMouseDown={onClose}
    >
      {/* Panel */}
      <div
        className="relative bg-white dark:bg-gray-900 rounded-xl shadow-2xl border border-gray-200 dark:border-gray-700 flex flex-col"
        style={{ width: "min(860px, 92vw)", height: "min(640px, 85vh)" }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center gap-3 px-4 py-2.5 border-b border-gray-100 dark:border-gray-800 flex-shrink-0">
          <span className="text-sm font-semibold text-gray-800 dark:text-gray-100 truncate flex-1">
            🔍 <span className="font-mono">{word}</span>
            <span className="font-sans font-normal text-gray-400 ml-2">— {listName}</span>
          </span>
          <button
            onClick={handleOpenInBrowser}
            title={resolvedUrl}
            className="flex-shrink-0 text-xs px-3 py-1 rounded border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
          >
            Open in Browser ↗
          </button>
          <button
            onClick={onClose}
            className="flex-shrink-0 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-xl leading-none ml-1"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 relative overflow-hidden rounded-b-xl">
          {!frameLoaded && !frameBlocked && (
            <div className="absolute inset-0 flex items-center justify-center bg-white dark:bg-gray-900 z-10">
              <span className="text-sm text-gray-400 animate-pulse">Loading…</span>
            </div>
          )}

          {frameBlocked ? (
            <div className="absolute inset-0 flex flex-col items-center justify-center gap-4 bg-white dark:bg-gray-900 px-8 text-center">
              <div className="text-3xl">🚫</div>
              <p className="text-sm text-gray-600 dark:text-gray-400">
                This site doesn't allow embedding. Open it in your browser instead.
              </p>
              <button
                onClick={handleOpenInBrowser}
                className="px-4 py-2 bg-blue-500 text-white text-sm rounded-lg hover:bg-blue-600 transition-colors"
              >
                Open in Browser ↗
              </button>
            </div>
          ) : (
            <iframe
              src={resolvedUrl}
              className="w-full h-full border-0"
              title={`Lookup: ${word}`}
              onLoad={() => setFrameLoaded(true)}
              // Sandbox is intentionally permissive — the user configured this URL.
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
            />
          )}
        </div>
      </div>
    </div>
  );
}
