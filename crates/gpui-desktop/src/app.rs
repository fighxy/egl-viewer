//! Root GPUI entity: Welcome ↔ Library shell, commands, dialogs (Phase 1–2 / D-129 / D-130).

use std::path::PathBuf;
use std::time::Duration;

use app_store::{BrowsePreferences, Locale, Theme, ViewMode};
use desktop_session::{Event, ItemsTarget, Session};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::{
    ActiveTheme, Disableable as _, StyledExt as _, WindowExt as _, h_flex, h_resizable,
    resizable_panel, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    PathPromptOptions, Pixels, Render, SharedString, Styled, Window, div, px,
};
use library_core::{ItemId, LibraryInfo, SortKey, is_web_link};

use crate::browse::details::{self, DetailsModel};
use crate::browse::grid::{self, ListedItem};
use crate::browse::preview::{self, PreviewState, SpaceGesture};
use crate::browse::preview_image::{self, PreviewPayload};
use crate::browse::sidebar;
use crate::commands::{self, *};
use crate::i18n::{self, t};
use crate::justified::{self, JustifiedLayout};
use crate::library::{self, EventMailbox, OpenLibraryState, Place, window_title};
use crate::platform;
use crate::screens::library as library_ui;
use crate::theme;

/// Rubber-band selection rectangle in grid content coordinates.
#[derive(Debug, Clone, Copy)]
pub struct RubberBand {
    pub start: (f32, f32),
    pub current: (f32, f32),
}

pub struct RootApp {
    session: Session,
    events: EventMailbox,
    focus: FocusHandle,
    locale: Locale,
    theme_pref: Theme,
    status: SharedString,
    pub(crate) library: Option<OpenLibraryState>,
    items: Vec<ListedItem>,
    justified: Option<JustifiedLayout>,
    grid_width: f32,
    grid_origin: (f32, f32),
    rubber: Option<RubberBand>,
    fullscreen: bool,
    quit_pending: bool,
    create_name: Entity<InputState>,
    create_location: SharedString,
    show_create: bool,
    details: DetailsModel,
    preview: Option<PreviewState>,
    preview_payload: Option<PreviewPayload>,
    space_gesture: Option<SpaceGesture>,
}

