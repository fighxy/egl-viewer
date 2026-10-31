// Typed wrappers over Tauri commands. The UI calls Rust only through this module and never
// talks to Eagle itself (docs/architecture.md §3, §8).
import { Channel, invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./generated/AppInfo";
import type { Backup } from "./generated/Backup";
import type { BackupKind } from "./generated/BackupKind";
import type { BrowsePreferences } from "./generated/BrowsePreferences";
import type { ConnectionSettings } from "./generated/ConnectionSettings";
import type { DuplicateAnswer } from "./generated/DuplicateAnswer";
import type { DuplicateNotice } from "./generated/DuplicateNotice";
import type { EditKind } from "./generated/EditKind";
import type { ExtensionCount } from "./generated/ExtensionCount";
import type { Folder } from "./generated/Folder";
import type { FolderChange } from "./generated/FolderChange";
import type { ImportFailure } from "./generated/ImportFailure";
import type { ImportKind } from "./generated/ImportKind";
import type { ImportPreview } from "./generated/ImportPreview";
import type { ImportProgress } from "./generated/ImportProgress";
import type { ImportState } from "./generated/ImportState";
import type { ImportTicket } from "./generated/ImportTicket";
import type { Item } from "./generated/Item";
import type { ItemCell } from "./generated/ItemCell";
import type { ItemEdit } from "./generated/ItemEdit";
import type { ItemId } from "./generated/ItemId";
import type { ItemsTarget } from "./generated/ItemsTarget";
import type { LibraryCheck } from "./generated/LibraryCheck";
import type { LibraryCounts } from "./generated/LibraryCounts";
import type { LibraryHolder } from "./generated/LibraryHolder";
import type { LibraryInfo } from "./generated/LibraryInfo";
import type { LibraryQuery } from "./generated/LibraryQuery";
import type { LibraryStats } from "./generated/LibraryStats";
import type { LocationProblem } from "./generated/LocationProblem";
import type { LockProgress } from "./generated/LockProgress";
import type { LoginMethod } from "./generated/LoginMethod";
import type { LoginProof } from "./generated/LoginProof";
import type { LoginStatus } from "./generated/LoginStatus";
import type { MissingOriginal } from "./generated/MissingOriginal";
import type { Palette } from "./generated/Palette";
import type { PivPolicy } from "./generated/PivPolicy";
import type { PivPrompt } from "./generated/PivPrompt";
import type { PivWrap } from "./generated/PivWrap";
import type { Preferences } from "./generated/Preferences";
import type { PreviewEntry } from "./generated/PreviewEntry";
import type { ReadOnlyReason } from "./generated/ReadOnlyReason";
import type { RecentLibrary } from "./generated/RecentLibrary";
import type { RecoveryKey } from "./generated/RecoveryKey";
import type { RepairOptions } from "./generated/RepairOptions";
import type { Scope } from "./generated/Scope";
import type { ScopeProgress } from "./generated/ScopeProgress";
import type { SelectionTags } from "./generated/SelectionTags";
import type { SmartDraft } from "./generated/SmartDraft";
import type { SmartFolder } from "./generated/SmartFolder";
import type { SmartFolderChange } from "./generated/SmartFolderChange";
import type { SortKey } from "./generated/SortKey";
import type { SourceError } from "./generated/SourceError";
import type { StartupResult } from "./generated/StartupResult";
import type { JsonValue } from "./generated/serde_json/JsonValue";
import type { Tag } from "./generated/Tag";
import type { TagGroup } from "./generated/TagGroup";
import type { TagGroupChange } from "./generated/TagGroupChange";
import type { TagMode } from "./generated/TagMode";
import type { Theme } from "./generated/Theme";
import type { TrashInfo } from "./generated/TrashInfo";
import type { UiLayout } from "./generated/UiLayout";
import type { UndoStatus } from "./generated/UndoStatus";
import type { ViewMode } from "./generated/ViewMode";
import type { ViewOpened } from "./generated/ViewOpened";
import type { ViewsInvalidated } from "./generated/ViewsInvalidated";

export type {
  AppInfo,
  Backup,
  BackupKind,
  BrowsePreferences,
  ConnectionSettings,
  DuplicateAnswer,
  DuplicateNotice,
  EditKind,
  ExtensionCount,
  Folder,
  FolderChange,
  ImportFailure,
  ImportKind,
  ImportPreview,
  ImportProgress,
  ImportState,
  ImportTicket,
  Item,
  ItemCell,
  ItemEdit,
  ItemId,
  ItemsTarget,
  JsonValue,
  LibraryCheck,
  LibraryCounts,
  LibraryHolder,
  LibraryInfo,
  LibraryQuery,
  LibraryStats,
  LocationProblem,
  LockProgress,
  LoginMethod,
  LoginProof,
  LoginStatus,
  MissingOriginal,
  Palette,
  PivPolicy,
  PivPrompt,
  PivWrap,
  Preferences,
  PreviewEntry,
  ReadOnlyReason,
  RecentLibrary,
  RecoveryKey,
  RepairOptions,
  Scope,
  ScopeProgress,
  SelectionTags,
  SmartDraft,
  SmartFolder,
  SmartFolderChange,
  SortKey,
  SourceError,
  StartupResult,
  Tag,
  TagGroup,
  TagGroupChange,
  TagMode,
  Theme,
  TrashInfo,
  UiLayout,
  UndoStatus,
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
  confirmBeforeTrash: true,
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
  addFiles: (
    paths: string[],
    folder: string | null = null,
    exclude: string[] = [],
    include: string[] = [],
    reviewed = false,
  ) => invoke<ImportTicket>("add_files", { paths, folder, exclude, include, reviewed }),
  previewImport: (path: string) => invoke<ImportPreview>("preview_import", { path }),
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
  // Undo, the trash and file actions (docs/spec.md §6). File bytes stay in Rust: these commands
  // name items, and Rust finds their files.
  undo: () => invoke<UndoStatus | null>("undo"),
  undoStatus: () => invoke<UndoStatus | null>("undo_status"),
  deletePermanently: (target: ItemsTarget) => invoke<number>("delete_permanently", { target }),
  emptyTrash: () => invoke<number>("empty_trash"),
  trashInfo: () => invoke<TrashInfo>("trash_info"),
  restoreTrash: () => invoke<number>("restore_trash"),
  openOriginal: (id: string) => invoke<void>("open_original", { id }),
  revealOriginals: (ids: string[]) => invoke<void>("reveal_originals", { ids }),
  copyOriginals: (ids: string[]) => invoke<number>("copy_originals", { ids }),
  openLink: (url: string) => invoke<void>("open_link", { url }),
  startDrag: (ids: string[]) => invoke<void>("start_drag", { ids }),
  // Library Settings (docs/spec.md §7.4, item 6).
  revealLibrary: () => invoke<void>("reveal_library"),
  backups: () => invoke<Backup[]>("backups"),
  backUpNow: () => invoke<Backup>("back_up_now"),
  restoreBackup: (name: string) => invoke<LibraryInfo>("restore_backup", { name }),
  renameLibrary: (name: string) => invoke<LibraryInfo>("rename_library", { name }),
  checkLibrary: () => invoke<LibraryCheck>("check_library"),
  repairLibrary: (options: RepairOptions) => invoke<LibraryCheck>("repair_library", { options }),
  libraryStats: () => invoke<LibraryStats>("library_stats"),
  rebuildThumbnails: () => invoke<number>("rebuild_thumbnails"),
  reorderItems: (folder: string, ids: string[], before: string | null) =>
    invoke<void>("reorder_items", { folder, ids, before }),
  // Tags (docs/spec.md §3 Tags). Names are exact, case-sensitive strings.
  selectionTags: (target: ItemsTarget) => invoke<SelectionTags>("selection_tags", { target }),
  tagVocabulary: () => invoke<Tag[]>("tag_vocabulary"),
  tagGroups: () => invoke<TagGroup[]>("tag_groups"),
  saveTagGroup: (change: TagGroupChange) => invoke<TagGroup[]>("save_tag_group", { change }),
  deleteTagGroup: (id: string) => invoke<TagGroup[]>("delete_tag_group", { id }),
  renameTag: (from: string, to: string) => invoke<void>("rename_tag", { from, to }),
  mergeTags: (source: string, target: string) => invoke<void>("merge_tags", { source, target }),
  deleteTag: (name: string) => invoke<void>("delete_tag", { name }),
  // Smart folders: Eagle's rules, stored unchanged (docs/architecture.md §16).
  smartFolders: () => invoke<SmartFolder[]>("smart_folders"),
  saveSmartFolder: (change: SmartFolderChange) =>
    invoke<SmartFolder[]>("save_smart_folder", { change }),
  deleteSmartFolder: (id: string) => invoke<SmartFolder[]>("delete_smart_folder", { id }),
  smartRules: (query: LibraryQuery) => invoke<SmartDraft>("smart_rules", { query }),
  extensions: () => invoke<ExtensionCount[]>("extensions"),
  // Locked folders (docs/age-locked-folders.md). Passwords go to Rust and are never stored.
  lockFolder: (
    id: string,
    password: string,
    label: string | null,
    labelColor: string | null,
    onProgress?: (progress: LockProgress) => void,
  ) => {
    const channel = new Channel<LockProgress>();
    if (onProgress) channel.onmessage = onProgress;
    return invoke<RecoveryKey>("lock_folder", {
      id,
      password,
      label,
      labelColor,
      onProgress: channel,
    });
  },
  unlockFolder: (id: string, password: string) => invoke<void>("unlock_folder", { id, password }),
  pivWraps: (id: string) => invoke<PivWrap[]>("piv_wraps", { id }),
  bindYubiKey: (id: string, password: string, policy: PivPolicy, pin: string | null) =>
    invoke<void>("bind_yubikey", { id, password, policy, pin }),
  unbindYubiKey: (id: string, password: string, serial: string) =>
    invoke<void>("unbind_yubikey", { id, password, serial }),
  unlockWithYubiKey: (id: string, serial: string, prompt: PivPrompt) =>
    invoke<void>("unlock_with_yubikey", { id, serial, prompt }),
  yubiKeyPresent: () => invoke<boolean>("yubikey_present"),
  loginStatus: () => invoke<LoginStatus>("login_status"),
  clearLogin: () => invoke<LoginStatus>("clear_login"),
  setLoginPassword: (password: string) => invoke<LoginStatus>("set_login_password", { password }),
  setLoginPin: (pin: string) => invoke<LoginStatus>("set_login_pin", { pin }),
  setLoginWebAuthn: () => invoke<LoginStatus>("set_login_webauthn"),
  unlockApp: (proof: LoginProof) => invoke<void>("unlock_app", { proof }),
  recoverFolder: (id: string, recovery: string, newPassword: string) =>
    invoke<void>("recover_folder", { id, recovery, newPassword }),
  relockFolder: (id: string) => invoke<void>("relock_folder", { id }),
  changeFolderPassword: (id: string, current: string, newPassword: string) =>
    invoke<void>("change_folder_password", { id, current, newPassword }),
  renewRecoveryKey: (id: string, current: string) =>
    invoke<RecoveryKey>("renew_recovery_key", { id, current }),
  removeFolderLock: (
    id: string,
    password: string,
    onProgress?: (progress: LockProgress) => void,
  ) => {
    const channel = new Channel<LockProgress>();
    if (onProgress) channel.onmessage = onProgress;
    return invoke<void>("remove_folder_lock", { id, password, onProgress: channel });
  },
  setLockLabel: (id: string, label: string | null, labelColor: string | null) =>
    invoke<void>("set_lock_label", { id, label, labelColor }),
  saveRecoveryFile: (path: string, folder: string, key: RecoveryKey) =>
    invoke<void>("save_recovery_file", { path, folder, key }),
};

/** Events from Rust (docs/architecture.md §8). */
export const EVENTS = {
  importProgress: "import-progress",
  closeRequested: "close-requested",
  viewsInvalidated: "views-invalidated",
} as const;
