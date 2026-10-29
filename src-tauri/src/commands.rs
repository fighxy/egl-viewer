//! IPC commands. Each one parses input, calls the core and returns a UI-shaped result.

use std::path::PathBuf;

use app_store::{ConnectionSettings, Preferences, RecentLibrary};
use library_core::{
    ExtensionCount, Folder, FolderChange, ImportProgress, ImportTicket, Item, ItemCell, ItemEdit,
    ItemId, LibraryCounts, LibraryInfo, LibraryQuery, RecoveryKey, SelectionTags, SmartFolder,
    SmartFolderChange, SourceError, Tag, TagGroup, TagGroupChange,
};
use library_native::DuplicateChoice;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use ts_rs::TS;

use crate::session::{Clipboard, Session, SmartDraft, StartupResult};
use crate::views::{ItemsTarget, ScopeProgress, ViewOpened, count};

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

#[tauri::command]
pub async fn folder_tree(app: AppHandle) -> Result<Vec<Folder>, SourceError> {
    blocking(app, Session::folder_tree).await
}

/// Creates, edits or moves a folder (a move under itself is refused); returns the new tree.
#[tauri::command]
pub async fn save_folder(app: AppHandle, change: FolderChange) -> Result<Vec<Folder>, SourceError> {
    blocking(app, move |session| session.save_folder(change)).await
}

/// How far encrypting or decrypting a locked folder's items got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct LockProgress {
    pub done: u32,
    pub total: u32,
}

/// Puts a lock on a folder (docs/age-locked-folders.md). The password is used once and never
/// stored; the recovery key comes back to be shown once.
#[tauri::command]
pub async fn lock_folder(
    app: AppHandle,
    id: String,
    password: String,
    label: Option<String>,
    label_color: Option<String>,
    on_progress: Channel<LockProgress>,
) -> Result<RecoveryKey, SourceError> {
    blocking(app, move |session| {
        session.lock_folder(&id, password, label, label_color, &|done, total| {
            let _ = on_progress.send(LockProgress { done, total });
        })
    })
    .await
}

#[tauri::command]
pub async fn unlock_folder(
    app: AppHandle,
    id: String,
    password: String,
) -> Result<(), SourceError> {
    blocking(app, move |session| session.unlock_folder(&id, password)).await
}

#[tauri::command]
pub async fn recover_folder(
    app: AppHandle,
    id: String,
    recovery: String,
    new_password: String,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.recover_folder(&id, &recovery, new_password)
    })
    .await
}

#[tauri::command]
pub async fn relock_folder(app: AppHandle, id: String) -> Result<(), SourceError> {
    blocking(app, move |session| session.relock_folder(&id)).await
}

#[tauri::command]
pub async fn change_folder_password(
    app: AppHandle,
    id: String,
    current: String,
    new_password: String,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.change_folder_password(&id, current, new_password)
    })
    .await
}

#[tauri::command]
pub async fn renew_recovery_key(
    app: AppHandle,
    id: String,
    current: String,
) -> Result<RecoveryKey, SourceError> {
    blocking(app, move |session| session.renew_recovery_key(&id, current)).await
}

#[tauri::command]
pub async fn remove_folder_lock(
    app: AppHandle,
    id: String,
    password: String,
    on_progress: Channel<LockProgress>,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.remove_folder_lock(&id, password, &|done, total| {
            let _ = on_progress.send(LockProgress { done, total });
        })
    })
    .await
}

#[tauri::command]
pub async fn set_lock_label(
    app: AppHandle,
    id: String,
    label: Option<String>,
    label_color: Option<String>,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.set_lock_label(&id, label, label_color)
    })
    .await
}

/// Saves the recovery key to a file the user picked, outside the library folder.
#[tauri::command]
pub async fn save_recovery_file(
    app: AppHandle,
    path: String,
    folder: String,
    key: RecoveryKey,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.save_recovery_file(&path, &folder, &key)
    })
    .await
}

#[tauri::command]
pub async fn delete_folder(
    app: AppHandle,
    id: String,
    trash_items: bool,
) -> Result<Vec<Folder>, SourceError> {
    blocking(app, move |session| session.delete_folder(&id, trash_items)).await
}

#[tauri::command]
pub async fn apply_item_edit(
    app: AppHandle,
    id: String,
    edit: ItemEdit,
) -> Result<Item, SourceError> {
    blocking(app, move |session| session.apply_item_edit(&id, edit)).await
}

