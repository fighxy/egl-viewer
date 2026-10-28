// One registry of commands: the menu bar, shortcuts and the Keyboard Shortcuts dialog are built
// from it, so a shortcut is defined once. Commands of later phases are absent, not disabled
// (docs/spec.md §7.1, D-025).
import type { LibraryInfo, RecentLibrary, Theme } from "@/ipc";

export type MenuId = "file" | "edit" | "view" | "library" | "help";

export const MENUS: readonly { id: MenuId; label: string }[] = [
  { id: "file", label: "File" },
  { id: "edit", label: "Edit" },
  { id: "view", label: "View" },
  { id: "library", label: "Library" },
  { id: "help", label: "Help" },
];

export type CommandState = {
  library: LibraryInfo | null;
  recent: RecentLibrary[];
  theme: Theme;
  fullscreen: boolean;
  leftOpen: boolean;
  rightOpen: boolean;
};

export type CommandActions = {
  newLibrary: () => void;
  openLibrary: () => void;
  openRecent: (path: string) => void;
  closeLibrary: () => void;
  settings: () => void;
  exit: () => void;
  setTheme: (theme: Theme) => void;
  toggleFullscreen: () => void;
  toggleSidebar: () => void;
  toggleDetails: () => void;
  shortcuts: () => void;
  about: () => void;
};

/** `true`, or the reason the command is disabled (shown as a tooltip). */
export type Availability = true | string;

export type SubmenuItem = {
  id: string;
  label: string;
  title?: string;
  /** Set for choices such as the theme: the submenu shows radio items. */
  checked?: boolean;
  run: (actions: CommandActions) => void;
};

export type Command = {
  id: string;
  label: string;
  menu: MenuId;
  /** Items with different groups are separated by a line. */
  group: number;
  shortcut?: string;
  /** The shortcut is shown but handled by Windows (Alt+F4). */
  systemShortcut?: boolean;
  checked?: (state: CommandState) => boolean;
  available?: (state: CommandState) => Availability;
  submenu?: (state: CommandState) => SubmenuItem[];
  run?: (actions: CommandActions) => void;
};

const needsLibrary = (state: CommandState): Availability =>
  state.library ? true : "Open a library first.";

const THEMES: readonly { theme: Theme; label: string }[] = [
  { theme: "system", label: "System" },
  { theme: "light", label: "Light" },
  { theme: "dark", label: "Dark" },
];

export const COMMANDS: readonly Command[] = [
  {
    id: "file.newLibrary",
    label: "New Library…",
    menu: "file",
    group: 0,
    run: (actions) => actions.newLibrary(),
  },
  {
    id: "file.openLibrary",
    label: "Open Library…",
    menu: "file",
    group: 0,
    shortcut: "Ctrl+O",
    run: (actions) => actions.openLibrary(),
  },
  {
    id: "file.openRecent",
    label: "Open Recent",
    menu: "file",
    group: 0,
    available: (state) =>
      state.recent.some((library) => !library.missing) ? true : "No recent libraries.",
    submenu: (state) =>
      state.recent
        .filter((library) => !library.missing)
        .map((library) => ({
          id: library.path,
          label: library.name,
          title: library.path,
          run: (actions) => actions.openRecent(library.path),
        })),
  },
  {
    id: "file.closeLibrary",
    label: "Close Library",
    menu: "file",
    group: 0,
    shortcut: "Ctrl+W",
    available: needsLibrary,
    run: (actions) => actions.closeLibrary(),
  },
  {
    id: "file.settings",
    label: "Settings…",
    menu: "file",
    group: 1,
    shortcut: "Ctrl+,",
    run: (actions) => actions.settings(),
  },
  {
    id: "file.exit",
    label: "Exit",
    menu: "file",
    group: 2,
    shortcut: "Alt+F4",
    systemShortcut: true,
    run: (actions) => actions.exit(),
  },
  {
    id: "view.theme",
    label: "Theme",
    menu: "view",
    group: 0,
    submenu: (state) =>
      THEMES.map(({ theme, label }) => ({
        id: theme,
        label,
        checked: state.theme === theme,
        run: (actions) => actions.setTheme(theme),
      })),
  },
  {
    id: "view.fullScreen",
    label: "Full Screen",
    menu: "view",
    group: 0,
    shortcut: "F11",
    checked: (state) => state.fullscreen,
    run: (actions) => actions.toggleFullscreen(),
  },
  {
    id: "view.sidebar",
    label: "Sidebar",
    menu: "view",
    group: 1,
    shortcut: "Ctrl+B",
    checked: (state) => state.leftOpen,
    available: needsLibrary,
    run: (actions) => actions.toggleSidebar(),
  },
  {
    id: "view.details",
    label: "Details Panel",
    menu: "view",
    group: 1,
    shortcut: "Ctrl+I",
    checked: (state) => state.rightOpen,
    available: needsLibrary,
    run: (actions) => actions.toggleDetails(),
  },
  {
    id: "help.shortcuts",
    label: "Keyboard Shortcuts",
    menu: "help",
    group: 0,
    shortcut: "F1",
    run: (actions) => actions.shortcuts(),
  },
  {
    id: "help.about",
    label: "About egl-viewer",
    menu: "help",
    group: 0,
    run: (actions) => actions.about(),
  },
];

export function availability(command: Command, state: CommandState): Availability {
  return command.available ? command.available(state) : true;
}

/** The menus that have commands, in menu bar order; empty menus are hidden. */
export function visibleMenus(commands: readonly Command[] = COMMANDS) {
  return MENUS.map((menu) => ({
    ...menu,
    commands: commands.filter((command) => command.menu === menu.id),
  })).filter((menu) => menu.commands.length > 0);
}

/** The command bound to a key combination such as `Ctrl+O`, unless Windows handles it. */
export function commandForShortcut(
  shortcut: string,
  commands: readonly Command[] = COMMANDS,
): Command | undefined {
  return commands.find((command) => command.shortcut === shortcut && !command.systemShortcut);
}
