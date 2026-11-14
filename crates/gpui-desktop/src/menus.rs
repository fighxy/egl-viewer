//! In-window menu bar. Library follows Eagle: Create, Open, Switch (Ctrl+L), Clear History,
//! Clear Cache and Reload. Merging two libraries is not a feature here, so it is not in the menu.

use std::path::PathBuf;

use app_store::{Locale, RecentLibrary, Theme, ViewMode, same_library_path};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::label::Label;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, px,
};
use library_core::{LibraryKind, SortKey};

use crate::app::RootApp;
use crate::browse::style;
use crate::commands::*;
use crate::i18n::{t, t_n};
use crate::library::Place;
use crate::platform;

#[derive(Clone)]
struct MenuSnap {
    locale: Locale,
    has_library: bool,
    writable: bool,
    fullscreen: bool,
    left_open: bool,
    right_open: bool,
    imports_open: bool,
    view_mode: ViewMode,
    show_names: bool,
    include_subfolders: bool,
    sort: SortKey,
    ascending: bool,
    folder_open: bool,
    theme: Theme,
    current_path: Option<String>,
    recent: Vec<RecentLibrary>,
    undo_label: String,
}

impl RootApp {
    fn menu_snap(&self) -> MenuSnap {
        let prefs = self.session.preferences();
        let lib = self.library.as_ref();
        let undo_label = match self.session.undo_status() {
            Some(status) => format!(
                "{} ({})",
                t(self.locale, "commands.edit.undo"),
                status.items
            ),
            None => t(self.locale, "commands.edit.undo"),
        };
        MenuSnap {
            locale: self.locale,
            has_library: lib.is_some(),
            writable: lib.is_some_and(|lib| !lib.info.read_only),
            fullscreen: self.fullscreen,
            left_open: lib.is_some_and(|lib| lib.left_open),
            right_open: lib.is_none_or(|lib| lib.right_open),
            imports_open: self.imports_open,
            view_mode: lib
                .map(|lib| lib.browse.view_mode)
                .unwrap_or(prefs.browse.view_mode),
            show_names: lib
                .map(|lib| lib.browse.show_names)
                .unwrap_or(prefs.browse.show_names),
            include_subfolders: lib
                .map(|lib| lib.browse.include_subfolders)
                .unwrap_or(prefs.browse.include_subfolders),
            sort: lib.map(|lib| lib.sort).unwrap_or(prefs.browse.sort),
            ascending: lib
                .map(|lib| lib.ascending)
                .unwrap_or(prefs.browse.ascending),
            folder_open: lib.is_some_and(|lib| matches!(lib.place, Place::Folder { .. })),
            theme: self.theme_pref,
            current_path: lib.map(|lib| lib.path.clone()),
            recent: self.session.recent_libraries(),
            undo_label,
        }
    }

