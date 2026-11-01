//! IPC commands. Each one parses input, calls the core and returns a UI-shaped result.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use app_store::{ConnectionSettings, LoginProof, LoginStatus, Preferences, RecentLibrary};
use library_core::{
    Backup, ExportPreview, ExportProgress, ExportRequest, ExportSelection, ExtensionCount, Folder,
    FolderChange, ImportPreview, ImportProgress, ImportTicket, Item, ItemCell, ItemEdit, ItemId,
    LibraryCheck, LibraryCounts, LibraryInfo, LibraryQuery, LibraryStats, PivPolicy, PivPrompt,
    PivWrap, RecoveryKey, RepairOptions, SelectionTags, SmartFolder, SmartFolderChange,
    SourceError, Tag, TagGroup, TagGroupChange, TrashInfo, UndoStatus, is_web_link,
};
use library_native::{DuplicateChoice, ImportFilter};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use ts_rs::TS;

use crate::session::{
    Clipboard, EagleImportRequest, EagleLibrarySummary, EagleSessionInfo, Session, SmartDraft,
    StartupResult,
};
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

/// The values the Connect screen starts with: the last successful connection, or Eagle on
/// this PC.
#[tauri::command]
pub fn connection_defaults(session: State<'_, Session>) -> ConnectionSettings {
    session.connection()
}

/// Connects to a running Eagle; every failure kind maps to its own copy on the Connect
/// screen (docs/spec.md §8.3 "Connect screen").
#[tauri::command]
pub async fn connect_eagle(
    app: AppHandle,
    host: String,
    port: u16,
    token: Option<String>,
) -> Result<EagleSessionInfo, SourceError> {
    let settings = ConnectionSettings { host, port };
    let token = token.filter(|token| !token.is_empty());
    blocking(app, move |session| session.connect_eagle(settings, token)).await
}

/// The live Eagle connection, if any.
#[tauri::command]
pub fn eagle_session(session: State<'_, Session>) -> Option<EagleSessionInfo> {
    session.eagle_session()
}

#[tauri::command]
pub async fn disconnect_eagle(app: AppHandle) -> Result<(), SourceError> {
    blocking(app, Session::disconnect_eagle).await
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

/// Stored width/height of every position: justified packing without whole `ItemCell` lists.
#[tauri::command]
pub async fn view_dimensions(app: AppHandle, view_id: u64) -> Result<Vec<[u32; 2]>, SourceError> {
    blocking(app, move |session| session.view_dimensions(view_id)).await
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
pub async fn piv_wraps(app: AppHandle, id: String) -> Result<Vec<PivWrap>, SourceError> {
    blocking(app, move |session| session.piv_wraps(&id)).await
}

/// Binds a YubiKey to one locked folder. `pin` is set only when `policy` is the PIN;
/// a touch does not take a PIN as well.
#[tauri::command]
pub async fn bind_yubikey(
    app: AppHandle,
    id: String,
    password: String,
    policy: PivPolicy,
    pin: Option<String>,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.bind_yubikey(&id, password, policy, pin)
    })
    .await
}

#[tauri::command]
pub async fn unbind_yubikey(
    app: AppHandle,
    id: String,
    password: String,
    serial: String,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.unbind_yubikey(&id, password, serial)
    })
    .await
}

#[tauri::command]
pub async fn unlock_with_yubikey(
    app: AppHandle,
    id: String,
    serial: String,
    prompt: PivPrompt,
) -> Result<(), SourceError> {
    blocking(app, move |session| {
        session.unlock_with_yubikey(&id, serial, prompt)
    })
    .await
}

#[tauri::command]
pub fn yubikey_present() -> bool {
    crate::yubikey::present()
}

#[tauri::command]
pub fn login_status(session: State<'_, Session>) -> LoginStatus {
    session.login_status()
}

#[tauri::command]
pub fn clear_login(session: State<'_, Session>) -> Result<LoginStatus, SourceError> {
    session.clear_login()
}

#[tauri::command]
pub fn set_login_password(
    session: State<'_, Session>,
    password: String,
) -> Result<LoginStatus, SourceError> {
    session.set_login_password(password)
}

#[tauri::command]
pub fn set_login_pin(session: State<'_, Session>, pin: String) -> Result<LoginStatus, SourceError> {
    session.set_login_pin(pin)
}

#[tauri::command]
pub fn set_login_webauthn(session: State<'_, Session>) -> Result<LoginStatus, SourceError> {
    session.set_login_webauthn()
}