impl RootApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let events = library::new_event_mailbox();
        let session = library::session_with_mailbox(events.clone());
        let prefs = session.preferences();
        theme::apply_theme(prefs.theme, cx);

        let create_name =
            cx.new(|cx| InputState::new(window, cx).default_value("My Library"));

        let mut app = Self {
            session,
            events,
            focus: cx.focus_handle(),
            locale: prefs.locale,
            theme_pref: prefs.theme,
            status: SharedString::from(""),
            library: None,
            items: Vec::new(),
            justified: None,
            grid_width: 800.0,
            grid_origin: (0.0, 0.0),
            rubber: None,
            fullscreen: false,
            quit_pending: false,
            create_name,
            create_location: SharedString::from(""),
            show_create: false,
            details: DetailsModel {
                count: 0,
                item: None,
                thumb: None,
                selection: None,
                stack_thumbs: Vec::new(),
            },
            preview: None,
            preview_payload: None,
            space_gesture: None,
        };
        app.create_location = SharedString::from(app.session.default_library_location());

        let startup = app.session.startup(platform::skip_reopen());
        if let Some(notice) = startup.notice {
            app.status = SharedString::from(notice);
        }
        if let Some(info) = startup.library {
            app.adopt_library(info, window, cx);
        }

        if let Some(arg) = std::env::args().nth(1).map(PathBuf::from) {
            if arg.is_dir() {
                let _ = app.open_path(arg, window, cx);
            }
        }

        let entity = cx.entity();
        window.on_window_should_close(cx, move |window, cx| {
            entity.update(cx, |this, cx| this.request_quit(window, cx))
        });

        // Poll Session Emit mailbox → Entity notify (views-invalidated).
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(32))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.poll_session_events(cx);
                        this.poll_preview_tick(cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        window.set_window_title(&window_title(None));
        app
    }

    fn poll_preview_tick(&mut self, cx: &mut Context<Self>) {
        let mut notify = false;
        if let Some(gesture) = self.space_gesture.as_mut() {
            if gesture.mark_held_if_due() {
                notify = true;
            }
            if !platform::space_key_down() {
                if let Some(gesture) = self.space_gesture.take() {
                    if gesture.end() {
                        self.close_preview(cx);
                        return;
                    }
                    notify = true;
                }
            }
        }
        if let Some(preview) = self.preview.as_mut() {
            if preview.tick_appear() {
                notify = true;
            }
        }
        if notify {
            cx.notify();
        }
    }

    fn poll_session_events(&mut self, cx: &mut Context<Self>) {
        let pending: Vec<Event> = self
            .events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default();
        if pending.is_empty() {
            return;
        }
        let mut needs_refresh = false;
        for event in pending {
            match event {
                Event::ViewsInvalidated(views) => {
                    if let Some(lib) = self.library.as_mut() {
                        if let Some(updated) =
                            views.views.iter().find(|v| v.view_id == lib.view_id)
                        {
                            lib.total = updated.total;
                            needs_refresh = true;
                        }
                    }
                }
                Event::ImportProgress(_) | Event::ExportProgress(_) => {}
            }
        }
        if needs_refresh {
            self.reload_current_window(cx);
        }
    }

    fn reload_current_window(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        match self.session.view_range(lib.view_id, 0, 500) {
            Ok(cells) => {
                lib.cells = cells;
                lib.dimensions = self
                    .session
                    .view_dimensions(lib.view_id)
                    .unwrap_or_default();
                self.items = grid::load_listed(&self.session, &lib.cells);
                self.recompute_justified();
                cx.notify();
            }
            Err(error) => {
                // View may have been dropped; fall back to a fresh open.
                self.status = SharedString::from(format!("View refresh failed: {error}"));
                self.refresh_view(cx);
            }
        }
    }

    fn recompute_justified(&mut self) {
        let Some(lib) = self.library.as_ref() else {
            self.justified = None;
            return;
        };
        if lib.browse.view_mode != ViewMode::Grid || lib.cells.is_empty() {
            self.justified = None;
            return;
        }
        self.justified = Some(grid::build_justified(lib, self.grid_width));
    }

    fn adopt_library(&mut self, info: LibraryInfo, window: &mut Window, cx: &mut Context<Self>) {
        match OpenLibraryState::load(&self.session, info) {
            Ok(state) => {
                window.set_window_title(&window_title(Some(&state.info)));
                self.items = grid::load_listed(&self.session, &state.cells);
                self.status = SharedString::from("");
                self.library = Some(state);
                self.preview = None;
                self.preview_payload = None;
                self.space_gesture = None;
                self.recompute_justified();
                self.refresh_details(cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("Open failed: {error}"));
                let _ = self.session.close_library();
                cx.notify();
            }
        }
    }

    fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.take() {
            lib.persist_session(&self.session);
            lib.persist_browse_prefs(&self.session);
            self.session.close_view(lib.view_id);
        }
        let _ = self.session.close_library();
        match self.session.open_library(&path.to_string_lossy()) {
            Ok(info) => self.adopt_library(info, window, cx),
            Err(error) => {
                self.status = SharedString::from(format!("Open failed: {error}"));
                self.library = None;
                self.items.clear();
                window.set_window_title(&window_title(None));
                cx.notify();
            }
        }
    }

    fn close_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.take() {
            lib.persist_session(&self.session);
            lib.persist_browse_prefs(&self.session);
            self.session.close_view(lib.view_id);
        }
        let _ = self.session.close_library();
        self.items.clear();
        self.justified = None;
        self.rubber = None;
        self.preview = None;
        self.preview_payload = None;
        self.space_gesture = None;
        self.details = DetailsModel {
            count: 0,
            item: None,
            thumb: None,
            selection: None,
            stack_thumbs: Vec::new(),
        };
        window.set_window_title(&window_title(None));
        cx.notify();
    }

    fn refresh_view(&mut self, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            if let Err(error) = lib.refresh_view(&self.session) {
                self.status = SharedString::from(format!("View failed: {error}"));
            } else {
                self.items = grid::load_listed(&self.session, &lib.cells);
                lib.persist_session(&self.session);
                lib.persist_browse_prefs(&self.session);
                self.recompute_justified();
            }
            cx.notify();
        }
    }

    fn set_sort(&mut self, sort: SortKey, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.sort = sort;
        }
        self.refresh_view(cx);
    }

    fn set_ascending(&mut self, ascending: bool, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.ascending = ascending;
        }
        self.refresh_view(cx);
    }

    pub(crate) fn set_place(&mut self, place: Place, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.place = place;
            lib.selection.clear();
        }
        self.refresh_view(cx);
    }

    fn pick_open_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from(t(self.locale, "welcome.open"))),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = match rx.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            if let Some(path) = path {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.open_path(path, window, cx);
                });
            }
        })
        .detach();
    }

    fn begin_create_library(&mut self, cx: &mut Context<Self>) {
        self.create_location = SharedString::from(self.session.default_library_location());
        self.show_create = true;
        cx.notify();
    }

    fn confirm_create_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.create_name.read(cx).value().to_string();
        let parent = self.create_location.to_string();
        self.show_create = false;
        match self.session.create_library(&parent, name.trim()) {
            Ok(info) => self.adopt_library(info, window, cx),
            Err(error) => {
                self.status = SharedString::from(format!("Create failed: {error}"));
                cx.notify();
            }
        }
    }

    fn pick_create_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from(t(self.locale, "createLibrary.location"))),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update(cx, |this, cx| {
                        this.create_location =
                            SharedString::from(path.to_string_lossy().to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn set_theme(&mut self, theme_pref: Theme, cx: &mut Context<Self>) {
        self.theme_pref = theme_pref;
        let mut prefs = self.session.preferences();
        prefs.theme = theme_pref;
        let _ = self.session.save_preferences(prefs);
        theme::apply_theme(theme_pref, cx);
        cx.notify();
    }

    fn set_locale(&mut self, locale: Locale, cx: &mut Context<Self>) {
        self.locale = locale;
        let mut prefs = self.session.preferences();
        prefs.locale = locale;
        let _ = self.session.save_preferences(prefs);
        cx.notify();
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let theme_pref = self.theme_pref;
        let prefs = self.session.preferences();
        let reopen = prefs.reopen_last_library;
        let confirm_trash = prefs.confirm_before_trash;
        let default_folder = prefs
            .default_library_parent
            .clone()
            .unwrap_or_else(|| self.session.default_library_location());
        let entity = cx.entity();

        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "settings.title")).child(
                v_flex()
                    .gap_3()
                    .p_2()
                    .w(px(440.))
                    .child(Label::new(t(locale, "settings.general")).font_bold())
                    .child(Label::new(format!(
                        "{}: {}",
                        t(locale, "settings.theme"),
                        t(locale, theme::theme_label_key(theme_pref))
                    )))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(theme_btn(locale, Theme::System, theme_pref, &entity))
                            .child(theme_btn(locale, Theme::Light, theme_pref, &entity))
                            .child(theme_btn(locale, Theme::Dark, theme_pref, &entity)),
                    )
                    .child(Label::new(t(locale, "settings.language")))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(locale_btn("en", Locale::En, locale, &entity))
                            .child(locale_btn("ru", Locale::Ru, locale, &entity))
                            .child(locale_btn("zh", Locale::ZhCn, locale, &entity)),
                    )
                    .child(pref_toggle(
                        "reopen",
                        t(locale, "settings.reopen"),
                        reopen,
                        &entity,
                        |p, on| p.reopen_last_library = on,
                    ))
                    .child(pref_toggle(
                        "trash",
                        t(locale, "settings.confirmTrash"),
                        confirm_trash,
                        &entity,
                        |p, on| p.confirm_before_trash = on,
                    ))
                    .child(Label::new(format!(
                        "{}: {default_folder}",
                        t(locale, "settings.defaultFolder")
                    )))
                    .child(
                        Button::new("settings-browse-default")
                            .label(t(locale, "common.browse"))
                            .on_click({
                                let entity = entity.clone();
                                move |_, _, cx| {
                                    let entity = entity.clone();
                                    let rx = cx.prompt_for_paths(PathPromptOptions {
                                        files: false,
                                        directories: true,
                                        multiple: false,
                                        prompt: Some(SharedString::from("Default library folder")),
                                    });
                                    cx.spawn(async move |cx| {
                                        if let Ok(Ok(Some(paths))) = rx.await {
                                            if let Some(path) = paths.into_iter().next() {
                                                entity
                                                    .update(cx, |this, cx| {
                                                        let mut prefs = this.session.preferences();
                                                        prefs.default_library_parent =
                                                            Some(path.to_string_lossy().into());
                                                        let _ = this.session.save_preferences(prefs);
                                                        cx.notify();
                                                    })
                                                    ;
                                            }
                                        }
                                    })
                                    .detach();
                                }
                            }),
                    ),
            )
        });
    }

    fn open_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let version = library::APP_VERSION;
        let commit = library::APP_COMMIT.unwrap_or("unknown");
        let os = std::env::consts::OS;
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "about.title")).child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .child(Label::new(format!(
                        "{}: {version}",
                        t(locale, "about.version")
                    )))
                    .child(Label::new(format!(
                        "{}: {commit}",
                        t(locale, "about.commit")
                    )))
                    .child(Label::new(format!(
                        "{}: gpui-kit 0.9.0",
                        t(locale, "about.toolkit")
                    )))
                    .child(Label::new(format!("{}: {os}", t(locale, "about.os")))),
            )
        });
    }

    fn open_sort_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let locale = self.locale;
        let current = lib.sort;
        let ascending = lib.ascending;
        let folder_open = matches!(lib.place, Place::Folder { .. });
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let keys: &[(&str, SortKey)] = if folder_open {
                &[
                    ("sort.added", SortKey::DateAdded),
                    ("sort.btime", SortKey::DateCreated),
                    ("sort.mtime", SortKey::DateModified),
                    ("sort.name", SortKey::Title),
                    ("sort.ext", SortKey::Extension),
                    ("sort.size", SortKey::FileSize),
                    ("sort.resolution", SortKey::Resolution),
                    ("sort.star", SortKey::Rating),
                    ("sort.tag", SortKey::FirstTag),
                    ("sort.random", SortKey::Random),
                    ("sort.manual", SortKey::Manual),
                ]
            } else {
                &[
                    ("sort.added", SortKey::DateAdded),
                    ("sort.btime", SortKey::DateCreated),
                    ("sort.mtime", SortKey::DateModified),
                    ("sort.name", SortKey::Title),
                    ("sort.ext", SortKey::Extension),
                    ("sort.size", SortKey::FileSize),
                    ("sort.resolution", SortKey::Resolution),
                    ("sort.star", SortKey::Rating),
                    ("sort.tag", SortKey::FirstTag),
                    ("sort.random", SortKey::Random),
                ]
            };
            dialog.title(t(locale, "sort.title")).child(
                v_flex()
                    .gap_1()
                    .p_2()
                    .w(px(320.))
                    .children(keys.iter().map(|(key, sort)| {
                        let entity = entity.clone();
                        let sort = *sort;
                        Button::new(SharedString::from(format!("sort-{key}")))
                            .label(t(locale, key))
                            .when(sort == current, |b| b.primary())
                            .when(sort != current, |b| b.ghost())
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| this.set_sort(sort, cx));
                            })
                            .into_any_element()
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .pt_2()
                            .child({
                                let entity = entity.clone();
                                Button::new("sort-asc")
                                    .label(t(locale, "sort.ascending"))
                                    .when(ascending, |b| b.primary())
                                    .when(!ascending, |b| b.ghost())
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| this.set_ascending(true, cx));
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Button::new("sort-desc")
                                    .label(t(locale, "sort.descending"))
                                    .when(!ascending, |b| b.primary())
                                    .when(ascending, |b| b.ghost())
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| this.set_ascending(false, cx));
                                    })
                            }),
                    ),
            )
        });
    }

    fn open_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let rows: Vec<(String, String)> = commands::SHORTCUTS
            .iter()
            .map(|e| (t(locale, e.label_key), e.keys.to_string()))
            .collect();
        window.open_dialog(cx, move |dialog, _, _| {
            let rows = rows.clone();
            dialog.title(t(locale, "shortcuts.title")).child(
                v_flex()
                    .gap_1()
                    .p_2()
                    .w(px(420.))
                    .children(rows.into_iter().map(|(label, keys)| {
                        h_flex()
                            .w_full()
                            .justify_between()
                            .child(Label::new(label).text_sm())
                            .child(Label::new(keys).text_sm())
                            .into_any_element()
                    })),
            )
        });
    }

    fn open_log_folder(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let dir = platform::log_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Err(error) = platform::open_in_file_manager(&dir) {
            self.status = SharedString::from(error);
            cx.notify();
        }
    }

    fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.quit_pending {
            self.shutdown();
            return true;
        }
        self.quit_pending = true;
        let locale = self.locale;
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(t(locale, "quit.title"))
                .child(Label::new(t(locale, "quit.message")))
                .on_ok({
                    let entity = entity.clone();
                    move |_, window, cx| {
                        entity
                            .update(cx, |this, _cx| {
                                this.shutdown();
                            })
                            ;
                        window.remove_window();
                        true
                    }
                })
                .on_cancel({
                    let entity = entity.clone();
                    move |_, _, cx| {
                        entity
                            .update(cx, |this, cx| {
                                this.quit_pending = false;
                                cx.notify();
                            })
                            ;
                        true
                    }
                })
        });
        false
    }

    fn shutdown(&mut self) {
        if let Some(lib) = self.library.take() {
            lib.persist_session(&self.session);
            lib.persist_browse_prefs(&self.session);
            self.session.close_view(lib.view_id);
        }
        let _ = self.session.close_library();
    }

    pub(crate) fn select_item(
        &mut self,
        index: usize,
        ctrl: bool,
        shift: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        let Some(cell) = lib.cells.get(index).cloned() else {
            return;
        };
        if shift {
            lib.selection.extend_to(&lib.cells, index);
        } else if ctrl {
            lib.selection.toggle(&cell.id.0, index);
        } else {
            lib.selection.select_only(&cell.id.0, index);
        }
        lib.persist_session(&self.session);
        self.refresh_details(cx);
    }

    pub(crate) fn begin_rubber(&mut self, window_pos: (f32, f32), cx: &mut Context<Self>) {
        let origin = self.grid_origin;
        let content = (window_pos.0 - origin.0, window_pos.1 - origin.1);
        self.rubber = Some(RubberBand {
            start: content,
            current: content,
        });
        cx.notify();
    }

    pub(crate) fn update_rubber(&mut self, window_pos: (f32, f32), cx: &mut Context<Self>) {
        let Some(band) = self.rubber.as_mut() else {
            return;
        };
        let origin = self.grid_origin;
        band.current = (window_pos.0 - origin.0, window_pos.1 - origin.1);
        cx.notify();
    }

    pub(crate) fn end_rubber(&mut self, cx: &mut Context<Self>) {
        let Some(band) = self.rubber.take() else {
            return;
        };
        let rect = justified::band_rect(band.start, band.current);
        if (rect.right - rect.left) < 3.0 && (rect.bottom - rect.top) < 3.0 {
            cx.notify();
            return;
        }
        let Some(lib) = self.library.as_mut() else {
            cx.notify();
            return;
        };
        let indices = if let Some(layout) = self.justified.as_ref() {
            justified::indices_in_rect(layout, lib.cells.len(), rect)
        } else {
            // List mode: one row ≈ 48px with 8px padding.
            let mut found = Vec::new();
            for index in 0..lib.cells.len() {
                let top = 8.0 + index as f32 * 48.0;
                let bottom = top + 48.0;
                if bottom >= rect.top && top <= rect.bottom {
                    found.push(index);
                }
            }
            found
        };
        if let Some(&focus) = indices.last() {
            lib.selection
                .select_indices(&lib.cells, &indices, focus);
            lib.persist_session(&self.session);
        }
        self.refresh_details(cx);
    }

    fn move_focus(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.preview.is_some() {
            match key {
                "left" => self.preview_step(-1, cx),
                "right" => self.preview_step(1, cx),
                _ => {}
            }
            return;
        }
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        let total = lib.cells.len();
        if total == 0 {
            return;
        }
        let page_rows = 5usize;
        let next = if let Some(layout) = self.justified.as_ref() {
            justified::move_focus_justified(key, lib.selection.focus, layout, page_rows)
        } else {
            let columns = if lib.browse.view_mode == ViewMode::List {
                1
            } else {
                ((self.grid_width / (lib.browse.thumbnail_size as f32 + 16.0)).floor() as usize)
                    .max(1)
            };
            justified::move_focus_list(key, lib.selection.focus, total, columns, page_rows)
        };
        let Some(index) = next else {
            return;
        };
        if let Some(cell) = lib.cells.get(index).cloned() {
            lib.selection.select_only(&cell.id.0, index);
            lib.persist_session(&self.session);
            self.refresh_details(cx);
        }
    }

    fn focus_details_panel(&mut self, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            lib.right_open = true;
            lib.persist_browse_prefs(&self.session);
            self.refresh_details(cx);
        }
    }

    fn refresh_details(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            self.details = DetailsModel {
                count: 0,
                item: None,
                thumb: None,
                selection: None,
                stack_thumbs: Vec::new(),
            };
            cx.notify();
            return;
        };
        let count = lib.selection.count();
        if count == 0 {
            self.details = DetailsModel {
                count: 0,
                item: None,
                thumb: None,
                selection: None,
                stack_thumbs: Vec::new(),
            };
            cx.notify();
            return;
        }
        if count == 1 {
            let id = lib
                .selection
                .ids
                .iter()
                .next()
                .cloned()
                .or_else(|| {
                    lib.selection
                        .focus
                        .and_then(|i| lib.cells.get(i).map(|c| c.id.0.clone()))
                });
            let Some(id) = id else {
                self.details.count = 1;
                cx.notify();
                return;
            };
            let item = self.session.item_detail(&id).ok().flatten();
            let thumb = lib
                .cells
                .iter()
                .find(|c| c.id.0 == id)
                .and_then(|cell| preview_image::thumb_image(&self.session, cell))
                .or_else(|| {
                    item.as_ref().and_then(|item| {
                        if item.no_thumbnail {
                            return None;
                        }
                        let cell = library_core::ItemCell {
                            id: item.id.clone(),
                            name: item.name.clone(),
                            ext: item.ext.clone(),
                            has_thumbnail: !item.no_thumbnail,
                            thumb_version: item.thumb_version,
                            ..Default::default()
                        };
                        preview_image::thumb_image(&self.session, &cell)
                    })
                });
            self.details = DetailsModel {
                count: 1,
                item,
                thumb,
                selection: None,
                stack_thumbs: Vec::new(),
            };
            cx.notify();
            return;
        }
        let ids: Vec<ItemId> = lib
            .selection
            .ids
            .iter()
            .cloned()
            .map(ItemId)
            .collect();
        let target = ItemsTarget {
            view_id: Some(lib.view_id),
            all: false,
            ids,
        };
        let selection = self.session.selection_tags(target).ok();
        let stack_thumbs = selection
            .as_ref()
            .map(|sel| {
                sel.stack
                    .iter()
                    .map(|cell| preview_image::thumb_image(&self.session, cell))
                    .collect()
            })
            .unwrap_or_default();
        self.details = DetailsModel {
            count,
            item: None,
            thumb: None,
            selection,
            stack_thumbs,
        };
        cx.notify();
    }

    fn selection_target(&self) -> Option<ItemsTarget> {
        let lib = self.library.as_ref()?;
        let ids: Vec<ItemId> = lib.selection.ids.iter().cloned().map(ItemId).collect();
        if ids.is_empty() {
            return None;
        }
        Some(ItemsTarget {
            view_id: Some(lib.view_id),
            all: false,
            ids,
        })
    }

    pub(crate) fn open_preview(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let index = lib
            .selection
            .focus
            .or_else(|| {
                lib.selection.ids.iter().next().and_then(|id| {
                    lib.cells.iter().position(|c| c.id.0 == *id)
                })
            });
        let Some(index) = index else {
            return;
        };
        self.open_preview_at(index, cx);
    }

    pub(crate) fn open_preview_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let Some(cell) = lib.cells.get(index).cloned() else {
            return;
        };
        let total = lib.total.max(lib.cells.len() as u32);
        let payload = preview_image::preview_payload(&self.session, &cell);
        let image = payload.as_ref().map(|p| p.to_image());
        self.preview_payload = payload;
        self.preview = Some(PreviewState::new(
            index,
            total,
            format!("{}.{}", cell.name, cell.ext),
            image,
        ));
        if let Some(lib) = self.library.as_mut() {
            lib.selection.select_only(&cell.id.0, index);
            lib.persist_session(&self.session);
        }
        self.refresh_details(cx);
    }

    pub(crate) fn close_preview(&mut self, cx: &mut Context<Self>) {
        self.preview = None;
        self.preview_payload = None;
        cx.notify();
    }

    pub(crate) fn preview_step(&mut self, step: i32, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_ref() else {
            return;
        };
        let next = preview.index as i32 + step;
        if next < 0 {
            return;
        }
        let next = next as usize;
        let total = self
            .library
            .as_ref()
            .map(|lib| lib.cells.len())
            .unwrap_or(0);
        if next >= total {
            return;
        }
        let mut state = PreviewState::new(next, preview.total, "", None);
        state.appear = 1.0;
        self.preview = Some(state);
        self.open_preview_at(next, cx);
    }

    pub(crate) fn preview_zoom(&mut self, factor: f32, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            let next = preview.zoom * factor;
            preview.zoom_to(next);
            cx.notify();
        }
    }

    pub(crate) fn preview_zoom_reset(&mut self, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            preview.zoom_to(1.0);
            cx.notify();
        }
    }

    pub(crate) fn preview_rotate(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        preview.rotate_cw();
        let degrees = preview.rotation;
        if let Some(payload) = self.preview_payload.as_ref() {
            preview.image = payload.rotated(degrees);
        }
        cx.notify();
    }

    pub(crate) fn preview_pan_start(&mut self, origin_x: f32, origin_y: f32, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            if preview.zoom > 1.0 {
                preview.drag_origin = Some((origin_x, origin_y));
                cx.notify();
            }
        }
    }

    pub(crate) fn preview_pan_move(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        let Some((ox, oy)) = preview.drag_origin else {
            return;
        };
        preview.offset = (x - ox, y - oy);
        cx.notify();
    }

    pub(crate) fn preview_pan_end(&mut self, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            preview.drag_origin = None;
            cx.notify();
        }
    }

    fn space_preview_down(&mut self, cx: &mut Context<Self>) {
        if self.library.is_none() {
            return;
        }
        if self.space_gesture.is_some() {
            return;
        }
        let is_open = self.preview.is_some();
        let (gesture, should_open) = SpaceGesture::begin(is_open);
        self.space_gesture = Some(gesture);
        if should_open {
            self.open_preview(cx);
        } else {
            cx.notify();
        }
    }

    pub(crate) fn open_original(&mut self, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        let id = lib
            .selection
            .focus
            .and_then(|i| lib.cells.get(i).map(|c| c.id.clone()))
            .or_else(|| lib.selection.ids.iter().next().map(|id| ItemId(id.clone())));
        let Some(id) = id else {
            return;
        };
        match self.session.original_paths(&[id]) {
            Ok(paths) => {
                for path in paths {
                    if let Err(error) = platform::open_path(&path) {
                        self.status = SharedString::from(error);
                    }
                }
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
            }
        }
        cx.notify();
    }

    pub(crate) fn reveal_selection(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.selection_target() else {
            return;
        };
        match self
            .session
            .selected_ids(target)
            .and_then(|ids| self.session.original_paths(&ids))
        {
            Ok(paths) => {
                if let Err(error) = platform::reveal_paths(&paths) {
                    self.status = SharedString::from(error);
                }
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
            }
        }
        cx.notify();
    }

    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.selection_target() else {
            return;
        };
        match self
            .session
            .selected_ids(target)
            .and_then(|ids| self.session.original_paths(&ids))
        {
            Ok(paths) => match platform::copy_files(&paths) {
                Ok(n) => {
                    self.status = SharedString::from(format!("Copied {n} file(s)"));
                }
                Err(error) => {
                    self.status = SharedString::from(error);
                }
            },
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
            }
        }
        cx.notify();
    }

    pub(crate) fn open_item_link(&mut self, url: &str, cx: &mut Context<Self>) {
        if !is_web_link(url) {
            self.status = SharedString::from(t(self.locale, "details.linkInvalid"));
            cx.notify();
            return;
        }
        if let Err(error) = platform::open_url(url) {
            self.status = SharedString::from(error);
        }
        cx.notify();
    }

    fn update_browse(&mut self, f: impl FnOnce(&mut BrowsePreferences), cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            f(&mut lib.browse);
            lib.browse = lib.browse.clamped();
            lib.persist_browse_prefs(&self.session);
            self.recompute_justified();
            cx.notify();
        }
    }

    fn persist_panel_widths(&mut self, left: Option<Pixels>, right: Option<Pixels>, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            if let Some(w) = left {
                lib.left_width = w.as_f32().round().clamp(160.0, 480.0) as u32;
            }
            if let Some(w) = right {
                lib.right_width = w.as_f32().round().clamp(200.0, 560.0) as u32;
            }
            lib.persist_browse_prefs(&self.session);
            let deducted = (if lib.left_open {
                lib.left_width as f32
            } else {
                0.0
            }) + (if lib.right_open {
                lib.right_width as f32
            } else {
                0.0
            });
            self.grid_width = (1100.0 - deducted).max(200.0);
            self.recompute_justified();
            cx.notify();
        }
    }
}

