//! GPUI actions + keymap (logical ids mirror `ui/src/commands/registry.ts`).
//! Commands of later features are absent, not disabled.

use gpui_kit::{App, KeyBinding, actions};

actions!(
    egl,
    [
        NewLibrary,
        OpenLibrary,
        CloseLibrary,
        Settings,
        LibrarySettings,
        BackupExport,
        DecryptArchive,
        ImportEagleLibrary,
        ConnectEagle,
        Exit,
        ThemeSystem,
        ThemeLight,
        ThemeDark,
        ToggleFullScreen,
        ToggleSidebar,
        ToggleDetails,
        Shortcuts,
        About,
        CheckForUpdates,
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
        Find,
        Undo,
        TrashSelection,
        RestoreSelection,
        DeletePermanently,
        EmptyTrash,
        AddFiles,
        ImportFolder,
        PasteFiles,
        NewFolder,
        RenameFolder,
        DeleteFolder,
        LockAll,
        ToggleImportsPanel,
        ClearFilters,
        SaveSmartFolder,
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
        SwitchLibrary,
        ClearRecentHistory,
        ClearCacheReload,
        RevealLibrary,
        FocusDetails,
        MoveFocusLeft,
        MoveFocusRight,
        MoveFocusUp,
        MoveFocusDown,
        MoveFocusHome,
        MoveFocusEnd,
        MoveFocusPageUp,
        MoveFocusPageDown,
        RenameSelection,
        AddToFolders,
        GoToFolder,
        RemoveFromFolder,
        AddTags,
        CopyTags,
        PasteTags,
        ClearTags,
        CopyName,
        CopyPath,
        Rate0,
        Rate1,
        Rate2,
        Rate3,
        Rate4,
        Rate5,
        LockFolder,
        ExportFolder,
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
        id: "file.addFiles",
        label_key: "commands.file.addFiles",
        keys: "Ctrl+Shift+A",
    },
    ShortcutEntry {
        id: "edit.undo",
        label_key: "commands.edit.undo",
        keys: "Ctrl+Z",
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
        id: "edit.trash",
        label_key: "commands.edit.trash",
        keys: "Delete",
    },
    ShortcutEntry {
        id: "edit.paste",
        label_key: "commands.edit.pasteFiles",
        keys: "Ctrl+V",
    },
    ShortcutEntry {
        id: "folder.new",
        label_key: "commands.folder.new",
        keys: "Ctrl+Shift+N",
    },
    ShortcutEntry {
        id: "library.switch",
        label_key: "libraryMenu.switch",
        keys: "Ctrl+L",
    },
    ShortcutEntry {
        id: "library.lockAll",
        label_key: "commands.library.lockAll",
        keys: "Ctrl+Shift+L",
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
        id: "item.rename",
        label_key: "itemMenu.rename",
        keys: "F2",
    },
    ShortcutEntry {
        id: "item.rate",
        label_key: "itemMenu.rateKeys",
        keys: "1 – 5, 0",
    },
    ShortcutEntry {
        id: "item.addToFolders",
        label_key: "itemMenu.addToFolders",
        keys: "Ctrl+Shift+J",
    },
    ShortcutEntry {
        id: "item.removeFromFolder",
        label_key: "itemMenu.removeFromFolder",
        keys: "Shift+Delete",
    },
    ShortcutEntry {
        id: "item.addTags",
        label_key: "itemMenu.addTags",
        keys: "Ctrl+T",
    },
    ShortcutEntry {
        id: "item.copyTags",
        label_key: "itemMenu.copyTags",
        keys: "Ctrl+Shift+C",
    },
    ShortcutEntry {
        id: "item.pasteTags",
        label_key: "itemMenu.pasteTags",
        keys: "Ctrl+Shift+V",
    },
    ShortcutEntry {
        id: "item.copyPath",
        label_key: "itemMenu.copyPath",
        keys: "Ctrl+Alt+C",
    },
    ShortcutEntry {
        id: "folder.goTo",
        label_key: "picker.goTitle",
        keys: "Ctrl+J",
    },
    ShortcutEntry {
        id: "help.shortcuts",
        label_key: "commands.help.shortcuts",
        keys: "F1",
    },
];