#[tauri::command]
pub fn unlock_app(session: State<'_, Session>, proof: LoginProof) -> Result<(), SourceError> {
    session.unlock_app(proof)
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

/// `Ctrl+Z`: takes back the last item edit; returns what it was, or `None`.
#[tauri::command]
pub async fn undo(app: AppHandle) -> Result<Option<UndoStatus>, SourceError> {
    blocking(app, Session::undo).await
}

/// What `Ctrl+Z` would take back.
#[tauri::command]
pub fn undo_status(session: State<'_, Session>) -> Option<UndoStatus> {
    session.undo_status()
}

/// Removes selected items of the trash for good, with their files.
#[tauri::command]
pub async fn delete_permanently(app: AppHandle, target: ItemsTarget) -> Result<u32, SourceError> {
    blocking(app, move |session| session.delete_permanently(target)).await
}

/// Empties the trash; `on_progress` hears `(done, total)` while rows and files go (D-074).
#[tauri::command]
pub async fn empty_trash(
    app: AppHandle,
    on_progress: Channel<LockProgress>,
) -> Result<u32, SourceError> {
    blocking(app, move |session| {
        session.empty_trash(&|done, total| {
            let _ = on_progress.send(LockProgress { done, total });
        })
    })
    .await
}

#[tauri::command]
pub async fn cancel_empty_trash(app: AppHandle) -> Result<(), SourceError> {
    blocking(app, |session| {
        session.cancel_empty_trash();
        Ok(())
    })
    .await
}

/// Items and bytes of the trash, for the context menu of the Trash row.
#[tauri::command]
pub async fn trash_info(app: AppHandle) -> Result<TrashInfo, SourceError> {
    blocking(app, Session::trash_info).await
}

/// Restore All of the trash context menu: every visible trash item comes back.
#[tauri::command]
pub async fn restore_trash(app: AppHandle) -> Result<u32, SourceError> {
    blocking(app, Session::restore_trash).await
}

/// Opens the original with the app Windows associates with its type.
#[tauri::command]
pub async fn open_original(app: AppHandle, id: ItemId) -> Result<(), SourceError> {
    let paths = blocking(app, move |session| session.original_paths(&[id])).await?;
    for path in paths {
        tauri_plugin_opener::open_path(&path, None::<&str>).map_err(opener_error)?;
    }
    Ok(())
}

/// Shows the originals selected in Explorer.
#[tauri::command]
pub async fn reveal_originals(app: AppHandle, ids: Vec<ItemId>) -> Result<(), SourceError> {
    let paths = blocking(app, move |session| session.original_paths(&ids)).await?;
    tauri_plugin_opener::reveal_items_in_dir(&paths).map_err(opener_error)
}

/// Puts the originals on the clipboard as files, ready to paste in Explorer or another app.
#[tauri::command]
pub async fn copy_originals(app: AppHandle, ids: Vec<ItemId>) -> Result<u32, SourceError> {
    let paths = blocking(app, move |session| session.original_paths(&ids)).await?;
    copy_files(&paths)
        .map_err(|error| SourceError::Storage(format!("copy to the clipboard: {error}")))?;
    Ok(u32::try_from(paths.len()).unwrap_or(u32::MAX))
}

/// On X11 and Wayland the copied files stay on the clipboard only while arboard's `Clipboard`
/// lives, so Copy File keeps one for the whole run. Windows keeps the data on its own.
static CLIPBOARD: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

fn copy_files(paths: &[PathBuf]) -> Result<(), arboard::Error> {
    let mut slot = CLIPBOARD.lock().unwrap_or_else(PoisonError::into_inner);
    let clipboard = match slot.as_mut() {
        Some(clipboard) => clipboard,
        None => slot.insert(arboard::Clipboard::new()?),
    };
    clipboard.set().file_list(paths)
}

/// Opens an item's link in the system browser; only `http` and `https` links.
#[tauri::command]
pub fn open_link(url: String) -> Result<(), SourceError> {
    if !is_web_link(&url) {
        return Err(SourceError::Invalid(
            "Only http and https links open in the browser.".into(),
        ));
    }
    tauri_plugin_opener::open_url(url.trim(), None::<&str>).map_err(opener_error)
}

/// Starts an OS drag of the originals, so they can be dropped into other apps.
#[tauri::command]
pub async fn start_drag(
    app: AppHandle,
    window: WebviewWindow,
    ids: Vec<ItemId>,
) -> Result<(), SourceError> {
    let (paths, image) = blocking(app.clone(), move |session| {
        let paths = session.original_paths(&ids)?;
        let image = ids.first().and_then(|id| session.drag_image(id));
        Ok((paths, image))
    })
    .await?;
    if paths.is_empty() {
        return Ok(());
    }
    app.run_on_main_thread(move || {
        if let Err(error) = crate::drag_out::start(&window, paths, image) {
            tracing::warn!("cannot drag files out of the window: {error}");
        }
    })
    .map_err(|error| SourceError::Storage(format!("start the drag: {error}")))
}

/// Library → Show in Explorer.
#[tauri::command]
pub async fn reveal_library(app: AppHandle) -> Result<(), SourceError> {
    let folder = blocking(app, Session::library_folder).await?;
    tauri_plugin_opener::reveal_item_in_dir(&folder).map_err(opener_error)
}

#[tauri::command]
pub async fn backups(app: AppHandle) -> Result<Vec<Backup>, SourceError> {
    blocking(app, Session::backups).await
}

#[tauri::command]
pub async fn back_up_now(app: AppHandle) -> Result<Backup, SourceError> {
    blocking(app, Session::back_up_now).await
}

/// Restores a backup (by file name) into the open library; the state before is backed up.
#[tauri::command]
pub async fn restore_backup(app: AppHandle, name: String) -> Result<LibraryInfo, SourceError> {
    blocking(app, move |session| session.restore_backup(&name)).await
}

/// What an export of `selection` would write: item count, bytes, and what stays hidden.
#[tauri::command]
pub async fn export_preview(
    app: AppHandle,
    selection: ExportSelection,
    include_subfolders: bool,
) -> Result<ExportPreview, SourceError> {
    blocking(app, move |session| {
        session.export_preview(selection, include_subfolders)
    })
    .await
}

/// Starts an export in the background; its progress arrives as `export-progress` events.
#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    request: ExportRequest,
) -> Result<ExportProgress, SourceError> {
    blocking(app, move |session| session.start_export(request)).await
}

