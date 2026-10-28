import type { Theme } from "@/ipc";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function systemPrefersDark(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia(DARK_QUERY).matches;
}

/** Puts the `dark` class on `<html>` for the chosen theme; `system` follows Windows. */
export function applyTheme(theme: Theme, root: HTMLElement = document.documentElement) {
  const dark = theme === "dark" || (theme === "system" && systemPrefersDark());
  root.classList.toggle("dark", dark);
}

/** Calls `onChange` when Windows switches between light and dark. */
export function watchSystemTheme(onChange: () => void): () => void {
  if (typeof window.matchMedia !== "function") return () => {};
  const query = window.matchMedia(DARK_QUERY);
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}