pub fn bind_keymap(cx: &mut App) {
    cx.bind_keys(key_bindings());
}

/// Every global shortcut. A focused text field takes plain keys (digits, Space, `R`) first.
fn key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("ctrl-o", OpenLibrary, None),
        KeyBinding::new("ctrl-w", CloseLibrary, None),
        KeyBinding::new("ctrl-,", Settings, None),
        KeyBinding::new("ctrl-a", SelectAll, None),
        KeyBinding::new("ctrl-f", Find, None),
        KeyBinding::new("ctrl-z", Undo, None),
        // Menus show the binding registered last: Delete, as the shortcut list says.
        KeyBinding::new("backspace", TrashSelection, None),
        KeyBinding::new("delete", TrashSelection, None),
        KeyBinding::new("ctrl-shift-a", AddFiles, None),
        KeyBinding::new("ctrl-v", PasteFiles, None),
        KeyBinding::new("ctrl-shift-n", NewFolder, None),
        KeyBinding::new("f2", RenameSelection, None),
        KeyBinding::new("ctrl-shift-j", AddToFolders, None),
        KeyBinding::new("ctrl-j", GoToFolder, None),
        KeyBinding::new("shift-delete", RemoveFromFolder, None),
        KeyBinding::new("ctrl-t", AddTags, None),
        KeyBinding::new("ctrl-shift-c", CopyTags, None),
        KeyBinding::new("ctrl-shift-v", PasteTags, None),
        KeyBinding::new("ctrl-alt-c", CopyPath, None),
        KeyBinding::new("0", Rate0, None),
        KeyBinding::new("1", Rate1, None),
        KeyBinding::new("2", Rate2, None),
        KeyBinding::new("3", Rate3, None),
        KeyBinding::new("4", Rate4, None),
        KeyBinding::new("5", Rate5, None),
        KeyBinding::new("ctrl-l", SwitchLibrary, None),
        KeyBinding::new("ctrl-shift-l", LockAll, None),
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
    ]
}

#[cfg(test)]
mod tests {
    use super::{SHORTCUTS, key_bindings};

    #[test]
    fn every_key_binding_parses_and_is_bound_once() {
        let bindings = key_bindings();
        let mut keys: Vec<String> = bindings
            .iter()
            .map(|binding| format!("{:?}", binding.keystrokes()))
            .collect();
        keys.sort_unstable();
        let total = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), total, "one keystroke bound to two actions");
        assert!(total > 50);
    }

    #[test]
    fn eagle_item_shortcuts_are_listed_once() {
        for (id, keys) in [
            ("item.rename", "F2"),
            ("item.addToFolders", "Ctrl+Shift+J"),
            ("folder.goTo", "Ctrl+J"),
            ("item.copyTags", "Ctrl+Shift+C"),
            ("item.pasteTags", "Ctrl+Shift+V"),
            ("item.copyPath", "Ctrl+Alt+C"),
            ("item.removeFromFolder", "Shift+Delete"),
        ] {
            let found: Vec<_> = SHORTCUTS.iter().filter(|entry| entry.id == id).collect();
            assert_eq!(found.len(), 1, "{id}");
            assert_eq!(found[0].keys, keys, "{id}");
        }
        let mut keys: Vec<&str> = SHORTCUTS.iter().map(|entry| entry.keys).collect();
        keys.sort_unstable();
        let total = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), total, "two commands share a shortcut");
    }

    #[test]
    fn switch_library_takes_ctrl_l() {
        let switch = SHORTCUTS
            .iter()
            .find(|entry| entry.id == "library.switch")
            .expect("switch shortcut");
        assert_eq!(switch.keys, "Ctrl+L");
        let lock = SHORTCUTS
            .iter()
            .find(|entry| entry.id == "library.lockAll")
            .expect("lock shortcut");
        assert_eq!(lock.keys, "Ctrl+Shift+L");
    }
}