/// Asks a running export to stop; its partial output is removed.
#[tauri::command]
pub async fn cancel_export(app: AppHandle, id: u64) -> Result<(), SourceError> {
    blocking(app, move |session| session.cancel_export(id)).await
}

/// Running exports, for a dialog that opens after its export started.
#[tauri::command]
pub async fn exports(app: AppHandle) -> Result<Vec<ExportProgress>, SourceError> {
    blocking(app, move |session| Ok::<_, SourceError>(session.exports())).await
}

/// Shows an exported file or folder in Explorer.
#[tauri::command]
pub async fn reveal_path(path: String) -> Result<(), SourceError> {
    tauri_plugin_opener::reveal_items_in_dir([PathBuf::from(path)]).map_err(opener_error)
}

#[tauri::command]
pub async fn rename_library(app: AppHandle, name: String) -> Result<LibraryInfo, SourceError> {
    blocking(app, move |session| session.rename_library(&name)).await
}

#[tauri::command]
pub async fn check_library(app: AppHandle) -> Result<LibraryCheck, SourceError> {
    blocking(app, Session::check_library).await
}

#[tauri::command]
pub async fn repair_library(
    app: AppHandle,
    options: RepairOptions,
) -> Result<LibraryCheck, SourceError> {
    blocking(app, move |session| session.repair_library(options)).await
}

#[tauri::command]
pub async fn library_stats(app: AppHandle) -> Result<LibraryStats, SourceError> {
    blocking(app, Session::library_stats).await
}

/// Makes every thumbnail again in the background; returns how many items wait.
#[tauri::command]
pub async fn rebuild_thumbnails(app: AppHandle) -> Result<u32, SourceError> {
    blocking(app, Session::rebuild_thumbnails).await
}

fn opener_error(error: tauri_plugin_opener::Error) -> SourceError {
    SourceError::Storage(format!("open with the system: {error}"))
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
/// their bytes never cross IPC. `exclude` and `include` are what a reviewed import confirmed:
/// relative paths from each source root, `/` separated; with `reviewed`, a file that is not an
/// image is skipped unless it is in `include`.
#[tauri::command]
pub async fn add_files(
    app: AppHandle,
    paths: Vec<String>,
    folder: Option<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    reviewed: bool,
) -> Result<ImportTicket, SourceError> {
    let filter = ImportFilter {
        exclude: exclude.into_iter().collect(),
        include: include.into_iter().collect(),
        reviewed,
    };
    blocking(app, move |session| {
        session.add_files_filtered(paths, folder, filter)
    })
    .await
}

/// Walks `path` for a reviewed import and answers what is there; nothing is copied.
#[tauri::command]
pub async fn preview_import(app: AppHandle, path: String) -> Result<ImportPreview, SourceError> {
    blocking(app, move |session| session.preview_import(&path)).await
}

/// Validates a picked Eagle `.library` folder for the import wizard: the library name, the
/// item count and the bytes under `images/` (docs/spec.md §8.2 step 1).
#[tauri::command]
pub async fn eagle_library_info(
    app: AppHandle,
    path: String,
) -> Result<EagleLibrarySummary, SourceError> {
    blocking(app, move |session| session.eagle_library_info(&path)).await
}

/// Starts importing an Eagle library folder in the background (docs/spec.md §8.2): into a
/// new native library, or into the open one. Its progress arrives as `import-progress`
/// events; the final one carries the report.
#[tauri::command]
pub async fn import_eagle_library(
    app: AppHandle,
    request: EagleImportRequest,
) -> Result<ImportTicket, SourceError> {
    blocking(app, move |session| session.import_eagle_library(request)).await
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

/// Quits after the user confirmed in the UI (the window's close was held back; D-072).
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
        let defaults = ConnectionSettings::default();
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
