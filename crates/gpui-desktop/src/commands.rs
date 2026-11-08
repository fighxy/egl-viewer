//! GPUI actions + keymap for Phase 1–2 (logical ids mirror `ui/src/commands/registry.ts`).
//! Commands of later phases are absent, not disabled.

use gpui_kit::{App, KeyBinding, actions};

actions!(
    egl,
    [
        NewLibrary,
        OpenLibrary,
        CloseLibrary,
        Settings,
        Exit,
        ThemeSystem,
        ThemeLight,
        ThemeDark,
        ToggleFullScreen,
        ToggleSidebar,
        ToggleDetails,
        Shortcuts,
        About,
        OpenLogFolder,
        ViewGrid,
        ViewList,
        LargerThumbs,
        SmallerThumbs,
        ToggleShowNames,
        ToggleIncludeSubfolders,
        SelectAll,
        Preview,
        ClosePreview,
        PreviewPrev,
        PreviewNext,
        PreviewZoomIn,
        PreviewZoomOut,
        PreviewZoomReset,
        PreviewRotate,
        SpacePreviewDown,
        OpenOriginal,
        RevealOriginals,
        CopyOriginals,
        FindStub,
        SortByAdded,
        SortByCreated,
        SortByModified,
        SortByName,
        SortByExt,
        SortBySize,
        SortByResolution,
        SortByRating,
        SortByTag,
        SortByRandom,
        SortByManual,
        SortAscending,
        SortDescending,
        OpenSortMenu,
        FocusDetails,
        MoveFocusLeft,
        MoveFocusRight,
        MoveFocusUp,
        MoveFocusDown,
        MoveFocusHome,
        MoveFocusEnd,
        MoveFocusPageUp,
        MoveFocusPageDown,
    ]
);

/// Shortcuts shown in Help → Keyboard Shortcuts and bound globally.
pub struct ShortcutEntry {
    #[allow(dead_code)]
    pub id: &'static str,
    pub label_key: &'static str,
    pub keys: &'static str,
}

pub const SHORTCUTS: &[ShortcutEntry] = &[
    ShortcutEntry {
        id: "file.openLibrary",
        label_key: "commands.file.openLibrary",
        keys: "Ctrl+O",
    },
    ShortcutEntry {
        id: "file.closeLibrary",
        label_key: "commands.file.closeLibrary",
        keys: "Ctrl+W",
    },
    ShortcutEntry {
        id: "file.settings",
        label_key: "commands.file.settings",
        keys: "Ctrl+,",
    },
    ShortcutEntry {
        id: "edit.selectAll",
        label_key: "commands.edit.selectAll",
        keys: "Ctrl+A",
    },
    ShortcutEntry {
        id: "edit.find",
        label_key: "commands.edit.find",
        keys: "Ctrl+F",
    },
    ShortcutEntry {
        id: "view.fullScreen",
        label_key: "commands.view.fullScreen",
        keys: "F11",
    },
    ShortcutEntry {
        id: "view.sidebar",
        label_key: "commands.view.sidebar",
        keys: "Ctrl+B",
    },
    ShortcutEntry {
        id: "view.details",
        label_key: "commands.view.details",
        keys: "Ctrl+I",
    },
    ShortcutEntry {
        id: "view.grid",
        label_key: "commands.view.grid",
        keys: "Ctrl+1",
    },
    ShortcutEntry {
        id: "view.list",
        label_key: "commands.view.list",
        keys: "Ctrl+2",
    },
    ShortcutEntry {
        id: "view.larger",
        label_key: "commands.view.larger",
        keys: "Ctrl+=",
    },
    ShortcutEntry {
        id: "view.smaller",
        label_key: "commands.view.smaller",
        keys: "Ctrl+-",
    },
    ShortcutEntry {
        id: "view.sortBy",
        label_key: "commands.view.sortBy",
        keys: "Ctrl+Shift+S",
    },
    ShortcutEntry {
        id: "view.preview",
        label_key: "commands.view.preview",
        keys: "Space",
    },
    ShortcutEntry {
        id: "item.openOriginal",
        label_key: "details.openDefault",
        keys: "Ctrl+Enter",
    },
    ShortcutEntry {
        id: "item.reveal",
        label_key: "details.showExplorer",
        keys: "Ctrl+Shift+R",
    },
    ShortcutEntry {
        id: "item.copy",
        label_key: "details.copyFile",
        keys: "Ctrl+C",
    },
    ShortcutEntry {
        id: "help.shortcuts",
        label_key: "commands.help.shortcuts",
        keys: "F1",
    },
];

pub fn bind_keymap(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-o", OpenLibrary, None),
        KeyBinding::new("ctrl-w", CloseLibrary, None),
        KeyBinding::new("ctrl-,", Settings, None),
        KeyBinding::new("ctrl-a", SelectAll, None),
        KeyBinding::new("ctrl-f", FindStub, None),
        KeyBinding::new("f11", ToggleFullScreen, None),
        KeyBinding::new("ctrl-b", ToggleSidebar, None),
        KeyBinding::new("ctrl-i", ToggleDetails, None),
        KeyBinding::new("ctrl-1", ViewGrid, None),
        KeyBinding::new("ctrl-2", ViewList, None),
        KeyBinding::new("ctrl-=", LargerThumbs, None),
        KeyBinding::new("ctrl--", SmallerThumbs, None),
        KeyBinding::new("ctrl-shift-s", OpenSortMenu, None),
        KeyBinding::new("f1", Shortcuts, None),
        KeyBinding::new("enter", FocusDetails, None),
        KeyBinding::new("space", SpacePreviewDown, None),
        KeyBinding::new("escape", ClosePreview, None),
        KeyBinding::new("r", PreviewRotate, None),
        KeyBinding::new("ctrl-enter", OpenOriginal, None),
        KeyBinding::new("ctrl-shift-r", RevealOriginals, None),
        KeyBinding::new("ctrl-c", CopyOriginals, None),
        KeyBinding::new("left", MoveFocusLeft, None),
        KeyBinding::new("right", MoveFocusRight, None),
        KeyBinding::new("up", MoveFocusUp, None),
        KeyBinding::new("down", MoveFocusDown, None),
        KeyBinding::new("home", MoveFocusHome, None),
        KeyBinding::new("end", MoveFocusEnd, None),
        KeyBinding::new("pageup", MoveFocusPageUp, None),
        KeyBinding::new("pagedown", MoveFocusPageDown, None),
    ]);
}
