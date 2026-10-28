//! IPC commands. Each one parses input, calls the core and returns a UI-shaped result.

use std::path::PathBuf;

use app_store::{ConnectionSettings, Preferences, RecentLibrary};
use library_core::{
    ImportProgress, ImportTicket, Item, ItemCell, ItemId, LibraryCounts, LibraryInfo, LibraryQuery,
    SourceError,
};
use library_native::DuplicateChoice;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

use crate::session::{Clipboard, Session, StartupResult};
use crate::views::{ScopeProgress, ViewOpened, count};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    /// The short git commit of the build, when it was built from a checkout.
    pub commit: Option<String>,
    /// The WebView2 (or WebKit) version.
    pub webview: Option<String>,
}

impl AppInfo {
    fn of_this_build() -> Self {
        Self {
            name: env!("CARGO_PKG_NAME").to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: option_env!("EGL_COMMIT").map(str::to_owned),
            webview: None,
        }
    }
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        webview: tauri::webview_version().ok(),
        ..AppInfo::of_this_build()
    }
}

/// The values the Connect screen starts with: Eagle on this PC.
#[tauri::command]
pub fn connection_defaults() -> ConnectionSettings {
    ConnectionSettings::default()
}

/// Where Create library starts: the preference, or `%USERPROFILE%\egl-library`.
#[tauri::command]
pub fn default_library_location(session: State<'_, Session>) -> String {
    session.default_library_location()
}

#[tauri::command]
pub async fn create_library(
    app: AppHandle,
    parent: String,
    name: String,
) -> Result<LibraryInfo, SourceError> {
    blocking(app, move |session| session.create_library(&parent, &name)).await
}

#[tauri::command]
pub async fn open_library(app: AppHandle, path: String) -> Result<LibraryInfo, SourceError> {
    blocking(app, move |session| session.open_library(&path)).await
}

#[tauri::command]
pub async fn close_library(app: AppHandle) -> Result<(), SourceError> {
    blocking(app, Session::close_library).await
}

#[tauri::command]
pub fn current_library(session: State<'_, Session>) -> Option<LibraryInfo> {
    session.current()
}

#[tauri::command]
pub async fn recent_libraries(app: AppHandle) -> Result<Vec<RecentLibrary>, SourceError> {
    blocking(app, |session| Ok(session.recent_libraries())).await
}

#[tauri::command]
pub fn forget_recent_library(session: State<'_, Session>, path: String) -> Result<(), SourceError> {
    session.forget_recent_library(&path)
}

#[tauri::command]
pub fn get_preferences(session: State<'_, Session>) -> Preferences {
    session.preferences()
}

#[tauri::command]
pub async fn set_preferences(
    app: AppHandle,
    preferences: Preferences,
) -> Result<Preferences, SourceError> {
    blocking(app, move |session| session.save_preferences(preferences)).await
}

/// Reopens the last library; `--no-reopen` on the command line skips it.
#[tauri::command]
pub async fn startup(app: AppHandle) -> Result<StartupResult, SourceError> {
    let no_reopen = std::env::args().any(|arg| arg == "--no-reopen");
    blocking(app, move |session| Ok(session.startup(no_reopen))).await
}

#[tauri::command]
pub async fn library_counts(app: AppHandle) -> Result<LibraryCounts, SourceError> {
    blocking(app, Session::counts).await
}

/// Opens a view for `query`; `on_progress` hears how far the scope loaded
/// (docs/architecture.md §6.5).
#[tauri::command]
pub async fn open_view(
    app: AppHandle,
    query: LibraryQuery,
    on_progress: Channel<ScopeProgress>,
) -> Result<ViewOpened, SourceError> {
    blocking(app, move |session| {
        session.open_view(query, &|loaded, total| {
            let _ = on_progress.send(ScopeProgress {
                loaded: count(loaded),
                total: count(total),
            });
        })
    })
    .await
}

/// A window of a view: at most 200 cells. Whole item lists never cross IPC.
#[tauri::command]
pub async fn view_range(
    app: AppHandle,
    view_id: u64,
    start: u32,
    len: u32,
) -> Result<Vec<ItemCell>, SourceError> {
    blocking(app, move |session| session.view_range(view_id, start, len)).await
}

