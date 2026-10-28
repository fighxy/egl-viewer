// The native window, only inside Tauri: show after the first paint, title, theme, full screen
// and close (docs/architecture.md §8).
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Theme } from "@/ipc";
import { inTauri } from "@/ipc";

function current() {
  return inTauri() ? getCurrentWindow() : null;
}

export async function showWindow() {
  await current()?.show();
}

export async function setWindowTitle(title: string) {
  document.title = title;
  await current()?.setTitle(title);
}

/** The title bar follows the theme; `system` hands it back to Windows. */
export async function setWindowTheme(theme: Theme) {
  await current()?.setTheme(theme === "system" ? null : theme);
}

export async function setFullscreen(fullscreen: boolean) {
  await current()?.setFullscreen(fullscreen);
}

export async function closeWindow() {
  await current()?.close();
}
