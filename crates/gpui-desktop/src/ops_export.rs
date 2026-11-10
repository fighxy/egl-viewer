//! Phase 4 Session-backed flows: export/backup/decrypt, Eagle import/connect, app login.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use app_store::{LoginMethod, LoginProof};
use desktop_session::{EagleImportRequest, EagleLibraryTarget};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::label::Label;
use gpui_kit::component::{Disableable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, InteractiveElement, IntoElement, ParentElement, PathPromptOptions,
    SharedString, Styled, Window, px,
};
use library_core::{
    Compression, EagleExportTarget, ExportFormat, ExportProgress, ExportRequest, ExportState,
    LibraryKind, MIN_PASSPHRASE_CHARS, ZipEncryption,
};

use crate::app::RootApp;
use crate::browse::format::format_bytes;
use crate::export_rules::{
    ExportPreset, FormatKind, LoginGate, build_archive_request, decrypt_default_folder,
    export_format, export_selection, export_state_label, is_terminal_export, passphrase_problem,
    validate_connect_host_port,
};
use crate::i18n::t;
use crate::platform;

/// Live export dialog / overlay state.
#[derive(Clone)]
pub struct ExportUi {
    pub preset: ExportPreset,
    pub format: FormatKind,
    pub compression: Compression,
    pub zip_encryption: ZipEncryption,
    pub encrypt: bool,
    pub include_subfolders: bool,
    pub destination: String,
    pub preview_items: u64,
    pub preview_bytes: u64,
    pub preview_hidden: u64,
    pub progress: Option<ExportProgress>,
    pub error: Option<String>,
    pub run_id: Option<u64>,
}

impl ExportUi {
    pub fn new(preset: ExportPreset, destination: String) -> Self {
        Self {
            preset,
            format: FormatKind::Zip,
            compression: Compression::Balanced,
            zip_encryption: ZipEncryption::Age,
            encrypt: false,
            include_subfolders: true,
            destination,
            preview_items: 0,
            preview_bytes: 0,
            preview_hidden: 0,
            progress: None,
            error: None,
            run_id: None,
        }
    }

    pub fn title_key(&self) -> &'static str {
        match &self.preset {
            ExportPreset::Backup => "exportUi.titleBackup",
            ExportPreset::Folder { .. } => "exportUi.titleExportFolder",
            ExportPreset::Tag { .. } => "exportUi.titleExportTag",
        }
    }
}

#[derive(Clone)]
pub struct DecryptUi {
    pub source: String,
    pub destination: String,
    pub busy: bool,
    pub progress_done: u64,
    pub progress_total: u64,
    pub result_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ImportEagleUi {
    pub eagle_path: String,
    pub summary_name: String,
    pub summary_items: u64,
    pub parent: String,
    pub import_trash: bool,
    pub error: Option<String>,
}

impl RootApp {
    pub(crate) fn ensure_login_gate(&mut self) {
        if !matches!(self.login_gate, LoginGate::Pending) {
            return;
        }
        match self.session.login_status() {
            Ok(status) => self.login_gate = LoginGate::from_status(status.method),
            Err(error) => {
                self.status = SharedString::from(format!("Login status: {error}"));
                self.login_gate = LoginGate::Open;
            }
        }
    }

    pub(crate) fn unlock_with_password(
        &mut self,
        secret: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let proof = match self.login_gate {
            LoginGate::Locked {
                method: LoginMethod::Password,
            } => LoginProof::Password { secret },
            LoginGate::Locked {
                method: LoginMethod::Pin,
            } => LoginProof::Pin { secret },
            LoginGate::Locked {
                method: LoginMethod::WebAuthn,
            } => LoginProof::WebAuthn,
            _ => return,
        };
        match self.session.unlock_app(proof) {
            Ok(()) => {
                self.login_gate = LoginGate::Open;
                self.status = SharedString::from("");
                self.run_deferred_startup(window, cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
            }
        }
        cx.notify();
    }

    pub(crate) fn unlock_webauthn(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.session.unlock_app(LoginProof::WebAuthn) {
            Ok(()) => {
                self.login_gate = LoginGate::Open;
                self.status = SharedString::from("");
                self.run_deferred_startup(window, cx);
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
            }
        }
        cx.notify();
    }

    /// Startup reopen runs after the gate opens (matches Tauri App.tsx).
    pub(crate) fn run_deferred_startup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.startup_done {
            return;
        }
        self.startup_done = true;
        if let Some(notice) = self.pending_startup_notice.take() {
            self.status = SharedString::from(notice);
        }
        if let Some(info) = self.pending_startup_library.take() {
            self.adopt_library(info, window, cx);
        }
        self.schedule_silent_update_check(cx);
        cx.notify();
    }