/// Ids only, for a selection range.
#[tauri::command]
pub async fn view_ids(
    app: AppHandle,
    view_id: u64,
    start: u32,
    len: u32,
) -> Result<Vec<ItemId>, SourceError> {
    blocking(app, move |session| session.view_ids(view_id, start, len)).await
}

#[tauri::command]
pub async fn close_view(app: AppHandle, view_id: u64) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.close_view(view_id);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn item_detail(app: AppHandle, id: String) -> Result<Option<Item>, SourceError> {
    blocking(app, move |session| session.item_detail(&id)).await
}

/// Files and folders by path (picked, dropped or pasted); their bytes never cross IPC.
#[tauri::command]
pub async fn add_files(app: AppHandle, paths: Vec<String>) -> Result<ImportTicket, SourceError> {
    blocking(app, move |session| session.add_files(paths)).await
}

/// Copied files or an image on the clipboard; `None` when there is nothing to add.
#[tauri::command]
pub async fn paste_files(app: AppHandle) -> Result<Option<ImportTicket>, SourceError> {
    blocking(app, |session| session.paste(&SystemClipboard)).await
}

/// The user's answer to the duplicate question of an import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DuplicateAnswer {
    Skip,
    AddAnyway,
    Cancel,
}

#[tauri::command]
pub fn answer_import(
    session: State<'_, Session>,
    id: u64,
    answer: DuplicateAnswer,
) -> Result<(), SourceError> {
    let choice = match answer {
        DuplicateAnswer::Skip => DuplicateChoice::Skip,
        DuplicateAnswer::AddAnyway => DuplicateChoice::AddAnyway,
        DuplicateAnswer::Cancel => DuplicateChoice::Cancel,
    };
    session.answer_import(id, choice)
}

#[tauri::command]
pub fn cancel_import(session: State<'_, Session>, id: u64) -> Result<(), SourceError> {
    session.cancel_import(id)
}

#[tauri::command]
pub fn imports(session: State<'_, Session>) -> Vec<ImportProgress> {
    session.imports()
}

/// Quits after the user agreed to stop running imports (the window's close was held back).
#[tauri::command]
pub async fn quit(app: AppHandle) -> Result<(), SourceError> {
    if let Some(window) = app.get_webview_window("main") {
        crate::window::remember(&window.as_ref().window(), app.state::<Session>().store());
    }
    let handle = app.clone();
    blocking(app, |session| {
        session.stop_imports();
        Ok(())
    })
    .await?;
    handle.exit(0);
    Ok(())
}

/// The system clipboard through `arboard`.
struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn files(&self) -> Option<Vec<PathBuf>> {
        arboard::Clipboard::new().ok()?.get().file_list().ok()
    }

    fn image(&self) -> Option<(u32, u32, Vec<u8>)> {
        let image = arboard::Clipboard::new().ok()?.get().image().ok()?;
        Some((
            u32::try_from(image.width).ok()?,
            u32::try_from(image.height).ok()?,
            image.bytes.into_owned(),
        ))
    }
}

/// Runs file and database work off the main thread.
async fn blocking<T, F>(app: AppHandle, work: F) -> Result<T, SourceError>
where
    T: Send + 'static,
    F: FnOnce(&Session) -> Result<T, SourceError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<Session>()))
        .await
        .map_err(|error| SourceError::Storage(format!("background task failed: {error}")))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_the_crate() {
        let info = AppInfo::of_this_build();
        assert_eq!(info.name, "egl-viewer");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn connection_defaults_point_at_this_pc() {
        let defaults = connection_defaults();
        assert_eq!(defaults.port, 41595);
        assert!(defaults.is_same_pc());
    }

    #[test]
    fn app_info_is_camel_case_on_the_wire() {
        let json = serde_json::to_value(AppInfo::of_this_build()).unwrap();
        assert!(json.get("name").is_some() && json.get("version").is_some());
        assert!(json.as_object().unwrap().contains_key("webview"));
    }
}
