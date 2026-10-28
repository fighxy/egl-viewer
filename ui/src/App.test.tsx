import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DEFAULT_PREFERENCES } from "@/ipc";
import { called, LIBRARY, mockBackend } from "@/test/backend";
import { App } from "./App";

async function openConnect() {
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Connect to a running Eagle" }));
  return user;
}

const RECENT = [
  { path: LIBRARY.path, name: "Refs", libraryId: LIBRARY.id, lastOpenedAt: 1, missing: false },
];

describe("App outside Tauri", () => {
  it("starts on the Welcome screen with the menu bar", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Welcome to egl-viewer" })).toBeInTheDocument();
    const menu = screen.getByRole("menubar", { name: "Menu" });
    expect(menu).toHaveTextContent("File");
    expect(menu).toHaveTextContent("View");
    expect(menu).toHaveTextContent("Help");
    expect(menu).not.toHaveTextContent("Edit");
    expect(menu).not.toHaveTextContent("Library");
    expect(screen.getByText("egl-viewer")).toBeInTheDocument();
  });

  it("opens the Connect screen with fallback defaults", async () => {
    render(<App />);
    await openConnect();
    expect(screen.getByText("Connect to Eagle")).toBeInTheDocument();
    expect(screen.getByLabelText("Host")).toHaveValue("127.0.0.1");
  });

  it("goes back to Welcome and clears the last message", async () => {
    render(<App />);
    const user = await openConnect();
    await user.click(screen.getByRole("button", { name: "Connect" }));
    expect(screen.getByRole("status")).toHaveTextContent("Connecting is not available yet.");
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("heading", { name: "Welcome to egl-viewer" })).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});

