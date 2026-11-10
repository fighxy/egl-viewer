//! Item and folder commands reached from the right-click menus and Eagle's shortcuts (D-140):
//! the menus themselves, rating keys, rename, tag copy and paste, name and path copy, and the
//! folder panel behind Add to Folder(s) and Go to Folder.

use std::collections::HashSet;

use app_store::Locale;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::label::Label;
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::{StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Action, ClipboardItem, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use library_core::{Folder, ItemEdit, ItemId, LibraryKind};

use crate::app::RootApp;
use crate::browse::details::DetailsModel;
use crate::commands::*;
use crate::i18n::{t, t_n};
use crate::library::{self, Place};
use crate::ops_mutations::EditField;

/// What a right-click menu needs to know about the open library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuSnap {
    pub locale: Locale,
    /// Selected items.
    pub count: usize,
    pub in_trash: bool,
    pub writable: bool,
    /// The open place is a folder.
    pub folder_open: bool,
    /// A native library: originals are files on this disk.
    pub native: bool,
    pub tags_copied: bool,
}

/// One command of a right-click menu. The menu builder maps it to a GPUI action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    Preview,
    OpenDefault,
    Reveal,
    CopyFile,
    CopyName,
    CopyPath,
    Rename,
    AddToFolders,
    RemoveFromFolder,
    AddTags,
    CopyTags,
    PasteTags,
    ClearTags,
    Trash,
    Restore,
    DeletePermanently,
    AddFiles,
    ImportFolder,
    Paste,
    NewFolder,
    SelectAll,
    RenameFolder,
    LockFolder,
    ExportFolder,
    DeleteFolder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEntry {
    Command {
        label: &'static str,
        command: MenuCommand,
        enabled: bool,
    },
    /// The Rating submenu: five stars down to none.
    Rating {
        enabled: bool,
    },
    Separator,
}

fn entry(label: &'static str, command: MenuCommand, enabled: bool) -> MenuEntry {
    MenuEntry::Command {
        label,
        command,
        enabled,
    }
}

/// The right-click menu of the grid and the list. With a selection it is the item menu; on
/// empty space with nothing selected it offers what adds to the view.
pub fn item_menu_model(snap: &MenuSnap) -> Vec<MenuEntry> {
    use MenuCommand as C;
    let write = snap.writable;
    if snap.count == 0 {
        return vec![
            entry("commands.file.addFiles", C::AddFiles, write && snap.native),
            entry(
                "commands.file.importFolder",
                C::ImportFolder,
                write && snap.native,
            ),
            entry("commands.edit.pasteFiles", C::Paste, write),
            MenuEntry::Separator,
            entry("commands.folder.new", C::NewFolder, write),
            MenuEntry::Separator,
            entry("commands.edit.selectAll", C::SelectAll, true),
        ];
    }
    let one = snap.count == 1;
    let files = snap.native;
    let mut out = vec![
        entry("commands.view.preview", C::Preview, true),
        entry("details.openDefault", C::OpenDefault, files),
        entry("details.showExplorer", C::Reveal, files),
        MenuEntry::Separator,
        entry(
            if one {
                "details.copyFile"
            } else {
                "details.copyFiles"
            },
            C::CopyFile,
            files,
        ),
        entry("itemMenu.copyName", C::CopyName, true),
        entry("itemMenu.copyPath", C::CopyPath, files),
        MenuEntry::Separator,
    ];
    if snap.in_trash {
        out.push(entry("trash.restore", C::Restore, write));
        out.push(entry("trash.delete", C::DeletePermanently, write));
        return out;
    }
    out.push(entry("itemMenu.rename", C::Rename, write && one));
    out.push(entry("itemMenu.addToFolders", C::AddToFolders, write));
    if snap.folder_open {
        out.push(entry(
            "itemMenu.removeFromFolder",
            C::RemoveFromFolder,
            write,
        ));
    }
    out.push(MenuEntry::Rating { enabled: write });
    out.push(MenuEntry::Separator);
    out.push(entry("itemMenu.addTags", C::AddTags, write));
    out.push(entry("itemMenu.copyTags", C::CopyTags, true));
    out.push(entry(
        "itemMenu.pasteTags",
        C::PasteTags,
        write && snap.tags_copied,
    ));
    out.push(entry("itemMenu.clearTags", C::ClearTags, write));
    out.push(MenuEntry::Separator);
    out.push(entry("commands.edit.trash", C::Trash, write));
    out
}

