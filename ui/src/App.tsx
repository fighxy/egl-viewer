import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { type CommandActions, type CommandState, needsWritableLibrary } from "@/commands/registry";
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
import { useView } from "@/features/browse/useView";
import { ConnectScreen } from "@/features/connect/ConnectScreen";
import { DuplicateDialog } from "@/features/imports/DuplicateDialog";
import { ImportsPanel } from "@/features/imports/ImportsPanel";
import { importSummary } from "@/features/imports/importText";
import { useImports } from "@/features/imports/useImports";
import { LibraryScreen, type ScopeId, scopeOf } from "@/features/library/LibraryScreen";
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
  inTauri,
  ipc,
  type LibraryCounts,
  type LibraryInfo,
  type Preferences,
  queryFor,
  type RecentLibrary,
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
  const [scope, setScope] = useState<ScopeId>("all");
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

  const browse = preferences.browse;
  const query = useMemo(
    () => (library ? queryFor(scopeOf(scope), browse) : null),
    [library, scope, browse],
  );
  const opened = useView(query);
  const total = opened.view?.total ?? 0;

  // A new library or scope starts with nothing selected; a new sort keeps the selected items.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset when the screen changes.
  useEffect(() => {
    setSelection(NOTHING);
    setPreviewIndex(null);
  }, [library?.path, scope]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: positions change with the order.
  useEffect(() => {
    setSelection((current) => ({ ...current, anchor: null, focus: null }));
  }, [browse.sort, browse.ascending]);
  const addingFiles = imports.running.some((progress) => progress.kind === "files");

  // Counts follow the added files, at most every 300 ms.
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
  }, [library, imports.added]);

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
      setScope("all");
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

  const addPaths = useCallback((paths: string[]) => {
    if (paths.length === 0) return;
    ipc.addFiles(paths).catch((error: unknown) => setStatus(describeError(error)));
  }, []);

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
          .pasteFiles()
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
      setSort: (sort) => changeBrowse({ ...browse, sort }, true),
      setAscending: (ascending) => changeBrowse({ ...browse, ascending }, true),
    }),
    [
      browse,
      changeBrowse,
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
      items: total,
      selected: selectedCount(selection, total),
    }),
    [library, recent, preferences, fullscreen, browse, total, selection],
  );

  const modalOpen =
    dialog !== null ||
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

  return (
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
          scope={scope}
          onScopeChange={setScope}
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
              />
            )
          }
          details={
            <DetailsPanel
              ref={detailsRef}
              count={selectedCount(selection, total)}
              itemId={singleId(selection)}
              onPreview={actions.preview}
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
      <DuplicateDialog
        waiting={imports.waiting}
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
  );
}
