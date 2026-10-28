import { QueryClient, QueryClientProvider, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import {
  type Availability,
  type CommandActions,
  type CommandState,
  needsWritableLibrary,
} from "@/commands/registry";
import { useShortcuts } from "@/commands/useShortcuts";
import { AboutDialog } from "@/features/about/AboutDialog";
import { DetailsPanel } from "@/features/browse/DetailsPanel";
import { ItemsView } from "@/features/browse/ItemsView";
import { Preview } from "@/features/browse/Preview";
import {
  everything,
  NOTHING,
  type Selection,
  selectedCount,
  singleId,
} from "@/features/browse/selection";
import { fetchCell, useView } from "@/features/browse/useView";
import { ConnectScreen } from "@/features/connect/ConnectScreen";
import { type DragPayload, DragProvider, type DropTarget } from "@/features/dnd/drag";
import { type FolderDialog, FolderDialogs } from "@/features/folders/FolderDialogs";
import { FolderProperties } from "@/features/folders/FolderProperties";
import { FolderTree, type LockAction } from "@/features/folders/FolderTree";
import {
  canMove,
  findFolder,
  folderPath,
  parentOf,
  siblingsOf,
  subtreeIds,
} from "@/features/folders/tree";
import { useExpanded } from "@/features/folders/useExpanded";
import { DuplicateDialog } from "@/features/imports/DuplicateDialog";
import { ImportsPanel } from "@/features/imports/ImportsPanel";
import { importSummary } from "@/features/imports/importText";
import { useImports } from "@/features/imports/useImports";
import { LibraryScreen, type Place, scopeOf } from "@/features/library/LibraryScreen";
import { type LockDialog, LockDialogs } from "@/features/locks/LockDialogs";
import { LockedFolders, lockedFolders } from "@/features/locks/LockedFolders";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { AppMenu } from "@/features/shell/AppMenu";
import { type Confirmation, ConfirmDialog } from "@/features/shell/ConfirmDialog";
import { ShortcutsDialog } from "@/features/shortcuts/ShortcutsDialog";
import { CreateLibraryDialog } from "@/features/welcome/CreateLibraryDialog";
import { WelcomeScreen } from "@/features/welcome/WelcomeScreen";
import {
  type AppInfo,
  type BrowsePreferences,
  type ConnectionSettings,
  DEFAULT_PREFERENCES,
  EVENTS,
  type Folder,
  type FolderChange,
  type ItemEdit,
  type ItemsTarget,
  inTauri,
  ipc,
  type LibraryCounts,
  type LibraryInfo,
  type Preferences,
  queryFor,
  type RecentLibrary,
  type SortKey,
  THUMBNAIL_SIZES,
  type UiLayout,
} from "@/ipc";
import {
  closeWindow,
  setFullscreen,
  setWindowTheme,
  setWindowTitle,
  showWindow,
} from "@/lib/appWindow";
import { installDesktopBehaviour } from "@/lib/desktop";
import { describeError } from "@/lib/errors";
import { applyTheme, watchSystemTheme } from "@/lib/theme";

/** Used until Rust answers, and in a plain browser without Tauri. */
export const FALLBACK_DEFAULTS: ConnectionSettings = { host: "127.0.0.1", port: 41595 };

type View = { kind: "welcome" } | { kind: "connect" } | { kind: "library"; library: LibraryInfo };
type DialogKind = "create" | "settings" | "about" | "shortcuts";

async function pickFolder(title: string, defaultPath?: string): Promise<string | null> {
  const picked = await openDialog({ directory: true, multiple: false, title, defaultPath });
  return typeof picked === "string" ? picked : null;
}

/** The v1 formats with previews, plus AVIF, which is stored with a placeholder (D-015). */
const IMAGE_EXTENSIONS = [
  "jpg",
  "jpeg",
  "png",
  "gif",
  "webp",
  "avif",
  "svg",
  "tif",
  "tiff",
  "bmp",
  "psd",
];

async function pickFiles(): Promise<string[]> {
  const picked = await openDialog({
    multiple: true,
    directory: false,
    title: "Add files",
    filters: [
      { name: "Images", extensions: IMAGE_EXTENSIONS },
      { name: "All files", extensions: ["*"] },
    ],
  });
  if (picked === null) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/**
 * Runs `update` as a View Transition when motion is on (D-028). Only in Chromium (WebView2):
 * WebKitGTK crashed on View Transitions while phase 3 was checked (D-040).
 */
function withTransition(update: () => void) {
  const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  const chromium = /\bChrome\//.test(navigator.userAgent);
  if (!document.startViewTransition || !chromium || reduced) {
    update();
    return;
  }
  document.startViewTransition(() => flushSync(update));
}

/** How far one Ctrl+= or Ctrl+- step changes the thumbnail size. */
const ZOOM_STEP = 20;

/** A key for a place, for effects and for the sort picked on that screen. */
function placeKey(place: Place): string {
  return place.kind === "folder" ? `folder:${place.id}` : place.kind;
}

/** Every folder id of a tree. */
function folderIds(tree: readonly Folder[]): Set<string> {
  const ids = new Set<string>();
  for (const folder of tree) for (const id of subtreeIds(folder)) ids.add(id);
  return ids;
}

/** A folder's full properties with some changed: `update` always sends all of them. */
function folderUpdate(
  folder: Folder,
  change: Partial<Extract<FolderChange, { type: "update" }>>,
): FolderChange {
  return {
    type: "update",
    id: folder.id,
    name: folder.name,
    description: folder.description,
    tags: folder.tags,
    iconColor: folder.iconColor,
    orderBy: folder.orderBy,
    sortAscending: folder.sortAscending,
    ...change,
  };
}

export function App() {
  // One cache per window: views and windows of cells from Rust (docs/architecture.md §8).
  const [client] = useState(
    () =>
      new QueryClient({
        defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
      }),
  );
  return (
    <QueryClientProvider client={client}>
      <AppShell />
    </QueryClientProvider>
  );
}

function AppShell() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [defaults, setDefaults] = useState<ConnectionSettings>(FALLBACK_DEFAULTS);
  const [view, setView] = useState<View>({ kind: "welcome" });
  const [status, setStatus] = useState<string>();
  const [preferences, setPreferences] = useState<Preferences>(DEFAULT_PREFERENCES);
  const [recent, setRecent] = useState<RecentLibrary[]>([]);
  const [dialog, setDialog] = useState<DialogKind | null>(null);
  const [fullscreen, setFullscreenState] = useState(false);
  const [builtInLocation, setBuiltInLocation] = useState("");
  const [counts, setCounts] = useState<LibraryCounts | null>(null);
  const [importsOpen, setImportsOpen] = useState(false);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [dropping, setDropping] = useState(false);
  const [place, setPlace] = useState<Place>({ kind: "all" });
  /** A sort picked on a screen whose folder has its own sort: that screen only. */
  const [screenSort, setScreenSort] = useState<{
    place: string;
    sort: SortKey;
    ascending: boolean;
  } | null>(null);
  const [folderDialog, setFolderDialog] = useState<FolderDialog | null>(null);
  const [lockDialog, setLockDialog] = useState<LockDialog | null>(null);
  const [selection, setSelection] = useState<Selection>(NOTHING);
  const [previewIndex, setPreviewIndex] = useState<number | null>(null);
  const [transitionId, setTransitionId] = useState<string | null>(null);
  const detailsRef = useRef<HTMLDivElement>(null);

  useEffect(() => installDesktopBehaviour(), []);

  useEffect(() => {
    applyTheme(preferences.theme);
    void setWindowTheme(preferences.theme).catch(() => {});
    if (preferences.theme !== "system") return;
    return watchSystemTheme(() => applyTheme("system"));
  }, [preferences.theme]);

  const library = view.kind === "library" ? view.library : null;
  const imports = useImports(library?.path ?? null);
  const client = useQueryClient();
  const writable: Availability = !library
    ? "Open a library first."
    : library.readOnly
      ? "This library is read-only."
      : true;

  // The folder tree with counts; writes answer with the new tree.
  const foldersKey = useMemo(() => ["folders", library?.path ?? null] as const, [library?.path]);
  const folders = useQuery({
    queryKey: foldersKey,
    queryFn: ipc.folderTree,
    enabled: library !== null && inTauri(),
  });
  const tree = useMemo(() => folders.data ?? [], [folders.data]);
  const [expanded, setExpanded] = useExpanded(library?.path ?? null);
  const openFolder = place.kind === "folder" ? findFolder(tree, place.id) : undefined;

  const browse = preferences.browse;
  const key = placeKey(place);
  const picked = screenSort?.place === key ? screenSort : null;
  const sort = picked?.sort ?? openFolder?.orderBy ?? browse.sort;
  const ascending = picked
    ? picked.ascending
    : openFolder?.orderBy
      ? (openFolder.sortAscending ?? true)
      : browse.ascending;
  const query = useMemo(
    () =>
      library
        ? queryFor(scopeOf(place, browse.includeSubfolders), { ...browse, sort, ascending })
        : null,
    [library, place, browse, sort, ascending],
  );
  const opened = useView(query);
  const total = opened.view?.total ?? 0;
  // Items move between cells only in a folder's own manual order, without subfolder items.
  const reorderable =
    writable === true &&
    openFolder !== undefined &&
    (!browse.includeSubfolders || openFolder.children.length === 0) &&
    sort === "manual" &&
    ascending;

  // A new library or place starts with nothing selected; a new sort keeps the selected items.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset when the screen changes.
  useEffect(() => {
    setSelection(NOTHING);
    setPreviewIndex(null);
  }, [library?.path, key]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: positions change with the order.
  useEffect(() => {
    setSelection((current) => ({ ...current, anchor: null, focus: null }));
  }, [sort, ascending]);

  // The open folder is gone (deleted) or locked again: back to All.
  useEffect(() => {
    if (place.kind !== "folder" || !folders.data) return;
    const folder = findFolder(folders.data, place.id);
    if (!folder || (folder.isLocked && !folder.isUnlocked)) setPlace({ kind: "all" });
  }, [place, folders.data]);
  const addingFiles = imports.running.some((progress) => progress.kind === "files");

  // Writes recompute the open views; counts of the left bar follow.
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (!inTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen(EVENTS.viewsInvalidated, () => {
      setRevision((count) => count + 1);
      void client.invalidateQueries({ queryKey: ["folders"] });
    })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [client]);

  // Counts follow the added files and other writes, at most every 300 ms.
  // biome-ignore lint/correctness/useExhaustiveDependencies: refetched whenever files were added.
  useEffect(() => {
    if (!library || !inTauri()) {
      setCounts(null);
      return;
    }
    const timer = window.setTimeout(() => {
      ipc
        .libraryCounts()
        .then(setCounts)
        .catch(() => setCounts(null));
    }, 300);
    return () => window.clearTimeout(timer);
  }, [library, imports.added, revision]);

  useEffect(() => {
    const title = library
      ? `${library.name}${library.readOnly ? " (read-only)" : ""} — egl-viewer`
      : "egl-viewer";
    void setWindowTitle(title).catch(() => {});
  }, [library]);

  const refreshRecent = useCallback(async () => {
    if (!inTauri()) return;
    setRecent(await ipc.recentLibraries().catch(() => []));
  }, []);

  useEffect(() => {
    let cancelled = false;
    const start = async () => {
      if (!inTauri()) return;
      ipc
        .appInfo()
        .then(setInfo)
        .catch(() => setInfo(null));
      ipc
        .connectionDefaults()
        .then(setDefaults)
        .catch(() => setDefaults(FALLBACK_DEFAULTS));
      const stored = await ipc.preferences().catch(() => DEFAULT_PREFERENCES);
      if (cancelled) return;
      setPreferences(stored);
      const result = await ipc
        .startup()
        .catch((error: unknown) => ({ library: null, notice: describeError(error) }));
      if (cancelled) return;
      if (result.library) setView({ kind: "library", library: result.library });
      if (result.notice) setStatus(result.notice);
      await refreshRecent();
    };
    void start().finally(() => {
      // Shown after the first paint, so a dark theme never flashes white (D-029).
      requestAnimationFrame(() => void showWindow().catch(() => {}));
    });
    return () => {
      cancelled = true;
    };
  }, [refreshRecent]);

  const libraryOpened = useCallback(
    (next: LibraryInfo) => {
      setView({ kind: "library", library: next });
      setPlace({ kind: "all" });
      setStatus(undefined);
      setDialog(null);
      void refreshRecent();
    },
    [refreshRecent],
  );

  const openPath = useCallback(
    async (path: string) => {
      try {
        libraryOpened(await ipc.openLibrary(path));
      } catch (error) {
        setStatus(describeError(error));
        void refreshRecent();
      }
    },
    [libraryOpened, refreshRecent],
  );

  const savePreferences = useCallback(async (next: Preferences) => {
    setPreferences(next);
    if (inTauri()) setPreferences(await ipc.setPreferences(next));
  }, []);

  const changeLayout = useCallback(
    (layout: UiLayout, persist: boolean) => {
      const next = { ...preferences, layout };
      setPreferences(next);
      if (persist && inTauri()) {
        ipc.setPreferences(next).catch((error: unknown) => setStatus(describeError(error)));
      }
    },
    [preferences],
  );

  const changeBrowse = useCallback(
    (next: BrowsePreferences, persist: boolean) => {
      const updated = { ...preferences, browse: next };
      setPreferences(updated);
      if (persist && inTauri()) {
        ipc.setPreferences(updated).catch((error: unknown) => setStatus(describeError(error)));
      }
    },
    [preferences],
  );

  const openPreview = useCallback((index: number, id: string | null) => {
    flushSync(() => setTransitionId(id));
    withTransition(() => setPreviewIndex(index));
  }, []);

  const closePreview = useCallback((id: string | null) => {
    withTransition(() => {
      setTransitionId(id);
      setPreviewIndex(null);
    });
  }, []);

  const focusDetails = useCallback(() => {
    if (!preferences.layout.rightOpen) {
      changeLayout({ ...preferences.layout, rightOpen: true }, true);
    }
    requestAnimationFrame(() => detailsRef.current?.focus());
  }, [preferences.layout, changeLayout]);

  /** Runs `action` now, or after the user agrees to stop the files being added. */
  const afterImports = useCallback(
    (action: () => void) => {
      if (!addingFiles) {
        action();
        return;
      }
      setConfirmation({
        title: "Files are still being added",
        description: "Stop adding them? Files added so far stay in the library.",
        confirm: "Stop adding",
        cancel: "Keep adding",
        onConfirm: action,
      });
    },
    [addingFiles],
  );

  // Files added while a folder is open go into it (docs/spec.md §4).
  const intoFolder = openFolder?.id ?? null;
  const addPaths = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) return;
      ipc.addFiles(paths, intoFolder).catch((error: unknown) => setStatus(describeError(error)));
    },
    [intoFolder],
  );

  const failed = useCallback((error: unknown) => setStatus(describeError(error)), []);

  /** Runs a folder write; its answer is the new tree. */
  const writeFolders = useCallback(
    (write: Promise<Folder[]>) =>
      write
        .then((next) => {
          client.setQueryData(foldersKey, next);
          return next;
        })
        .catch((error: unknown) => {
          failed(error);
          return null;
        }),
    [client, foldersKey, failed],
  );

  const saveFolder = useCallback(
    (change: FolderChange) => writeFolders(ipc.saveFolder(change)),
    [writeFolders],
  );

  const createFolder = useCallback(
    (name: string, parent: string | null) => {
      const before = folderIds(tree);
      void saveFolder({ type: "create", name, parent, description: "", iconColor: null }).then(
        (next) => {
          if (!next) return;
          setFolderDialog(null);
          if (parent) setExpanded(parent, true);
          // The new folder opens, ready for files.
          const created = [...folderIds(next)].find((id) => !before.has(id));
          if (created) setPlace({ kind: "folder", id: created });
        },
      );
    },
    [tree, saveFolder, setExpanded],
  );

  const changeFolder = useCallback(
    (id: string, change: Partial<Extract<FolderChange, { type: "update" }>>) => {
      const folder = findFolder(tree, id);
      if (folder) void saveFolder(folderUpdate(folder, change));
    },
    [tree, saveFolder],
  );

  /** Moves a folder to position `index` among the children of `parent`. */
  const moveFolder = useCallback(
    (id: string, parent: string | null, index?: number) => {
      const siblings = (parent ? (findFolder(tree, parent)?.children ?? []) : tree).filter(
        (folder) => folder.id !== id,
      );
      void saveFolder({ type: "move", id, parent, index: index ?? siblings.length }).then(
        (next) => {
          if (next && parent) setExpanded(parent, true);
        },
      );
    },
    [tree, saveFolder, setExpanded],
  );

  const deleteFolder = useCallback(
    (id: string, trashItems: boolean) => {
      void writeFolders(ipc.deleteFolder(id, trashItems)).then((next) => {
        if (next) setFolderDialog(null);
      });
    },
    [writeFolders],
  );

  /** The selected items as an edit target; Select All stays a flag (docs/architecture.md §8). */
  const selectedTarget = useCallback(
    (): ItemsTarget => ({
      viewId: opened.view?.viewId ?? null,
      all: selection.all,
      ids: [...selection.ids],
    }),
    [opened.view, selection],
  );

  const editItems = useCallback(
    (target: ItemsTarget, edit: ItemEdit) => {
      ipc.applyBatchEdit(target, edit).catch(failed);
    },
    [failed],
  );

  const changeSort = useCallback(
    (next: SortKey, nextAscending: boolean) => {
      // A folder with its own sort keeps it; another pick holds for this screen only.
      if (openFolder?.orderBy) {
        setScreenSort({ place: key, sort: next, ascending: nextAscending });
        return;
      }
      // Manual means nothing outside the folder, so it becomes the folder's own sort, first
      // to last.
      if (openFolder && next === "manual") {
        changeFolder(openFolder.id, { orderBy: next, sortAscending: true });
        return;
      }
      setScreenSort(null);
      changeBrowse({ ...browse, sort: next, ascending: nextAscending }, true);
    },
    [openFolder, key, changeFolder, changeBrowse, browse],
  );

  /** A folder row was chosen: open it, or ask for its password when it is locked. */
  const openOrUnlock = useCallback(
    (id: string) => {
      const folder = findFolder(tree, id);
      if (folder?.isLocked && !folder.isUnlocked) setLockDialog({ kind: "unlock", id });
      else setPlace({ kind: "folder", id });
    },
    [tree],
  );

  const relock = useCallback(
    (ids: string[]) => {
      setSelection(NOTHING);
      Promise.all(ids.map((id) => ipc.relockFolder(id)))
        .then(() => client.invalidateQueries({ queryKey: ["folders"] }))
        .catch(failed);
    },
    [client, failed],
  );

  const lockAction = useCallback(
    (id: string, action: LockAction) => {
      if (action === "relock") relock([id]);
      else setLockDialog({ kind: action, id });
    },
    [relock],
  );

  const canDrop = useCallback(
    (payload: DragPayload, target: DropTarget) => {
      if (writable !== true) return false;
      if (payload.kind === "folder") {
        if (target.kind === "root") return true;
        if (target.kind !== "folder" || target.id === payload.id) return false;
        const parent = target.zone === "inside" ? target.id : (parentOf(tree, target.id) ?? null);
        const into = parent ? findFolder(tree, parent) : undefined;
        if (into?.isLocked && !into.isUnlocked) return false;
        return canMove(tree, payload.id, parent);
      }
      if (target.kind === "folder") {
        // A locked folder takes items only while it is unlocked.
        const folder = findFolder(tree, target.id);
        return !folder?.isLocked || folder.isUnlocked;
      }
      if (target.kind === "cell") {
        return reorderable && !payload.target.all && !payload.target.ids.includes(target.id);
      }
      return false;
    },
    [writable, tree, reorderable],
  );

  const onDrop = useCallback(
    (payload: DragPayload, target: DropTarget) => {
      if (payload.kind === "folder") {
        if (target.kind === "root") moveFolder(payload.id, null);
        else if (target.kind === "folder" && target.zone === "inside") {
          moveFolder(payload.id, target.id);
        } else if (target.kind === "folder") {
          const parent = parentOf(tree, target.id) ?? null;
          const siblings = siblingsOf(tree, target.id).filter((folder) => folder.id !== payload.id);
          const at = siblings.findIndex((folder) => folder.id === target.id);
          moveFolder(payload.id, parent, at + (target.zone === "after" ? 1 : 0));
        }
        return;
      }
      if (target.kind === "folder") {
        editItems(payload.target, { type: "addToFolders", value: [target.id] });
      } else if (target.kind === "cell" && openFolder && opened.view) {
        const viewId = opened.view.viewId;
        const folder = openFolder.id;
        const before =
          target.side === "before"
            ? Promise.resolve<string | null>(target.id)
            : target.index + 1 < total
              ? fetchCell(client, viewId, target.index + 1).then((cell) => cell?.id ?? null)
              : Promise.resolve(null);
        void before.then((id) => ipc.reorderItems(folder, payload.target.ids, id)).catch(failed);
      }
    },
    [tree, moveFolder, editItems, openFolder, opened.view, total, client, failed],
  );

  const actions = useMemo<CommandActions>(
    () => ({
      newLibrary: () => afterImports(() => setDialog("create")),
      openLibrary: () =>
        afterImports(() => {
          pickFolder("Open library")
            .then((path) => (path ? openPath(path) : undefined))
            .catch((error: unknown) => setStatus(describeError(error)));
        }),
      openRecent: (path) => afterImports(() => void openPath(path)),
      closeLibrary: () =>
        afterImports(() => {
          ipc
            .closeLibrary()
            .then(() => {
              setView({ kind: "welcome" });
              setStatus(undefined);
              return refreshRecent();
            })
            .catch((error: unknown) => setStatus(describeError(error)));
        }),
      addFiles: () => {
        pickFiles()
          .then(addPaths)
          .catch((error: unknown) => setStatus(describeError(error)));
      },
      paste: () => {
        ipc
          .pasteFiles(intoFolder)
          .then((ticket) => {
            if (ticket === null) setStatus("Nothing to paste: copy an image or files first.");
          })
          .catch((error: unknown) => setStatus(describeError(error)));
      },
      settings: () => {
        setDialog("settings");
        if (inTauri() && builtInLocation === "") {
          ipc
            .defaultLibraryLocation()
            .then(setBuiltInLocation)
            .catch(() => {});
        }
      },
      exit: () => void closeWindow().catch(() => {}),
      setTheme: (theme) => {
        savePreferences({ ...preferences, theme }).catch((error: unknown) =>
          setStatus(describeError(error)),
        );
      },
      toggleFullscreen: () => {
        const next = !fullscreen;
        setFullscreenState(next);
        void setFullscreen(next).catch(() => {});
      },
      toggleSidebar: () =>
        changeLayout({ ...preferences.layout, leftOpen: !preferences.layout.leftOpen }, true),
      toggleDetails: () =>
        changeLayout({ ...preferences.layout, rightOpen: !preferences.layout.rightOpen }, true),
      shortcuts: () => setDialog("shortcuts"),
      about: () => setDialog("about"),
      selectAll: () => setSelection((current) => everything(current)),
      preview: () => openPreview(selection.focus ?? 0, singleId(selection)),
      setViewMode: (viewMode) => changeBrowse({ ...browse, viewMode }, true),
      zoom: (step) => {
        const size = Math.min(
          THUMBNAIL_SIZES.max,
          Math.max(THUMBNAIL_SIZES.min, browse.thumbnailSize + step * ZOOM_STEP),
        );
        changeBrowse({ ...browse, thumbnailSize: size }, true);
      },
      toggleNames: () => changeBrowse({ ...browse, showNames: !browse.showNames }, true),
      setSort: (next) => changeSort(next, ascending),
      setAscending: (next) => changeSort(sort, next),
      newFolder: () => setFolderDialog({ kind: "create", parent: null }),
      toggleSubfolders: () =>
        changeBrowse({ ...browse, includeSubfolders: !browse.includeSubfolders }, true),
      lockAll: () =>
        relock(
          lockedFolders(tree)
            .filter((folder) => folder.isUnlocked)
            .map((folder) => folder.id),
        ),
    }),
    [
      browse,
      changeBrowse,
      changeSort,
      sort,
      ascending,
      intoFolder,
      openPreview,
      selection,
      afterImports,
      addPaths,
      openPath,
      refreshRecent,
      savePreferences,
      changeLayout,
      preferences,
      fullscreen,
      builtInLocation,
      relock,
      tree,
    ],
  );

  const commandState = useMemo<CommandState>(
    () => ({
      library,
      recent,
      theme: preferences.theme,
      fullscreen,
      leftOpen: preferences.layout.leftOpen,
      rightOpen: preferences.layout.rightOpen,
      browse,
      sort,
      ascending,
      folderOpen: openFolder !== undefined,
      unlockedFolders: lockedFolders(tree).filter((folder) => folder.isUnlocked).length,
      items: total,
      selected: selectedCount(selection, total),
    }),
    [
      library,
      recent,
      preferences,
      fullscreen,
      browse,
      sort,
      ascending,
      openFolder,
      total,
      selection,
      tree,
    ],
  );

  const modalOpen =
    dialog !== null ||
    folderDialog !== null ||
    lockDialog !== null ||
    importsOpen ||
    confirmation !== null ||
    imports.waiting !== undefined ||
    previewIndex !== null;
  useShortcuts(commandState, actions, !modalOpen);

  // Files dragged onto the window: an overlay, then an import (paths only, D-029).
  const canAdd = needsWritableLibrary(commandState);
  useEffect(() => {
    if (!inTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") setDropping(true);
        else if (payload.type === "leave") setDropping(false);
        else if (payload.type === "drop") {
          setDropping(false);
          if (library && canAdd === true) addPaths(payload.paths);
          else if (library) setStatus(canAdd === true ? undefined : canAdd);
        }
      })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [library, canAdd, addPaths]);

  // The window's close was held back because files are being added.
  useEffect(() => {
    if (!inTauri()) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen(EVENTS.closeRequested, () =>
      setConfirmation({
        title: "Files are still being added",
        description: "Stop adding them and quit? Files added so far stay in the library.",
        confirm: "Stop and quit",
        cancel: "Keep adding",
        onConfirm: () => void ipc.quit().catch(() => {}),
      }),
    )
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const latestImport = imports.running[0] ?? imports.imports[0];

  const loadDefaultLocation = useCallback(() => ipc.defaultLibraryLocation(), []);
  const browseFolders = useCallback(
    (current: string) => pickFolder("Choose a folder", current || undefined),
    [],
  );
  const closeDialog = (open: boolean) => {
    if (!open) setDialog(null);
  };

  const folderTitle = openFolder
    ? folderPath(tree, openFolder.id)
        .map((folder) => folder.name)
        .join(" / ")
    : undefined;

  return (
    <DragProvider canDrop={canDrop} onDrop={onDrop}>
      <div className="flex h-screen flex-col bg-background text-foreground">
        <AppMenu state={commandState} actions={actions} />
        {view.kind === "library" ? (
          <LibraryScreen
            library={view.library}
            layout={preferences.layout}
            onLayoutChange={changeLayout}
            onOpenForEditing={() => {
              if (view.library.path) void openPath(view.library.path);
            }}
            status={status}
            onDismissStatus={() => setStatus(undefined)}
            counts={counts}
            canAdd={canAdd}
            onAddFiles={actions.addFiles}
            importStatus={latestImport ? importSummary(latestImport) : undefined}
            onOpenImports={() => setImportsOpen(true)}
            dropping={dropping}
            place={place}
            onPlaceChange={setPlace}
            title={folderTitle}
            dropLabel={openFolder ? `“${openFolder.name}”` : "the library"}
            sort={sort}
            ascending={ascending}
            onSortChange={changeSort}
            folderTree={
              <FolderTree
                folders={tree}
                selectedId={openFolder?.id ?? null}
                includeSubfolders={browse.includeSubfolders}
                writable={writable}
                expanded={expanded}
                onExpandedChange={setExpanded}
                onSelect={openOrUnlock}
                onIncludeSubfoldersChange={actions.toggleSubfolders}
                onCreate={(parent) => setFolderDialog({ kind: "create", parent })}
                onRename={(id, name) => changeFolder(id, { name: name.trim() })}
                onColor={(id, iconColor) => changeFolder(id, { iconColor })}
                onMoveTo={(id) => setFolderDialog({ kind: "move", id })}
                onDelete={(id) => setFolderDialog({ kind: "delete", id })}
                onLock={lockAction}
              />
            }
            lockedFolders={
              <LockedFolders
                folders={tree}
                selectedId={openFolder?.id ?? null}
                onUnlock={(id) => setLockDialog({ kind: "unlock", id })}
                onOpen={(id) => setPlace({ kind: "folder", id })}
              />
            }
            browse={browse}
            onBrowseChange={changeBrowse}
            itemCount={opened.view?.total}
            selectedCount={selectedCount(selection, total)}
            viewStatus={
              opened.error ??
              (opened.view === null && opened.progress
                ? `Loading ${opened.progress.loaded.toLocaleString("en")} of ${opened.progress.total.toLocaleString("en")}…`
                : undefined)
            }
            content={
              opened.view && (
                <ItemsView
                  viewId={opened.view.viewId}
                  total={opened.view.total}
                  mode={browse.viewMode}
                  cellSize={browse.thumbnailSize}
                  showNames={browse.showNames}
                  selection={selection}
                  onSelectionChange={setSelection}
                  onOpen={openPreview}
                  onEnter={focusDetails}
                  onZoom={actions.zoom}
                  onLost={opened.reopen}
                  transitionId={previewIndex === null ? transitionId : null}
                  draggable={writable === true}
                  reorderable={reorderable}
                />
              )
            }
            details={
              <DetailsPanel
                ref={detailsRef}
                count={selectedCount(selection, total)}
                itemId={singleId(selection)}
                onPreview={actions.preview}
                folders={tree}
                writable={writable}
                onFolderChange={(folder, member) => {
                  const id = singleId(selection);
                  if (!id) return;
                  ipc
                    .applyItemEdit(id, {
                      type: member ? "addToFolders" : "removeFromFolders",
                      value: [folder],
                    })
                    .then((item) => client.setQueryData(["item", id], item))
                    .catch(failed);
                }}
                onAddToFolder={(folder) =>
                  editItems(selectedTarget(), { type: "addToFolders", value: [folder] })
                }
                empty={
                  openFolder && (
                    <FolderProperties
                      folder={openFolder}
                      writable={writable}
                      includeSubfolders={browse.includeSubfolders}
                      onLockLabel={(label, color) => {
                        ipc
                          .setLockLabel(openFolder.id, label, color)
                          .then(() => client.invalidateQueries({ queryKey: ["folders"] }))
                          .catch(failed);
                      }}
                      onSave={(change) => {
                        // A new own sort shows at once, over a sort picked on this screen.
                        if (
                          change.orderBy !== openFolder.orderBy ||
                          change.sortAscending !== openFolder.sortAscending
                        ) {
                          setScreenSort(null);
                        }
                        void saveFolder(change);
                      }}
                    />
                  )
                }
              />
            }
          />
        ) : (
          <>
            <main className="flex flex-1 items-center justify-center overflow-y-auto p-6">
              {view.kind === "welcome" ? (
                <WelcomeScreen
                  status={status}
                  recent={recent}
                  onCreateLibrary={actions.newLibrary}
                  onOpenLibrary={actions.openLibrary}
                  onOpenRecent={(path) => void openPath(path)}
                  onForgetRecent={(path) => {
                    ipc
                      .forgetRecentLibrary(path)
                      .then(refreshRecent)
                      .catch((error: unknown) => setStatus(describeError(error)));
                  }}
                  // The Eagle import arrives in phase 8 (docs/spec.md §9).
                  onImportEagleLibrary={() =>
                    setStatus("Importing an Eagle library is not available yet.")
                  }
                  onConnectEagle={() => {
                    setStatus(undefined);
                    setView({ kind: "connect" });
                  }}
                />
              ) : (
                <ConnectScreen
                  key={`${defaults.host}:${defaults.port}`}
                  defaults={defaults}
                  status={status}
                  // Phase 9 connects for real (docs/spec.md §9).
                  onConnect={() => setStatus("Connecting is not available yet.")}
                  onBack={() => {
                    setStatus(undefined);
                    setView({ kind: "welcome" });
                  }}
                />
              )}
            </main>
            <footer className="p-3 text-center text-xs text-muted-foreground">
              {info ? `${info.name} ${info.version}` : "egl-viewer"}
            </footer>
          </>
        )}

        <CreateLibraryDialog
          open={dialog === "create"}
          onOpenChange={closeDialog}
          loadDefaultLocation={loadDefaultLocation}
          browse={browseFolders}
          create={ipc.createLibrary}
          onCreated={libraryOpened}
        />
        <SettingsDialog
          open={dialog === "settings"}
          onOpenChange={closeDialog}
          preferences={preferences}
          builtInLocation={builtInLocation}
          browse={browseFolders}
          save={savePreferences}
        />
        <AboutDialog open={dialog === "about"} onOpenChange={closeDialog} info={info} />
        <ShortcutsDialog open={dialog === "shortcuts"} onOpenChange={closeDialog} />
        <ImportsPanel
          open={importsOpen}
          onOpenChange={setImportsOpen}
          imports={imports.imports}
          onCancel={(id) => void ipc.cancelImport(id).catch(() => {})}
        />
        <LockDialogs
          dialog={lockDialog}
          folders={tree}
          onClose={() => setLockDialog(null)}
          onChanged={(opened) => {
            // Items appeared or went: the selection may name hidden ones.
            setSelection(NOTHING);
            setRevision((count) => count + 1);
            // The folder opens once the tree says it is unlocked, or the screen would bounce
            // back to All.
            void client.invalidateQueries({ queryKey: ["folders"] }).then(() => {
              if (opened) setPlace({ kind: "folder", id: opened });
            });
          }}
        />
        <FolderDialogs
          dialog={folderDialog}
          folders={tree}
          onClose={() => setFolderDialog(null)}
          onCreate={createFolder}
          onMove={(id, parent) => {
            setFolderDialog(null);
            moveFolder(id, parent);
          }}
          onDelete={deleteFolder}
        />
        <DuplicateDialog
          waiting={imports.waiting}
          folders={tree}
          onAnswer={(id, answer) =>
            void ipc
              .answerImport(id, answer)
              .catch((error: unknown) => setStatus(describeError(error)))
          }
        />
        <ConfirmDialog confirmation={confirmation} onClose={() => setConfirmation(null)} />
        {opened.view && (
          <Preview
            viewId={opened.view.viewId}
            total={opened.view.total}
            index={previewIndex}
            onIndexChange={setPreviewIndex}
            onClose={closePreview}
            onLost={opened.reopen}
          />
        )}
      </div>
    </DragProvider>
  );
}
