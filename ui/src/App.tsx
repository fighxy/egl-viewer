import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { CommandActions, CommandState } from "@/commands/registry";
import { useShortcuts } from "@/commands/useShortcuts";
import { AboutDialog } from "@/features/about/AboutDialog";
import { ConnectScreen } from "@/features/connect/ConnectScreen";
import { LibraryScreen } from "@/features/library/LibraryScreen";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { AppMenu } from "@/features/shell/AppMenu";
import { ShortcutsDialog } from "@/features/shortcuts/ShortcutsDialog";
import { CreateLibraryDialog } from "@/features/welcome/CreateLibraryDialog";
import { WelcomeScreen } from "@/features/welcome/WelcomeScreen";
import {
  type AppInfo,
  type ConnectionSettings,
  DEFAULT_PREFERENCES,
  inTauri,
  ipc,
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

  useEffect(() => installDesktopBehaviour(), []);

  useEffect(() => {
    applyTheme(preferences.theme);
    void setWindowTheme(preferences.theme).catch(() => {});
    if (preferences.theme !== "system") return;
    return watchSystemTheme(() => applyTheme("system"));
  }, [preferences.theme]);

  const library = view.kind === "library" ? view.library : null;
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

  const actions = useMemo<CommandActions>(
    () => ({
      newLibrary: () => setDialog("create"),
      openLibrary: () => {
        pickFolder("Open library")
          .then((path) => (path ? openPath(path) : undefined))
          .catch((error: unknown) => setStatus(describeError(error)));
      },
      openRecent: (path) => void openPath(path),
      closeLibrary: () => {
        ipc
          .closeLibrary()
          .then(() => {
            setView({ kind: "welcome" });
            setStatus(undefined);
            return refreshRecent();
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

  useShortcuts(commandState, actions, dialog === null);

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
    </div>
  );
}
