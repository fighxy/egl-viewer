// A fake Rust side for UI tests: every command the UI may call, overridable per test.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { DEFAULT_PREFERENCES, type LibraryInfo } from "@/ipc";

export const LIBRARY: LibraryInfo = {
  kind: "native",
  id: "01JLIB",
  name: "Refs",
  path: "C:\\Users\\me\\egl-library\\Refs.eglib",
  modificationTime: null,
  readOnly: false,
  readOnlyReason: null,
  holder: null,
};

type Handler = (args: Record<string, unknown>) => unknown;
export type Call = { cmd: string; args: Record<string, unknown> };

export function mockBackend(overrides: Record<string, Handler> = {}): Call[] {
  const calls: Call[] = [];
  const handlers: Record<string, Handler> = {
    app_info: () => ({ name: "egl-viewer", version: "9.9.9", commit: "abc1234", webview: "141.0" }),
    connection_defaults: () => ({ host: "127.0.0.1", port: 41595 }),
    get_preferences: () => DEFAULT_PREFERENCES,
    set_preferences: (args) => args.preferences,
    startup: () => ({ library: null, notice: null }),
    recent_libraries: () => [],
    forget_recent_library: () => null,
    default_library_location: () => "C:\\Users\\me\\egl-library",
    create_library: (args) => ({ ...LIBRARY, name: String(args.name) }),
    open_library: () => LIBRARY,
    close_library: () => null,
    "plugin:dialog|open": () => "C:\\Users\\me\\Libraries\\Refs.eglib",
    ...overrides,
  };
  mockWindows("main");
  mockIPC((cmd, args) => {
    const payload = (args ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args: payload });
    if (cmd.startsWith("plugin:window|")) return null;
    const handler = handlers[cmd];
    if (!handler) throw new Error(`unexpected command ${cmd}`);
    return handler(payload);
  });
  return calls;
}

export function called(calls: Call[], cmd: string): Call[] {
  return calls.filter((call) => call.cmd === cmd);
}