fn theme_btn(
    locale: Locale,
    theme_pref: Theme,
    current: Theme,
    entity: &Entity<RootApp>,
) -> Button {
    let entity = entity.clone();
    Button::new(SharedString::from(format!("theme-{theme_pref:?}")))
        .label(t(locale, theme::theme_label_key(theme_pref)))
        .when(theme_pref == current, |b| b.primary())
        .when(theme_pref != current, |b| b.ghost())
        .on_click(move |_, _, cx| {
            entity
                .update(cx, |this, cx| this.set_theme(theme_pref, cx))
                ;
        })
}

fn locale_btn(
    id: &'static str,
    locale: Locale,
    current: Locale,
    entity: &Entity<RootApp>,
) -> Button {
    let entity = entity.clone();
    let label = match locale {
        Locale::En => "English",
        Locale::Ru => "Русский",
        Locale::ZhCn => "简体中文",
    };
    Button::new(id)
        .label(label)
        .when(locale == current, |b| b.primary())
        .when(locale != current, |b| b.ghost())
        .on_click(move |_, _, cx| {
            entity
                .update(cx, |this, cx| this.set_locale(locale, cx))
                ;
        })
}

fn pref_toggle(
    id: &'static str,
    label: String,
    on: bool,
    entity: &Entity<RootApp>,
    update: impl Fn(&mut app_store::Preferences, bool) + 'static,
) -> Button {
    let entity = entity.clone();
    Button::new(id)
        .label(format!("{} ({})", label, if on { "on" } else { "off" }))
        .on_click(move |_, _, cx| {
            let next = !on;
            entity
                .update(cx, |this, cx| {
                    let mut prefs = this.session.preferences();
                    update(&mut prefs, next);
                    let _ = this.session.save_preferences(prefs);
                    cx.notify();
                })
                ;
        })
}