describe("App inside Tauri", () => {
  it("takes the version and the Eagle defaults from Rust", async () => {
    mockBackend({ connection_defaults: () => ({ host: "10.0.0.5", port: 41000 }) });
    render(<App />);
    expect(await screen.findByText("egl-viewer 9.9.9")).toBeInTheDocument();
    await openConnect();
    expect(await screen.findByDisplayValue("10.0.0.5")).toBeInTheDocument();
  });

  it("reopens the last library on start", async () => {
    mockBackend({ startup: () => ({ library: LIBRARY, notice: null }) });
    render(<App />);
    expect(await screen.findByRole("navigation", { name: "Library" })).toHaveTextContent("Refs");
    await waitFor(() => expect(document.title).toBe("Refs — egl-viewer"));
  });

  it("says why the last library was not reopened", async () => {
    mockBackend({
      startup: () => ({ library: null, notice: '"Refs" is open in another egl-viewer window.' }),
    });
    render(<App />);
    expect(await screen.findByRole("status")).toHaveTextContent("open in another egl-viewer");
  });

  it("creates a library in the default folder and opens it", async () => {
    const calls = mockBackend();
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Create library" }));
    expect(await screen.findByDisplayValue("C:\\Users\\me\\egl-library")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Name"), "Moodboard");
    await user.click(screen.getByRole("button", { name: "Create" }));
    expect(await screen.findByRole("navigation", { name: "Library" })).toHaveTextContent(
      "Moodboard",
    );
    expect(called(calls, "create_library")[0]?.args).toEqual({
      parent: "C:\\Users\\me\\egl-library",
      name: "Moodboard",
    });
  });

  it("opens a library picked in the folder dialog", async () => {
    const calls = mockBackend();
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open library" }));
    expect(await screen.findByRole("navigation", { name: "Library" })).toBeInTheDocument();
    expect(called(calls, "open_library")[0]?.args).toEqual({
      path: "C:\\Users\\me\\Libraries\\Refs.eglib",
    });
  });

  it("explains a folder that is not a library", async () => {
    mockBackend({
      open_library: () => {
        throw { kind: "notALibrary", detail: { eagle: true } };
      },
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open library" }));
    expect(await screen.findByRole("status")).toHaveTextContent("This is an Eagle library.");
    expect(screen.getByRole("heading", { name: "Welcome to egl-viewer" })).toBeInTheDocument();
  });

  it("opens and forgets recent libraries", async () => {
    const calls = mockBackend({ recent_libraries: () => RECENT });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: /^Refs/ }));
    expect(await screen.findByRole("navigation", { name: "Library" })).toBeInTheDocument();
    expect(called(calls, "open_library")[0]?.args).toEqual({ path: LIBRARY.path });
  });

  it("forgets a recent library", async () => {
    const calls = mockBackend({ recent_libraries: () => RECENT });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Remove Refs from the list" }));
    await waitFor(() =>
      expect(called(calls, "forget_recent_library")[0]?.args).toEqual({ path: LIBRARY.path }),
    );
  });

  it("closes the library with Ctrl+W and returns to Welcome", async () => {
    const calls = mockBackend({ startup: () => ({ library: LIBRARY, notice: null }) });
    render(<App />);
    await screen.findByRole("navigation", { name: "Library" });
    fireEvent.keyDown(window, { key: "w", ctrlKey: true });
    expect(
      await screen.findByRole("heading", { name: "Welcome to egl-viewer" }),
    ).toBeInTheDocument();
    expect(called(calls, "close_library")).toHaveLength(1);
  });

  it("retries the lock with Open for editing", async () => {
    const readOnly = { ...LIBRARY, readOnly: true, readOnlyReason: "otherInstance" as const };
    const calls = mockBackend({ startup: () => ({ library: readOnly, notice: null }) });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Open for editing" }));
    await waitFor(() =>
      expect(screen.queryByRole("status", { name: "Read-only library" })).not.toBeInTheDocument(),
    );
    expect(called(calls, "open_library")[0]?.args).toEqual({ path: LIBRARY.path });
  });

  it("saves settings opened with Ctrl+, and applies the theme", async () => {
    const calls = mockBackend();
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("egl-viewer 9.9.9");
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    await user.click(await screen.findByRole("radio", { name: "Dark" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(document.documentElement).toHaveClass("dark"));
    expect(called(calls, "set_preferences").at(-1)?.args).toEqual({
      preferences: { ...DEFAULT_PREFERENCES, theme: "dark" },
    });
  });

  it("shows the keyboard shortcuts with F1 and the version in About", async () => {
    mockBackend();
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("egl-viewer 9.9.9");
    fireEvent.keyDown(window, { key: "F1" });
    const shortcuts = await screen.findByRole("dialog", { name: "Keyboard shortcuts" });
    expect(shortcuts).toHaveTextContent("Open Library");
    expect(shortcuts).toHaveTextContent("Ctrl+O");
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("menuitem", { name: "Help" }));
    await user.click(await screen.findByRole("menuitem", { name: "About egl-viewer" }));
    const about = await screen.findByRole("dialog", { name: "About egl-viewer" });
    expect(about).toHaveTextContent("9.9.9");
    expect(about).toHaveTextContent("abc1234");
  });

  it("does not run shortcuts for commands that need a library", async () => {
    const calls = mockBackend();
    render(<App />);
    await screen.findByText("egl-viewer 9.9.9");
    fireEvent.keyDown(window, { key: "w", ctrlKey: true });
    expect(called(calls, "close_library")).toHaveLength(0);
  });
});

describe("App without a library on start", () => {
  it("shows the window after the first paint", async () => {
    const shown: string[] = [];
    mockBackend();
    mockIPC((cmd) => {
      shown.push(cmd);
      if (cmd === "get_preferences") return DEFAULT_PREFERENCES;
      if (cmd === "startup") return { library: null, notice: null };
      if (cmd === "recent_libraries") return [];
      if (cmd === "app_info")
        return { name: "egl-viewer", version: "1", commit: null, webview: null };
      if (cmd === "connection_defaults") return { host: "127.0.0.1", port: 41595 };
      return null;
    });
    render(<App />);
    await waitFor(() => expect(shown).toContain("plugin:window|show"));
  });
});
