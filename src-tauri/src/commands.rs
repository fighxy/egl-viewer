//! IPC commands. Each one parses input, calls the core and returns a UI-shaped result.

use app_store::{ConnectionSettings, Preferences, RecentLibrary};
use library_core::{LibraryInfo, SourceError};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

use crate::session::{Session, StartupResult};

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
