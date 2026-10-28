// Typed wrappers over Tauri commands. The UI calls Rust only through this module and never
// talks to Eagle itself (docs/architecture.md §3, §8).
import { Channel, invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./generated/AppInfo";
import type { BrowsePreferences } from "./generated/BrowsePreferences";
import type { ConnectionSettings } from "./generated/ConnectionSettings";
import type { DuplicateAnswer } from "./generated/DuplicateAnswer";
import type { DuplicateNotice } from "./generated/DuplicateNotice";
import type { Folder } from "./generated/Folder";
import type { FolderChange } from "./generated/FolderChange";
import type { ImportFailure } from "./generated/ImportFailure";
import type { ImportKind } from "./generated/ImportKind";
import type { ImportProgress } from "./generated/ImportProgress";
import type { ImportState } from "./generated/ImportState";
import type { ImportTicket } from "./generated/ImportTicket";
import type { Item } from "./generated/Item";
import type { ItemCell } from "./generated/ItemCell";
import type { ItemEdit } from "./generated/ItemEdit";
import type { ItemId } from "./generated/ItemId";
import type { ItemsTarget } from "./generated/ItemsTarget";
import type { LibraryCounts } from "./generated/LibraryCounts";
import type { LibraryHolder } from "./generated/LibraryHolder";
import type { LibraryInfo } from "./generated/LibraryInfo";
import type { LibraryQuery } from "./generated/LibraryQuery";
import type { LocationProblem } from "./generated/LocationProblem";
import type { Palette } from "./generated/Palette";
import type { Preferences } from "./generated/Preferences";
import type { ReadOnlyReason } from "./generated/ReadOnlyReason";
import type { RecentLibrary } from "./generated/RecentLibrary";
import type { Scope } from "./generated/Scope";
import type { ScopeProgress } from "./generated/ScopeProgress";
import type { SortKey } from "./generated/SortKey";
import type { SourceError } from "./generated/SourceError";
import type { StartupResult } from "./generated/StartupResult";
import type { Theme } from "./generated/Theme";
import type { UiLayout } from "./generated/UiLayout";
import type { ViewMode } from "./generated/ViewMode";
import type { ViewOpened } from "./generated/ViewOpened";
import type { ViewsInvalidated } from "./generated/ViewsInvalidated";

export type {
  AppInfo,
  BrowsePreferences,
  ConnectionSettings,
  DuplicateAnswer,
  DuplicateNotice,
  Folder,
  FolderChange,
  ImportFailure,
  ImportKind,
  ImportProgress,
  ImportState,
  ImportTicket,
  Item,
  ItemCell,
  ItemEdit,
  ItemId,
  ItemsTarget,
  LibraryCounts,
  LibraryHolder,
  LibraryInfo,
  LibraryQuery,
  LocationProblem,
  Palette,
  Preferences,
  ReadOnlyReason,
  RecentLibrary,
  Scope,
  ScopeProgress,
  SortKey,
  SourceError,
  StartupResult,
  Theme,
  UiLayout,
  ViewMode,
  ViewOpened,
  ViewsInvalidated,
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
  browse: {
    viewMode: "grid",
    thumbnailSize: 160,
    showNames: true,
    sort: "added",
    ascending: false,
    includeSubfolders: false,
  },
};

/** The smallest and largest grid cell, as Rust clamps them (`BrowsePreferences`). */
export const THUMBNAIL_SIZES = { min: 80, max: 320 } as const;

/** A query for `scope` with no chips and no search, sorted as the preferences say. */
export function queryFor(scope: Scope, browse: BrowsePreferences): LibraryQuery {
  return {
    scope,
    tagMode: "any",
    tags: [],
    extraFolderIds: [],
    folderMode: "any",
    ext: null,
    rating: null,
    shape: null,
    text: null,
    sort: browse.sort,
    ascending: browse.ascending,
  };
}

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
  addFiles: (paths: string[], folder: string | null = null) =>
    invoke<ImportTicket>("add_files", { paths, folder }),
  pasteFiles: (folder: string | null = null) =>
    invoke<ImportTicket | null>("paste_files", { folder }),
  answerImport: (id: number, answer: DuplicateAnswer) =>
    invoke<void>("answer_import", { id, answer }),
  cancelImport: (id: number) => invoke<void>("cancel_import", { id }),
  imports: () => invoke<ImportProgress[]>("imports"),
  quit: () => invoke<void>("quit"),
  openView: (query: LibraryQuery, onProgress?: (progress: ScopeProgress) => void) => {
    const channel = new Channel<ScopeProgress>();
    if (onProgress) channel.onmessage = onProgress;
    return invoke<ViewOpened>("open_view", { query, onProgress: channel });
  },
  viewRange: (viewId: number, start: number, len: number) =>
    invoke<ItemCell[]>("view_range", { viewId, start, len }),
  viewIds: (viewId: number, start: number, len: number) =>
    invoke<ItemId[]>("view_ids", { viewId, start, len }),
  closeView: (viewId: number) => invoke<void>("close_view", { viewId }),
  itemDetail: (id: string) => invoke<Item | null>("item_detail", { id }),
  folderTree: () => invoke<Folder[]>("folder_tree"),
  saveFolder: (change: FolderChange) => invoke<Folder[]>("save_folder", { change }),
  deleteFolder: (id: string, trashItems: boolean) =>
    invoke<Folder[]>("delete_folder", { id, trashItems }),
  applyItemEdit: (id: string, edit: ItemEdit) => invoke<Item>("apply_item_edit", { id, edit }),
  applyBatchEdit: (target: ItemsTarget, edit: ItemEdit) =>
    invoke<number>("apply_batch_edit", { target, edit }),
  reorderItems: (folder: string, ids: string[], before: string | null) =>
    invoke<void>("reorder_items", { folder, ids, before }),
};

/** Events from Rust (docs/architecture.md §8). */
export const EVENTS = {
  importProgress: "import-progress",
  closeRequested: "close-requested",
  viewsInvalidated: "views-invalidated",
} as const;