impl Render for RootApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let locale = self.locale;
        let has_library = self.library.is_some();
        let focus = self.focus.clone();

        v_flex()
            .id("root")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .track_focus(&focus)
            .on_action(cx.listener(|this, _: &NewLibrary, _, cx| {
                this.begin_create_library(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenLibrary, window, cx| {
                this.pick_open_library(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseLibrary, window, cx| {
                this.close_library(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Settings, window, cx| {
                this.open_settings(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Exit, window, cx| {
                let _ = this.request_quit(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ThemeSystem, _, cx| {
                this.set_theme(Theme::System, cx);
            }))
            .on_action(cx.listener(|this, _: &ThemeLight, _, cx| {
                this.set_theme(Theme::Light, cx);
            }))
            .on_action(cx.listener(|this, _: &ThemeDark, _, cx| {
                this.set_theme(Theme::Dark, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleFullScreen, window, cx| {
                window.toggle_fullscreen();
                this.fullscreen = window.is_fullscreen();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                if let Some(lib) = this.library.as_mut() {
                    lib.left_open = !lib.left_open;
                    lib.persist_browse_prefs(&this.session);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleDetails, _, cx| {
                if let Some(lib) = this.library.as_mut() {
                    lib.right_open = !lib.right_open;
                    lib.persist_browse_prefs(&this.session);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Shortcuts, window, cx| {
                this.open_shortcuts(window, cx);
            }))
            .on_action(cx.listener(|this, _: &About, window, cx| {
                this.open_about(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenLogFolder, window, cx| {
                this.open_log_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ViewGrid, _, cx| {
                this.update_browse(|b| b.view_mode = ViewMode::Grid, cx);
            }))
            .on_action(cx.listener(|this, _: &ViewList, _, cx| {
                this.update_browse(|b| b.view_mode = ViewMode::List, cx);
            }))
            .on_action(cx.listener(|this, _: &LargerThumbs, _, cx| {
                this.update_browse(
                    |b| {
                        b.thumbnail_size =
                            (b.thumbnail_size + 20).min(BrowsePreferences::MAX_THUMBNAIL);
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &SmallerThumbs, _, cx| {
                this.update_browse(
                    |b| {
                        b.thumbnail_size = b
                            .thumbnail_size
                            .saturating_sub(20)
                            .max(BrowsePreferences::MIN_THUMBNAIL);
                    },
                    cx,
                );
            }))
            .on_action(cx.listener(|this, _: &ToggleShowNames, _, cx| {
                this.update_browse(|b| b.show_names = !b.show_names, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleIncludeSubfolders, _, cx| {
                if let Some(lib) = this.library.as_mut() {
                    lib.browse.include_subfolders = !lib.browse.include_subfolders;
                }
                this.refresh_view(cx);
            }))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                if let Some(lib) = this.library.as_mut() {
                    lib.selection.select_all(&lib.cells);
                    lib.persist_session(&this.session);
                    this.refresh_details(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Preview, _, cx| {
                this.open_preview(cx);
            }))
            .on_action(cx.listener(|this, _: &ClosePreview, _, cx| {
                if this.preview.is_some() {
                    this.close_preview(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &PreviewRotate, _, cx| {
                if this.preview.is_some() {
                    this.preview_rotate(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SpacePreviewDown, _, cx| {
                this.space_preview_down(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenOriginal, _, cx| {
                this.open_original(cx);
            }))
            .on_action(cx.listener(|this, _: &RevealOriginals, _, cx| {
                this.reveal_selection(cx);
            }))
            .on_action(cx.listener(|this, _: &CopyOriginals, _, cx| {
                this.copy_selection(cx);
            }))
            .on_action(cx.listener(|this, _: &FindStub, _, cx| {
                this.status = SharedString::from("Search arrives with filters in Phase 3.");
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SortByAdded, _, cx| {
                this.set_sort(SortKey::DateAdded, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByCreated, _, cx| {
                this.set_sort(SortKey::DateCreated, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByModified, _, cx| {
                this.set_sort(SortKey::DateModified, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByName, _, cx| {
                this.set_sort(SortKey::Title, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByExt, _, cx| {
                this.set_sort(SortKey::Extension, cx);
            }))
            .on_action(cx.listener(|this, _: &SortBySize, _, cx| {
                this.set_sort(SortKey::FileSize, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByResolution, _, cx| {
                this.set_sort(SortKey::Resolution, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByRating, _, cx| {
                this.set_sort(SortKey::Rating, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByTag, _, cx| {
                this.set_sort(SortKey::FirstTag, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByRandom, _, cx| {
                this.set_sort(SortKey::Random, cx);
            }))
            .on_action(cx.listener(|this, _: &SortByManual, _, cx| {
                this.set_sort(SortKey::Manual, cx);
            }))
            .on_action(cx.listener(|this, _: &SortAscending, _, cx| {
                this.set_ascending(true, cx);
            }))
            .on_action(cx.listener(|this, _: &SortDescending, _, cx| {
                this.set_ascending(false, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSortMenu, window, cx| {
                this.open_sort_menu(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusDetails, _, cx| {
                this.focus_details_panel(cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusLeft, _, cx| {
                this.move_focus("left", cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusRight, _, cx| {
                this.move_focus("right", cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusUp, _, cx| {
                this.move_focus("up", cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusDown, _, cx| {
                this.move_focus("down", cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusHome, _, cx| {
                this.move_focus("home", cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusEnd, _, cx| {
                this.move_focus("end", cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusPageUp, _, cx| {
                this.move_focus("pageup", cx);
            }))
            .on_action(cx.listener(|this, _: &MoveFocusPageDown, _, cx| {
                this.move_focus("pagedown", cx);
            }))
            .child(self.render_menubar(has_library, &theme, locale, cx))
            .when(self.show_create, |el| {
                el.child(self.render_create_overlay(&theme, locale, cx))
            })
            .when(
                self.library
                    .as_ref()
                    .is_some_and(|lib| lib.info.read_only),
                |el| el.child(library_ui::render_read_only_banner(locale)),
            )
            .child(if self.library.is_some() {
                self.render_library(&theme, locale, cx).into_any_element()
            } else {
                self.render_welcome(&theme, locale, cx).into_any_element()
            })
            .when_some(self.preview.as_ref(), |el, preview| {
                el.child(preview::render_preview(preview, locale, cx.entity()))
            })
    }
}

impl RootApp {
    fn render_menubar(
        &self,
        has_library: bool,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .id("menubar")
            .w_full()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(theme.border)
            .child(
                Button::new("mb-new")
                    .ghost()
                    .label(t(locale, "commands.file.newLibrary"))
                    .on_click(cx.listener(|this, _, _, cx| this.begin_create_library(cx))),
            )
            .child(
                Button::new("mb-open")
                    .ghost()
                    .label(t(locale, "commands.file.openLibrary"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_open_library(window, cx);
                    })),
            )
            .child(
                Button::new("mb-close")
                    .ghost()
                    .label(t(locale, "commands.file.closeLibrary"))
                    .disabled(!has_library)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_library(window, cx);
                    })),
            )
            .child(
                Button::new("mb-settings")
                    .ghost()
                    .label(t(locale, "commands.file.settings"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_settings(window, cx);
                    })),
            )
            .child(div().flex_1())
            .child(
                Button::new("mb-shortcuts")
                    .ghost()
                    .label(t(locale, "commands.help.shortcuts"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_shortcuts(window, cx);
                    })),
            )
            .child(
                Button::new("mb-logs")
                    .ghost()
                    .label(t(locale, "commands.help.openLogFolder"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_log_folder(window, cx);
                    })),
            )
            .child(
                Button::new("mb-about")
                    .ghost()
                    .label(t(locale, "commands.help.about"))
                    .on_click(cx.listener(|this, _, window, cx| this.open_about(window, cx))),
            )
            .child(
                Button::new("mb-exit")
                    .ghost()
                    .label(t(locale, "commands.file.exit"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let _ = this.request_quit(window, cx);
                    })),
            )
    }

    fn render_create_overlay(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("create-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.background.opacity(0.55))
            .child(
                v_flex()
                    .w(px(420.))
                    .gap_3()
                    .p_4()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(Label::new(t(locale, "createLibrary.title")).font_bold())
                    .child(Label::new(t(locale, "createLibrary.name")).text_sm())
                    .child(Input::new(&self.create_name))
                    .child(Label::new(t(locale, "createLibrary.location")).text_sm())
                    .child(
                        Label::new(self.create_location.clone())
                            .text_xs()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("create-browse")
                                    .label(t(locale, "common.browse"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.pick_create_location(window, cx);
                                    })),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("create-cancel")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_create = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("create-ok")
                                    .primary()
                                    .label(t(locale, "createLibrary.create"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_create_library(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_welcome(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let recent = self.session.recent_libraries();
        let status = self.status.clone();
        v_flex()
            .id("welcome")
            .size_full()
            .items_center()
            .justify_center()
            .p_8()
            .child(
                v_flex()
                    .w(px(480.))
                    .gap_4()
                    .p_6()
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.secondary)
                    .child(Label::new(t(locale, "welcome.title")).text_xl().font_bold())
                    .child(
                        Label::new(t(locale, "welcome.description"))
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .child(
                                Button::new("welcome-create")
                                    .primary()
                                    .label(t(locale, "welcome.create"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.begin_create_library(cx);
                                    })),
                            )
                            .child(
                                Button::new("welcome-open")
                                    .label(t(locale, "welcome.open"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.pick_open_library(window, cx);
                                    })),
                            ),
                    )
                    .child(Label::new(t(locale, "welcome.recent")).text_sm().font_bold())
                    .child(if recent.is_empty() {
                        Label::new(t(locale, "welcome.noRecent"))
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .into_any_element()
                    } else {
                        v_flex()
                            .w_full()
                            .gap_1()
                            .children(recent.into_iter().map(|library| {
                                let path_open = library.path.clone();
                                let path_forget = library.path.clone();
                                h_flex()
                                    .id(SharedString::from(format!("recent-{}", library.path)))
                                    .w_full()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "recent-open-{}",
                                            library.path
                                        )))
                                        .ghost()
                                        .label(format!(
                                            "{}{}",
                                            library.name,
                                            if library.missing { " (missing)" } else { "" }
                                        ))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.open_path(PathBuf::from(&path_open), window, cx);
                                        })),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "forget-{}",
                                            library.path
                                        )))
                                        .ghost()
                                        .label("×")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let _ = this
                                                .session
                                                .forget_recent_library(&path_forget);
                                            cx.notify();
                                        })),
                                    )
                                    .into_any_element()
                            }))
                            .into_any_element()
                    })
                    .child(
                        v_flex()
                            .w_full()
                            .gap_1()
                            .pt_4()
                            .border_t_1()
                            .border_color(theme.border)
                            .child(Label::new(t(locale, "welcome.eagle")).text_sm().font_bold())
                            .child(
                                Label::new(t(locale, "welcome.eagleLater"))
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                Button::new("welcome-import-eagle")
                                    .ghost()
                                    .label(t(locale, "welcome.importEagle"))
                                    .disabled(true),
                            )
                            .child(
                                Button::new("welcome-connect-eagle")
                                    .ghost()
                                    .label(t(locale, "welcome.connectEagle"))
                                    .disabled(true),
                            ),
                    )
                    .when(!status.is_empty(), |el| {
                        el.child(
                            Label::new(status)
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        )
                    }),
            )
    }

    fn render_library(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let lib = self.library.as_ref().unwrap();
        let left_w = lib.left_width as f32;
        let right_w = lib.right_width as f32;
        let mode = lib.browse.view_mode;
        let entity = cx.entity();
        let left_open = lib.left_open;
        let right_open = lib.right_open;

        v_flex()
            .id("library-shell")
            .flex_1()
            .size_full()
            .min_h(px(0.))
            .child(
                h_flex()
                    .id("toolbar")
                    .w_full()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(Label::new(lib.info.name.clone()).font_bold())
                    .child(
                        Label::new(i18n::t_n(locale, "shell.statusItems", lib.total)).text_sm(),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("tb-sort")
                            .ghost()
                            .label(t(locale, "commands.view.sortBy"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_sort_menu(window, cx);
                            })),
                    )
                    .child(
                        Button::new("tb-grid")
                            .label(t(locale, "commands.view.grid"))
                            .when(mode == ViewMode::Grid, |b| b.primary())
                            .when(mode != ViewMode::Grid, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.update_browse(|b| b.view_mode = ViewMode::Grid, cx);
                            })),
                    )
                    .child(
                        Button::new("tb-list")
                            .label(t(locale, "commands.view.list"))
                            .when(mode == ViewMode::List, |b| b.primary())
                            .when(mode != ViewMode::List, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.update_browse(|b| b.view_mode = ViewMode::List, cx);
                            })),
                    )
                    .child(
                        Button::new("tb-smaller")
                            .ghost()
                            .label("−")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.update_browse(
                                    |b| {
                                        b.thumbnail_size = b
                                            .thumbnail_size
                                            .saturating_sub(20)
                                            .max(BrowsePreferences::MIN_THUMBNAIL);
                                    },
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("tb-larger")
                            .ghost()
                            .label("+")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.update_browse(
                                    |b| {
                                        b.thumbnail_size = (b.thumbnail_size + 20)
                                            .min(BrowsePreferences::MAX_THUMBNAIL);
                                    },
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("tb-names")
                            .label(t(locale, "commands.view.showNames"))
                            .when(lib.browse.show_names, |b| b.primary())
                            .when(!lib.browse.show_names, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.update_browse(|b| b.show_names = !b.show_names, cx);
                            })),
                    )
                    .child(
                        Button::new("tb-sub")
                            .label(t(locale, "commands.view.includeSubfolders"))
                            .when(lib.browse.include_subfolders, |b| b.primary())
                            .when(!lib.browse.include_subfolders, |b| b.ghost())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(lib) = this.library.as_mut() {
                                    lib.browse.include_subfolders = !lib.browse.include_subfolders;
                                }
                                this.refresh_view(cx);
                            })),
                    ),
            )
            .child(
                h_resizable("shell-body")
                    .child(
                        resizable_panel()
                            .size(px(left_w))
                            .size_range(px(160.)..px(480.))
                            .visible(left_open)
                            .child(
                                div()
                                    .id("left-pane")
                                    .size_full()
                                    .border_r_1()
                                    .border_color(theme.border)
                                    .child(sidebar::render_sidebar(lib, locale, entity.clone())),
                            ),
                    )
                    .child(
                        resizable_panel().child(
                            div()
                                .on_children_prepainted({
                                    let entity = entity.clone();
                                    move |bounds, _, cx| {
                                        if let Some(first) = bounds.first() {
                                            let width = first.size.width.as_f32();
                                            let origin =
                                                (first.origin.x.as_f32(), first.origin.y.as_f32());
                                            entity.update(cx, |this, cx| {
                                                let width_changed =
                                                    (this.grid_width - width).abs() > 1.0;
                                                this.grid_origin = origin;
                                                if width_changed && width > 40.0 {
                                                    this.grid_width = width;
                                                    this.recompute_justified();
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    }
                                })
                                .id("center-pane")
                                .size_full()
                                .overflow_hidden()
                                .child(grid::render_items(
                                    lib,
                                    &self.items,
                                    locale,
                                    theme,
                                    entity.clone(),
                                    self.justified.as_ref(),
                                    self.rubber.as_ref(),
                                    self.grid_width,
                                )),
                        ),
                    )
                    .child(
                        resizable_panel()
                            .size(px(right_w))
                            .size_range(px(200.)..px(560.))
                            .visible(right_open)
                            .child(
                                div()
                                    .id("right-pane")
                                    .size_full()
                                    .border_l_1()
                                    .border_color(theme.border)
                                    .child(details::render_details(
                                        lib,
                                        &self.details,
                                        locale,
                                        theme,
                                        entity.clone(),
                                    )),
                            ),
                    )
                    .on_resize({
                        let entity = entity.clone();
                        move |state, _, cx| {
                            let sizes = state.read(cx).sizes().clone();
                            entity.update(cx, |this, cx| {
                                let left = if left_open {
                                    sizes.first().copied()
                                } else {
                                    None
                                };
                                let right = if right_open {
                                    sizes.last().copied().filter(|_| sizes.len() >= 2)
                                } else {
                                    None
                                };
                                this.persist_panel_widths(left, right, cx);
                            });
                        }
                    }),
            )
            .child(library_ui::render_status_bar(lib, locale))
    }
}