/// One edit for a selection (ids, or a whole view except some); returns how many changed.
#[tauri::command]
pub async fn apply_batch_edit(
    app: AppHandle,
    target: ItemsTarget,
    edit: ItemEdit,
) -> Result<u32, SourceError> {
    blocking(app, move |session| session.apply_batch_edit(target, edit)).await
}

/// The tags of a selection with how many selected items carry each.
#[tauri::command]
pub async fn selection_tags(
    app: AppHandle,
    target: ItemsTarget,
) -> Result<SelectionTags, SourceError> {
    blocking(app, move |session| session.selection_tags(target)).await
}

/// Every tag with its item count, for suggestions and the Tags filter.
#[tauri::command]
pub async fn tag_vocabulary(app: AppHandle) -> Result<Vec<Tag>, SourceError> {
    blocking(app, Session::tag_vocabulary).await
}

#[tauri::command]
pub async fn tag_groups(app: AppHandle) -> Result<Vec<TagGroup>, SourceError> {
    blocking(app, Session::tag_groups).await
}

#[tauri::command]
pub async fn save_tag_group(
    app: AppHandle,
    change: TagGroupChange,
) -> Result<Vec<TagGroup>, SourceError> {
    blocking(app, move |session| session.save_tag_group(change)).await
}

#[tauri::command]
pub async fn delete_tag_group(app: AppHandle, id: String) -> Result<Vec<TagGroup>, SourceError> {
    blocking(app, move |session| session.delete_tag_group(&id)).await
}

/// Renames a tag on every item; onto an existing tag, the two merge.
#[tauri::command]
pub async fn rename_tag(app: AppHandle, from: String, to: String) -> Result<(), SourceError> {
    blocking(app, move |session| session.rename_tag(&from, &to)).await
}

#[tauri::command]
pub async fn merge_tags(app: AppHandle, source: String, target: String) -> Result<(), SourceError> {
    blocking(app, move |session| session.merge_tags(&source, &target)).await
}

#[tauri::command]
pub async fn delete_tag(app: AppHandle, name: String) -> Result<(), SourceError> {
    blocking(app, move |session| session.delete_tag(&name)).await
}

#[tauri::command]
pub async fn smart_folders(app: AppHandle) -> Result<Vec<SmartFolder>, SourceError> {
    blocking(app, Session::smart_folders).await
}

/// Creates or edits a smart folder (Eagle's rules, stored unchanged); returns the tree.
#[tauri::command]
pub async fn save_smart_folder(
    app: AppHandle,
    change: SmartFolderChange,
) -> Result<Vec<SmartFolder>, SourceError> {
    blocking(app, move |session| session.save_smart_folder(change)).await
}

#[tauri::command]
pub async fn delete_smart_folder(
    app: AppHandle,
    id: String,
) -> Result<Vec<SmartFolder>, SourceError> {
    blocking(app, move |session| session.delete_smart_folder(&id)).await
}

/// "Save as smart folder": the rules that keep what the screen of `query` shows.
#[tauri::command]
pub async fn smart_rules(app: AppHandle, query: LibraryQuery) -> Result<SmartDraft, SourceError> {
    blocking(app, move |session| session.smart_rules(&query)).await
}

/// File types with their item counts, for the Type filter.
#[tauri::command]
pub async fn extensions(app: AppHandle) -> Result<Vec<ExtensionCount>, SourceError> {
    blocking(app, Session::extensions).await
}

/// Manual order: `ids` go before `before` (or to the end) in `folder`.
#[tauri::command]
pub async fn reorder_items(
    app: AppHandle,
    folder: String,
    ids: Vec<ItemId>,
    before: Option<ItemId>,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.reorder_items(&folder, ids, before)
    })
    .await
}

/// Files and folders by path (picked, dropped or pasted), into `folder` when one is open;
/// their bytes never cross IPC.
#[tauri::command]
pub async fn add_files(
    app: AppHandle,
    paths: Vec<String>,
    folder: Option<String>,
) -> Result<ImportTicket, SourceError> {
    blocking(app, move |session| session.add_files(paths, folder)).await
}

/// Copied files or an image on the clipboard; `None` when there is nothing to add.
#[tauri::command]
pub async fn paste_files(
    app: AppHandle,
    folder: Option<String>,
) -> Result<Option<ImportTicket>, SourceError> {
    blocking(app, move |session| session.paste(&SystemClipboard, folder)).await
}

/// The user's answer to the duplicate question of an import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DuplicateAnswer {
    Skip,
    AddAnyway,
    /// The existing item joins the folder the file was going into.
    AddToFolder,
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
        DuplicateAnswer::AddToFolder => DuplicateChoice::AddToFolder,
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
