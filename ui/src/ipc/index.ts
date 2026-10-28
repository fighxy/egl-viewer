// Typed wrappers over Tauri commands. The UI calls Rust only through this module and never
// talks to Eagle itself (docs/architecture.md §3, §8).
import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./generated/AppInfo";
import type { ConnectionSettings } from "./generated/ConnectionSettings";
import type { DuplicateAnswer } from "./generated/DuplicateAnswer";
import type { DuplicateNotice } from "./generated/DuplicateNotice";
import type { ImportFailure } from "./generated/ImportFailure";
import type { ImportKind } from "./generated/ImportKind";
import type { ImportProgress } from "./generated/ImportProgress";
import type { ImportState } from "./generated/ImportState";
import type { ImportTicket } from "./generated/ImportTicket";
import type { LibraryCounts } from "./generated/LibraryCounts";
import type { LibraryHolder } from "./generated/LibraryHolder";
import type { LibraryInfo } from "./generated/LibraryInfo";
import type { LocationProblem } from "./generated/LocationProblem";
import type { Preferences } from "./generated/Preferences";
import type { ReadOnlyReason } from "./generated/ReadOnlyReason";
import type { RecentLibrary } from "./generated/RecentLibrary";
import type { SourceError } from "./generated/SourceError";
import type { StartupResult } from "./generated/StartupResult";
import type { Theme } from "./generated/Theme";
import type { UiLayout } from "./generated/UiLayout";

export type {
  AppInfo,
  ConnectionSettings,
  DuplicateAnswer,
  DuplicateNotice,
  ImportFailure,
  ImportKind,
  ImportProgress,
  ImportState,
  ImportTicket,
  LibraryCounts,
  LibraryHolder,
  LibraryInfo,
  LocationProblem,
  Preferences,
  ReadOnlyReason,
  RecentLibrary,
  SourceError,
  StartupResult,
  Theme,
  UiLayout,
};

/** True inside the Tauri window; false in a plain browser, for example the Vite dev server. */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const DEFAULT_PREFERENCES: Preferences = {
  theme: "system",
  reopenLastLibrary: true,
  defaultLibraryParent: null,
  layout: { leftWidth: 240, rightWidth: 320, leftOpen: true, rightOpen: true },
};

export const ipc = {
  appInfo: () => invoke<AppInfo>("app_info"),
  connectionDefaults: () => invoke<ConnectionSettings>("connection_defaults"),
  defaultLibraryLocation: () => invoke<string>("default_library_location"),
  createLibrary: (parent: string, name: string) =>
    invoke<LibraryInfo>("create_library", { parent, name }),
  openLibrary: (path: string) => invoke<LibraryInfo>("open_library", { path }),
  closeLibrary: () => invoke<void>("close_library"),
  currentLibrary: () => invoke<LibraryInfo | null>("current_library"),
  recentLibraries: () => invoke<RecentLibrary[]>("recent_libraries"),
  forgetRecentLibrary: (path: string) => invoke<void>("forget_recent_library", { path }),
  preferences: () => invoke<Preferences>("get_preferences"),
  setPreferences: (preferences: Preferences) =>
    invoke<Preferences>("set_preferences", { preferences }),
  startup: () => invoke<StartupResult>("startup"),
  libraryCounts: () => invoke<LibraryCounts>("library_counts"),
  addFiles: (paths: string[]) => invoke<ImportTicket>("add_files", { paths }),
  pasteFiles: () => invoke<ImportTicket | null>("paste_files"),
  answerImport: (id: number, answer: DuplicateAnswer) =>
    invoke<void>("answer_import", { id, answer }),
  cancelImport: (id: number) => invoke<void>("cancel_import", { id }),
  imports: () => invoke<ImportProgress[]>("imports"),
  quit: () => invoke<void>("quit"),
};

/** Events from Rust (docs/architecture.md §8). */
export const EVENTS = {
  importProgress: "import-progress",
  closeRequested: "close-requested",
} as const;
