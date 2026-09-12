export type AppearanceMode = "light" | "dark" | "system";

let systemDarkListener: ((e: MediaQueryListEvent) => void) | null = null;
let systemMQ: MediaQueryList | null = null;

export function applyTheme(mode: AppearanceMode) {
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