/// The right-click menu of the folder tree. A row that is clicked is opened first, so the
/// folder commands act on the open folder.
pub fn folder_menu_model(snap: &MenuSnap) -> Vec<MenuEntry> {
    use MenuCommand as C;
    let write = snap.writable;
    if !snap.folder_open {
        return vec![entry("commands.folder.new", C::NewFolder, write)];
    }
    vec![
        entry("folderMenu.newSubfolder", C::NewFolder, write),
        entry("folderMenu.rename", C::RenameFolder, write),
        MenuEntry::Separator,
        entry("folderMenu.lock", C::LockFolder, write && snap.native),
        entry("folderMenu.export", C::ExportFolder, snap.native),
        MenuEntry::Separator,
        entry("folderMenu.delete", C::DeleteFolder, write),
    ]
}

fn action_for(command: MenuCommand) -> Box<dyn Action> {
    use MenuCommand as C;
    match command {
        C::Preview => Box::new(Preview),
        C::OpenDefault => Box::new(OpenOriginal),
        C::Reveal => Box::new(RevealOriginals),
        C::CopyFile => Box::new(CopyOriginals),
        C::CopyName => Box::new(CopyName),
        C::CopyPath => Box::new(CopyPath),
        C::Rename => Box::new(RenameSelection),
        C::AddToFolders => Box::new(AddToFolders),
        C::RemoveFromFolder => Box::new(RemoveFromFolder),
        C::AddTags => Box::new(AddTags),
        C::CopyTags => Box::new(CopyTags),
        C::PasteTags => Box::new(PasteTags),
        C::ClearTags => Box::new(ClearTags),
        C::Trash => Box::new(TrashSelection),
        C::Restore => Box::new(RestoreSelection),
        C::DeletePermanently => Box::new(DeletePermanently),
        C::AddFiles => Box::new(AddFiles),
        C::ImportFolder => Box::new(ImportFolder),
        C::Paste => Box::new(PasteFiles),
        C::NewFolder => Box::new(NewFolder),
        C::SelectAll => Box::new(SelectAll),
        C::RenameFolder => Box::new(RenameFolder),
        C::LockFolder => Box::new(LockFolder),
        C::ExportFolder => Box::new(ExportFolder),
        C::DeleteFolder => Box::new(DeleteFolder),
    }
}

/// Label of one row of the Rating submenu.
pub fn rating_label(locale: Locale, star: u8) -> String {
    if star == 0 {
        t(locale, "itemMenu.removeRating")
    } else {
        "★".repeat(usize::from(star))
    }
}

/// Fills a popup menu from a menu model.
pub fn build_menu(
    mut menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
    locale: Locale,
    entries: &[MenuEntry],
) -> PopupMenu {
    for entry in entries {
        menu = match *entry {
            MenuEntry::Separator => menu.separator(),
            MenuEntry::Command {
                label,
                command,
                enabled,
            } => menu.menu_with_disabled(t(locale, label), action_for(command), !enabled),
            MenuEntry::Rating { enabled } => menu.submenu(
                t(locale, "itemMenu.rating"),
                window,
                cx,
                move |menu, _, _| {
                    let stars: [(u8, Box<dyn Action>); 6] = [
                        (5, Box::new(Rate5)),
                        (4, Box::new(Rate4)),
                        (3, Box::new(Rate3)),
                        (2, Box::new(Rate2)),
                        (1, Box::new(Rate1)),
                        (0, Box::new(Rate0)),
                    ];
                    stars.into_iter().fold(menu, |menu, (star, action)| {
                        menu.menu_with_disabled(rating_label(locale, star), action, !enabled)
                    })
                },
            ),
        };
    }
    menu
}

