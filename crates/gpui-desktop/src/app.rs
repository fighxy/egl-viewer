//! Root GPUI entity: Welcome ↔ Library shell, commands, dialogs (D-129–D-133).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_store::{BrowsePreferences, Locale, Preferences, Theme, ViewMode};
use desktop_session::{Event, ItemsTarget, Session};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{
    ActiveTheme, Icon, Selectable as _, Sizable as _, StyledExt as _, WindowExt as _, h_flex,
    h_resizable, resizable_panel, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, ExternalPaths, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, Pixels, Render, SharedString, StatefulInteractiveElement,
    Styled, Subscription, Window, div, px,
};
use library_core::{DuplicateNotice, ImportState, ItemId, LibraryInfo, SortKey, is_web_link};
use library_native::DuplicateChoice;

use crate::browse::details::{self, DetailsModel};
use crate::browse::grid::{self, ListedItem};
use crate::browse::preview::{self, PreviewState, SpaceGesture};
use crate::browse::preview_image::{self, PreviewPayload};
use crate::browse::sidebar;
use crate::browse::style;
use crate::commands::{self, *};
use crate::export_rules::LoginGate;
use crate::i18n::{self, t, t_n};
use crate::justified::{self, JustifiedLayout};
use crate::library::{self, EventMailbox, OpenLibraryState, Place, window_title};
use crate::ops_export::{DecryptUi, ExportUi, ImportEagleUi};
use crate::ops_items::PickerMode;
use crate::platform;
use crate::screens::library as library_ui;
use crate::theme;
use crate::updater::{self, CheckOutcome};

/// Rubber-band selection rectangle in grid content coordinates.
#[derive(Debug, Clone, Copy)]
pub struct RubberBand {
    pub start: (f32, f32),
    pub current: (f32, f32),
}

/// Background Empty Trash completion (thread → UI poll).
pub(crate) type EmptyTrashResult = Arc<Mutex<Option<Result<u32, String>>>>;

/// Background update-check completion (thread → UI poll).
type UpdateCheckResult = Arc<Mutex<Option<Result<CheckOutcome, String>>>>;

pub struct RootApp {
    pub(crate) session: Arc<Session>,
    events: EventMailbox,
    pub(crate) focus: FocusHandle,
    pub(crate) locale: Locale,
    pub(crate) theme_pref: Theme,
    pub(crate) status: SharedString,
    pub(crate) library: Option<OpenLibraryState>,
    items: Vec<ListedItem>,
    justified: Option<JustifiedLayout>,
    grid_width: f32,
    grid_origin: (f32, f32),
    pub(crate) rubber: Option<RubberBand>,
    /// True while an in-window DnD is active (suppresses rubber-band).
    pub(crate) drag_active: bool,
    /// Last folder row hit during an in-window folder drag (`id`, zone).
    pub(crate) folder_drop_zone: Option<(String, crate::dnd::FolderZone)>,
    pub(crate) fullscreen: bool,
    pub(crate) show_switch: bool,
    pub(crate) show_settings: bool,
    pub(crate) settings_section: crate::settings_ui::SettingsSection,
    pub(crate) settings_draft: Preferences,
    pub(crate) settings_query: SharedString,
    pub(crate) settings_search: Entity<InputState>,
    pub(crate) settings_password: Entity<InputState>,
    pub(crate) settings_pin: Entity<InputState>,
    _settings_search_sub: Option<Subscription>,
    quit_pending: bool,
    create_name: Entity<InputState>,
    create_location: SharedString,
    show_create: bool,
    pub(crate) details: DetailsModel,
    preview: Option<PreviewState>,
    preview_payload: Option<PreviewPayload>,
    space_gesture: Option<SpaceGesture>,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) imports_open: bool,
    pub(crate) pending_duplicate: Option<(u64, DuplicateNotice)>,
    pub(crate) emptying_trash: bool,
    pub(crate) empty_trash_progress: (u32, u32),
    pub(crate) empty_trash_result: Option<EmptyTrashResult>,
    pub(crate) tag_vocabulary: Vec<library_core::Tag>,
    pub(crate) tag_groups: Vec<library_core::TagGroup>,
    pub(crate) extensions: Vec<library_core::ExtensionCount>,
    _search_sub: Option<Subscription>,
    // Login gate + export / decrypt / Eagle connect (D-132).
    pub(crate) login_gate: LoginGate,
    pub(crate) startup_done: bool,
    pub(crate) pending_startup_library: Option<LibraryInfo>,
    pub(crate) pending_startup_notice: Option<String>,
    pub(crate) login_secret: Entity<InputState>,
    pub(crate) show_connect: bool,
    pub(crate) connect_host: Entity<InputState>,
    pub(crate) connect_port: Entity<InputState>,
    pub(crate) connect_token: Entity<InputState>,
    pub(crate) export_ui: Option<ExportUi>,
    pub(crate) export_password: Entity<InputState>,
    pub(crate) export_confirm: Entity<InputState>,
    pub(crate) eagle_host_input: Entity<InputState>,
    pub(crate) eagle_port_input: Entity<InputState>,
    pub(crate) eagle_token_input: Entity<InputState>,
    pub(crate) decrypt_ui: Option<DecryptUi>,
    pub(crate) decrypt_password: Entity<InputState>,
    pub(crate) decrypt_result:
        Option<Arc<Mutex<Option<Result<Option<library_core::DecryptedArchive>, String>>>>>,
    pub(crate) decrypt_progress: Option<Arc<Mutex<(u64, u64)>>>,
    pub(crate) import_eagle_ui: Option<ImportEagleUi>,
    pub(crate) import_name: Entity<InputState>,
    pub(crate) eagle_badge: Option<String>,
    /// Background update check (Help → Check for Updates / silent after gate).
    update_check_result: Option<UpdateCheckResult>,
    update_check_started: bool,
    pending_update_url: Option<String>,
    /// Go to Folder / Add to Folder(s) panel (D-140).
    pub(crate) folder_picker: Option<crate::ops_items::FolderPickerUi>,
    pub(crate) picker_input: Entity<InputState>,
    _picker_sub: Option<Subscription>,
    /// Tags taken by Copy Tags, for Paste Tags. Kept inside the app, as Eagle does.
    pub(crate) tag_clipboard: Vec<String>,
    /// The toolbar's thumbnail size slider (D-141).
    thumb_slider: Entity<SliderState>,
    _thumb_slider_sub: Option<Subscription>,
}

