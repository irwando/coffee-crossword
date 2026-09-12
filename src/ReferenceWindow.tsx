import { useEffect, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { ReferenceFull, ReferenceCompact } from "./PatternReference";
import { applyTheme, type AppearanceMode } from "./theme";

// Standalone window rendered when the Pattern Reference panel is popped out
// (see main.tsx — selected via the ?window=reference URL query param). Has
// no access to the main window's search state, so clicking a pattern emits
// an app-wide event for the main window to act on instead of running a
// search directly.
export default function ReferenceWindow() {
  // Seeded from the URL at open time; kept live afterward via
  // "reference:style-changed" (App.tsx emits it whenever referenceMode
  // changes, so Full/Compact switches while popped out take effect here too).
  const [style, setStyle] = useState(() => new URLSearchParams(window.location.search).get("style"));

  useEffect(() => {
    const appearance = new URLSearchParams(window.location.search).get("appearance") as AppearanceMode | null;
    applyTheme(appearance ?? "system");
  }, []);

  useEffect(() => {
    const unlisten = listen<string>("reference:style-changed", (e) => setStyle(e.payload));
    return () => { unlisten.then((u) => u()); };
  }, []);

  const handlePatternClick = (pattern: string) => {
    emit("reference:pattern-clicked", pattern);
  };

  // Routed through a Rust command rather than getCurrentWindow().close() —
  // closing a window from JS needs the core:window:allow-close permission,
  // which this window isn't granted; a custom Tauri command isn't ACL-gated.
  const handleDock = () => {
    invoke("close_reference_window").catch(console.error);
  };

  const headerAction = { label: "Dock ⇲", onClick: handleDock };

  return (
    <div className="h-screen overflow-y-auto p-3 bg-white dark:bg-gray-900">
      {style === "compact" ? (
        <ReferenceCompact onPatternClick={handlePatternClick} headerAction={headerAction} />
      ) : (
        <ReferenceFull onPatternClick={handlePatternClick} headerAction={headerAction} />
      )}
    </div>
  );
}
