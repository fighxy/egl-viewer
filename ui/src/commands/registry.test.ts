import { describe, expect, it, vi } from "vitest";
import { DEFAULT_PREFERENCES } from "@/ipc";
import { LIBRARY } from "@/test/backend";
import {
  availability,
  COMMANDS,
  type CommandActions,
  type CommandState,
  commandForShortcut,
  visibleMenus,
} from "./registry";

const state = (change: Partial<CommandState> = {}): CommandState => ({
  library: null,
  recent: [],
  theme: "system",
  fullscreen: false,
  leftOpen: true,
  rightOpen: true,
  browse: DEFAULT_PREFERENCES.browse,
  sort: DEFAULT_PREFERENCES.browse.sort,
  ascending: DEFAULT_PREFERENCES.browse.ascending,
  folderOpen: false,
  unlockedFolders: 0,
  items: 0,
  selected: 0,
  ...change,
});

const command = (id: string) => {
  const found = COMMANDS.find((candidate) => candidate.id === id);
  if (!found) throw new Error(`no command ${id}`);
  return found;
};

describe("command registry", () => {
  it("shows only menus that have commands in this phase", () => {
    expect(visibleMenus().map((menu) => menu.label)).toEqual([
      "File",
      "Edit",
      "View",
      "Library",
      "Help",
    ]);
  });

  it("adds and pastes only into a library that can be written", () => {
    for (const id of ["file.addFiles", "edit.paste"]) {
      expect(availability(command(id), state())).toBe("Open a library first.");
      expect(availability(command(id), state({ library: { ...LIBRARY, readOnly: true } }))).toBe(
        "This library is read-only.",
      );
      expect(availability(command(id), state({ library: LIBRARY }))).toBe(true);
    }
    expect(commandForShortcut("Ctrl+V")?.id).toBe("edit.paste");
  });

  it("defines each shortcut once", () => {
    const shortcuts = COMMANDS.flatMap((candidate) => candidate.shortcut ?? []);
    expect(new Set(shortcuts).size).toBe(shortcuts.length);
  });

  it("finds commands by shortcut, except those Windows handles", () => {
    expect(commandForShortcut("Ctrl+O")?.id).toBe("file.openLibrary");
    expect(commandForShortcut("Ctrl+,")?.id).toBe("file.settings");
    expect(commandForShortcut("F11")?.id).toBe("view.fullScreen");
    expect(commandForShortcut("Alt+F4")).toBeUndefined();
    expect(commandForShortcut("Ctrl+Q")).toBeUndefined();
  });

  it("disables library commands without a library and says why", () => {
    for (const id of ["file.closeLibrary", "view.sidebar", "view.details"]) {
      expect(availability(command(id), state())).toBe("Open a library first.");
      expect(availability(command(id), state({ library: LIBRARY }))).toBe(true);
    }
  });

  it("lists recent libraries that are still there", () => {
    const recent = [
      { path: "C:\\A.eglib", name: "A", libraryId: null, lastOpenedAt: 1, missing: false },
      { path: "D:\\B.eglib", name: "B", libraryId: null, lastOpenedAt: 2, missing: true },
    ];
    const openRecent = command("file.openRecent");
    expect(availability(openRecent, state())).toBe("No recent libraries.");
    const items = openRecent.submenu?.(state({ recent })) ?? [];
    expect(items.map((item) => item.label)).toEqual(["A"]);
    const actions = { openRecent: vi.fn() } as unknown as CommandActions;
    items[0]?.run(actions);
    expect(actions.openRecent).toHaveBeenCalledWith("C:\\A.eglib");
  });

  it("offers the three themes with the current one checked", () => {
    const items = command("view.theme").submenu?.(state({ theme: "dark" })) ?? [];
    expect(items.map((item) => [item.label, item.checked])).toEqual([
      ["System", false],
      ["Light", false],
      ["Dark", true],
    ]);
  });
});