/// The builder a `context_menu` wants: reads the library when the menu opens, after the
/// right click has selected its cell or opened its folder.
pub fn context_menu_builder(
    entity: Entity<RootApp>,
    model: fn(&MenuSnap) -> Vec<MenuEntry>,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, window, cx| {
        let Some(snap) = entity.read(cx).menu_snap_for_context() else {
            return menu;
        };
        build_menu(menu, window, cx, snap.locale, &model(&snap))
    }
}

/// A folder the folder panel can offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderEntry {
    pub id: String,
    pub name: String,
    pub depth: u32,
    /// Names from the root down, joined with " / ".
    pub path: String,
}

/// The folder tree as rows, parents before children. Locked folders are left out: they are
/// listed only under Locked folders (D-059).
pub fn flatten_pickable(folders: &[Folder]) -> Vec<FolderEntry> {
    fn walk(folders: &[Folder], depth: u32, parent: &str, out: &mut Vec<FolderEntry>) {
        for folder in folders.iter().filter(|folder| !folder.is_locked) {
            let path = if parent.is_empty() {
                folder.name.clone()
            } else {
                format!("{parent} / {}", folder.name)
            };
            out.push(FolderEntry {
                id: folder.id.clone(),
                name: folder.name.clone(),
                depth,
                path: path.clone(),
            });
            walk(&folder.children, depth + 1, &path, out);
        }
    }
    let mut out = Vec::new();
    walk(folders, 0, "", &mut out);
    out
}

/// Rows whose name contains the query, in tree order. Case is ignored; an empty query keeps
/// every row.
pub fn filter_entries<'a>(entries: &'a [FolderEntry], query: &str) -> Vec<&'a FolderEntry> {
    let needle = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|entry| needle.is_empty() || entry.name.to_lowercase().contains(&needle))
        .collect()
}

/// The edit behind Apply in Add to Folder(s). One item: the ticks are its complete folder
/// set, and nothing is sent when they did not change. Several items: the ticked folders are
/// added, and no item leaves a folder.
pub fn folder_edit(
    single: bool,
    entries: &[FolderEntry],
    initial: &HashSet<String>,
    checked: &HashSet<String>,
) -> Option<ItemEdit> {
    if single {
        if initial == checked {
            return None;
        }
        // Tree order first, then what the panel does not list (locked folders): the item
        // stays in those.
        let mut ids: Vec<String> = entries
            .iter()
            .filter(|entry| checked.contains(&entry.id))
            .map(|entry| entry.id.clone())
            .collect();
        let mut unlisted: Vec<String> = checked
            .iter()
            .filter(|id| !entries.iter().any(|entry| &entry.id == *id))
            .cloned()
            .collect();
        unlisted.sort();
        ids.extend(unlisted);
        return Some(ItemEdit::SetFolders(ids));
    }
    let ids: Vec<String> = entries
        .iter()
        .filter(|entry| checked.contains(&entry.id))
        .map(|entry| entry.id.clone())
        .collect();
    if ids.is_empty() {
        None
    } else {
        Some(ItemEdit::AddToFolders(ids))
    }
}