/// One step of Larger / Smaller Thumbnails, kept inside the allowed sizes.
fn stepped_thumbnail_size(current: u32, delta: i32) -> u32 {
    let next = i64::from(current) + i64::from(delta);
    next.clamp(
        i64::from(BrowsePreferences::MIN_THUMBNAIL),
        i64::from(BrowsePreferences::MAX_THUMBNAIL),
    ) as u32
}

impl RootApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let events = library::new_event_mailbox();
        let session = Arc::new(library::session_with_mailbox(events.clone()));
        let prefs = session.preferences();
        theme::apply_theme(prefs.theme, cx);

        let create_name = cx.new(|cx| InputState::new(window, cx).default_value("My Library"));
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::i18n::t(prefs.locale, "shell.search"))
        });
        let login_secret = cx.new(|cx| InputState::new(window, cx).masked(true));
        let connect_host = cx.new(|cx| InputState::new(window, cx));
        let connect_port = cx.new(|cx| InputState::new(window, cx));
        let connect_token = cx.new(|cx| InputState::new(window, cx).masked(true));
        let export_password = cx.new(|cx| InputState::new(window, cx).masked(true));
        let export_confirm = cx.new(|cx| InputState::new(window, cx).masked(true));
        let eagle_host_input = cx.new(|cx| InputState::new(window, cx));
        let eagle_port_input = cx.new(|cx| InputState::new(window, cx));
        let eagle_token_input = cx.new(|cx| InputState::new(window, cx).masked(true));
        let decrypt_password = cx.new(|cx| InputState::new(window, cx).masked(true));
        let import_name = cx.new(|cx| InputState::new(window, cx));
        let settings_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::i18n::t(prefs.locale, "settings.search"))
        });
        let settings_password = cx.new(|cx| InputState::new(window, cx).masked(true));
        let settings_pin = cx.new(|cx| InputState::new(window, cx).masked(true));
        let thumb_slider = cx.new(|_| {
            SliderState::new()
                .min(BrowsePreferences::MIN_THUMBNAIL as f32)
                .max(BrowsePreferences::MAX_THUMBNAIL as f32)
                .step(10.)
                .default_value(prefs.browse.thumbnail_size as f32)
        });
        let picker_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::i18n::t(prefs.locale, "picker.search"))
        });

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
            drag_active: false,
            folder_drop_zone: None,
            fullscreen: false,
            show_switch: false,
            show_settings: false,
            settings_section: crate::settings_ui::SettingsSection::General,
            settings_draft: prefs.clone(),
            settings_query: SharedString::default(),
            settings_search: settings_search.clone(),
            settings_password,
            settings_pin,
            _settings_search_sub: None,
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
            search_input: search_input.clone(),
            imports_open: false,
            pending_duplicate: None,
            emptying_trash: false,
            empty_trash_progress: (0, 0),
            empty_trash_result: None,
            tag_vocabulary: Vec::new(),
            tag_groups: Vec::new(),
            extensions: Vec::new(),
            _search_sub: None,
            login_gate: LoginGate::Pending,
            startup_done: false,
            pending_startup_library: None,
            pending_startup_notice: None,
            login_secret,
            show_connect: false,
            connect_host,
            connect_port,
            connect_token,
            export_ui: None,
            export_password,
            export_confirm,
            eagle_host_input,
            eagle_port_input,
            eagle_token_input,
            decrypt_ui: None,
            decrypt_password,
            decrypt_result: None,
            decrypt_progress: None,
            import_eagle_ui: None,
            import_name,
            eagle_badge: None,
            update_check_result: None,
            update_check_started: false,
            pending_update_url: None,
            folder_picker: None,
            picker_input: picker_input.clone(),
            _picker_sub: None,
            tag_clipboard: Vec::new(),
            thumb_slider: thumb_slider.clone(),
            _thumb_slider_sub: None,
        };
        app._thumb_slider_sub = Some(cx.subscribe(
            &thumb_slider,
            |this, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    this.set_thumbnail_size(value.start().round() as u32, false, cx);
                }
                SliderEvent::Release(value) => {
                    this.set_thumbnail_size(value.start().round() as u32, true, cx);
                }
            },
        ));
        app._picker_sub = Some(cx.subscribe_in(
            &picker_input,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => this.picker_confirm(window, cx),
                _ => {}
            },
        ));
        app.create_location = SharedString::from(app.session.default_library_location());
        app._search_sub = Some(cx.subscribe(
            &search_input,
            |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    let text = input.read(cx).value().to_string();
                    this.apply_search(text, cx);
                }
            },
        ));
        app._settings_search_sub = Some(cx.subscribe(
            &settings_search,
            |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let text = input.read(cx).value().to_string();
                    this.settings_query = SharedString::from(text.clone());
                    let visible = crate::settings_ui::visible_sections(this.locale, &text);
                    if !visible.contains(&this.settings_section)
                        && let Some(section) = visible.first().copied()
                    {
                        this.settings_section = section;
                    }
                    cx.notify();
                }
            },
        ));

        app.ensure_login_gate();
        // Drop Spike/Fixture (and other) rows whose folders were deleted after tests or moves.
        let _ = app.session.prune_missing_recent();
        let startup = app.session.startup(platform::skip_reopen());
        if app.login_gate.is_open() {
            app.startup_done = true;
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
            app.schedule_silent_update_check(cx);
        } else {
            app.pending_startup_library = startup.library;
            app.pending_startup_notice = startup.notice;
        }

        let entity = cx.entity();
        window.on_window_should_close(cx, move |window, cx| {
            entity.update(cx, |this, cx| this.request_quit(window, cx))
        });

        // Poll Session Emit mailbox → Entity notify (views-invalidated / import).
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(32))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.poll_session_events(cx);
                        this.poll_preview_tick(cx);
                        this.poll_empty_trash(cx);
                        this.poll_decrypt(cx);
                        this.poll_update_check(cx);
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

    const LICENSES: &'static str = include_str!("../assets/THIRD_PARTY_LICENSES.txt");

    pub(crate) fn schedule_silent_update_check(&mut self, cx: &mut Context<Self>) {
        if self.update_check_started {
            return;
        }
        // Skip network in automated tests / offline packaging smoke.
        if std::env::var_os("EGL_SKIP_UPDATE_CHECK").is_some() {
            self.update_check_started = true;
            return;
        }
        self.update_check_started = true;
        let slot: UpdateCheckResult = Arc::new(Mutex::new(None));
        self.update_check_result = Some(slot.clone());
        let current = library::APP_VERSION.to_string();
        std::thread::spawn(move || {
            let outcome = updater::check_latest(&current, updater::DEFAULT_ENDPOINT)
                .map_err(|e| e.to_string());
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(outcome);
            }
        });
        cx.notify();
    }

    fn start_update_check(&mut self, interactive: bool, cx: &mut Context<Self>) {
        if interactive {
            self.status = SharedString::from(t(self.locale, "updates.checking"));
        }
        let slot: UpdateCheckResult = Arc::new(Mutex::new(None));
        self.update_check_result = Some(slot.clone());
        self.update_check_started = true;
        let current = library::APP_VERSION.to_string();
        std::thread::spawn(move || {
            let outcome = updater::check_latest(&current, updater::DEFAULT_ENDPOINT)
                .map_err(|e| e.to_string());
            if let Ok(mut guard) = slot.lock() {
                *guard = Some(outcome);
            }
        });
        cx.notify();
    }

    fn poll_update_check(&mut self, cx: &mut Context<Self>) {
        let Some(slot) = self.update_check_result.clone() else {
            return;
        };
        let finished = slot.lock().ok().and_then(|mut g| g.take());
        let Some(finished) = finished else {
            return;
        };
        self.update_check_result = None;
        match finished {
            Ok(CheckOutcome::UpToDate { .. }) => {
                // Silent checks stay quiet when current; interactive sets status.
                if self.status.as_ref() == t(self.locale, "updates.checking") {
                    self.status = SharedString::from(t(self.locale, "updates.upToDate"));
                }
            }
            Ok(CheckOutcome::Update(update)) => {
                self.pending_update_url = Some(update.url.clone());
                self.status = SharedString::from(
                    t(self.locale, "updates.silentAvailable").replace("{version}", &update.version),
                );
            }
            Err(error) => {
                if self.status.as_ref() == t(self.locale, "updates.checking") {
                    self.status =
                        SharedString::from(format!("{}: {error}", t(self.locale, "updates.error")));
                }
            }
        }
        cx.notify();
    }

    fn poll_empty_trash(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self.empty_trash_result.clone() else {
            return;
        };
        let finished = result.lock().ok().and_then(|mut g| g.take());
        if let Some(finished) = finished {
            self.emptying_trash = false;
            self.empty_trash_result = None;
            match finished {
                Ok(n) => {
                    self.status = SharedString::from(t_n(self.locale, "trash.emptied", n));
                    if let Some(lib) = self.library.as_mut() {
                        lib.selection.clear();
                    }
                    self.refresh_view(cx);
                    self.refresh_details(cx);
                }
                Err(error) => {
                    self.status = SharedString::from(error);
                    cx.notify();
                }
            }
        } else if self.emptying_trash {
            cx.notify();
        }
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
                        if let Some(updated) = views.views.iter().find(|v| v.view_id == lib.view_id)
                        {
                            lib.total = updated.total;
                            needs_refresh = true;
                        }
                    }
                }
                Event::ImportProgress(progress) => {
                    self.status = SharedString::from(format!(
                        "{} {}/{} (+{} dup={} fail={})",
                        t(self.locale, "import.progress"),
                        progress.processed,
                        progress.total,
                        progress.added,
                        progress.duplicates,
                        progress.failed
                    ));
                    if progress.state == ImportState::WaitingForDecision {
                        if let Some(dup) = progress.duplicate.clone() {
                            self.pending_duplicate = Some((progress.id, dup));
                        }
                    }
                    if matches!(progress.state, ImportState::Done | ImportState::Cancelled) {
                        needs_refresh = true;
                    }
                    cx.notify();
                }
                Event::ExportProgress(progress) => {
                    self.on_export_progress(progress, cx);
                }
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

    pub(crate) fn recompute_justified(&mut self) {
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

    pub(crate) fn adopt_library(
        &mut self,
        info: LibraryInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match OpenLibraryState::load(&self.session, info) {
            Ok(state) => {
                window.set_window_title(&window_title(Some(&state.info)));
                let search = state.filters.search_text.clone();
                self.items = grid::load_listed(&self.session, &state.cells);
                self.status = SharedString::from("");
                self.library = Some(state);
                self.preview = None;
                self.preview_payload = None;
                self.space_gesture = None;
                self.pending_duplicate = None;
                self.tag_vocabulary = self.session.tag_vocabulary().unwrap_or_default();
                self.tag_groups = self.session.tag_groups().unwrap_or_default();
                self.extensions = self.session.extensions().unwrap_or_default();
                self.search_input.update(cx, |input, cx| {
                    input.set_value(search, window, cx);
                });
                self.sync_thumb_slider(window, cx);
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

    pub(crate) fn drop_preview(&mut self) {
        self.preview = None;
        self.preview_payload = None;
        self.space_gesture = None;
    }

    pub(crate) fn open_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
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
        self.eagle_badge = None;
        self.export_ui = None;
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

    pub(crate) fn refresh_view(&mut self, cx: &mut Context<Self>) {
        if let Some(lib) = self.library.as_mut() {
            if let Err(error) = lib.refresh_view(&self.session) {
                self.status = SharedString::from(format!("View failed: {error}"));
            } else {
                self.items = grid::load_listed(&self.session, &lib.cells);
                lib.persist_session(&self.session);
                lib.persist_browse_prefs(&self.session);
                self.recompute_justified();
            }
            // The right bar follows the view: a new place clears the selection, and the
            // bar must not keep showing the item that was selected before.
            self.refresh_details(cx);
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
        if self.show_settings {
            self.settings_draft.theme = theme_pref;
        }
        let mut prefs = self.session.preferences();
        prefs.theme = theme_pref;
        let _ = self.session.save_preferences(prefs);
        theme::apply_theme(theme_pref, cx);
        cx.notify();
    }

    fn open_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let version = library::APP_VERSION;
        let commit = library::APP_COMMIT.unwrap_or("unknown");
        let os = std::env::consts::OS;
        let entity = cx.entity();
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
                    .child(Label::new(format!("{}: {os}", t(locale, "about.os"))))
                    .child(
                        Button::new("about-licenses")
                            .label(t(locale, "about.licenses"))
                            .on_click({
                                let entity = entity.clone();
                                move |_, window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.open_licenses(window, cx);
                                    });
                                }
                            }),
                    ),
            )
        });
    }

    fn open_licenses(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let text = SharedString::from(Self::LICENSES);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(t(locale, "about.licensesTitle")).child(
                v_flex()
                    .gap_2()
                    .p_2()
                    .w(px(520.))
                    .max_h(px(360.))
                    .child(Label::new(text.clone()).text_xs()),
            )
        });
    }

    fn open_check_for_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.locale;
        let entity = cx.entity();
        let url = self.pending_update_url.clone();
        let has_url = url.is_some();
        window.open_dialog(cx, move |dialog, _, _| {
            let mut body = v_flex().gap_2().p_2().child(Label::new(if has_url {
                t(locale, "updates.available")
            } else {
                t(locale, "updates.checking")
            }));
            if let Some(url) = url.clone() {
                body = body.child(
                    Button::new("upd-download")
                        .primary()
                        .label(t(locale, "updates.download"))
                        .on_click(move |_, _, _| {
                            let _ = platform::open_url(&url);
                        }),
                );
            } else {
                body = body.child(
                    Button::new("upd-check-now")
                        .primary()
                        .label(t(locale, "commands.help.checkForUpdates"))
                        .on_click({
                            let entity = entity.clone();
                            move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.start_update_check(true, cx);
                                });
                            }
                        }),
                );
            }
            dialog
                .title(t(locale, "commands.help.checkForUpdates"))
                .child(body)
        });
        if !has_url {
            self.start_update_check(true, cx);
        }
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
                            .ghost()
                            .selected(sort == current)
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
                                    .ghost()
                                    .selected(ascending)
                                    .on_click(move |_, _, cx| {
                                        entity.update(cx, |this, cx| this.set_ascending(true, cx));
                                    })
                            })
                            .child({
                                let entity = entity.clone();
                                Button::new("sort-desc")
                                    .label(t(locale, "sort.descending"))
                                    .ghost()
                                    .selected(!ascending)
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
                        entity.update(cx, |this, _cx| {
                            this.shutdown();
                        });
                        window.remove_window();
                        true
                    }
                })
                .on_cancel({
                    let entity = entity.clone();
                    move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.quit_pending = false;
                            cx.notify();
                        });
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
        if self.drag_active {
            return;
        }
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
            lib.selection.select_indices(&lib.cells, &indices, focus);
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

    pub(crate) fn refresh_details(&mut self, cx: &mut Context<Self>) {
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
            let id = lib.selection.ids.iter().next().cloned().or_else(|| {
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
        let ids: Vec<ItemId> = lib.selection.ids.iter().cloned().map(ItemId).collect();
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

    pub(crate) fn selection_target(&self) -> Option<ItemsTarget> {
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
        let index = lib.selection.focus.or_else(|| {
            lib.selection
                .ids
                .iter()
                .next()
                .and_then(|id| lib.cells.iter().position(|c| c.id.0 == *id))
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

    pub(crate) fn preview_is_open(&self) -> bool {
        self.preview.is_some()
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

    pub(crate) fn preview_pan_start(
        &mut self,
        origin_x: f32,
        origin_y: f32,
        cx: &mut Context<Self>,
    ) {
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

    fn persist_panel_widths(
        &mut self,
        left: Option<Pixels>,
        right: Option<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        let mut changed = false;
        if let Some(w) = left {
            let next = w.as_f32().round().clamp(160.0, 480.0) as u32;
            if lib.left_width != next {
                lib.left_width = next;
                changed = true;
            }
        }
        if let Some(w) = right {
            let next = w.as_f32().round().clamp(200.0, 560.0) as u32;
            if lib.right_width != next {
                lib.right_width = next;
                changed = true;
            }
        }
        // h_resizable calls on_resize during layout. Notifying on every call re-enters
        // render on the same stack and overflows when a library is first opened.
        if !changed {
            return;
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

impl Render for RootApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let locale = self.locale;
        let focus = self.focus.clone();

        if !self.login_gate.is_open() {
            return v_flex()
                .id("root")
                .size_full()
                .bg(theme.background)
                .text_color(theme.foreground)
                .child(self.render_login_gate(&theme, locale, cx))
                .into_any_element();
        }

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
            .on_action(cx.listener(|this, _: &CheckForUpdates, window, cx| {
                this.open_check_for_updates(window, cx);
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
            .on_action(cx.listener(|this, _: &LargerThumbs, window, cx| {
                this.step_thumbnail_size(20, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SmallerThumbs, window, cx| {
                this.step_thumbnail_size(-20, window, cx);
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
            .on_action(cx.listener(|this, _: &ClosePreview, window, cx| {
                if this.folder_picker.is_some() {
                    this.close_folder_picker(window, cx);
                } else if this.preview.is_some() {
                    this.close_preview(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RenameSelection, window, cx| {
                this.rename_selection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &AddToFolders, window, cx| {
                this.open_folder_picker(PickerMode::AddTo, window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToFolder, window, cx| {
                this.open_folder_picker(PickerMode::GoTo, window, cx);
            }))
            .on_action(cx.listener(|this, _: &RemoveFromFolder, _, cx| {
                this.remove_from_open_folder(cx);
            }))
            .on_action(cx.listener(|this, _: &AddTags, window, cx| {
                if this.selection_target().is_some() {
                    this.open_add_tag_dialog(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CopyTags, _, cx| {
                this.copy_tags(cx);
            }))
            .on_action(cx.listener(|this, _: &PasteTags, _, cx| {
                this.paste_tags(cx);
            }))
            .on_action(cx.listener(|this, _: &ClearTags, _, cx| {
                this.clear_tags(cx);
            }))
            .on_action(cx.listener(|this, _: &CopyName, _, cx| {
                this.copy_names(cx);
            }))
            .on_action(cx.listener(|this, _: &CopyPath, _, cx| {
                this.copy_paths(cx);
            }))
            .on_action(cx.listener(|this, _: &Rate0, _, cx| {
                this.rate_selection(0, cx);
            }))
            .on_action(cx.listener(|this, _: &Rate1, _, cx| {
                this.rate_selection(1, cx);
            }))
            .on_action(cx.listener(|this, _: &Rate2, _, cx| {
                this.rate_selection(2, cx);
            }))
            .on_action(cx.listener(|this, _: &Rate3, _, cx| {
                this.rate_selection(3, cx);
            }))
            .on_action(cx.listener(|this, _: &Rate4, _, cx| {
                this.rate_selection(4, cx);
            }))
            .on_action(cx.listener(|this, _: &Rate5, _, cx| {
                this.rate_selection(5, cx);
            }))
            .on_action(cx.listener(|this, _: &LockFolder, window, cx| {
                this.lock_open_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ExportFolder, window, cx| {
                this.export_open_folder(window, cx);
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
            .on_action(cx.listener(|this, _: &Find, window, cx| {
                this.focus_search(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Undo, _, cx| {
                this.undo_last(cx);
            }))
            .on_action(cx.listener(|this, _: &TrashSelection, window, cx| {
                if this
                    .library
                    .as_ref()
                    .is_some_and(|lib| matches!(lib.place, Place::Trash))
                {
                    this.delete_permanently_selection(window, cx);
                } else {
                    this.trash_selection(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RestoreSelection, _, cx| {
                this.restore_selection(cx);
            }))
            .on_action(cx.listener(|this, _: &DeletePermanently, window, cx| {
                this.delete_permanently_selection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &EmptyTrash, window, cx| {
                this.empty_trash(window, cx);
            }))
            .on_action(cx.listener(|this, _: &AddFiles, window, cx| {
                this.pick_add_files(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ImportFolder, window, cx| {
                this.pick_import_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &PasteFiles, _, cx| {
                this.paste_files(cx);
            }))
            .on_action(cx.listener(|this, _: &NewFolder, window, cx| {
                this.create_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &RenameFolder, window, cx| {
                this.rename_open_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &DeleteFolder, window, cx| {
                this.delete_open_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SwitchLibrary, _, cx| {
                this.toggle_switch(cx);
            }))
            .on_action(cx.listener(|this, _: &ClearRecentHistory, _, cx| {
                this.clear_recent_history(cx);
            }))
            .on_action(cx.listener(|this, _: &ClearCacheReload, _, cx| {
                this.clear_cache_and_reload(cx);
            }))
            .on_action(cx.listener(|this, _: &RevealLibrary, _, cx| {
                this.reveal_library(cx);
            }))
            .on_action(cx.listener(|this, _: &LockAll, _, cx| {
                this.lock_all_unlocked(cx);
            }))
            .on_action(cx.listener(|this, _: &LibrarySettings, window, cx| {
                this.open_library_settings(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ClearFilters, window, cx| {
                this.clear_filters(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveSmartFolder, window, cx| {
                this.save_as_smart_folder(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleImportsPanel, _, cx| {
                this.imports_open = !this.imports_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &BackupExport, window, cx| {
                this.open_backup_export(window, cx);
            }))
            .on_action(cx.listener(|this, _: &DecryptArchive, window, cx| {
                this.open_decrypt_archive(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ImportEagleLibrary, window, cx| {
                this.begin_import_eagle(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ConnectEagle, window, cx| {
                this.begin_connect_eagle(window, cx);
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
            .child(self.render_menubar(&theme, cx))
            .when(self.show_create, |el| {
                el.child(self.render_create_overlay(&theme, locale, cx))
            })
            .when(
                self.library.as_ref().is_some_and(|lib| lib.info.read_only),
                |el| el.child(library_ui::render_read_only_banner(locale)),
            )
            .child(if self.library.is_some() {
                self.render_library(&theme, locale, cx)
            } else {
                self.render_welcome(&theme, locale, cx).into_any_element()
            })
            .when_some(self.preview.as_ref(), |el, preview| {
                el.child(preview::render_preview(preview, locale, cx.entity()))
            })
            .when(self.export_ui.is_some(), |el| {
                el.child(self.render_export_overlay(&theme, locale, cx))
            })
            .when(self.decrypt_ui.is_some(), |el| {
                el.child(self.render_decrypt_overlay(&theme, locale, cx))
            })
            .when(self.show_connect, |el| {
                el.child(self.render_connect_overlay(&theme, locale, cx))
            })
            .when(self.import_eagle_ui.is_some(), |el| {
                el.child(self.render_import_eagle_overlay(&theme, locale, cx))
            })
            .when(self.show_switch, |el| {
                el.child(self.render_switch_overlay(&theme, locale, cx))
            })
            .when(self.show_settings, |el| {
                el.child(self.render_settings(&theme, locale, cx))
            })
            .when_some(self.folder_picker.as_ref(), |el, picker| {
                el.child(self.render_folder_picker(picker, &theme, locale, cx))
            })
            .into_any_element()
    }
}

impl RootApp {
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
        // Forget rows whose library.db is gone so Welcome stays clean after temp fixtures.
        let _ = self.session.prune_missing_recent();
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
                    .child(
                        Label::new(t(locale, "welcome.recent"))
                            .text_sm()
                            .font_bold(),
                    )
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
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.open_path(
                                                    PathBuf::from(&path_open),
                                                    window,
                                                    cx,
                                                );
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "forget-{}",
                                            library.path
                                        )))
                                        .ghost()
                                        .label("×")
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                let _ = this
                                                    .session
                                                    .forget_recent_library(&path_forget);
                                                cx.notify();
                                            }),
                                        ),
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
                                Button::new("welcome-import-eagle")
                                    .ghost()
                                    .label(t(locale, "welcome.importEagle"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.begin_import_eagle(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("welcome-connect-eagle")
                                    .ghost()
                                    .label(t(locale, "welcome.connectEagle"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.begin_connect_eagle(window, cx);
                                    })),
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
    ) -> gpui_kit::AnyElement {
        let lib = self.library.as_ref().unwrap();
        let left_w = lib.left_width as f32;
        let right_w = lib.right_width as f32;
        let mode = lib.browse.view_mode;
        let entity = cx.entity();
        let left_open = lib.left_open;
        let right_open = lib.right_open;
        // Split + type-erase: one nested `impl IntoElement` for the whole shell overflows
        // the main thread stack on Windows debug builds when opening a library.
        let toolbar = self.render_library_toolbar(lib, theme, locale, mode, entity.clone(), cx);
        let body = self.render_library_body(
            lib,
            theme,
            locale,
            left_w,
            right_w,
            left_open,
            right_open,
            entity.clone(),
            cx,
        );

        v_flex()
            .id("library-shell")
            .flex_1()
            .size_full()
            .min_h(px(0.))
            .on_drop({
                let entity = entity.clone();
                move |paths: &ExternalPaths, _, cx| {
                    entity.update(cx, |this, cx| this.handle_external_drop(paths, cx));
                }
            })
            .child(toolbar)
            .child(body)
            .child(library_ui::render_status_bar(lib, locale, theme))
            .when(!self.status.is_empty(), |el| {
                el.child(
                    h_flex().id("status-message").w_full().px_3().py_1().child(
                        Label::new(self.status.clone())
                            .text_xs()
                            .text_color(theme.muted_foreground),
                    ),
                )
            })
            .when(self.imports_open, |el| {
                el.child(self.render_imports_panel(locale, theme, cx))
            })
            .when_some(self.pending_duplicate.clone(), |el, (id, notice)| {
                el.child(self.render_duplicate_banner(id, &notice, locale, cx))
            })
            .when(self.emptying_trash, |el| {
                let (done, total) = self.empty_trash_progress;
                el.child(
                    h_flex()
                        .id("empty-trash-chrome")
                        .w_full()
                        .items_center()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(theme.border)
                        .child(
                            Label::new(format!("{} — {done}/{total}", t(locale, "trash.emptying")))
                                .text_sm()
                                .flex_1(),
                        )
                        .child(
                            Button::new("empty-trash-cancel")
                                .outline()
                                .label(t(locale, "trash.cancelEmpty"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.cancel_empty_trash(cx);
                                })),
                        ),
                )
            })
            .into_any_element()
    }

    #[inline(never)]
    fn render_library_toolbar(
        &self,
        lib: &OpenLibraryState,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        mode: ViewMode,
        entity: Entity<Self>,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let title = style::place_title(&lib.place, &lib.folders, &lib.smart_folders, locale);

        // Row 1: where you are, the thumbnail size, the view switches, search.
        let top = h_flex()
            .id("toolbar")
            .w_full()
            .h(px(44.))
            .flex_none()
            .items_center()
            .gap_2()
            .px_3()
            .child(
                div()
                    .max_w(px(320.))
                    .font_bold()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(title),
            )
            .child(
                Label::new(i18n::t_n(locale, "shell.statusItems", lib.total))
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
            .child(div().flex_1())
            .child(
                h_flex()
                    .id("thumb-size")
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("tb-smaller")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Minus)
                            .tooltip(t(locale, "commands.view.smaller"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.step_thumbnail_size(-20, window, cx);
                            })),
                    )
                    .child(
                        div().w(px(120.)).child(
                            Slider::new(&self.thumb_slider)
                                .accessibility_label(t(locale, "settings.thumbnailSize")),
                        ),
                    )
                    .child(
                        Button::new("tb-larger")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip(t(locale, "commands.view.larger"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.step_thumbnail_size(20, window, cx);
                            })),
                    ),
            )
            .child(div().flex_1())
            .child(
                Button::new("tb-add")
                    .ghost()
                    .small()
                    .icon(IconName::Plus)
                    .tooltip(t(locale, "commands.file.addFiles"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pick_add_files(window, cx);
                    })),
            )
            .child(
                Button::new("tb-imports")
                    .ghost()
                    .small()
                    .icon(IconName::Download)
                    .tooltip(t(locale, "import.panel"))
                    .selected(self.imports_open)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.imports_open = !this.imports_open;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("tb-sort")
                    .ghost()
                    .small()
                    .icon(IconName::ArrowUpDown)
                    .tooltip(t(locale, "commands.view.sortBy"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_sort_menu(window, cx);
                    })),
            )
            .child(
                Button::new("tb-sub")
                    .ghost()
                    .small()
                    .icon(IconName::FolderTree)
                    .tooltip(t(locale, "commands.view.includeSubfolders"))
                    .selected(lib.browse.include_subfolders)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(lib) = this.library.as_mut() {
                            lib.browse.include_subfolders = !lib.browse.include_subfolders;
                        }
                        this.refresh_view(cx);
                    })),
            )
            .child(
                Button::new("tb-names")
                    .ghost()
                    .small()
                    .icon(IconName::Captions)
                    .tooltip(t(locale, "commands.view.showNames"))
                    .selected(lib.browse.show_names)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_browse(|b| b.show_names = !b.show_names, cx);
                    })),
            )
            .child(
                h_flex()
                    .id("view-mode")
                    .items_center()
                    .child(
                        Button::new("tb-grid")
                            .ghost()
                            .small()
                            .icon(IconName::LayoutGrid)
                            .tooltip(t(locale, "commands.view.grid"))
                            .selected(mode == ViewMode::Grid)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.update_browse(|b| b.view_mode = ViewMode::Grid, cx);
                            })),
                    )
                    .child(
                        Button::new("tb-list")
                            .ghost()
                            .small()
                            .icon(IconName::List)
                            .tooltip(t(locale, "commands.view.list"))
                            .selected(mode == ViewMode::List)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.update_browse(|b| b.view_mode = ViewMode::List, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("search-field")
                    .w(px(220.))
                    .child(Input::new(&self.search_input).small()),
            );

        // Row 2: the filter bar.
        let filter_button = |id: &'static str, icon: IconName, label: String, active: bool| {
            Button::new(id)
                .ghost()
                .small()
                .icon(icon)
                .label(label)
                .selected(active)
        };
        let tags_label = if lib.filters.tags.is_empty() {
            t(locale, "filters.tags")
        } else {
            format!("{} ({})", t(locale, "filters.tags"), lib.filters.tags.len())
        };
        let folders_label = if lib.filters.folders.is_empty() {
            t(locale, "filters.folders")
        } else {
            format!(
                "{} ({})",
                t(locale, "filters.folders"),
                lib.filters.folders.len()
            )
        };
        let filters = h_flex()
            .id("filter-bar")
            .w_full()
            .h(px(36.))
            .flex_none()
            .items_center()
            .gap_1()
            .px_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                filter_button(
                    "tb-filter-tags",
                    IconName::Tag,
                    tags_label,
                    !lib.filters.tags.is_empty(),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_tag_mode_menu(window, cx);
                })),
            )
            .child(
                filter_button(
                    "tb-filter-folders",
                    IconName::Folder,
                    folders_label,
                    !lib.filters.folders.is_empty(),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_folder_filter_menu(window, cx);
                })),
            )
            .child(
                filter_button(
                    "tb-filter-type",
                    IconName::Image,
                    lib.filters
                        .ext
                        .as_ref()
                        .map(|e| e.to_uppercase())
                        .unwrap_or_else(|| t(locale, "filters.type")),
                    lib.filters.ext.is_some(),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_filter_type_menu(window, cx);
                })),
            )
            .child(
                filter_button(
                    "tb-filter-shape",
                    IconName::Shapes,
                    lib.filters
                        .shape
                        .clone()
                        .unwrap_or_else(|| t(locale, "filters.shape")),
                    lib.filters.shape.is_some(),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_filter_shape_menu(window, cx);
                })),
            )
            .child(
                h_flex()
                    .id("rating-filter")
                    .items_center()
                    .ml_2()
                    .child(
                        Icon::new(IconName::Star)
                            .small()
                            .text_color(theme.muted_foreground),
                    )
                    .children((0..=5).map(|star| {
                        let selected = lib.filters.rating == Some(star);
                        let entity = entity.clone();
                        Button::new(SharedString::from(format!("rate-filter-{star}")))
                            .ghost()
                            .xsmall()
                            .selected(selected)
                            .label(star.to_string())
                            .tooltip(if star == 0 {
                                t(locale, "filters.unratedHint")
                            } else {
                                "★".repeat(usize::from(star))
                            })
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    let next = if this
                                        .library
                                        .as_ref()
                                        .is_some_and(|lib| lib.filters.rating == Some(star))
                                    {
                                        None
                                    } else {
                                        Some(star)
                                    };
                                    this.set_rating_filter(next, cx);
                                });
                            })
                    })),
            )
            .when(!lib.filters.is_empty(), |el| {
                el.child(
                    Button::new("tb-clear-filters")
                        .ghost()
                        .small()
                        .icon(IconName::X)
                        .label(t(locale, "filters.clear"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.clear_filters(window, cx);
                        })),
                )
            })
            .child(div().flex_1())
            .when_some(
                lib.place.open_folder_id().map(|s| s.to_owned()),
                |el, id| {
                    let name =
                        library::folder_name(&lib.folders, &id).unwrap_or_else(|| id.clone());
                    el.child(
                        Button::new("tb-export-folder")
                            .ghost()
                            .small()
                            .icon(IconName::Upload)
                            .tooltip(t(locale, "exportUi.titleExportFolder"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_export_folder(id.clone(), name.clone(), window, cx);
                            })),
                    )
                },
            )
            .when(lib.filters.tags.len() == 1, |el| {
                let tag = lib.filters.tags[0].clone();
                el.child(
                    Button::new("tb-export-tag")
                        .ghost()
                        .small()
                        .icon(IconName::Upload)
                        .tooltip(t(locale, "exportUi.titleExportTag"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_export_tag(tag.clone(), window, cx);
                        })),
                )
            });

        v_flex()
            .id("toolbar-rows")
            .w_full()
            .flex_none()
            .child(top)
            .child(filters)
            .into_any_element()
    }

    /// The thumbnail size from the toolbar slider. Dragging repacks the grid on every step
    /// and writes the preference once, on release.
    fn set_thumbnail_size(&mut self, size: u32, persist: bool, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_mut() else {
            return;
        };
        let size = size.clamp(
            BrowsePreferences::MIN_THUMBNAIL,
            BrowsePreferences::MAX_THUMBNAIL,
        );
        let changed = lib.browse.thumbnail_size != size;
        lib.browse.thumbnail_size = size;
        if persist {
            lib.persist_browse_prefs(&self.session);
        }
        if changed {
            self.recompute_justified();
            cx.notify();
        }
    }

    /// Larger / Smaller Thumbnails: one step, and the slider follows.
    fn step_thumbnail_size(&mut self, delta: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.library.as_ref().map(|lib| lib.browse.thumbnail_size) else {
            return;
        };
        let next = stepped_thumbnail_size(current, delta);
        self.set_thumbnail_size(next, true, cx);
        self.sync_thumb_slider(window, cx);
    }

    /// Puts the slider's thumb where the open library's thumbnail size is.
    pub(crate) fn sync_thumb_slider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(size) = self.library.as_ref().map(|lib| lib.browse.thumbnail_size) else {
            return;
        };
        self.thumb_slider.update(cx, |slider, cx| {
            slider.set_value(size as f32, window, cx);
        });
    }

    #[inline(never)]
    fn render_library_body(
        &self,
        lib: &OpenLibraryState,
        theme: &gpui_kit::component::theme::Theme,
        locale: Locale,
        left_w: f32,
        right_w: f32,
        left_open: bool,
        right_open: bool,
        entity: Entity<Self>,
        _cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
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
                            .child(sidebar::render_sidebar(
                                lib,
                                locale,
                                theme,
                                entity.clone(),
                                &self.tag_vocabulary,
                            )),
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
                                    let origin = (first.origin.x.as_f32(), first.origin.y.as_f32());
                                    let needs_relayout = (entity.read(cx).grid_width - width).abs()
                                        > 1.0
                                        && width > 40.0;
                                    entity.update(cx, |this, _cx| {
                                        this.grid_origin = origin;
                                        if needs_relayout {
                                            this.grid_width = width;
                                            this.recompute_justified();
                                        }
                                    });
                                    if needs_relayout {
                                        let entity = entity.clone();
                                        cx.spawn(async move |cx| {
                                            let _ = entity.update(cx, |_this, cx| {
                                                cx.notify();
                                            });
                                        })
                                        .detach();
                                    }
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
                    // Defer width persistence out of the layout/resize pass.
                    // Synchronous cx.notify() here re-enters render and overflows.
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
                    let entity = entity.clone();
                    cx.spawn(async move |cx| {
                        let _ = entity.update(cx, |this, cx| {
                            this.persist_panel_widths(left, right, cx);
                        });
                    })
                    .detach();
                }
            })
            .into_any_element()
    }

    fn render_imports_panel(
        &self,
        locale: Locale,
        theme: &gpui_kit::component::theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let imports = self.session.imports();
        v_flex()
            .id("imports-panel")
            .w_full()
            .max_h(px(160.))
            .border_t_1()
            .border_color(theme.border)
            .p_2()
            .gap_1()
            .overflow_y_scroll()
            .child(
                h_flex()
                    .justify_between()
                    .child(Label::new(t(locale, "import.panel")).font_bold())
                    .child(
                        Button::new("imports-close")
                            .ghost()
                            .label(t(locale, "common.cancel"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.imports_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .when(imports.is_empty(), |el| {
                el.child(
                    Label::new(t(locale, "import.empty"))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
            })
            .children(imports.into_iter().map(|progress| {
                let id = progress.id;
                let entity = cx.entity();
                h_flex()
                    .id(SharedString::from(format!("import-{id}")))
                    .gap_2()
                    .items_center()
                    .child(
                        Label::new(format!(
                            "#{} {:?} {}/{}",
                            id, progress.state, progress.processed, progress.total
                        ))
                        .text_xs()
                        .flex_1(),
                    )
                    .child(
                        Button::new(SharedString::from(format!("cancel-import-{id}")))
                            .ghost()
                            .label(t(locale, "import.cancel"))
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| this.cancel_import(id, cx));
                            }),
                    )
            }))
    }

    fn render_duplicate_banner(
        &self,
        id: u64,
        notice: &DuplicateNotice,
        locale: Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity();
        h_flex()
            .id("duplicate-banner")
            .w_full()
            .gap_2()
            .px_3()
            .py_2()
            .border_t_1()
            .child(
                Label::new(format!(
                    "{}: {}",
                    t(locale, "import.duplicate"),
                    notice.path
                ))
                .text_sm()
                .flex_1(),
            )
            .child({
                let entity = entity.clone();
                Button::new("dup-skip")
                    .outline()
                    .label(t(locale, "import.skip"))
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.answer_duplicate(id, DuplicateChoice::Skip, cx);
                        });
                    })
            })
            .child({
                let entity = entity.clone();
                Button::new("dup-add")
                    .outline()
                    .label(t(locale, "import.addAnyway"))
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.answer_duplicate(id, DuplicateChoice::AddAnyway, cx);
                        });
                    })
            })
            .when(notice.target_folder.is_some(), |el| {
                let entity = entity.clone();
                el.child(
                    Button::new("dup-folder")
                        .outline()
                        .label(t(locale, "import.addToFolder"))
                        .on_click(move |_, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.answer_duplicate(id, DuplicateChoice::AddToFolder, cx);
                            });
                        }),
                )
            })
            .child({
                let entity = entity.clone();
                Button::new("dup-cancel")
                    .ghost()
                    .label(t(locale, "import.cancel"))
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.answer_duplicate(id, DuplicateChoice::Cancel, cx);
                        });
                    })
            })
    }
}
