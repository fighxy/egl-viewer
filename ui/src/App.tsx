import { useEffect, useState } from "react";
import { ConnectScreen } from "@/features/connect/ConnectScreen";
import { WelcomeScreen } from "@/features/welcome/WelcomeScreen";
import { type AppInfo, type ConnectionSettings, inTauri, ipc } from "@/ipc";

/** Used until Rust answers, and in a plain browser without Tauri. */
export const FALLBACK_DEFAULTS: ConnectionSettings = { host: "127.0.0.1", port: 41595 };

type Screen = "welcome" | "connect";

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [defaults, setDefaults] = useState<ConnectionSettings>(FALLBACK_DEFAULTS);
  const [screen, setScreen] = useState<Screen>("welcome");
  const [status, setStatus] = useState<string>();

  useEffect(() => {
    if (!inTauri()) {
      return;
    }
    ipc
      .appInfo()
      .then(setInfo)
      .catch(() => setInfo(null));
    ipc
      .connectionDefaults()
      .then(setDefaults)
      .catch(() => setDefaults(FALLBACK_DEFAULTS));
  }, []);

  const show = (next: Screen) => {
    setStatus(undefined);
    setScreen(next);
  };

  return (
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <main className="flex flex-1 items-center justify-center p-6">
        {screen === "welcome" ? (
          // Libraries arrive in phase 1, the Eagle import in phase 8 (docs/spec.md §9).
          <WelcomeScreen
            status={status}
            onCreateLibrary={() => setStatus("Creating a library is not available yet.")}
            onOpenLibrary={() => setStatus("Opening a library is not available yet.")}
            onImportEagleLibrary={() =>
              setStatus("Importing an Eagle library is not available yet.")
            }
            onConnectEagle={() => show("connect")}
          />
        ) : (
          <ConnectScreen
            key={`${defaults.host}:${defaults.port}`}
            defaults={defaults}
            status={status}
            // Phase 9 connects for real (docs/spec.md §9).
            onConnect={() => setStatus("Connecting is not available yet.")}
            onBack={() => show("welcome")}
          />
        )}
      </main>
      <footer className="p-3 text-center text-xs text-muted-foreground">
        {info ? `${info.name} ${info.version}` : "egl-viewer"}
      </footer>
    </div>
  );
}