/// Every tag on the selection, in the order the right bar shows them.
pub fn selection_tag_names(details: &DetailsModel) -> Vec<String> {
    if let Some(item) = details.item.as_ref() {
        return item.tags.clone();
    }
    details
        .selection
        .as_ref()
        .map(|selection| selection.tags.iter().map(|tag| tag.name.clone()).collect())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerMode {
    /// `Ctrl+J`: open a folder.
    GoTo,
    /// `Ctrl+Shift+J`: put the selection into folders.
    AddTo,
}

/// The open folder panel.
pub struct FolderPickerUi {
    pub mode: PickerMode,
    pub entries: Vec<FolderEntry>,
    /// Add to Folder(s) for one item starts from its folders.
    pub single: bool,
    pub initial: HashSet<String>,
    pub checked: HashSet<String>,
}

impl RootApp {
    pub(crate) fn menu_snap_for_context(&self) -> Option<MenuSnap> {
        let lib = self.library.as_ref()?;
        Some(MenuSnap {
            locale: self.locale,
            count: lib.selection.count(),
            in_trash: matches!(lib.place, Place::Trash),
            writable: !lib.info.read_only,
            folder_open: matches!(lib.place, Place::Folder { .. }),
            native: lib.info.kind == LibraryKind::Native,
            tags_copied: !self.tag_clipboard.is_empty(),
        })
    }

    /// A right click on a cell: a cell outside the selection becomes the selection, a cell
    /// inside it keeps the whole selection for the menu.
    pub(crate) fn select_for_context(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        let Some(cell) = lib.cells.get(index) else {
            return;
        };
        if lib.selection.contains(&cell.id) {
            return;
        }
        let id = cell.id.0.clone();
        lib.selection.select_only(&id, index);
        lib.persist_session(&self.session);
        self.refresh_details(cx);
    }

    /// A right click on a folder row opens the folder, so the menu's commands act on it.
    pub(crate) fn open_folder_for_context(&mut self, id: String, cx: &mut Context<Self>) {
        let already = self
            .library
            .as_ref()
            .is_some_and(|lib| lib.place.open_folder_id() == Some(id.as_str()));
        if !already {
            self.set_place(Place::Folder { id }, cx);
        }
    }

    fn say(&mut self, key: &str, cx: &mut Context<Self>) {
        self.status = SharedString::from(t(self.locale, key));
        cx.notify();
    }

    /// `1`–`5` rate the selection and `0` removes the rating. In the preview `0` keeps its
    /// meaning there: fit the image.
    pub(crate) fn rate_selection(&mut self, star: u8, cx: &mut Context<Self>) {
        if star == 0 && self.preview_is_open() {
            self.preview_zoom_reset(cx);
            return;
        }
        let in_trash = self
            .library
            .as_ref()
            .is_none_or(|lib| matches!(lib.place, Place::Trash));
        if in_trash || self.selection_target().is_none() {
            return;
        }
        self.set_rating(star, cx);
    }

    /// `F2`: the one selected item, or the open folder when nothing is selected.
    pub(crate) fn rename_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.library.as_ref().map_or(0, |lib| lib.selection.count());
        match count {
            0 => self.rename_open_folder(window, cx),
            1 => {
                if self.library.as_ref().is_some_and(|lib| lib.info.read_only) {
                    self.say("errors.readOnly", cx);
                    return;
                }
                self.open_edit_field_dialog(window, cx, EditField::Name);
            }
            _ => self.say("itemMenu.renameOne", cx),
        }
    }

    pub(crate) fn remove_from_open_folder(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self
            .library
            .as_ref()
            .and_then(|lib| lib.place.open_folder_id().map(str::to_owned))
        else {
            self.say("folder.selectFirst", cx);
            return;
        };
        self.apply_edit_to_selection(ItemEdit::RemoveFromFolders(vec![id]), cx);
    }

    pub(crate) fn copy_tags(&mut self, cx: &mut Context<Self>) {
        if self.selection_target().is_none() {
            return;
        }
        let tags = selection_tag_names(&self.details);
        if tags.is_empty() {
            self.say("itemMenu.noTags", cx);
            return;
        }
        self.status =
            SharedString::from(t_n(self.locale, "itemMenu.tagsCopied", tags.len() as u32));
        self.tag_clipboard = tags;
        cx.notify();
    }

    pub(crate) fn paste_tags(&mut self, cx: &mut Context<Self>) {
        if self.tag_clipboard.is_empty() {
            self.say("itemMenu.noTagsCopied", cx);
            return;
        }
        self.apply_edit_to_selection(ItemEdit::AddTags(self.tag_clipboard.clone()), cx);
    }

    pub(crate) fn clear_tags(&mut self, cx: &mut Context<Self>) {
        let tags = selection_tag_names(&self.details);
        if tags.is_empty() {
            return;
        }
        self.apply_edit_to_selection(ItemEdit::RemoveTags(tags), cx);
    }

    /// The names of the selected items with their extensions, one per line, in view order.
    pub(crate) fn copy_names(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let names: Vec<String> = lib
            .cells
            .iter()
            .filter(|cell| lib.selection.contains(&cell.id))
            .map(|cell| file_name(&cell.name, &cell.ext))
            .collect();
        if names.is_empty() {
            return;
        }
        let count = names.len() as u32;
        cx.write_to_clipboard(ClipboardItem::new_string(names.join("\n")));
        self.status = SharedString::from(t_n(self.locale, "itemMenu.namesCopied", count));
        cx.notify();
    }

    /// The paths of the selected originals, one per line.
    pub(crate) fn copy_paths(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.selection_target() else {
            return;
        };
        match self
            .session
            .selected_ids(target)
            .and_then(|ids: Vec<ItemId>| self.session.original_paths(&ids))
        {
            Ok(paths) => {
                let count = paths.len() as u32;
                let text = paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.status = SharedString::from(t_n(self.locale, "itemMenu.pathsCopied", count));
            }
            Err(error) => {
                self.status = SharedString::from(error.to_string());
            }
        }
        cx.notify();
    }

    pub(crate) fn export_open_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let folder = self.library.as_ref().and_then(|lib| {
            let id = lib.place.open_folder_id()?;
            let name = library::folder_name(&lib.folders, id)?;
            Some((id.to_owned(), name))
        });
        match folder {
            Some((id, name)) => self.open_export_folder(id, name, window, cx),
            None => self.say("folder.selectFirst", cx),
        }
    }

    pub(crate) fn open_folder_picker(
        &mut self,
        mode: PickerMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let count = lib.selection.count();
        if mode == PickerMode::AddTo {
            if lib.info.read_only {
                self.say("errors.readOnly", cx);
                return;
            }
            if count == 0 || matches!(lib.place, Place::Trash) {
                return;
            }
        }
        let entries = flatten_pickable(&lib.folders);
        let single = count == 1;
        let initial: HashSet<String> = if mode == PickerMode::AddTo && single {
            self.details
                .item
                .as_ref()
                .map(|item| item.folders.iter().cloned().collect())
                .unwrap_or_default()
        } else {
            HashSet::new()
        };
        self.folder_picker = Some(FolderPickerUi {
            mode,
            entries,
            single,
            checked: initial.clone(),
            initial,
        });
        self.picker_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn close_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder_picker.take().is_some() {
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    fn picker_query(&self, cx: &Context<Self>) -> String {
        self.picker_input.read(cx).value().to_string()
    }

    /// A click on a row: Go to Folder opens it, Add to Folder(s) ticks it.
    pub(crate) fn picker_pick(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = self.folder_picker.as_mut() else {
            return;
        };
        match picker.mode {
            PickerMode::GoTo => {
                self.close_folder_picker(window, cx);
                self.set_place(Place::Folder { id }, cx);
            }
            PickerMode::AddTo => {
                if !picker.checked.remove(&id) {
                    picker.checked.insert(id);
                }
                cx.notify();
            }
        }
    }

    /// Enter in the panel's field, or its main button. Go to Folder opens the first match.
    /// Add to Folder(s) with nothing ticked takes the first match of a typed name, so a name
    /// and Enter is enough.
    pub(crate) fn picker_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.picker_query(cx);
        let Some(picker) = self.folder_picker.as_mut() else {
            return;
        };
        let first = filter_entries(&picker.entries, &query)
            .first()
            .map(|entry| entry.id.clone());
        match picker.mode {
            PickerMode::GoTo => {
                if let Some(id) = first {
                    self.close_folder_picker(window, cx);
                    self.set_place(Place::Folder { id }, cx);
                }
            }
            PickerMode::AddTo => {
                if picker.checked == picker.initial
                    && !query.trim().is_empty()
                    && let Some(id) = first
                {
                    picker.checked.insert(id);
                }
                let edit = folder_edit(
                    picker.single,
                    &picker.entries,
                    &picker.initial,
                    &picker.checked,
                );
                self.close_folder_picker(window, cx);
                if let Some(edit) = edit {
                    self.apply_edit_to_selection(edit, cx);
                }
            }
        }
    }

    pub(crate) fn render_folder_picker(
        &self,
        picker: &FolderPickerUi,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity();
        let query = self.picker_query(cx);
        let searching = !query.trim().is_empty();
        let rows = filter_entries(&picker.entries, &query);
        let add = picker.mode == PickerMode::AddTo;
        let title = if add {
            t(locale, "picker.addTitle")
        } else {
            t(locale, "picker.goTitle")
        };
        let empty_key = if picker.entries.is_empty() {
            "picker.noFolders"
        } else {
            "picker.noMatch"
        };
        v_flex()
            .id("folder-picker-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .items_center()
            .justify_center()
            .bg(theme.background.opacity(0.55))
            .child(
                v_flex()
                    .id("folder-picker")
                    .w(px(420.))
                    .max_h(px(520.))
                    .gap_2()
                    .p_4()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(Label::new(title).font_bold())
                    .child(Input::new(&self.picker_input))
                    .child(if rows.is_empty() {
                        Label::new(t(locale, empty_key))
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .into_any_element()
                    } else {
                        v_flex()
                            .id("folder-picker-list")
                            .w_full()
                            .max_h(px(340.))
                            .overflow_y_scroll()
                            .children(rows.into_iter().map(|row| {
                                let id = row.id.clone();
                                let ticked = picker.checked.contains(&row.id);
                                let label = if searching {
                                    row.path.clone()
                                } else {
                                    row.name.clone()
                                };
                                let indent = if searching { 0.0 } else { row.depth as f32 };
                                let entity = entity.clone();
                                h_flex()
                                    .id(SharedString::from(format!("pick-{}", row.id)))
                                    .w_full()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .py_1()
                                    .pl(px(8.0 + indent * 12.0))
                                    .rounded(theme.radius)
                                    .cursor_pointer()
                                    .hover(|el| el.bg(theme.accent.opacity(0.25)))
                                    .aria_label(row.path.clone())
                                    .on_click(move |_, window, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.picker_pick(id.clone(), window, cx);
                                        });
                                    })
                                    .when(add, |el| {
                                        el.child(div().w(px(16.)).child(Label::new(if ticked {
                                            "✓"
                                        } else {
                                            ""
                                        })))
                                    })
                                    .child(Label::new(label).text_sm())
                            }))
                            .into_any_element()
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                Button::new("picker-cancel")
                                    .ghost()
                                    .label(t(locale, "common.cancel"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_folder_picker(window, cx);
                                    })),
                            )
                            .when(add, |el| {
                                el.child(
                                    Button::new("picker-apply")
                                        .primary()
                                        .label(t(locale, "picker.apply"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.picker_confirm(window, cx);
                                        })),
                                )
                            }),
                    ),
            )
    }
}

