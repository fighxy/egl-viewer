// One registry of commands: the menu bar, shortcuts and the Keyboard Shortcuts dialog are built
// from it, so a shortcut is defined once. Commands of later phases are absent, not disabled
// (docs/spec.md §7.1, D-025).
import {
  type BrowsePreferences,
  type LibraryInfo,
  type RecentLibrary,
  type SortKey,
  THUMBNAIL_SIZES,
  type Theme,
  type ViewMode,
} from "@/ipc";

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
  browse: BrowsePreferences;
  /** Items in the open view. */
  items: number;
  /** Selected items in the open view. */
  selected: number;
};

export type CommandActions = {
  newLibrary: () => void;
  openLibrary: () => void;
  openRecent: (path: string) => void;
  closeLibrary: () => void;
  addFiles: () => void;
  paste: () => void;
  settings: () => void;
  exit: () => void;
  setTheme: (theme: Theme) => void;
  toggleFullscreen: () => void;
  toggleSidebar: () => void;
  toggleDetails: () => void;
  shortcuts: () => void;
  about: () => void;
  selectAll: () => void;
  preview: () => void;
  setViewMode: (mode: ViewMode) => void;
  /** `1` larger thumbnails, `-1` smaller. */
  zoom: (step: 1 | -1) => void;
  toggleNames: () => void;
  setSort: (key: SortKey) => void;
  setAscending: (ascending: boolean) => void;
};

/** `true`, or the reason the command is disabled (shown as a tooltip). */
export type Availability = true | string;

export type SubmenuItem = {
  id: string;
  label: string;
  title?: string;
  /** Set for choices such as the theme: the submenu shows radio items. */
  checked?: boolean;
  /** Items with different groups are separate choices, separated by a line. */
  group?: number;
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

/** Adding files and pasting: a library that can be written. */
export const needsWritableLibrary = (state: CommandState): Availability => {
  if (!state.library) return "Open a library first.";
  return state.library.readOnly ? "This library is read-only." : true;
};

/** The sort keys of the menu (docs/spec.md §5). Duration waits for video, Manual for folders. */
export const SORT_KEYS: readonly { key: SortKey; label: string }[] = [
  { key: "added", label: "Date Added" },
  { key: "btime", label: "Date Created" },
  { key: "mtime", label: "Date Modified" },
  { key: "name", label: "Title" },
  { key: "ext", label: "Extension" },
  { key: "size", label: "File Size" },
  { key: "resolution", label: "Dimensions" },
  { key: "star", label: "Rating" },
  { key: "tag", label: "Tags" },
  { key: "random", label: "Random" },
];

/** Sort By: the keys, then the direction. */
export function sortItems(state: CommandState): SubmenuItem[] {
  const { sort, ascending } = state.browse;
  return [
    ...SORT_KEYS.map(({ key, label }) => ({
      id: key,
      label,
      group: 0,
      checked: sort === key,
      run: (actions: CommandActions) => actions.setSort(key),
    })),
    {
      id: "ascending",
      label: "Ascending",
      group: 1,
      checked: ascending,
      run: (actions: CommandActions) => actions.setAscending(true),
    },
    {
      id: "descending",
      label: "Descending",
      group: 1,
      checked: !ascending,
      run: (actions: CommandActions) => actions.setAscending(false),
    },
  ];
}

const inGrid = (state: CommandState): Availability => {
  if (!state.library) return "Open a library first.";
  return state.browse.viewMode === "grid" ? true : "Thumbnails have a size in the grid.";
};

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
    id: "file.addFiles",
    label: "Add Files…",
    menu: "file",
    group: 1,
    available: needsWritableLibrary,
    run: (actions) => actions.addFiles(),
  },
  {
    id: "file.settings",
    label: "Settings…",
    menu: "file",
    group: 2,
    shortcut: "Ctrl+,",
    run: (actions) => actions.settings(),
  },
  {
    id: "file.exit",
    label: "Exit",
    menu: "file",
    group: 3,
    shortcut: "Alt+F4",
    systemShortcut: true,
    run: (actions) => actions.exit(),
  },
  {
    id: "edit.paste",
    label: "Paste",
    menu: "edit",
    group: 0,
    shortcut: "Ctrl+V",
    available: needsWritableLibrary,
    run: (actions) => actions.paste(),
  },
  {
    id: "edit.selectAll",
    label: "Select All",
    menu: "edit",
    group: 1,
    shortcut: "Ctrl+A",
    available: (state) => {
      if (!state.library) return "Open a library first.";
      return state.items > 0 ? true : "Nothing to select.";
    },
    run: (actions) => actions.selectAll(),
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
    id: "view.grid",
    label: "Grid",
    menu: "view",
    group: 2,
    checked: (state) => state.browse.viewMode === "grid",
    available: needsLibrary,
    run: (actions) => actions.setViewMode("grid"),
  },
  {
    id: "view.list",
    label: "List",
    menu: "view",
    group: 2,
    checked: (state) => state.browse.viewMode === "list",
    available: needsLibrary,
    run: (actions) => actions.setViewMode("list"),
  },
  {
    id: "view.larger",
    label: "Larger Thumbnails",
    menu: "view",
    group: 2,
    shortcut: "Ctrl+=",
    available: (state) => {
      const grid = inGrid(state);
      if (grid !== true) return grid;
      return state.browse.thumbnailSize < THUMBNAIL_SIZES.max ? true : "Already the largest.";
    },
    run: (actions) => actions.zoom(1),
  },
  {
    id: "view.smaller",
    label: "Smaller Thumbnails",
    menu: "view",
    group: 2,
    shortcut: "Ctrl+-",
    available: (state) => {
      const grid = inGrid(state);
      if (grid !== true) return grid;
      return state.browse.thumbnailSize > THUMBNAIL_SIZES.min ? true : "Already the smallest.";
    },
    run: (actions) => actions.zoom(-1),
  },
  {
    id: "view.showNames",
    label: "Show Names",
    menu: "view",
    group: 2,
    checked: (state) => state.browse.showNames,
    available: inGrid,
    run: (actions) => actions.toggleNames(),
  },
  {
    id: "view.sortBy",
    label: "Sort By",
    menu: "view",
    group: 2,
    available: needsLibrary,
    submenu: sortItems,
  },
  {
    id: "view.preview",
    label: "Preview",
    menu: "view",
    group: 3,
    shortcut: "Space",
    available: (state) => {
      if (!state.library) return "Open a library first.";
      return state.selected > 0 ? true : "Select an item first.";
    },
    run: (actions) => actions.preview(),
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

/**
 * Keys that a focused widget handles itself (the ARIA grid pattern); listed in the Keyboard
 * Shortcuts dialog, not dispatched from here.
 */
export const WIDGET_KEYS: readonly { area: string; keys: string; label: string }[] = [
  { area: "Grid", keys: "Arrow keys", label: "Move the selection" },
  { area: "Grid", keys: "Shift+Arrow keys", label: "Extend the selection" },
  { area: "Grid", keys: "Home / End", label: "First / last item" },
  { area: "Grid", keys: "Enter", label: "Go to the details panel" },
  { area: "Grid", keys: "Ctrl+Wheel", label: "Thumbnail size" },
  { area: "Preview", keys: "← / →", label: "Previous / next item" },
  { area: "Preview", keys: "+ / − / 0", label: "Zoom in / out / fit" },
  { area: "Preview", keys: "Esc", label: "Close" },
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