    pub(crate) fn open_backup_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_export(ExportPreset::Backup, window, cx);
    }

    pub(crate) fn open_export_folder(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_export(ExportPreset::Folder { id, name }, window, cx);
    }

    pub(crate) fn open_export_tag(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_export(ExportPreset::Tag { name }, window, cx);
    }

    fn open_export(&mut self, preset: ExportPreset, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lib) = self.library.as_ref() else {
            return;
        };
        if matches!(lib.info.kind, LibraryKind::EagleApi) && matches!(preset, ExportPreset::Backup)
        {
            // Backup archives are native-only; Eagle live uses Export into Eagle from folders.
            self.status = SharedString::from(t(self.locale, "exportUi.nativeOnly"));
            cx.notify();
            return;
        }
        let prefs = self.session.preferences();
        let destination = prefs
            .export_destination
            .clone()
            .unwrap_or_else(|| self.session.default_library_location());
        let mut form = ExportUi::new(preset, destination);
        let selection = export_selection(&form.preset, true);
        if let Ok(preview) = self
            .session
            .export_preview(selection, form.include_subfolders)
        {
            form.preview_items = preview.items;
            form.preview_bytes = preview.bytes;
            form.preview_hidden = preview.hidden;
        }
        // Reopened while an export runs: follow it.
        if let Some(latest) = self.session.exports().into_iter().next()
            && !is_terminal_export(latest.state)
        {
            form.run_id = Some(latest.id);
            form.progress = Some(latest);
        }
        self.export_password = cx.new(|cx| InputState::new(window, cx).masked(true));
        self.export_confirm = cx.new(|cx| InputState::new(window, cx).masked(true));
        self.eagle_host_input = cx.new(|cx| {
            let defaults = self.session.connection();
            InputState::new(window, cx).default_value(defaults.host)
        });
        self.eagle_port_input = cx.new(|cx| {
            let defaults = self.session.connection();
            InputState::new(window, cx).default_value(defaults.port.to_string())
        });
        self.eagle_token_input = cx.new(|cx| InputState::new(window, cx).masked(true));
        self.export_ui = Some(form);
        self.show_connect = false;
        self.decrypt_ui = None;
        self.import_eagle_ui = None;
        cx.notify();
    }

    pub(crate) fn close_export_ui(&mut self, cx: &mut Context<Self>) {
        self.export_ui = None;
        cx.notify();
    }

    pub(crate) fn set_export_format(&mut self, format: FormatKind, cx: &mut Context<Self>) {
        if let Some(form) = self.export_ui.as_mut() {
            form.format = format;
            if format == FormatKind::Mirror {
                form.encrypt = false;
            }
            cx.notify();
        }
    }

    pub(crate) fn toggle_export_encrypt(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = self.export_ui.as_mut() {
            if form.format == FormatKind::Mirror || form.format == FormatKind::Eagle {
                form.encrypt = false;
            } else {
                form.encrypt = !form.encrypt;
            }
            cx.notify();
        }
    }

    pub(crate) fn pick_export_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from(t(self.locale, "exportUi.where"))),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    if let Some(form) = this.export_ui.as_mut() {
                        form.destination = path.to_string_lossy().into_owned();
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn start_export_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = self.export_ui.clone() else {
            return;
        };
        if form.run_id.is_some()
            && form
                .progress
                .as_ref()
                .is_some_and(|p| !is_terminal_export(p.state))
        {
            return;
        }
        let password = self.export_password.read(cx).value().to_string();
        let confirm = self.export_confirm.read(cx).value().to_string();
        if form.format != FormatKind::Eagle {
            if let Some(problem) = passphrase_problem(form.encrypt, &password, &confirm) {
                if let Some(ui) = self.export_ui.as_mut() {
                    ui.error = Some(problem.to_string());
                }
                cx.notify();
                return;
            }
            if form.destination.trim().is_empty() {
                if let Some(ui) = self.export_ui.as_mut() {
                    ui.error = Some(t(self.locale, "exportUi.chooseWhere"));
                }
                cx.notify();
                return;
            }
            // Refuse writing into the open library folder.
            if let Ok(lib_path) = self.session.library_folder() {
                let dest = PathBuf::from(&form.destination);
                if dest.starts_with(&lib_path) || lib_path.starts_with(&dest) {
                    if let Some(ui) = self.export_ui.as_mut() {
                        ui.error = Some(t(self.locale, "exportUi.refuseLibraryDest"));
                    }
                    cx.notify();
                    return;
                }
            }
        }

        let selection = export_selection(&form.preset, true);
        let request = if form.format == FormatKind::Eagle {
            let host = self.eagle_host_input.read(cx).value().to_string();
            let port = self.eagle_port_input.read(cx).value().to_string();
            let token = self.eagle_token_input.read(cx).value().to_string();
            // Clear token from the field after send (D-080).
            self.eagle_token_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            let settings = match validate_connect_host_port(&host, &port) {
                Ok(s) => s,
                Err(error) => {
                    if let Some(ui) = self.export_ui.as_mut() {
                        ui.error = Some(error);
                    }
                    cx.notify();
                    return;
                }
            };
            ExportRequest {
                selection,
                include_subfolders: form.include_subfolders,
                format: ExportFormat::Mirror,
                destination: String::new(),
                encrypt: None,
                eagle: Some(EagleExportTarget {
                    host: settings.host,
                    port: settings.port,
                    token: (!token.is_empty()).then_some(token),
                }),
            }
        } else {
            let format = export_format(form.format, form.compression, form.zip_encryption);
            build_archive_request(
                selection,
                form.include_subfolders,
                format,
                form.destination.clone(),
                form.encrypt,
                password,
            )
        };

        match self.session.start_export(request) {
            Ok(progress) => {
                if form.format != FormatKind::Eagle {
                    let mut prefs = self.session.preferences();
                    prefs.export_destination = Some(form.destination.clone());
                    let _ = self.session.save_preferences(prefs);
                }
                if let Some(ui) = self.export_ui.as_mut() {
                    ui.run_id = Some(progress.id);
                    ui.progress = Some(progress);
                    ui.error = None;
                }
                cx.notify();
            }
            Err(error) => {
                if let Some(ui) = self.export_ui.as_mut() {
                    ui.error = Some(error.to_string());
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn cancel_export_run(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.export_ui.as_ref().and_then(|u| u.run_id) else {
            return;
        };
        let _ = self.session.cancel_export(id);
        cx.notify();
    }

    pub(crate) fn reveal_export_result(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self
            .export_ui
            .as_ref()
            .and_then(|u| u.progress.as_ref())
            .and_then(|p| p.summary.as_ref())
            .map(|s| s.path.clone())
        else {
            return;
        };
        match self.session.may_reveal(&path) {
            Ok(allowed) => {
                if let Err(error) = platform::open_in_file_manager(&allowed) {
                    self.status = SharedString::from(error);
                }
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
            }
        }
        cx.notify();
    }

    pub(crate) fn on_export_progress(&mut self, progress: ExportProgress, cx: &mut Context<Self>) {
        if let Some(ui) = self.export_ui.as_mut()
            && (ui.run_id == Some(progress.id) || ui.run_id.is_none())
        {
            ui.run_id = Some(progress.id);
            ui.progress = Some(progress.clone());
        }
        self.status = SharedString::from(format!(
            "{} {}/{}",
            export_state_label(progress.state),
            progress.done,
            progress.total
        ));
        if is_terminal_export(progress.state)
            && progress.state == ExportState::Done
            && let Some(summary) = &progress.summary
        {
            self.status = SharedString::from(format!(
                "Exported {} items · {}",
                summary.items,
                format_bytes(Some(summary.bytes))
            ));
        }
        cx.notify();
    }

    pub(crate) fn open_decrypt_archive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.decrypt_password = cx.new(|cx| InputState::new(window, cx).masked(true));
        self.decrypt_ui = Some(DecryptUi {
            source: String::new(),
            destination: String::new(),
            busy: false,
            progress_done: 0,
            progress_total: 0,
            result_path: None,
            error: None,
        });
        self.export_ui = None;
        self.show_connect = false;
        self.import_eagle_ui = None;
        cx.notify();
    }

    pub(crate) fn pick_decrypt_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t(self.locale, "decryptUi.pickTitle"))),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                let path_str = path.to_string_lossy().into_owned();
                let _ = this.update(cx, |this, cx| {
                    if let Some(ui) = this.decrypt_ui.as_mut() {
                        ui.source = path_str.clone();
                        if ui.destination.is_empty() {
                            ui.destination = decrypt_default_folder(&path_str);
                        }
                        ui.result_path = None;
                        ui.error = None;
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn pick_decrypt_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from(t(self.locale, "decryptUi.saveIn"))),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    if let Some(ui) = this.decrypt_ui.as_mut() {
                        ui.destination = path.to_string_lossy().into_owned();
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn start_decrypt(&mut self, cx: &mut Context<Self>) {
        let Some(ui) = self.decrypt_ui.clone() else {
            return;
        };
        if ui.busy {
            return;
        }
        let password = self.decrypt_password.read(cx).value().to_string();
        if ui.source.trim().is_empty() {
            if let Some(d) = self.decrypt_ui.as_mut() {
                d.error = Some(t(self.locale, "decryptUi.chooseArchive"));
            }
            cx.notify();
            return;
        }
        if ui.destination.trim().is_empty() {
            if let Some(d) = self.decrypt_ui.as_mut() {
                d.error = Some(t(self.locale, "decryptUi.chooseFolder"));
            }
            cx.notify();
            return;
        }
        if password.is_empty() {
            if let Some(d) = self.decrypt_ui.as_mut() {
                d.error = Some(t(self.locale, "exportUi.typePassword"));
            }
            cx.notify();
            return;
        }
        if let Some(d) = self.decrypt_ui.as_mut() {
            d.busy = true;
            d.error = None;
            d.result_path = None;
        }
        let session = Arc::clone(&self.session);
        let source = ui.source.clone();
        let destination = ui.destination.clone();
        let result_slot: crate::app::DecryptResultSlot = Arc::new(Mutex::new(None));
        let progress_slot: Arc<Mutex<(u64, u64)>> = Arc::new(Mutex::new((0, 0)));
        self.decrypt_result = Some(result_slot.clone());
        self.decrypt_progress = Some(progress_slot.clone());
        thread::spawn(move || {
            let progress = progress_slot;
            let outcome =
                session.decrypt_archive(&source, &destination, &password, &mut |done, total| {
                    if let Ok(mut slot) = progress.lock() {
                        *slot = (done, total);
                    }
                });
            if let Ok(mut slot) = result_slot.lock() {
                *slot = Some(outcome.map_err(|e| e.to_string()));
            }
        });
        cx.notify();
    }

    pub(crate) fn cancel_decrypt_run(&mut self, cx: &mut Context<Self>) {
        self.session.cancel_decrypt();
        cx.notify();
    }

    pub(crate) fn poll_decrypt(&mut self, cx: &mut Context<Self>) {
        if let Some(progress) = self.decrypt_progress.clone()
            && let Ok(slot) = progress.lock()
            && let Some(ui) = self.decrypt_ui.as_mut()
        {
            ui.progress_done = slot.0;
            ui.progress_total = slot.1;
        }
        let Some(result) = self.decrypt_result.clone() else {
            return;
        };
        let finished = result.lock().ok().and_then(|mut g| g.take());
        let Some(finished) = finished else {
            return;
        };
        self.decrypt_result = None;
        self.decrypt_progress = None;
        match finished {
            Ok(Some(archive)) => {
                if let Some(ui) = self.decrypt_ui.as_mut() {
                    ui.busy = false;
                    ui.result_path = Some(archive.path.clone());
                    ui.error = None;
                }
                self.status = SharedString::from(format!(
                    "Decrypted · {}",
                    format_bytes(Some(archive.bytes))
                ));
            }
            Ok(None) => {
                if let Some(ui) = self.decrypt_ui.as_mut() {
                    ui.busy = false;
                    ui.error = Some(t(self.locale, "decryptUi.cancelled"));
                }
            }
            Err(error) => {
                if let Some(ui) = self.decrypt_ui.as_mut() {
                    ui.busy = false;
                    ui.error = Some(error);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn begin_connect_eagle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let defaults = self.session.connection();
        self.connect_host = cx.new(|cx| InputState::new(window, cx).default_value(defaults.host));
        self.connect_port =
            cx.new(|cx| InputState::new(window, cx).default_value(defaults.port.to_string()));
        self.connect_token = cx.new(|cx| InputState::new(window, cx).masked(true));
        self.show_connect = true;
        self.export_ui = None;
        self.decrypt_ui = None;
        self.import_eagle_ui = None;
        cx.notify();
    }

    pub(crate) fn confirm_connect_eagle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.connect_host.read(cx).value().to_string();
        let port = self.connect_port.read(cx).value().to_string();
        let token = self.connect_token.read(cx).value().to_string();
        // Clear token after send (D-080).
        self.connect_token.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        let settings = match validate_connect_host_port(&host, &port) {
            Ok(s) => s,
            Err(error) => {
                self.status = SharedString::from(error);
                cx.notify();
                return;
            }
        };
        if let Some(lib) = self.library.take() {
            lib.persist_session(&self.session);
            lib.persist_browse_prefs(&self.session);
            self.session.close_view(lib.view_id);
        }
        match self
            .session
            .connect_eagle(settings, (!token.is_empty()).then_some(token))
        {
            Ok(info) => {
                self.show_connect = false;
                self.eagle_badge = Some(format!(
                    "Eagle {} · {}:{}",
                    info.eagle_version, info.host, info.port
                ));
                self.adopt_library(info.library, window, cx);
                if !info.same_pc {
                    self.status = SharedString::from(t(self.locale, "connect.remoteHint"));
                }
            }
            Err(error) => {
                self.status = SharedString::from(format!("{error}"));
                cx.notify();
            }
        }
    }

    pub(crate) fn begin_import_eagle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = self.session.default_library_location();
        self.import_name = cx.new(|cx| InputState::new(window, cx).default_value("Imported Eagle"));
        self.import_eagle_ui = Some(ImportEagleUi {
            eagle_path: String::new(),
            summary_name: String::new(),
            summary_items: 0,
            parent,
            import_trash: false,
            error: None,
        });
        self.show_connect = false;
        self.export_ui = None;
        self.decrypt_ui = None;
        cx.notify();
    }

    pub(crate) fn pick_eagle_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(SharedString::from(t(self.locale, "importEagle.pick"))),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                let path_str = path.to_string_lossy().into_owned();
                let _ = this.update_in(cx, |this, window, cx| {
                    match this.session.eagle_library_info(&path_str) {
                        Ok(summary) => {
                            let name = summary.name.clone();
                            this.import_name.update(cx, |input, cx| {
                                input.set_value(&name, window, cx);
                            });
                            if let Some(ui) = this.import_eagle_ui.as_mut() {
                                ui.eagle_path = path_str;
                                ui.summary_name = summary.name;
                                ui.summary_items = u64::from(summary.item_count);
                                ui.error = None;
                            }
                        }
                        Err(error) => {
                            if let Some(ui) = this.import_eagle_ui.as_mut() {
                                ui.error = Some(error.to_string());
                            }
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn start_import_eagle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.import_eagle_ui.clone() else {
            return;
        };
        if ui.eagle_path.is_empty() {
            if let Some(form) = self.import_eagle_ui.as_mut() {
                form.error = Some(t(self.locale, "importEagle.choose"));
            }
            cx.notify();
            return;
        }
        let name = self.import_name.read(cx).value().to_string();
        let name = name.trim();
        if name.is_empty() {
            if let Some(form) = self.import_eagle_ui.as_mut() {
                form.error = Some(t(self.locale, "importEagle.nameRequired"));
            }
            cx.notify();
            return;
        }
        let request = EagleImportRequest {
            eagle_path: ui.eagle_path,
            create: Some(EagleLibraryTarget {
                parent: ui.parent,
                name: name.to_owned(),
            }),
            import_trash: ui.import_trash,
        };
        match self.session.import_eagle_library(request) {
            Ok(ticket) => {
                self.import_eagle_ui = None;
                self.imports_open = true;
                self.status = SharedString::from(format!(
                    "{} #{}",
                    t(self.locale, "importEagle.started"),
                    ticket.id
                ));
                // The import creates+opens the native library; refresh from session.
                if let Some(info) = self.session.current() {
                    self.adopt_library(info, window, cx);
                } else {
                    cx.notify();
                }
            }
            Err(error) => {
                if let Some(form) = self.import_eagle_ui.as_mut() {
                    form.error = Some(error.to_string());
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn login_method_label(locale: app_store::Locale, method: LoginMethod) -> String {
        match method {
            LoginMethod::None => t(locale, "settings.loginNone"),
            LoginMethod::Password => t(locale, "login.password"),
            LoginMethod::Pin => t(locale, "login.pin"),
            LoginMethod::WebAuthn => t(locale, "login.webAuthn"),
        }
    }
}

impl RootApp {
    pub(crate) fn render_login_gate(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: app_store::Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let method = match &self.login_gate {
            LoginGate::Locked { method } => *method,
            _ => LoginMethod::None,
        };
        let status = self.status.clone();
        v_flex()
            .id("login-gate")
            .size_full()
            .items_center()
            .justify_center()
            .p_8()
            .child(
                v_flex()
                    .w(px(360.))
                    .gap_3()
                    .p_6()
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.secondary)
                    .child(Label::new("egl-viewer").text_xl().font_bold())
                    .child(
                        Label::new(match method {
                            LoginMethod::Password => t(locale, "login.passwordHint"),
                            LoginMethod::Pin => t(locale, "login.pinHint"),
                            LoginMethod::WebAuthn => t(locale, "login.webAuthnHint"),
                            LoginMethod::None => String::new(),
                        })
                        .text_sm()
                        .text_color(theme.muted_foreground),
                    )
                    .when(
                        matches!(method, LoginMethod::Password | LoginMethod::Pin),
                        |el| {
                            el.child(Input::new(&self.login_secret)).child(
                                Button::new("login-submit")
                                    .primary()
                                    .label(t(locale, "login.submit"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let secret = this.login_secret.read(cx).value().to_string();
                                        this.unlock_with_password(secret, window, cx);
                                    })),
                            )
                        },
                    )
                    .when(matches!(method, LoginMethod::WebAuthn), |el| {
                        el.child(
                            Button::new("login-webauthn")
                                .primary()
                                .label(t(locale, "login.touchYubiKey"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.unlock_webauthn(window, cx);
                                })),
                        )
                    })
                    .when(!status.is_empty(), |el| {
                        el.child(Label::new(status).text_sm().text_color(theme.danger))
                    }),
            )
    }

    pub(crate) fn render_export_overlay(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: app_store::Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let form = self.export_ui.as_ref().unwrap();
        let running = form
            .progress
            .as_ref()
            .is_some_and(|p| !is_terminal_export(p.state));
        let title = t(locale, form.title_key());
        v_flex()
            .id("export-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.background.opacity(0.55))
            .child(
                v_flex()
                    .w(px(520.))
                    .max_h(px(640.))
                    .gap_2()
                    .p_4()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(Label::new(title).font_bold().text_lg())
                    .child(
                        Label::new(format!(
                            "{} · {}{}",
                            form.preview_items,
                            format_bytes(Some(form.preview_bytes)),
                            if form.preview_hidden > 0 {
                                format!(" · {} hidden", form.preview_hidden)
                            } else {
                                String::new()
                            }
                        ))
                        .text_sm()
                        .text_color(theme.muted_foreground),
                    )
                    .child(Label::new(t(locale, "exportUi.format")).font_bold())
                    .child(
                        h_flex()
                            .gap_1()
                            .child(format_btn(
                                "fmt-zip",
                                "ZIP",
                                FormatKind::Zip,
                                form.format,
                                cx,
                            ))
                            .child(format_btn(
                                "fmt-7z",
                                "7z",
                                FormatKind::SevenZip,
                                form.format,
                                cx,
                            ))
                            .child(format_btn(
                                "fmt-mirror",
                                t(locale, "exportUi.mirror"),
                                FormatKind::Mirror,
                                form.format,
                                cx,
                            ))
                            .child(format_btn(
                                "fmt-eagle",
                                t(locale, "exportUi.intoEagle"),
                                FormatKind::Eagle,
                                form.format,
                                cx,
                            )),
                    )
                    .when(form.format != FormatKind::Eagle, |el| {
                        el.child(Label::new(t(locale, "exportUi.where")).text_sm())
                            .child(
                                Label::new(form.destination.clone())
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                Button::new("export-browse")
                                    .label(t(locale, "common.browse"))
                                    .disabled(running)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.pick_export_destination(window, cx);
                                    })),
                            )
                    })
                    .when(
                        matches!(form.format, FormatKind::Zip | FormatKind::SevenZip),
                        |el| {
                            el.child(
                                Button::new("export-encrypt")
                                    .ghost()
                                    .label(format!(
                                        "{} {}",
                                        if form.encrypt { "☑" } else { "☐" },
                                        t(locale, "exportUi.encrypt")
                                    ))
                                    .disabled(running)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_export_encrypt(cx);
                                    })),
                            )
                            .when(form.encrypt, |el| {
                                el.child(Input::new(&self.export_password))
                                    .child(Input::new(&self.export_confirm))
                                    .when(form.format == FormatKind::Zip, |el| {
                                        el.child(
                                            h_flex()
                                                .gap_1()
                                                .child(
                                                    Button::new("enc-age")
                                                        .ghost()
                                                        .label(t(locale, "exportUi.protectionAge"))
                                                        .disabled(running)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            if let Some(ui) =
                                                                this.export_ui.as_mut()
                                                            {
                                                                ui.zip_encryption =
                                                                    ZipEncryption::Age;
                                                            }
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    Button::new("enc-aes")
                                                        .ghost()
                                                        .label(t(locale, "exportUi.protectionAes"))
                                                        .disabled(running)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            if let Some(ui) =
                                                                this.export_ui.as_mut()
                                                            {
                                                                ui.zip_encryption =
                                                                    ZipEncryption::Aes;
                                                            }
                                                            cx.notify();
                                                        })),
                                                ),
                                        )
                                    })
                            })
                        },
                    )
                    .when(form.format == FormatKind::Eagle, |el| {
                        el.child(Label::new(t(locale, "connect.host")).text_sm())
                            .child(Input::new(&self.eagle_host_input))
                            .child(Label::new(t(locale, "connect.port")).text_sm())
                            .child(Input::new(&self.eagle_port_input))
                            .child(Label::new(t(locale, "connect.token")).text_sm())
                            .child(Input::new(&self.eagle_token_input))
                            .child(
                                Label::new(t(locale, "exportUi.eagleLockedWarning"))
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            )
                    })
                    .when_some(form.progress.as_ref(), |el, progress| {
                        el.child(
                            Label::new(format!(
                                "{} {}/{} · {}",
                                export_state_label(progress.state),
                                progress.done,
                                progress.total,
                                format_bytes(Some(progress.bytes_done))
                            ))
                            .text_sm(),
                        )
                    })
                    .when_some(form.error.as_ref(), |el, error| {
                        el.child(Label::new(error.clone()).text_sm().text_color(theme.danger))
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("export-close")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_export_ui(cx);
                                    })),
                            )
                            .child(div_flex())
                            .when(running, |el| {
                                el.child(
                                    Button::new("export-cancel-run")
                                        .label(t(locale, "common.cancel"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cancel_export_run(cx);
                                        })),
                                )
                            })
                            .when(
                                form.progress
                                    .as_ref()
                                    .is_some_and(|p| p.state == ExportState::Done),
                                |el| {
                                    el.child(
                                        Button::new("export-reveal")
                                            .label(t(locale, "librarySettings.reveal"))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.reveal_export_result(cx);
                                            })),
                                    )
                                },
                            )
                            .child(
                                Button::new("export-start")
                                    .primary()
                                    .label(match &form.preset {
                                        ExportPreset::Backup => t(locale, "exportUi.backUp"),
                                        _ => t(locale, "exportUi.export"),
                                    })
                                    .disabled(running)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_export_run(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        Label::new(format!(
                            "Passphrase min {MIN_PASSPHRASE_CHARS} chars when encrypting"
                        ))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                    ),
            )
    }

    pub(crate) fn render_connect_overlay(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: app_store::Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let status = self.status.clone();
        v_flex()
            .id("connect-overlay")
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
                    .child(Label::new(t(locale, "connect.title")).font_bold().text_lg())
                    .child(
                        Label::new(t(locale, "connect.description"))
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(Label::new(t(locale, "connect.host")).text_sm())
                    .child(Input::new(&self.connect_host))
                    .child(Label::new(t(locale, "connect.port")).text_sm())
                    .child(Input::new(&self.connect_port))
                    .child(Label::new(t(locale, "connect.token")).text_sm())
                    .child(Input::new(&self.connect_token))
                    .child(
                        Label::new(t(locale, "connect.tokenHint"))
                            .text_xs()
                            .text_color(theme.muted_foreground),
                    )
                    .when(!status.is_empty(), |el| {
                        el.child(
                            Label::new(status)
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("connect-back")
                                    .label(t(locale, "connect.back"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_connect = false;
                                        cx.notify();
                                    })),
                            )
                            .child(div_flex())
                            .child(
                                Button::new("connect-ok")
                                    .primary()
                                    .label(t(locale, "connect.connect"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_connect_eagle(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    pub(crate) fn render_decrypt_overlay(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: app_store::Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ui = self.decrypt_ui.as_ref().unwrap();
        v_flex()
            .id("decrypt-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.background.opacity(0.55))
            .child(
                v_flex()
                    .w(px(480.))
                    .gap_2()
                    .p_4()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(
                        Label::new(t(locale, "decryptUi.title"))
                            .font_bold()
                            .text_lg(),
                    )
                    .child(
                        Label::new(t(locale, "decryptUi.description"))
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(Label::new(t(locale, "decryptUi.archive")).text_sm())
                    .child(
                        Label::new(if ui.source.is_empty() {
                            "—".into()
                        } else {
                            ui.source.clone()
                        })
                        .text_xs()
                        .text_color(theme.muted_foreground),
                    )
                    .child(
                        Button::new("decrypt-pick")
                            .label(t(locale, "decryptUi.choose"))
                            .disabled(ui.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pick_decrypt_source(window, cx);
                            })),
                    )
                    .child(Label::new(t(locale, "decryptUi.saveIn")).text_sm())
                    .child(
                        Label::new(if ui.destination.is_empty() {
                            "—".into()
                        } else {
                            ui.destination.clone()
                        })
                        .text_xs()
                        .text_color(theme.muted_foreground),
                    )
                    .child(
                        Button::new("decrypt-dest")
                            .label(t(locale, "common.browse"))
                            .disabled(ui.busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pick_decrypt_destination(window, cx);
                            })),
                    )
                    .child(Label::new(t(locale, "exportUi.typePassword")).text_sm())
                    .child(Input::new(&self.decrypt_password))
                    .when(ui.busy, |el| {
                        el.child(
                            Label::new(format!(
                                "{} {}/{}",
                                t(locale, "decryptUi.decrypting"),
                                ui.progress_done,
                                ui.progress_total
                            ))
                            .text_sm(),
                        )
                    })
                    .when_some(ui.result_path.as_ref(), |el, path| {
                        el.child(Label::new(format!("Done: {path}")).text_sm())
                    })
                    .when_some(ui.error.as_ref(), |el, error| {
                        el.child(Label::new(error.clone()).text_sm().text_color(theme.danger))
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("decrypt-close")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.decrypt_ui.as_ref().is_some_and(|u| u.busy) {
                                            this.cancel_decrypt_run(cx);
                                        }
                                        this.decrypt_ui = None;
                                        cx.notify();
                                    })),
                            )
                            .child(div_flex())
                            .child(
                                Button::new("decrypt-go")
                                    .primary()
                                    .label(t(locale, "decryptUi.decrypt"))
                                    .disabled(ui.busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.start_decrypt(cx);
                                    })),
                            ),
                    ),
            )
    }

    pub(crate) fn render_import_eagle_overlay(
        &self,
        theme: &gpui_kit::component::theme::Theme,
        locale: app_store::Locale,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ui = self.import_eagle_ui.as_ref().unwrap();
        v_flex()
            .id("import-eagle-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.background.opacity(0.55))
            .child(
                v_flex()
                    .w(px(480.))
                    .gap_2()
                    .p_4()
                    .rounded(theme.radius_lg)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(
                        Label::new(t(locale, "importEagle.title"))
                            .font_bold()
                            .text_lg(),
                    )
                    .child(
                        Label::new(t(locale, "importEagle.description"))
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        Label::new(if ui.eagle_path.is_empty() {
                            t(locale, "importEagle.choose")
                        } else {
                            format!("{} · {} items", ui.summary_name, ui.summary_items)
                        })
                        .text_sm(),
                    )
                    .child(
                        Button::new("import-eagle-pick")
                            .label(t(locale, "importEagle.pick"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pick_eagle_library(window, cx);
                            })),
                    )
                    .child(Label::new(t(locale, "createLibrary.name")).text_sm())
                    .child(Input::new(&self.import_name))
                    .child(
                        Label::new(format!(
                            "{}: {}",
                            t(locale, "createLibrary.location"),
                            ui.parent
                        ))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                    )
                    .child(
                        Button::new("import-eagle-trash")
                            .ghost()
                            .label(format!(
                                "{} {}",
                                if ui.import_trash { "☑" } else { "☐" },
                                t(locale, "importEagle.importTrash")
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(ui) = this.import_eagle_ui.as_mut() {
                                    ui.import_trash = !ui.import_trash;
                                }
                                cx.notify();
                            })),
                    )
                    .when_some(ui.error.as_ref(), |el, error| {
                        el.child(Label::new(error.clone()).text_sm().text_color(theme.danger))
                    })
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("import-eagle-close")
                                    .label(t(locale, "common.cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.import_eagle_ui = None;
                                        cx.notify();
                                    })),
                            )
                            .child(div_flex())
                            .child(
                                Button::new("import-eagle-go")
                                    .primary()
                                    .label(t(locale, "importEagle.start"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_import_eagle(window, cx);
                                    })),
                            ),
                    ),
            )
    }
}

fn div_flex() -> gpui_kit::Div {
    gpui_kit::div().flex_1()
}

fn format_btn(
    id: &'static str,
    label: impl Into<SharedString>,
    kind: FormatKind,
    current: FormatKind,
    cx: &mut Context<RootApp>,
) -> impl IntoElement {
    let selected = kind == current;
    Button::new(id)
        .when(selected, |b| b.primary())
        .when(!selected, |b| b.ghost())
        .label(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_export_format(kind, cx);
        }))
}