    pub(crate) fn render_menubar(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let snap = self.menu_snap();
        let entity = cx.entity();
        let locale = snap.locale;
        h_flex()
            .id("menubar")
            .w_full()
            .h(px(style::MENU_BAR_HEIGHT))
            .flex_none()
            .items_center()
            .gap_1()
            .px_2()
            .border_b_1()
            .border_color(theme.border)
            .child({
                let snap = snap.clone();
                menu_button("menu-file", t(locale, "menus.file"))
                    .dropdown_menu(move |menu, _, _| file_menu(menu, &snap))
            })
            .child({
                let snap = snap.clone();
                menu_button("menu-edit", t(locale, "menus.edit"))
                    .dropdown_menu(move |menu, _, _| edit_menu(menu, &snap))
            })
            .child({
                let snap = snap.clone();
                menu_button("menu-view", t(locale, "menus.view"))
                    .dropdown_menu(move |menu, window, cx| view_menu(menu, window, cx, &snap))
            })
            .child({
                let snap = snap.clone();
                let entity = entity.clone();
                menu_button("menu-library", t(locale, "menus.library")).dropdown_menu(
                    move |menu, window, cx| library_menu(menu, window, cx, &snap, entity.clone()),
                )
            })
            .child({
                let snap = snap.clone();
                menu_button("menu-help", t(locale, "menus.help"))
                    .dropdown_menu(move |menu, _, _| help_menu(menu, &snap))
            })
            .when_some(self.eagle_badge.clone(), |el, badge| {
                el.child(
                    Label::new(badge)
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
            })
    }

    pub(crate) fn render_switch_overlay(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let recent = self.session.recent_libraries();
        let current = self.library.as_ref().map(|lib| lib.path.clone());
        let entity = cx.entity();
        v_flex()
            .id("switch-overlay")
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .bg(theme.background.opacity(0.55))
            .child(
                v_flex()
                    .w(px(420.))
                    .max_h(px(480.))
                    .gap_2()
                    .p_4()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(Label::new(t(locale, "libraryMenu.switchTitle")).font_bold())
                    .child(if recent.is_empty() {
                        Label::new(t(locale, "libraryMenu.noRecent"))
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .into_any_element()
                    } else {
                        v_flex()
                            .id("switch-list")
                            .w_full()
                            .gap_1()
                            .max_h(px(320.))
                            .overflow_y_scrollbar()
                            .children(recent.into_iter().map(|library| {
                                switch_row(
                                    library,
                                    current.as_deref(),
                                    locale,
                                    theme,
                                    entity.clone(),
                                )
                            }))
                            .into_any_element()
                    })
                    .child(
                        h_flex().justify_end().child(
                            Button::new("switch-cancel")
                                .ghost()
                                .label(t(locale, "common.cancel"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_switch = false;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
    }

    pub(crate) fn toggle_switch(&mut self, cx: &mut Context<Self>) {
        self.show_switch = !self.show_switch;
        cx.notify();
    }

    pub(crate) fn clear_recent_history(&mut self, cx: &mut Context<Self>) {
        let keep = self.library.as_ref().map(|lib| lib.path.clone());
        match self.session.clear_recent_history(keep.as_deref()) {
            Ok(0) => {
                self.status = t(self.locale, "libraryMenu.nothingToClear").into();
            }
            Ok(_) => {
                self.status = t(self.locale, "libraryMenu.historyCleared").into();
            }
            Err(error) => {
                self.status = error.to_string().into();
            }
        }
        cx.notify();
    }

    /// Clears the thumbnail cache we own (native `thumbs/`) by asking for previews again,
    /// then reloads the open view. Eagle serves its own thumbnails; a read-only library
    /// reloads without a rebuild.
    pub(crate) fn clear_cache_and_reload(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let kind = lib.info.kind;
        let read_only = lib.info.read_only;
        self.drop_preview(cx);
        if kind == LibraryKind::Native && !read_only {
            match self.session.rebuild_thumbnails() {
                Ok(waiting) => {
                    self.status = t_n(self.locale, "libraryMenu.cacheRebuilt", waiting).into();
                    self.refresh_view(cx);
                }
                Err(error) => {
                    self.status = error.to_string().into();
                    cx.notify();
                }
            }
            return;
        }
        let key = if kind == LibraryKind::Native {
            "libraryMenu.cacheReadOnly"
        } else {
            "libraryMenu.cacheEagle"
        };
        self.status = t(self.locale, key).into();
        self.refresh_view(cx);
    }

    pub(crate) fn reveal_library(&mut self, cx: &mut Context<Self>) {
        match self.session.library_folder() {
            Ok(path) => {
                if let Err(error) = platform::open_in_file_manager(&path) {
                    self.status = error.into();
                }
            }
            Err(error) => {
                self.status = error.to_string().into();
            }
        }
        cx.notify();
    }
}

/// Small buttons keep the menu bar as tall as one line of text (D-157).
fn menu_button(id: &'static str, label: String) -> Button {
    Button::new(id).ghost().small().compact().label(label)
}

fn file_menu(menu: PopupMenu, snap: &MenuSnap) -> PopupMenu {
    let locale = snap.locale;
    let closed = !snap.has_library;
    let locked = !snap.writable;
    menu.menu_with_disabled(
        t(locale, "commands.file.addFiles"),
        Box::new(AddFiles),
        locked,
    )
    .menu_with_disabled(
        t(locale, "commands.file.importFolder"),
        Box::new(ImportFolder),
        locked,
    )
    .separator()
    .menu_with_disabled(
        t(locale, "commands.file.closeLibrary"),
        Box::new(CloseLibrary),
        closed,
    )
    .separator()
    .menu(
        t(locale, "commands.file.decryptArchive"),
        Box::new(DecryptArchive),
    )
    .menu(
        t(locale, "commands.file.importEagle"),
        Box::new(ImportEagleLibrary),
    )
    .menu(
        t(locale, "commands.file.connectEagle"),
        Box::new(ConnectEagle),
    )
    .separator()
    .menu(t(locale, "commands.file.settings"), Box::new(Settings))
    .menu(t(locale, "commands.file.exit"), Box::new(Exit))
}

fn edit_menu(menu: PopupMenu, snap: &MenuSnap) -> PopupMenu {
    let locale = snap.locale;
    let closed = !snap.has_library;
    let locked = !snap.writable;
    menu.menu(snap.undo_label.clone(), Box::new(Undo))
        .menu_with_disabled(
            t(locale, "commands.edit.selectAll"),
            Box::new(SelectAll),
            closed,
        )
        .menu_with_disabled(t(locale, "commands.edit.find"), Box::new(Find), closed)
        .separator()
        .menu_with_disabled(
            t(locale, "commands.edit.pasteFiles"),
            Box::new(PasteFiles),
            locked,
        )
        .menu_with_disabled(
            t(locale, "commands.edit.trash"),
            Box::new(TrashSelection),
            locked,
        )
}

fn view_menu(
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
    snap: &MenuSnap,
) -> PopupMenu {
    let locale = snap.locale;
    let closed = !snap.has_library;
    let theme = snap.theme;
    let sort = snap.sort;
    let ascending = snap.ascending;
    let folder_open = snap.folder_open;
    menu.submenu(
        t(locale, "commands.view.theme"),
        window,
        cx,
        move |menu, _, _| {
            menu.menu_with_check(
                t(locale, "themes.system"),
                theme == Theme::System,
                Box::new(ThemeSystem),
            )
            .menu_with_check(
                t(locale, "themes.light"),
                theme == Theme::Light,
                Box::new(ThemeLight),
            )
            .menu_with_check(
                t(locale, "themes.dark"),
                theme == Theme::Dark,
                Box::new(ThemeDark),
            )
        },
    )
    .menu_with_check(
        t(locale, "commands.view.fullScreen"),
        snap.fullscreen,
        Box::new(ToggleFullScreen),
    )
    .separator()
    .menu_with_check_and_disabled(
        t(locale, "commands.view.sidebar"),
        snap.left_open,
        Box::new(ToggleSidebar),
        closed,
    )
    .menu_with_check_and_disabled(
        t(locale, "commands.view.details"),
        snap.right_open,
        Box::new(ToggleDetails),
        closed,
    )
    // The Imports panel: the toolbar shows its button only while it has something (D-175).
    .menu_with_check_and_disabled(
        t(locale, "commands.view.imports"),
        snap.imports_open,
        Box::new(ToggleImportsPanel),
        closed,
    )
    .separator()
    .menu_with_check_and_disabled(
        t(locale, "commands.view.grid"),
        snap.view_mode == ViewMode::Grid,
        Box::new(ViewGrid),
        closed,
    )
    .menu_with_check_and_disabled(
        t(locale, "commands.view.list"),
        snap.view_mode == ViewMode::List,
        Box::new(ViewList),
        closed,
    )
    .menu_with_disabled(
        t(locale, "commands.view.larger"),
        Box::new(LargerThumbs),
        closed,
    )
    .menu_with_disabled(
        t(locale, "commands.view.smaller"),
        Box::new(SmallerThumbs),
        closed,
    )
    .menu_with_check_and_disabled(
        t(locale, "commands.view.showNames"),
        snap.show_names,
        Box::new(ToggleShowNames),
        closed,
    )
    .submenu(
        t(locale, "commands.view.sortBy"),
        window,
        cx,
        move |menu, _, _| {
            let mut menu = menu
                .menu_with_check(
                    t(locale, "sort.added"),
                    sort == SortKey::DateAdded,
                    Box::new(SortByAdded),
                )
                .menu_with_check(
                    t(locale, "sort.btime"),
                    sort == SortKey::DateCreated,
                    Box::new(SortByCreated),
                )
                .menu_with_check(
                    t(locale, "sort.mtime"),
                    sort == SortKey::DateModified,
                    Box::new(SortByModified),
                )
                .menu_with_check(
                    t(locale, "sort.name"),
                    sort == SortKey::Title,
                    Box::new(SortByName),
                )
                .menu_with_check(
                    t(locale, "sort.ext"),
                    sort == SortKey::Extension,
                    Box::new(SortByExt),
                )
                .menu_with_check(
                    t(locale, "sort.size"),
                    sort == SortKey::FileSize,
                    Box::new(SortBySize),
                )
                .menu_with_check(
                    t(locale, "sort.resolution"),
                    sort == SortKey::Resolution,
                    Box::new(SortByResolution),
                )
                .menu_with_check(
                    t(locale, "sort.star"),
                    sort == SortKey::Rating,
                    Box::new(SortByRating),
                )
                .menu_with_check(
                    t(locale, "sort.tag"),
                    sort == SortKey::FirstTag,
                    Box::new(SortByTag),
                )
                .menu_with_check(
                    t(locale, "sort.random"),
                    sort == SortKey::Random,
                    Box::new(SortByRandom),
                );
            if folder_open {
                menu = menu.menu_with_check(
                    t(locale, "sort.manual"),
                    sort == SortKey::Manual,
                    Box::new(SortByManual),
                );
            }
            menu.separator()
                .menu_with_check(
                    t(locale, "sort.ascending"),
                    ascending,
                    Box::new(SortAscending),
                )
                .menu_with_check(
                    t(locale, "sort.descending"),
                    !ascending,
                    Box::new(SortDescending),
                )
        },
    )
    .separator()
    .menu_with_disabled(
        t(locale, "commands.view.preview"),
        Box::new(Preview),
        closed,
    )
    .menu_with_check_and_disabled(
        t(locale, "commands.view.includeSubfolders"),
        snap.include_subfolders,
        Box::new(ToggleIncludeSubfolders),
        closed,
    )
}

fn library_menu(
    menu: PopupMenu,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
    snap: &MenuSnap,
    entity: Entity<RootApp>,
) -> PopupMenu {
    let locale = snap.locale;
    let closed = !snap.has_library;
    let locked = !snap.writable;
    let recent = snap.recent.clone();
    let current = snap.current_path.clone();
    menu.menu(t(locale, "libraryMenu.create"), Box::new(NewLibrary))
        .menu(t(locale, "libraryMenu.open"), Box::new(OpenLibrary))
        .submenu(
            format!("{}    Ctrl+L", t(locale, "libraryMenu.switch")),
            window,
            cx,
            move |menu, _, _| {
                switch_submenu(
                    menu,
                    locale,
                    recent.clone(),
                    current.clone(),
                    entity.clone(),
                )
            },
        )
        .separator()
        .menu(
            t(locale, "libraryMenu.clearHistory"),
            Box::new(ClearRecentHistory),
        )
        .menu_with_disabled(
            t(locale, "libraryMenu.clearCache"),
            Box::new(ClearCacheReload),
            closed,
        )
        .separator()
        .menu_with_disabled(
            t(locale, "commands.folder.new"),
            Box::new(NewFolder),
            locked,
        )
        .menu_with_disabled(
            t(locale, "commands.library.lockAll"),
            Box::new(LockAll),
            locked,
        )
        .separator()
        .menu_with_disabled(
            t(locale, "commands.library.settings"),
            Box::new(LibrarySettings),
            closed,
        )
        .menu_with_disabled(
            t(locale, "commands.library.backup"),
            Box::new(BackupExport),
            closed,
        )
        .menu_with_disabled(
            t(locale, "librarySettings.reveal"),
            Box::new(RevealLibrary),
            closed,
        )
}

fn switch_submenu(
    menu: PopupMenu,
    locale: Locale,
    recent: Vec<RecentLibrary>,
    current: Option<String>,
    entity: Entity<RootApp>,
) -> PopupMenu {
    if recent.is_empty() {
        return menu.label(t(locale, "libraryMenu.noRecent"));
    }
    let mut menu = menu;
    for library in recent {
        let path = library.path.clone();
        let is_current = current
            .as_ref()
            .is_some_and(|open| same_library_path(open, &path));
        let name = if library.missing {
            format!("{} ({})", library.name, t(locale, "common.missing"))
        } else {
            library.name.clone()
        };
        let path_label = path.clone();
        menu = menu.item(
            PopupMenuItem::element({
                let name = name.clone();
                let path_label = path_label.clone();
                move |_window, cx: &mut App| {
                    let muted = cx.theme().muted_foreground;
                    v_flex()
                        .w(px(280.))
                        .child(Label::new(name.clone()).text_sm())
                        .child(Label::new(path_label.clone()).text_xs().text_color(muted))
                }
            })
            .checked(is_current)
            .on_click({
                let entity = entity.clone();
                move |_event, window, cx| {
                    let path = path.clone();
                    entity.update(cx, |_this, cx| {
                        cx.defer_in(window, move |this, window, cx| {
                            this.show_switch = false;
                            this.open_path(PathBuf::from(path), window, cx);
                        });
                    });
                }
            }),
        );
    }
    menu
}

fn help_menu(menu: PopupMenu, snap: &MenuSnap) -> PopupMenu {
    let locale = snap.locale;
    menu.menu(t(locale, "commands.help.shortcuts"), Box::new(Shortcuts))
        .menu(
            t(locale, "commands.help.checkForUpdates"),
            Box::new(CheckForUpdates),
        )
        .separator()
        .menu(
            t(locale, "commands.help.openLogFolder"),
            Box::new(OpenLogFolder),
        )
        .menu(t(locale, "commands.help.about"), Box::new(About))
}

fn switch_row(
    library: RecentLibrary,
    current: Option<&str>,
    locale: Locale,
    theme: &gpui_kit::component::theme::Theme,
    entity: Entity<RootApp>,
) -> gpui_kit::AnyElement {
    let path = library.path.clone();
    let is_current = current.is_some_and(|open| same_library_path(open, &path));
    let mark = if is_current { "✓  " } else { "    " };
    let missing = if library.missing {
        format!(" ({})", t(locale, "common.missing"))
    } else {
        String::new()
    };
    v_flex()
        .id(gpui_kit::SharedString::from(format!(
            "switch-{}",
            library.path
        )))
        .w_full()
        .gap_0()
        .px_2()
        .py_1()
        .rounded(theme.radius)
        .when(is_current, |el| el.bg(theme.accent))
        .cursor_pointer()
        .on_click({
            let entity = entity.clone();
            move |_event, window, cx| {
                let path = path.clone();
                entity.update(cx, |_this, cx| {
                    cx.defer_in(window, move |this, window, cx| {
                        this.show_switch = false;
                        this.open_path(PathBuf::from(path), window, cx);
                    });
                });
            }
        })
        .child(Label::new(format!("{mark}{}{missing}", library.name)).text_sm())
        .child(
            Label::new(library.path)
                .text_xs()
                .text_color(theme.muted_foreground),
        )
        .into_any_element()
}