/// `name.ext`, or the bare name of a file without an extension.
pub fn file_name(name: &str, ext: &str) -> String {
    if ext.is_empty() {
        name.to_owned()
    } else {
        format!("{name}.{ext}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use library_core::{Item, SelectionTags, Tag};

    fn snap(count: usize) -> MenuSnap {
        MenuSnap {
            locale: Locale::En,
            count,
            in_trash: false,
            writable: true,
            folder_open: false,
            native: true,
            tags_copied: false,
        }
    }

    fn commands(entries: &[MenuEntry]) -> Vec<MenuCommand> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Command { command, .. } => Some(*command),
                _ => None,
            })
            .collect()
    }

    fn enabled(entries: &[MenuEntry], wanted: MenuCommand) -> Option<bool> {
        entries.iter().find_map(|entry| match entry {
            MenuEntry::Command {
                command, enabled, ..
            } if *command == wanted => Some(*enabled),
            _ => None,
        })
    }

    fn folder(id: &str, name: &str, children: Vec<Folder>) -> Folder {
        Folder {
            id: id.into(),
            name: name.into(),
            children,
            ..Folder::default()
        }
    }

    fn tree() -> Vec<Folder> {
        vec![
            folder(
                "a",
                "Art",
                vec![folder("a1", "Sketches", vec![]), {
                    let mut locked = folder("a2", "Private", vec![]);
                    locked.is_locked = true;
                    locked
                }],
            ),
            folder("b", "Brand art", vec![]),
        ]
    }

    #[test]
    fn empty_space_menu_adds_and_selects() {
        let entries = item_menu_model(&snap(0));
        assert_eq!(
            commands(&entries),
            vec![
                MenuCommand::AddFiles,
                MenuCommand::ImportFolder,
                MenuCommand::Paste,
                MenuCommand::NewFolder,
                MenuCommand::SelectAll,
            ]
        );
    }

    #[test]
    fn item_menu_for_one_item_offers_rename_rating_and_trash() {
        let entries = item_menu_model(&snap(1));
        assert_eq!(enabled(&entries, MenuCommand::Rename), Some(true));
        assert!(entries.contains(&MenuEntry::Rating { enabled: true }));
        assert_eq!(enabled(&entries, MenuCommand::Trash), Some(true));
        assert_eq!(enabled(&entries, MenuCommand::RemoveFromFolder), None);
        assert_eq!(enabled(&entries, MenuCommand::PasteTags), Some(false));
        assert_eq!(
            entries.last().copied().and_then(|entry| match entry {
                MenuEntry::Command { command, .. } => Some(command),
                _ => None,
            }),
            Some(MenuCommand::Trash)
        );
    }

    #[test]
    fn rename_needs_exactly_one_item() {
        let entries = item_menu_model(&snap(3));
        assert_eq!(enabled(&entries, MenuCommand::Rename), Some(false));
        assert_eq!(enabled(&entries, MenuCommand::AddToFolders), Some(true));
    }

    #[test]
    fn remove_from_folder_only_inside_a_folder() {
        let mut inside = snap(2);
        inside.folder_open = true;
        let entries = item_menu_model(&inside);
        assert_eq!(enabled(&entries, MenuCommand::RemoveFromFolder), Some(true));
    }

    #[test]
    fn paste_tags_needs_copied_tags() {
        let mut copied = snap(1);
        copied.tags_copied = true;
        let entries = item_menu_model(&copied);
        assert_eq!(enabled(&entries, MenuCommand::PasteTags), Some(true));
    }

    #[test]
    fn trash_menu_restores_and_deletes_only() {
        let mut trash = snap(2);
        trash.in_trash = true;
        let entries = item_menu_model(&trash);
        let list = commands(&entries);
        assert!(list.contains(&MenuCommand::Restore));
        assert!(list.contains(&MenuCommand::DeletePermanently));
        assert!(!list.contains(&MenuCommand::Trash));
        assert!(!list.contains(&MenuCommand::Rename));
        assert!(
            !entries
                .iter()
                .any(|e| matches!(e, MenuEntry::Rating { .. }))
        );
    }

    #[test]
    fn read_only_library_keeps_reading_commands_only() {
        let mut read_only = snap(1);
        read_only.writable = false;
        let entries = item_menu_model(&read_only);
        assert_eq!(enabled(&entries, MenuCommand::Preview), Some(true));
        assert_eq!(enabled(&entries, MenuCommand::CopyFile), Some(true));
        assert_eq!(enabled(&entries, MenuCommand::CopyTags), Some(true));
        for command in [
            MenuCommand::Rename,
            MenuCommand::AddToFolders,
            MenuCommand::AddTags,
            MenuCommand::ClearTags,
            MenuCommand::Trash,
        ] {
            assert_eq!(enabled(&entries, command), Some(false), "{command:?}");
        }
        assert!(entries.contains(&MenuEntry::Rating { enabled: false }));
    }

    #[test]
    fn eagle_library_has_no_local_files_to_open_or_copy() {
        let mut eagle = snap(1);
        eagle.native = false;
        let entries = item_menu_model(&eagle);
        assert_eq!(enabled(&entries, MenuCommand::OpenDefault), Some(false));
        assert_eq!(enabled(&entries, MenuCommand::Reveal), Some(false));
        assert_eq!(enabled(&entries, MenuCommand::CopyPath), Some(false));
        assert_eq!(enabled(&entries, MenuCommand::CopyName), Some(true));
        assert_eq!(enabled(&entries, MenuCommand::Trash), Some(true));
    }

    #[test]
    fn folder_menu_follows_the_open_place() {
        let outside = folder_menu_model(&snap(0));
        assert_eq!(commands(&outside), vec![MenuCommand::NewFolder]);
        let mut open = snap(0);
        open.folder_open = true;
        assert_eq!(
            commands(&folder_menu_model(&open)),
            vec![
                MenuCommand::NewFolder,
                MenuCommand::RenameFolder,
                MenuCommand::LockFolder,
                MenuCommand::ExportFolder,
                MenuCommand::DeleteFolder,
            ]
        );
    }

    #[test]
    fn every_menu_label_is_in_the_english_catalog() {
        let mut snaps = vec![snap(0), snap(1), snap(2)];
        let mut trash = snap(1);
        trash.in_trash = true;
        snaps.push(trash);
        let mut open = snap(1);
        open.folder_open = true;
        snaps.push(open);
        for snap in &snaps {
            let entries = item_menu_model(snap)
                .into_iter()
                .chain(folder_menu_model(snap));
            for entry in entries {
                if let MenuEntry::Command { label, .. } = entry {
                    assert_ne!(t(Locale::En, label), label, "missing {label}");
                    assert_ne!(t(Locale::Ru, label), label, "missing ru {label}");
                    assert_ne!(t(Locale::ZhCn, label), label, "missing zh {label}");
                }
            }
        }
        assert_eq!(rating_label(Locale::En, 3), "★★★");
        assert_eq!(rating_label(Locale::En, 0), "Remove Rating");
    }

    #[test]
    fn flatten_skips_locked_folders_and_builds_paths() {
        let entries = flatten_pickable(&tree());
        let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "a1", "b"]);
        assert_eq!(entries[1].depth, 1);
        assert_eq!(entries[1].path, "Art / Sketches");
    }

    #[test]
    fn filter_ignores_case_and_keeps_tree_order() {
        let entries = flatten_pickable(&tree());
        let found: Vec<&str> = filter_entries(&entries, " ART ")
            .into_iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(found, vec!["a", "b"]);
        assert_eq!(filter_entries(&entries, "").len(), 3);
        assert!(filter_entries(&entries, "zzz").is_empty());
    }

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn one_item_sends_the_complete_folder_set() {
        let entries = flatten_pickable(&tree());
        let edit = folder_edit(true, &entries, &set(&["a"]), &set(&["b", "a1"]));
        assert_eq!(
            edit,
            Some(ItemEdit::SetFolders(vec!["a1".into(), "b".into()]))
        );
        assert_eq!(
            folder_edit(true, &entries, &set(&["a"]), &set(&["a"])),
            None
        );
        assert_eq!(
            folder_edit(true, &entries, &set(&["a"]), &set(&[])),
            Some(ItemEdit::SetFolders(Vec::new()))
        );
    }

    #[test]
    fn one_item_stays_in_a_folder_the_panel_does_not_list() {
        let entries = flatten_pickable(&tree());
        let edit = folder_edit(true, &entries, &set(&["a2"]), &set(&["a2", "b"]));
        assert_eq!(
            edit,
            Some(ItemEdit::SetFolders(vec!["b".into(), "a2".into()]))
        );
    }

    #[test]
    fn several_items_are_only_added() {
        let entries = flatten_pickable(&tree());
        assert_eq!(
            folder_edit(false, &entries, &set(&[]), &set(&["b", "a"])),
            Some(ItemEdit::AddToFolders(vec!["a".into(), "b".into()]))
        );
        assert_eq!(folder_edit(false, &entries, &set(&[]), &set(&[])), None);
    }

    #[test]
    fn tag_names_come_from_the_item_or_the_selection() {
        let mut details = DetailsModel {
            count: 1,
            item: Some(Item {
                tags: vec!["Red".into(), "red".into()],
                ..Item::default()
            }),
            thumb: None,
            selection: None,
            stack_thumbs: Vec::new(),
        };
        // Tags are case-sensitive: both stay.
        assert_eq!(selection_tag_names(&details), vec!["Red", "red"]);
        details.item = None;
        details.count = 2;
        details.selection = Some(SelectionTags {
            items: 2,
            tags: vec![Tag {
                name: "blue".into(),
                image_count: Some(2),
            }],
            size: 0,
            stack: Vec::new(),
        });
        assert_eq!(selection_tag_names(&details), vec!["blue"]);
        details.selection = None;
        assert!(selection_tag_names(&details).is_empty());
    }

    #[test]
    fn file_name_joins_name_and_extension() {
        assert_eq!(file_name("cat", "png"), "cat.png");
        assert_eq!(file_name("README", ""), "README");
    }
}
