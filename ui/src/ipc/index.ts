// Typed wrappers over Tauri commands. The UI calls Rust only through this module and never
// talks to Eagle itself (docs/architecture.md §3, §8).
import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./generated/AppInfo";
import type { ConnectionSettings } from "./generated/ConnectionSettings";

export type { AppInfo, ConnectionSettings };

/** True inside the Tauri window; false in a plain browser, for example the Vite dev server. */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const ipc = {
  appInfo: () => invoke<AppInfo>("app_info"),
  connectionDefaults: () => invoke<ConnectionSettings>("connection_defaults"),
};
