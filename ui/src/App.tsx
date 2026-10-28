import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useState } from "react";
import { type CommandActions, type CommandState, needsWritableLibrary } from "@/commands/registry";
import { useShortcuts } from "@/commands/useShortcuts";
import { AboutDialog } from "@/features/about/AboutDialog";
import { ConnectScreen } from "@/features/connect/ConnectScreen";
import { DuplicateDialog } from "@/features/imports/DuplicateDialog";
import { ImportsPanel } from "@/features/imports/ImportsPanel";
import { importSummary } from "@/features/imports/importText";
import { useImports } from "@/features/imports/useImports";
import { LibraryScreen } from "@/features/library/LibraryScreen";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { AppMenu } from "@/features/shell/AppMenu";
import { type Confirmation, ConfirmDialog } from "@/features/shell/ConfirmDialog";
import { ShortcutsDialog } from "@/features/shortcuts/ShortcutsDialog";
import { CreateLibraryDialog } from "@/features/welcome/CreateLibraryDialog";
import { WelcomeScreen } from "@/features/welcome/WelcomeScreen";
import {
  type AppInfo,
  type ConnectionSettings,
  DEFAULT_PREFERENCES,
  EVENTS,
  inTauri,
  ipc,
  type LibraryCounts,
  type LibraryInfo,
  type Preferences,
  type RecentLibrary,
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

export function App() {
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

  useEffect(() => installDesktopBehaviour(), []);

  useEffect(() => {
    applyTheme(preferences.theme);
    void setWindowTheme(preferences.theme).catch(() => {});
    if (preferences.theme !== "system") return;
    return watchSystemTheme(() => applyTheme("system"));
  }, [preferences.theme]);

  const library = view.kind === "library" ? view.library : null;
  const imports = useImports(library?.path ?? null);
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

  const opened = useCallback(
    (next: LibraryInfo) => {
      setView({ kind: "library", library: next });
      setStatus(undefined);
      setDialog(null);
      void refreshRecent();
    },
    [refreshRecent],
  );

  const openPath = useCallback(
    async (path: string) => {
      try {
        opened(await ipc.openLibrary(path));
      } catch (error) {
        setStatus(describeError(error));
        void refreshRecent();
      }
    },
    [opened, refreshRecent],
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
    }),
    [
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
    }),
    [library, recent, preferences, fullscreen],
  );

  const modalOpen =
    dialog !== null || importsOpen || confirmation !== null || imports.waiting !== undefined;
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
  const browse = useCallback(
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
        browse={browse}
        create={ipc.createLibrary}
        onCreated={opened}
      />
      <SettingsDialog
        open={dialog === "settings"}
        onOpenChange={closeDialog}
        preferences={preferences}
        builtInLocation={builtInLocation}
        browse={browse}
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
    </div>
  );
}
