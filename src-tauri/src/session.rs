//! The desktop session: the native library through [`LibraryService`] (D-091), a running Eagle
//! as the other source, and the app's own files (recent libraries, preferences, login), without
//! Tauri: what the commands call, and what the tests drive (docs/architecture.md §7.4, §9, D-022,
//! D-024).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use app_service::{LibraryService, no_library};
use app_store::{
    AppStore, ConnectionSettings, KeyringStore, LibrarySession, LoginProof, LoginStatus,
    Preferences, RecentLibrary, SecretStore, SyncEnvironment, check_location,
    default_library_parent, display_path, eagle_token_name,
};
use eagle_api::EagleSource;
use library_core::{
    AddTarget, Backup, DecryptedArchive, ExportPreview, ExportProgress, ExportRequest,
    ExportSelection, ExtensionCount, Folder, FolderChange, ImportFailure, ImportKind,
    ImportPreview, ImportProgress, ImportState, ImportTicket, Item, ItemCell, ItemEdit, ItemId,
    LibraryCheck, LibraryCounts, LibraryInfo, LibraryQuery, LibrarySource, LibraryStats,
    LocationProblem, NewFile, PivPolicy, PivPrompt, PivWrap, Progress, ReadOnlyReason, RecoveryKey,
    RepairOptions, SelectionTags, SmartFolder, SmartFolderChange, SourceError, Tag, TagGroup,
    TagGroupChange, Thumbnail, TrashInfo, UndoStatus,
};
use library_native::{
    DuplicateChoice, EagleImportOptions, ImportFilter, check_enrollment, import_eagle,
};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use app_service::{SmartDraft, lock, now_ms};

use crate::views::{
    Blocking, Emit, ImageRequest, ItemsTarget, ViewOpened, count, invalidated, ready,
};
use crate::webauthn::SystemWebAuthn;
use crate::yubikey;

/// The app's icon, for the library folder in the file manager (D-117).
const FOLDER_ICON: library_native::FolderIcon<'static> = library_native::FolderIcon {
    ico: include_bytes!("../icons/icon.ico"),
    png: include_bytes!("../icons/128x128.png"),
};

/// What the app does on start: the reopened library, or why it shows Welcome instead.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StartupResult {
    pub library: Option<LibraryInfo>,
    /// Shown on Welcome when the last library was not reopened for a reason worth saying.
    pub notice: Option<String>,
}

/// What the import wizard shows for a picked Eagle library (docs/spec.md §8.2 step 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EagleLibrarySummary {
    pub path: String,
    pub name: String,
    /// Items including the trash (`mtime.json` `all`).
    pub item_count: u32,
    /// Bytes of everything under `images/`.
    pub bytes: u64,
}

/// Where the import of an Eagle library goes (docs/spec.md §8.2 step 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EagleImportRequest {
    pub eagle_path: String,
    /// A new native library to create and import into; `None` imports into the open library
    /// (its Eagle folders go under a root folder named after the Eagle library).
    pub create: Option<EagleLibraryTarget>,
    /// Import Eagle's trash items into the trash; off by default.
    pub import_trash: bool,
}

/// A new native library made for the import: where, and under which name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EagleLibraryTarget {
    pub parent: String,
    pub name: String,
}

/// A live connection to a running Eagle (docs/spec.md §8.3 "Connect screen").
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EagleSessionInfo {
    pub library: LibraryInfo,
    pub eagle_version: String,
    /// Eagle runs on this PC: files are added by path and thumbnails are read from disk.
    pub same_pc: bool,
    pub host: String,
    pub port: u16,
}

/// What is on the clipboard: copied files, or an image (RGBA pixels).
pub trait Clipboard {
    fn files(&self) -> Option<Vec<PathBuf>>;
    fn image(&self) -> Option<(u32, u32, Vec<u8>)>;
}

pub struct Session {
    store: AppStore,
    home: PathBuf,
    sync: SyncEnvironment,
    /// The native library, its views, undo and tasks. A running Eagle shares the views and
    /// the import tasks, so view and task ids stay unique across both sources.
    service: LibraryService,
    /// A live Eagle connection; exclusive with the native library (docs/spec.md §8.3).
    eagle: Mutex<Option<Arc<EagleSource>>>,
    secrets: Arc<dyn SecretStore>,
    /// "Importing…" cells of files on their way to a running Eagle (docs/spec.md §8.3);
    /// appended to every open view until the items exist. `Arc`: the add thread holds it.
    pending: Arc<Mutex<Vec<ItemCell>>>,
    /// The cancel flag of the Decrypt Archive that runs, if one does (D-110).
    decrypting: Mutex<Option<Arc<AtomicBool>>>,
}

impl Session {
    pub fn new(
        store: AppStore,
        home: PathBuf,
        app_version: impl Into<String>,
        sync: SyncEnvironment,
        emit: Emit,
    ) -> Self {
        Self::with_secrets(store, home, app_version, sync, emit, Arc::new(KeyringStore))
    }

    pub fn with_secrets(
        store: AppStore,
        home: PathBuf,
        app_version: impl Into<String>,
        sync: SyncEnvironment,
        emit: Emit,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            store,
            home,
            sync,
            service: LibraryService::new(app_version, emit),
            eagle: Mutex::new(None),
            secrets,
            pending: Arc::new(Mutex::new(Vec::new())),
            decrypting: Mutex::new(None),
        }
    }

    pub fn store(&self) -> &AppStore {
        &self.store
    }

    /// The native library service, for the tests.
    #[cfg(test)]
    pub fn service(&self) -> &LibraryService {
        &self.service
    }

    /// The preference, or `%USERPROFILE%\egl-library`.
    pub fn default_library_location(&self) -> String {
        self.store
            .preferences()
            .default_library_parent
            .unwrap_or_else(|| display_path(&default_library_parent(&self.home)))
    }

    pub fn create_library(&self, parent: &str, name: &str) -> Result<LibraryInfo, SourceError> {
        let parent = PathBuf::from(parent);
        check_location(&parent, &self.sync).map_err(SourceError::UnsupportedLocation)?;
        if !parent.exists() && display_path(&parent) == self.default_library_location() {
            std::fs::create_dir_all(&parent)
                .map_err(|error| SourceError::Storage(format!("create the folder: {error}")))?;
        }
        let root = library_native::create_library(&parent, name, self.service.app_version())?;
        self.open_library(&root.to_string_lossy())
    }

    /// Opens a library folder. The library that is open stays open until this one has opened;
    /// then its imports stop and it closes (D-024). Opening the read-only library again
    /// retries its lock. A sync folder opens read-only; a network path is refused (D-021).
    pub fn open_library(&self, path: &str) -> Result<LibraryInfo, SourceError> {
        let path = PathBuf::from(path);
        let sync_folder = match check_location(&path, &self.sync) {
            Ok(()) => false,
            Err(LocationProblem::SyncFolder) => true,
            Err(problem) => return Err(SourceError::UnsupportedLocation(problem)),
        };
        let key = display_path(&path);
        let info = self.service.open_library(Path::new(&key), sync_folder)?;
        // The app's icon on the folder in Explorer and Dolphin (D-117), on a library this
        // instance writes; a library made before gets it on its next open.
        if info.read_only_reason.is_none() {
            library_native::decorate_library_folder(Path::new(&key), &FOLDER_ICON);
        }
        // One source at a time (docs/spec.md §8.3): a live Eagle connection ends here.
        lock(&self.eagle).take();
        lock(&self.pending).clear();
        self.remember(&info);
        Ok(info)
    }

    fn remember(&self, info: &LibraryInfo) {
        if let Some(path) = &info.path
            && let Err(error) =
                self.store
                    .remember_opened(path, &info.name, info.id.as_deref(), now_ms())
        {
            tracing::warn!("cannot update recent libraries: {error}");
        }
    }

    /// Close Library: imports stop, back to Welcome, and no reopen on the next start. Also
    /// drops a live Eagle connection: for the UI both mean "back to Welcome".
    pub fn close_library(&self) -> Result<(), SourceError> {
        lock(&self.eagle).take();
        lock(&self.pending).clear();
        self.service.close_library()?;
        self.store.set_closed_explicitly(true)
    }

    pub fn current(&self) -> Option<LibraryInfo> {
        if let Some(source) = self.eagle() {
            return Some(source.info());
        }
        self.service.info()
    }

    pub fn counts(&self) -> Result<LibraryCounts, SourceError> {
        if let Some(source) = self.eagle() {
            return Ok(source.counts()?);
        }
        self.service.counts()
    }

    /// The values the Connect screen starts with: the last successful connection, or Eagle
    /// on this PC.
    pub fn connection(&self) -> ConnectionSettings {
        self.store.connection()
    }

    /// Connects to a running Eagle and opens the library it has open. The library that is
    /// open stays open until the connection has succeeded; then it closes (D-024). The token:
    /// the given one, saved to the secret store on success; otherwise the stored one; possibly
    /// empty, which only the same PC accepts (docs/spec.md §8.3 "Auth").
    pub fn connect_eagle(
        &self,
        settings: ConnectionSettings,
        token: Option<String>,
    ) -> Result<EagleSessionInfo, SourceError> {
        settings.validate()?;
        let settings = ConnectionSettings {
            host: settings.host.trim().to_owned(),
            ..settings
        };
        let name = eagle_token_name(&settings.host, settings.port);
        // Move the plaintext into SecretString at once so a later panic cannot leave it in a
        // plain String; the Connect form also clears its field after send (D-080).
        let given = token.is_some();
        let token = match token {
            Some(token) => SecretString::from(token),
            None => SecretString::from(self.secrets.get(&name)?.unwrap_or_default()),
        };
        let source = Arc::new(EagleSource::connect(
            &settings.host,
            settings.port,
            token.clone(),
            settings.is_tailscale(),
        )?);
        // Only a token that connected is saved: a typo never replaces the working one.
        if given {
            self.secrets.set(&name, token.expose_secret())?;
        }
        let info = EagleSessionInfo {
            library: source.info(),
            eagle_version: source.eagle_version().to_owned(),
            same_pc: settings.is_same_pc(),
            host: settings.host.clone(),
            port: settings.port,
        };
        // The previous source closes here, after the new one connected. The session switches
        // either way: a library that fails to close cleanly is already out of the session.
        if let Err(error) = self.service.close_library() {
            tracing::warn!("cannot close the previous library cleanly: {error}");
        }
        lock(&self.pending).clear();
        lock(&self.eagle).replace(source);
        self.store.save_connection(&settings)?;
        Ok(info)
    }

    /// The live connection, so the UI can show the library again without reconnecting.
    pub fn eagle_session(&self) -> Option<EagleSessionInfo> {
        let source = self.eagle()?;
        let settings = self.store.connection();
        Some(EagleSessionInfo {
            library: source.info(),
            eagle_version: source.eagle_version().to_owned(),
            same_pc: settings.is_same_pc(),
            host: settings.host,
            port: settings.port,
        })
    }

    /// Disconnect: back to Welcome.
    pub fn disconnect_eagle(&self) -> Result<(), SourceError> {
        // A running add to Eagle stops between files; its thread is joined before the
        // connection goes, so nothing writes to Eagle after Disconnect.
        self.service.imports().stop();
        lock(&self.eagle).take();
        lock(&self.pending).clear();
        lock(self.service.views()).clear();
        Ok(())
    }

    /// Opens a view of the open library for `query` (docs/architecture.md §6.5). The total
    /// includes the "Importing…" cells of a running-Eagle add.
    pub fn open_view(
        &self,
        query: LibraryQuery,
        progress: Progress<'_>,
    ) -> Result<ViewOpened, SourceError> {
        let opened = match self.eagle() {
            Some(source) => self
                .service
                .open_view_of(source.as_ref(), query, progress)?,
            None => self.service.open_view(query, progress)?,
        };
        Ok(ViewOpened {
            total: opened.total + count(lock(&self.pending).len()),
            ..opened
        })
    }

    /// At most `MAX_WINDOW` cells of view `view_id`, from `start`, then the pending cells.
    pub fn view_range(
        &self,
        view_id: u64,
        start: u32,
        len: u32,
    ) -> Result<Vec<ItemCell>, SourceError> {
        let pending = lock(&self.pending).clone();
        // The view part returns at most `MAX_WINDOW` cells; capping the window first keeps the
        // pending cells at their own positions after it.
        self.service.with_view(view_id, |view| {
            combined(
                start as usize,
                (len as usize).min(library_core::MAX_WINDOW),
                view.total(),
                &pending,
                |at, take| view.range(at, take),
            )
        })
    }

    /// The ids of positions `start..start + len` of a view, for selections, then the
    /// pending cells' ids.
    pub fn view_ids(&self, view_id: u64, start: u32, len: u32) -> Result<Vec<ItemId>, SourceError> {
        let pending: Vec<ItemId> = lock(&self.pending)
            .iter()
            .map(|cell| cell.id.clone())
            .collect();
        self.service.with_view(view_id, |view| {
            combined(
                start as usize,
                len as usize,
                view.total(),
                &pending,
                |at, take| view.ids(at, take),
            )
        })
    }

    /// Width and height of every position in the view, for justified packing (D-067);
    /// pending cells are squares (zeros).
    pub fn view_dimensions(&self, view_id: u64) -> Result<Vec<[u32; 2]>, SourceError> {
        let pending = lock(&self.pending).len();
        let mut dimensions = self.service.view_dimensions(view_id)?;
        dimensions.extend(std::iter::repeat_n([0, 0], pending));
        Ok(dimensions)
    }

    pub fn close_view(&self, view_id: u64) {
        self.service.close_view(view_id);
    }

    /// The full item for the details panel.
    pub fn item_detail(&self, id: &str) -> Result<Option<Item>, SourceError> {
        match self.eagle() {
            Some(source) => ready(source.item(&ItemId(id.to_owned()))),
            None => self.service.item(id),
        }
    }

    /// What the `thumb` scheme serves for a URL path: a thumbnail or an original of the open
    /// library, or nothing (404). `version` is the cell's `thumbVersion` from the URL's
    /// `?v=`; a running Eagle keys its thumbnail cache by it.
    pub fn image(&self, path: &str, version: Option<i64>) -> Option<Thumbnail> {
        let Some(source) = self.eagle() else {
            return self.service.image(path);
        };
        match ImageRequest::parse(path)? {
            ImageRequest::Thumbnail(id) => source
                .thumbnail_versioned(&id, version.unwrap_or(i64::MIN))
                .ok()
                .flatten(),
            ImageRequest::Original(id) => source.original_file(&id).ok().flatten(),
        }
    }

    pub fn recent_libraries(&self) -> Vec<RecentLibrary> {
        self.store.recent_libraries()
    }

    pub fn forget_recent_library(&self, path: &str) -> Result<(), SourceError> {
        self.store.forget_recent(path)
    }

    /// The last place, filters and selection for a library path (phase 12 / D-078).
    pub fn library_session(&self, path: &str) -> Option<LibrarySession> {
        self.store.library_session(path)
    }

    pub fn save_library_session(
        &self,
        path: &str,
        session: LibrarySession,
    ) -> Result<(), SourceError> {
        self.store.save_library_session(path, session)
    }

    pub fn preferences(&self) -> Preferences {
        self.store.preferences()
    }

    /// Stores preferences after checking the default folder like a library location.
    pub fn save_preferences(&self, preferences: Preferences) -> Result<Preferences, SourceError> {
        let mut preferences = preferences;
        if let Some(folder) = preferences.default_library_parent.take() {
            let folder = folder.trim();
            if !folder.is_empty() {
                check_location(Path::new(folder), &self.sync)
                    .map_err(SourceError::UnsupportedLocation)?;
                preferences.default_library_parent = Some(display_path(Path::new(folder)));
            }
        }
        self.store.save_preferences(&preferences)?;
        Ok(self.store.preferences())
    }

    /// Reopens the last library unless it was closed explicitly, the preference is off,
    /// `no_reopen` is set, or another instance has it open.
    pub fn startup(&self, no_reopen: bool) -> StartupResult {
        let nothing = StartupResult {
            library: None,
            notice: None,
        };
        if no_reopen || !self.store.preferences().reopen_last_library {
            return nothing;
        }
        let Some(path) = self.store.library_to_reopen() else {
            return nothing;
        };
        match self.open_library(&path) {
            Ok(info) if info.read_only_reason == Some(ReadOnlyReason::OtherInstance) => {
                drop(self.service.take_library());
                StartupResult {
                    library: None,
                    notice: Some(format!(
                        "\"{}\" is open in another egl-viewer window.",
                        info.name
                    )),
                }
            }
            Ok(info) => StartupResult {
                library: Some(info),
                notice: None,
            },
            Err(error) => StartupResult {
                library: None,
                notice: Some(format!("The last library could not be opened: {error}.")),
            },
        }
    }

    /// Adds files and folders to the open library in a background task, into `folder` when
    /// one is open (docs/spec.md §4).
    #[cfg(test)]
    pub fn add_files(
        &self,
        paths: Vec<String>,
        folder: Option<String>,
    ) -> Result<ImportTicket, SourceError> {
        self.add_files_filtered(paths, folder, ImportFilter::default())
    }

    /// `add_files` for a reviewed import: `filter` is what its preview dialog confirmed.
    pub fn add_files_filtered(
        &self,
        paths: Vec<String>,
        folder: Option<String>,
        filter: ImportFilter,
    ) -> Result<ImportTicket, SourceError> {
        if let Some(source) = self.eagle() {
            return self.start_eagle_add(source, paths, folder, filter);
        }
        let paths = paths.into_iter().map(PathBuf::from).collect();
        self.service.add_files(paths, folder, filter)
    }

    /// Adding files to a running Eagle: one `item/add` per accepted file, an "Importing…"
    /// cell until the item exists, then the snapshots drop and the views reload
    /// (docs/spec.md §8.3 "Adding files to a running Eagle").
    fn start_eagle_add(
        &self,
        source: Arc<EagleSource>,
        paths: Vec<String>,
        folder: Option<String>,
        filter: ImportFilter,
    ) -> Result<ImportTicket, SourceError> {
        let sources: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let files: Vec<NewFile> = library_native::files_to_add(&sources, &filter)
            .into_iter()
            .map(|path| NewFile {
                path: display_path(&path),
                name: None,
            })
            .collect();
        let target = AddTarget {
            folders: folder.into_iter().collect(),
            ..AddTarget::default()
        };
        let emit = Arc::clone(self.service.emit());
        let views = Arc::clone(self.service.views());
        let pending = Arc::clone(&self.pending);
        let accepted = Arc::new(Mutex::new(Vec::new()));
        let id =
            self.service
                .imports()
                .spawn(ImportKind::Files, Arc::clone(&emit), move |context| {
                    let id = context.id;
                    let task_cancel = context.cancel_flag();
                    let mut progress = ImportProgress {
                        id,
                        kind: ImportKind::Files,
                        state: ImportState::Adding,
                        total: count(files.len()),
                        processed: 0,
                        added: 0,
                        duplicates: 0,
                        failed: 0,
                        previews_total: 0,
                        previews_done: 0,
                        current: None,
                        duplicate: None,
                        failures: Vec::new(),
                        skipped: Vec::new(),
                        report: None,
                        started_at: now_ms(),
                        finished_at: None,
                    };
                    context.report(&progress);
                    let result = {
                        let pending = Arc::clone(&pending);
                        let emit = Arc::clone(&emit);
                        let views = Arc::clone(&views);
                        let accepted = Arc::clone(&accepted);
                        source.start_add(files, &target, move |added| {
                            if let Ok(mut ids) = accepted.lock() {
                                ids.push(added.id.clone());
                            }
                            lock(&pending).push(EagleSource::pending_cell(&added));
                            // The grid picks the new cell up: same views, bigger totals.
                            let extra = lock(&pending).len();
                            let totals = lock(&views).totals();
                            let totals = totals
                                .into_iter()
                                .map(|(view_id, total)| (view_id, total + extra))
                                .collect();
                            if let Some(event) = invalidated(totals) {
                                emit(event);
                            }
                        })
                    };
                    match result {
                        Ok(pendings) => {
                            for added in pendings {
                                progress.processed += 1;
                                progress.added += 1;
                                progress.current = Some(added.name.clone());
                                context.report(&progress);
                                let appeared =
                                    source.wait_for_item(&added.id, task_cancel).unwrap_or(None);
                                // The single data-URL form ignores `star` (correction 11).
                                if appeared.is_some()
                                    && !source.same_pc()
                                    && let Some(star) = target.star
                                {
                                    let _ = source.finish_add_star(&added.id, star);
                                }
                                if task_cancel.load(Ordering::Relaxed) {
                                    progress.state = ImportState::Cancelled;
                                    break;
                                }
                            }
                            if !matches!(progress.state, ImportState::Cancelled) {
                                progress.state = ImportState::Done;
                            }
                        }
                        Err(error) => {
                            progress.failed = progress.total - progress.processed;
                            progress.failures.push(ImportFailure {
                                path: progress.current.clone().unwrap_or_default(),
                                reason: error.to_string(),
                            });
                            // Finished with failures, not cancelled: nobody cancelled it.
                            progress.state = ImportState::Done;
                        }
                    }
                    progress.current = None;
                    // The placeholders leave the grid; the views reload with the real items.
                    let accepted = lock(&accepted);
                    lock(&pending).retain(|cell| !accepted.contains(&cell.id.0));
                    drop(accepted);
                    if let Err(error) = source.invalidate_all() {
                        tracing::warn!("cannot drop the Eagle cache after an add: {error}");
                    }
                    let source = Blocking(source.as_ref());
                    match lock(&views).refresh(&source).map(invalidated) {
                        Ok(Some(event)) => emit(event),
                        Ok(None) => {}
                        Err(error) => tracing::warn!("cannot refresh the views: {error}"),
                    }
                    progress.finished_at = Some(now_ms());
                    context.report(&progress);
                    progress
                })?;
        Ok(ImportTicket { id })
    }

    /// Walks `path` for a reviewed import and answers what is there; refuses like the import
    /// (a read-only library, or a folder already inside this one). Native libraries only.
    pub fn preview_import(&self, path: &str) -> Result<ImportPreview, SourceError> {
        self.require_native("preview a folder import")?;
        self.service.preview_import(Path::new(path))
    }

    /// Validates a picked Eagle `.library` folder and answers what an import would take
    /// (docs/spec.md §8.2 step 1). Reading only.
    pub fn eagle_library_info(&self, path: &str) -> Result<EagleLibrarySummary, SourceError> {
        let eagle = eagle_folder::EagleLibrary::open(Path::new(path)).map_err(eagle_error)?;
        let summary = eagle.summary().map_err(eagle_error)?;
        Ok(EagleLibrarySummary {
            path: display_path(eagle.root()),
            name: summary.name,
            item_count: summary.item_count,
            bytes: summary.bytes,
        })
    }

    /// Imports an Eagle library folder in a background task (docs/spec.md §8.2). Into a new
    /// native library, created and opened here, or into the open library under a root folder
    /// named after the Eagle library. The task can be cancelled and resumed later.
    pub fn import_eagle_library(
        &self,
        request: EagleImportRequest,
    ) -> Result<ImportTicket, SourceError> {
        let eagle = eagle_folder::EagleLibrary::open(Path::new(&request.eagle_path))
            .map_err(eagle_error)?;
        let root_folder = match &request.create {
            Some(target) => {
                self.create_library(&target.parent, &target.name)?;
                None
            }
            None => Some(eagle.name().to_owned()),
        };
        let library = self.service.writable_library()?;
        let options = EagleImportOptions {
            import_trash: request.import_trash,
            root_folder,
        };
        let id = self.service.start(
            library,
            ImportKind::Eagle,
            move |library, id, observer, cancel| {
                import_eagle(library, &eagle, &options, id, observer, cancel)
            },
        )?;
        Ok(ImportTicket { id })
    }

    /// The folder tree with counts.
    pub fn folder_tree(&self) -> Result<Vec<Folder>, SourceError> {
        match self.eagle() {
            Some(source) => ready(source.folder_tree()),
            None => self.service.folder_tree(),
        }
    }

    /// Creates, edits or moves a folder; returns the new tree.
    pub fn save_folder(&self, change: FolderChange) -> Result<Vec<Folder>, SourceError> {
        let Some(source) = self.eagle() else {
            return self.service.save_folder(change);
        };
        let tree = ready(source.save_folder(change))?;
        self.refresh_views();
        Ok(tree)
    }

    /// Deletes a folder and its subfolders; returns the new tree. Native libraries only:
    /// Eagle's API cannot delete folders (docs/spec.md §8.3 "Writes").
    pub fn delete_folder(&self, id: &str, trash_items: bool) -> Result<Vec<Folder>, SourceError> {
        self.require_native("delete a folder")?;
        self.service.delete_folder(id, trash_items)
    }

    /// Edits one item; `Ctrl+Z` can take the edit back (native library; a running Eagle
    /// cannot give the previous states back through its API).
    pub fn apply_item_edit(&self, id: &str, edit: ItemEdit) -> Result<Item, SourceError> {
        let Some(source) = self.eagle() else {
            return self.service.apply_item_edit(id, edit);
        };
        let item = ready(source.apply_item_edit(&ItemId(id.to_owned()), edit))?;
        self.refresh_views();
        Ok(item)
    }

    /// One edit for a selection; returns how many items changed. `Ctrl+Z` can take it back
    /// (native library only, like [`Session::apply_item_edit`]).
    pub fn apply_batch_edit(
        &self,
        target: ItemsTarget,
        edit: ItemEdit,
    ) -> Result<u32, SourceError> {
        let Some(source) = self.eagle() else {
            return self.service.apply_batch_edit(target, edit);
        };
        let ids = self.service.target_ids(target)?;
        let edited = ready(source.apply_batch_edit(&ids, edit))?;
        self.refresh_views();
        Ok(edited)
    }

    /// `Ctrl+Z`: applies the inverse of the last item edit as new edits. Native libraries
    /// only: a running Eagle never pushes onto the undo stack (docs/spec.md §8.3 "Writes").
    pub fn undo(&self) -> Result<Option<UndoStatus>, SourceError> {
        self.require_native("undo item edits")?;
        self.service.undo()
    }

    /// What `Ctrl+Z` would take back, for the Edit menu.
    pub fn undo_status(&self) -> Option<UndoStatus> {
        self.service.undo_status()
    }

    /// Delete Permanently: the selected items of the trash, with their files. No undo.
    /// Native libraries only: Eagle's API cannot empty or purge the trash.
    pub fn delete_permanently(&self, target: ItemsTarget) -> Result<u32, SourceError> {
        self.require_native("delete permanently")?;
        self.service.delete_permanently(target)
    }

    /// Empty Trash, with the files (D-074). Native libraries only.
    pub fn empty_trash(&self, progress: &dyn Fn(u32, u32)) -> Result<u32, SourceError> {
        self.require_native("empty the trash")?;
        self.service.empty_trash(progress)
    }

    pub fn cancel_empty_trash(&self) {
        self.service.cancel_empty_trash();
    }

    /// Items and bytes of the trash, for the context menu of its row. Native libraries only:
    /// Eagle's API lists no trashed items.
    pub fn trash_info(&self) -> Result<TrashInfo, SourceError> {
        self.require_native("read trash info")?;
        self.service.trash_info()
    }

    /// Restore All of the trash context menu. `Ctrl+Z` can take it back. Native libraries only.
    pub fn restore_trash(&self) -> Result<u32, SourceError> {
        self.require_native("restore from the trash")?;
        self.service.restore_trash()
    }

    /// The originals of the selected items, for Open, Show in Explorer, Copy File and dragging
    /// out. Refused for an item of a locked folder: its file is encrypted.
    pub fn original_paths(&self, ids: &[ItemId]) -> Result<Vec<PathBuf>, SourceError> {
        self.service.original_paths(ids)
    }

    /// The ids of a selection, Select All resolved in Rust from its view: the UI never fetches
    /// every id of a view to send them back (docs/architecture.md §8).
    pub fn selected_ids(&self, target: ItemsTarget) -> Result<Vec<ItemId>, SourceError> {
        self.service.target_ids(target)
    }

    /// The image under the cursor while an item is dragged out: its thumbnail, small.
    pub fn drag_image(&self, id: &ItemId) -> Option<Vec<u8>> {
        self.service.drag_image(id)
    }

    /// The library folder, for Library → Show in Explorer.
    pub fn library_folder(&self) -> Result<PathBuf, SourceError> {
        Ok(self
            .service
            .current_library()
            .ok_or_else(no_library)?
            .root()
            .to_path_buf())
    }

    pub fn backups(&self) -> Result<Vec<Backup>, SourceError> {
        self.service.backups()
    }

    pub fn back_up_now(&self) -> Result<Backup, SourceError> {
        self.service.back_up_now()
    }

    /// Restores a backup into the open library. Imports stop first; views, undo and unlocked
    /// folders start over, as after opening the library.
    pub fn restore_backup(&self, name: &str) -> Result<LibraryInfo, SourceError> {
        self.service.restore_backup(name)
    }

    pub fn rename_library(&self, name: &str) -> Result<LibraryInfo, SourceError> {
        let info = self.service.rename_library(name)?;
        self.remember(&info);
        Ok(info)
    }

    pub fn check_library(&self) -> Result<LibraryCheck, SourceError> {
        self.service.check_library()
    }

    pub fn repair_library(&self, options: RepairOptions) -> Result<LibraryCheck, SourceError> {
        self.service.repair_library(options)
    }

    pub fn library_stats(&self) -> Result<LibraryStats, SourceError> {
        self.service.library_stats()
    }

    /// What an export of `selection` would write, from the database alone (docs/spec.md §7.4
    /// item 12). A read-only library exports fine.
    pub fn export_preview(
        &self,
        selection: ExportSelection,
        include_subfolders: bool,
    ) -> Result<ExportPreview, SourceError> {
        self.service.export_preview(selection, include_subfolders)
    }

    /// Starts an export in the background and returns its first progress: into a folder tree
    /// or an archive by the service, into a running Eagle here (docs/spec.md §8.4).
    pub fn start_export(&self, request: ExportRequest) -> Result<ExportProgress, SourceError> {
        let Some(target) = &request.eagle else {
            return self.service.start_export(request);
        };
        let library = self.service.current_library().ok_or_else(no_library)?;
        // The token typed in the dialog, else the one Connect saved for that host and port.
        // It stays a `SecretString` in Rust (D-080, D-089).
        let token = match target.token.as_deref().filter(|token| !token.is_empty()) {
            Some(token) => SecretString::from(token.to_owned()),
            None => SecretString::from(
                self.secrets
                    .get(&eagle_token_name(target.host.trim(), target.port))?
                    .unwrap_or_default(),
            ),
        };
        let mut request = request;
        if let Some(target) = &mut request.eagle {
            target.token = None;
        }
        self.service
            .exports()
            .spawn(Arc::clone(self.service.emit()), move |context| {
                // The engine reports failures in the progress.
                let mut on_progress = |progress: ExportProgress| context.report(progress);
                let result = crate::export_eagle::export_into_eagle(
                    &library,
                    context.id,
                    &request,
                    token,
                    context.cancel_flag(),
                    &mut on_progress,
                );
                context.report(result);
                // Nothing on this PC to show in Explorer.
                None
            })
    }

    /// Opens an age archive with its passphrase into `destination` (D-110); `None` when
    /// `cancel_decrypt` stopped it. One runs at a time.
    pub fn decrypt_archive(
        &self,
        source: &str,
        destination: &str,
        passphrase: &str,
        on_progress: &mut dyn FnMut(u64, u64),
    ) -> Result<Option<DecryptedArchive>, SourceError> {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut running = lock(&self.decrypting);
            if running.is_some() {
                return Err(SourceError::Invalid(
                    "An archive is being decrypted already.".into(),
                ));
            }
            *running = Some(Arc::clone(&cancel));
        }
        let result = self.service.decrypt_archive(
            Path::new(source),
            Path::new(destination),
            passphrase,
            &cancel,
            on_progress,
        );
        *lock(&self.decrypting) = None;
        result
    }

    /// Stops the running Decrypt Archive; it removes what it wrote.
    pub fn cancel_decrypt(&self) {
        if let Some(cancel) = lock(&self.decrypting).as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Show in Explorer after an export: only a path an export of this session wrote, so the
    /// WebView cannot reveal arbitrary paths (D-089).
    pub fn may_reveal(&self, path: &str) -> Result<PathBuf, SourceError> {
        self.service.exports().may_reveal(path)
    }

    /// Export tasks for the dialog to catch up with after it opens.
    pub fn exports(&self) -> Vec<ExportProgress> {
        self.service.exports().list()
    }

    /// An export is still running (its final event not sent yet).
    pub fn has_running_exports(&self) -> bool {
        self.service.exports().has_running()
    }

    /// Asks a running export to stop; its partial output is removed by the engine.
    pub fn cancel_export(&self, id: u64) -> Result<(), SourceError> {
        self.service.exports().cancel(id)
    }

    /// Rebuild Thumbnails: every preview is made again by the preview task. Returns how many
    /// items wait.
    pub fn rebuild_thumbnails(&self) -> Result<u32, SourceError> {
        self.service.rebuild_thumbnails()
    }

    /// The tag vocabulary with item counts (trash excluded).
    pub fn tag_vocabulary(&self) -> Result<Vec<Tag>, SourceError> {
        match self.eagle() {
            Some(source) => ready(source.tags()),
            None => self.service.tags(),
        }
    }

    pub fn tag_groups(&self) -> Result<Vec<TagGroup>, SourceError> {
        match self.eagle() {
            Some(source) => ready(source.tag_groups()),
            None => self.service.tag_groups(),
        }
    }

    /// Creates or changes a tag group; returns every group. Item tags do not change.
    /// Native libraries only: Eagle's API has no tag-group writes.
    pub fn save_tag_group(&self, change: TagGroupChange) -> Result<Vec<TagGroup>, SourceError> {
        self.require_native("edit tag groups")?;
        self.service.save_tag_group(change)
    }

    pub fn delete_tag_group(&self, id: &str) -> Result<Vec<TagGroup>, SourceError> {
        self.require_native("delete a tag group")?;
        self.service.delete_tag_group(id)
    }

    /// Renames a tag everywhere; onto an existing name, the two merge.
    pub fn rename_tag(&self, from: &str, to: &str) -> Result<(), SourceError> {
        let Some(source) = self.eagle() else {
            return self.service.rename_tag(from, to);
        };
        ready(source.rename_tag(from, to))?;
        self.refresh_views();
        Ok(())
    }

    pub fn merge_tags(&self, from: &str, into: &str) -> Result<(), SourceError> {
        let Some(source) = self.eagle() else {
            return self.service.merge_tags(from, into);
        };
        ready(source.merge_tags(from, into))?;
        self.refresh_views();
        Ok(())
    }

    /// Removes a tag from every item.
    pub fn delete_tag(&self, name: &str) -> Result<(), SourceError> {
        let Some(source) = self.eagle() else {
            return self.service.delete_tag(name);
        };
        ready(source.delete_tag(name))?;
        self.refresh_views();
        Ok(())
    }

    /// The tags of a selection, each with the number of selected items that carry it, read
    /// from the view so that Select All needs no id list.
    pub fn selection_tags(&self, target: ItemsTarget) -> Result<SelectionTags, SourceError> {
        self.service.selection_tags(target)
    }

    /// The smart folder tree, each with the rules this build cannot evaluate.
    pub fn smart_folders(&self) -> Result<Vec<SmartFolder>, SourceError> {
        match self.eagle() {
            Some(source) => ready(source.smart_folders()),
            None => self.service.smart_folders(),
        }
    }

    /// Creates or edits a smart folder; returns the tree. Open views of it follow.
    /// Native libraries only: Eagle's API has no smart-folder writes.
    pub fn save_smart_folder(
        &self,
        change: SmartFolderChange,
    ) -> Result<Vec<SmartFolder>, SourceError> {
        self.require_native("edit smart folders")?;
        self.service.save_smart_folder(change)
    }

    pub fn delete_smart_folder(&self, id: &str) -> Result<Vec<SmartFolder>, SourceError> {
        self.require_native("delete a smart folder")?;
        self.service.delete_smart_folder(id)
    }

    /// The rules for "Save as smart folder"; refused with the reason for what rules cannot hold
    /// (search text, the trash, nothing to keep).
    pub fn smart_rules(&self, query: &LibraryQuery) -> Result<SmartDraft, SourceError> {
        match self.eagle() {
            Some(source) => LibraryService::smart_rules_of(source.as_ref(), query),
            None => self.service.smart_rules(query),
        }
    }

    /// File types with their item counts, for the Type filter.
    pub fn extensions(&self) -> Result<Vec<ExtensionCount>, SourceError> {
        match self.eagle() {
            Some(source) => ready(source.extensions()),
            None => self.service.extensions(),
        }
    }

    /// Manual order inside `folder`.
    pub fn reorder_items(
        &self,
        folder: &str,
        ids: Vec<ItemId>,
        before: Option<ItemId>,
    ) -> Result<(), SourceError> {
        let Some(source) = self.eagle() else {
            return self.service.reorder_items(folder, ids, before);
        };
        ready(source.reorder_items(folder, &ids, before.as_ref()))?;
        self.refresh_views();
        Ok(())
    }

    /// Locks a folder: its items are encrypted (`progress(done, total)`), then it is hidden.
    /// The recovery key comes back once, to be shown (docs/age-locked-folders.md §7).
    pub fn lock_folder(
        &self,
        id: &str,
        password: String,
        label: Option<String>,
        label_color: Option<String>,
        progress: &dyn Fn(u32, u32),
    ) -> Result<RecoveryKey, SourceError> {
        if self.eagle().is_some() {
            return Err(SourceError::NotSupported(
                "Eagle's API cannot set a password on a folder".into(),
            ));
        }
        self.service
            .lock_folder(id, password, label, label_color, progress)
    }

    /// Opens a locked folder for this session; read-only instances may too. For a running
    /// Eagle this goes through the v1 API and also unlocks the folder in the Eagle desktop
    /// until Eagle restarts (docs/spec.md §8.3 "Writes").
    pub fn unlock_folder(&self, id: &str, password: String) -> Result<(), SourceError> {
        let Some(source) = self.eagle() else {
            return self.service.unlock_folder(id, password);
        };
        source.unlock_folder(id, &password)?;
        self.refresh_views();
        Ok(())
    }

    /// YubiKeys bound to one locked folder. Nothing secret is in the list.
    pub fn piv_wraps(&self, id: &str) -> Result<Vec<PivWrap>, SourceError> {
        self.service.piv_wraps(id)
    }

    /// Adds a YubiKey as a third wrap. Touch and PIN are alternatives. With no card, the
    /// password and the recovery key are unchanged.
    pub fn bind_yubikey(
        &self,
        id: &str,
        password: String,
        policy: PivPolicy,
        pin: Option<String>,
    ) -> Result<(), SourceError> {
        check_enrollment(policy, pin.as_deref())?;
        let card = yubikey::enroll(policy, pin.as_deref())?;
        self.service.bind_piv(id, password, &card)
    }

    /// Drops one card's wrap. The password still opens the folder.
    pub fn unbind_yubikey(
        &self,
        id: &str,
        password: String,
        serial: String,
    ) -> Result<(), SourceError> {
        self.service.unbind_piv(id, password, &serial)
    }

    /// Opens one folder with its YubiKey. A missing card does not lock the others out.
    pub fn unlock_with_yubikey(
        &self,
        id: &str,
        serial: String,
        prompt: PivPrompt,
    ) -> Result<(), SourceError> {
        let prompt_for_card = prompt.clone();
        self.service
            .unlock_with_piv(id, &serial, &prompt, |slot, ephemeral| {
                yubikey::agree(&serial, slot, &prompt_for_card, ephemeral)
            })
    }

    pub fn login_status(&self) -> Result<LoginStatus, SourceError> {
        self.store.login_status()
    }

    pub fn clear_login(&self) -> Result<LoginStatus, SourceError> {
        self.store.clear_login()
    }

    pub fn set_login_password(&self, password: String) -> Result<LoginStatus, SourceError> {
        self.store.set_login_password(&SecretString::from(password))
    }

    pub fn set_login_pin(&self, pin: String) -> Result<LoginStatus, SourceError> {
        self.store.set_login_pin(&SecretString::from(pin))
    }

    /// Replaces the login with a YubiKey 5C touch. The credential does not unwrap folders.
    pub fn set_login_webauthn(&self) -> Result<LoginStatus, SourceError> {
        self.store.enroll_webauthn(&mut SystemWebAuthn)
    }

    /// The app gate. WebAuthn is a touch; password and PIN are the other, separate choices.
    pub fn unlock_app(&self, proof: LoginProof) -> Result<(), SourceError> {
        match proof {
            LoginProof::WebAuthn => self.store.login_webauthn(&mut SystemWebAuthn),
            other => self.store.unlock_app(&other),
        }
    }

    /// The recovery key or phrase opens the folder and sets a new password.
    pub fn recover_folder(
        &self,
        id: &str,
        recovery: &str,
        new_password: String,
    ) -> Result<(), SourceError> {
        self.service.recover_folder(id, recovery, new_password)
    }

    /// Locks an open folder again: its key leaves memory and its items leave the views.
    pub fn relock_folder(&self, id: &str) -> Result<(), SourceError> {
        self.service.relock_folder(id)
    }

    pub fn change_folder_password(
        &self,
        id: &str,
        current: String,
        new_password: String,
    ) -> Result<(), SourceError> {
        self.service
            .change_folder_password(id, current, new_password)
    }

    /// A new recovery key; the old one stops working. Shown once.
    pub fn renew_recovery_key(
        &self,
        id: &str,
        current: String,
    ) -> Result<RecoveryKey, SourceError> {
        self.service.renew_recovery_key(id, current)
    }

    /// Takes the lock off and decrypts the folder (`progress(done, total)`).
    pub fn remove_folder_lock(
        &self,
        id: &str,
        password: String,
        progress: &dyn Fn(u32, u32),
    ) -> Result<(), SourceError> {
        self.service.remove_folder_lock(id, password, progress)
    }

    pub fn set_lock_label(
        &self,
        id: &str,
        label: Option<String>,
        label_color: Option<String>,
    ) -> Result<(), SourceError> {
        self.service.set_lock_label(id, label, label_color)
    }

    /// Writes the recovery key to a text file the user chose, outside the library folder: the
    /// library never holds it (docs/age-locked-folders.md §7).
    pub fn save_recovery_file(
        &self,
        path: &str,
        folder: &str,
        key: &RecoveryKey,
    ) -> Result<(), SourceError> {
        self.service
            .save_recovery_file(Path::new(path), folder, key)
    }

    /// Recomputes the open views after a write and tells the UI. The totals include the
    /// "Importing…" cells of a running-Eagle add, as `open_view` does.
    fn refresh_views(&self) {
        let extra = lock(&self.pending).len();
        match self.eagle() {
            Some(source) => self.service.refresh_views_of(source.as_ref(), extra),
            None => {
                if let Ok(source) = self.service.source() {
                    self.service.refresh_views_of(&source, extra);
                }
            }
        }
    }

    /// Adds what is on the clipboard: copied files, or a pasted image as
    /// `Pasted YYYY-MM-DD HH-MM-SS.png` (UTC). `None` when there is nothing to add.
    pub fn paste(
        &self,
        clipboard: &dyn Clipboard,
        folder: Option<String>,
    ) -> Result<Option<ImportTicket>, SourceError> {
        self.service.writable_library()?;
        if let Some(files) = clipboard.files().filter(|files| !files.is_empty()) {
            return self
                .service
                .add_files(files, folder, ImportFilter::default())
                .map(Some);
        }
        let Some((width, height, rgba)) = clipboard.image() else {
            return Ok(None);
        };
        let png = media::png_from_rgba(width, height, rgba)
            .map_err(|error| SourceError::Invalid(error.to_string()))?;
        self.service.add_png(png, folder).map(Some)
    }

    /// The answer to the duplicate question of import `id`.
    pub fn answer_import(&self, id: u64, choice: DuplicateChoice) -> Result<(), SourceError> {
        self.service.answer_import(id, choice)
    }

    /// Stops import `id` after the current file; added files stay.
    pub fn cancel_import(&self, id: u64) -> Result<(), SourceError> {
        self.service.cancel_import(id)
    }

    /// Running imports first, then finished ones, newest first.
    pub fn imports(&self) -> Vec<ImportProgress> {
        self.service.imports().list()
    }

    /// Adding files or importing an Eagle library is running (previews do not count).
    pub fn has_running_imports(&self) -> bool {
        self.service.imports().has_running()
    }

    /// Quitting: imports stop and the library closes without being marked closed explicitly.
    pub fn shutdown(&self) {
        lock(&self.eagle).take();
        if let Err(error) = self.service.close_library() {
            tracing::warn!("closing the library on exit failed: {error}");
        }
    }

    /// Cancels every import and waits for their threads; their added files stay.
    pub fn stop_imports(&self) {
        self.service.imports().stop();
    }

    #[cfg(test)]
    fn current_library(&self) -> Option<Arc<library_native::Library>> {
        self.service.current_library()
    }

    /// Native-only writes: a live Eagle session answers `NotSupported` instead of the
    /// misleading "Open a library first".
    fn require_native(&self, what: &str) -> Result<(), SourceError> {
        if self.eagle().is_some() {
            return Err(SourceError::NotSupported(format!(
                "Eagle's API cannot {what}"
            )));
        }
        Ok(())
    }

    /// The live Eagle connection, cloned out from under the lock. Callers bind it with `let`
    /// or `if let` on this call, never on `lock(&self.eagle)` itself: in edition 2024 the guard
    /// of an `if let` scrutinee lives through the then-block, and anything there that reaches
    /// `refresh_views` would lock `eagle` again and deadlock (D-089).
    fn eagle(&self) -> Option<Arc<EagleSource>> {
        lock(&self.eagle).clone()
    }
}

/// The positions `start..start + len` of a view of `total` items followed by `pending`
/// entries: the view part through `view_part`, the rest straight from `pending`.
fn combined<T: Clone>(
    start: usize,
    len: usize,
    total: usize,
    pending: &[T],
    view_part: impl FnOnce(usize, usize) -> Vec<T>,
) -> Vec<T> {
    let end = (start + len).min(total + pending.len());
    if start >= end {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(end - start);
    if start < total {
        out.extend(view_part(start, end.min(total) - start));
    }
    let from_pending = end.saturating_sub(start.max(total));
    if from_pending > 0 {
        let at = start.saturating_sub(total);
        out.extend(pending[at..at + from_pending].iter().cloned());
    }
    out
}

/// How reading an Eagle folder fails, for the wizard (docs/spec.md §8.2 step 1).
fn eagle_error(error: eagle_folder::EagleError) -> SourceError {
    match error {
        eagle_folder::EagleError::NotFound(_) => SourceError::NotFound,
        eagle_folder::EagleError::NotAnEagleLibrary(_) => SourceError::NotALibrary { eagle: true },
        other => SourceError::Invalid(format!("reading the Eagle library: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::time::Duration;

    use library_core::{NetworkError, TagMode};
    use library_native::{Library, OpenOptions};

    use super::*;
    use crate::views::Event;

    struct Fixture {
        _data: tempfile::TempDir,
        home: tempfile::TempDir,
        session: Session,
    }

    fn fixture() -> Fixture {
        fixture_with(SyncEnvironment::default())
    }

    fn fixture_with(sync: SyncEnvironment) -> Fixture {
        let data = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        // Tests never touch the OS keyring (D-080); a missing default store on Linux
        // would otherwise fail when Connect looks up a saved token.
        let session = Session::with_secrets(
            AppStore::new(data.path()),
            home.path().to_path_buf(),
            "0.1.0-test",
            sync,
            Arc::new(|_: Event| {}),
            Arc::new(app_store::MemoryStore::default()),
        );
        Fixture {
            _data: data,
            home,
            session,
        }
    }

    #[test]
    fn connecting_to_nothing_is_refused_and_leaves_the_session_empty() {
        let f = fixture();
        let settings = ConnectionSettings {
            host: "127.0.0.1".into(),
            port: 9, // discard: nothing listens there
        };
        let error = f.session.connect_eagle(settings, None).unwrap_err();
        assert!(
            matches!(error, SourceError::Network(NetworkError::Refused)),
            "unexpected: {error}"
        );
        assert!(f.session.current().is_none());
        assert!(f.session.eagle_session().is_none());
    }

    #[test]
    fn an_invalid_host_is_rejected_before_any_request() {
        let f = fixture();
        let settings = ConnectionSettings {
            host: "my pc".into(),
            port: 41595,
        };
        let error = f.session.connect_eagle(settings, None).unwrap_err();
        assert!(matches!(error, SourceError::Invalid(_)));
    }

    #[test]
    fn disconnect_without_a_connection_is_fine() {
        let f = fixture();
        f.session.disconnect_eagle().unwrap();
        assert!(f.session.current().is_none());
    }

    #[test]
    fn create_in_the_default_folder_makes_it_and_remembers_the_library() {
        let f = fixture();
        let parent = f.session.default_library_location();
        assert!(parent.ends_with("egl-library"));
        let info = f.session.create_library(&parent, "Refs").unwrap();
        assert_eq!(info.name, "Refs");
        assert!(!info.read_only);
        let recent = f.session.recent_libraries();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "Refs");
        assert!(recent[0].path.ends_with("Refs.eglib"));
        assert_eq!(f.session.current().unwrap().id, info.id);
        // D-117: the folder shows the app's icon in Explorer and Dolphin.
        let root = Path::new(&recent[0].path);
        if cfg!(windows) {
            assert!(root.join("desktop.ini").is_file());
            assert!(root.join(".egl-icon.ico").is_file());
        } else if cfg!(target_os = "linux") {
            assert!(root.join(".directory").is_file());
            assert!(root.join(".egl-icon.png").is_file());
        }
    }

    #[test]
    fn a_library_reopens_on_start_unless_closed_explicitly() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        let info = f.session.create_library(&parent, "Daily").unwrap();
        f.session.shutdown();
        assert!(f.session.current().is_none());

        let start = f.session.startup(false);
        assert_eq!(start.library.unwrap().id, info.id);
        assert_eq!(start.notice, None);

        f.session.close_library().unwrap();
        assert_eq!(f.session.startup(false).library, None);

        f.session
            .open_library(&f.session.recent_libraries()[0].path)
            .unwrap();
        f.session.shutdown();
        assert_eq!(f.session.startup(true).library, None, "--no-reopen");
    }

    #[test]
    fn the_last_library_open_elsewhere_is_not_reopened_but_mentioned() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        f.session.create_library(&parent, "Busy").unwrap();
        let path = f.session.recent_libraries()[0].path.clone();
        f.session.shutdown();

        let other = Library::open(Path::new(&path), &OpenOptions::new("other")).unwrap();
        let start = f.session.startup(false);
        assert_eq!(start.library, None);
        assert!(start.notice.unwrap().contains("Busy"));
        assert!(f.session.current().is_none());

        // Opening it on purpose is read-only, and works for editing once the other closes.
        let info = f.session.open_library(&path).unwrap();
        assert_eq!(info.read_only_reason, Some(ReadOnlyReason::OtherInstance));
        other.close().unwrap();
        let info = f.session.open_library(&path).unwrap();
        assert!(!info.read_only);
    }

    #[test]
    fn switching_keeps_the_current_library_when_the_new_one_fails() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        let first = f.session.create_library(&parent, "First").unwrap();
        let not_a_library = f.home.path().join("plain");
        fs::create_dir(&not_a_library).unwrap();
        assert_eq!(
            f.session.open_library(&not_a_library.to_string_lossy()),
            Err(SourceError::NotALibrary { eagle: false })
        );
        assert_eq!(f.session.current().unwrap().id, first.id);
    }

    #[test]
    fn sync_folders_are_refused_for_create_and_read_only_for_open() {
        let home = tempfile::tempdir().unwrap();
        let cloud = home.path().join("OneDrive");
        fs::create_dir(&cloud).unwrap();
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        f.session.create_library(&parent, "Moved").unwrap();
        f.session.shutdown();
        let moved = cloud.join("Moved.eglib");
        fs::rename(f.home.path().join("Moved.eglib"), &moved).unwrap();
        // A read-only library gets no icon files (D-117): it is written by nobody.
        for name in [
            ".directory",
            ".egl-icon.png",
            ".egl-icon.ico",
            "desktop.ini",
        ] {
            let _ = fs::remove_file(moved.join(name));
        }

        let g = fixture_with(SyncEnvironment {
            roots: vec![cloud.clone()],
        });
        assert_eq!(
            g.session.create_library(&cloud.to_string_lossy(), "New"),
            Err(SourceError::UnsupportedLocation(
                LocationProblem::SyncFolder
            ))
        );
        let info = g.session.open_library(&moved.to_string_lossy()).unwrap();
        assert_eq!(info.read_only_reason, Some(ReadOnlyReason::SyncFolder));
        assert!(!moved.join(".directory").exists() && !moved.join("desktop.ini").exists());
        assert_eq!(
            g.session.open_library(r"\\nas\share\Lib.eglib"),
            Err(SourceError::UnsupportedLocation(
                LocationProblem::NetworkPath
            ))
        );
        let preferences = Preferences {
            default_library_parent: Some(cloud.to_string_lossy().into_owned()),
            ..Preferences::default()
        };
        assert_eq!(
            g.session.save_preferences(preferences),
            Err(SourceError::UnsupportedLocation(
                LocationProblem::SyncFolder
            ))
        );
    }

    #[test]
    fn the_default_folder_can_be_changed() {
        let f = fixture();
        let elsewhere = f.home.path().join("Libraries");
        fs::create_dir(&elsewhere).unwrap();
        let saved = f
            .session
            .save_preferences(Preferences {
                default_library_parent: Some(elsewhere.to_string_lossy().into_owned()),
                ..Preferences::default()
            })
            .unwrap();
        assert_eq!(
            saved.default_library_parent.as_deref(),
            Some(display_path(&elsewhere).as_str())
        );
        assert_eq!(
            f.session.default_library_location(),
            display_path(&elsewhere)
        );
    }

    /// The committed synthetic Eagle library with real images, in `root` (D-034).
    fn eagle_fixture(root: &std::path::Path) -> std::path::PathBuf {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/eagle-library/Synthetic.library");
        let target = root.join("Synthetic.library");
        copy_dir(&source, &target);
        let images = target.join("images");
        let jpg = |seed: u32| media::fixtures::jpeg(96, 64, seed);
        let png = |seed: u32| media::fixtures::raster(image::ImageFormat::Png, 96, 64, seed);
        let thumb = |seed: u32| media::fixtures::raster(image::ImageFormat::Png, 32, 32, seed);
        fs::write(images.join("EITEMPOSTER1.info/sunset-poster.jpg"), jpg(1)).unwrap();
        fs::write(
            images.join("EITEMPOSTER2.info/duplicate-poster.jpg"),
            jpg(1),
        )
        .unwrap();
        fs::write(images.join("EITEMLOCKED1.info/secret-scan.png"), png(2)).unwrap();
        fs::write(
            images.join("EITEMLOCKED1.info/secret-scan_thumbnail.png"),
            thumb(12),
        )
        .unwrap();
        fs::write(images.join("EITEMTRASH1.info/old-draft.jpg"), jpg(3)).unwrap();
        fs::write(
            images.join("EITEMTRASH1.info/old-draft_thumbnail.png"),
            thumb(13),
        )
        .unwrap();
        fs::write(images.join("EITEMNOTHUMB1.info/tiny-icon.png"), png(4)).unwrap();
        fs::write(images.join("EITEMLOOSE01.info/loose-photo.jpg"), jpg(5)).unwrap();
        fs::write(
            images.join("EITEMLOOSE01.info/loose-photo_thumbnail.png"),
            thumb(15),
        )
        .unwrap();
        target
    }

    fn copy_dir(source: &std::path::Path, target: &std::path::Path) {
        fs::create_dir_all(target).unwrap();
        for entry in fs::read_dir(source).unwrap().flatten() {
            let to = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &to);
            } else {
                fs::copy(entry.path(), &to).unwrap();
            }
        }
    }

    #[test]
    fn importing_an_eagle_library_creates_and_fills_the_library() {
        let f = fixture();
        let eagle = eagle_fixture(f.home.path());
        let eagle_path = display_path(&eagle);

        let summary = f.session.eagle_library_info(&eagle_path).unwrap();
        assert_eq!(summary.name, "Synthetic");
        assert_eq!(summary.item_count, 7);
        assert!(summary.bytes > 0);

        let parent = display_path(f.home.path());
        let ticket = f
            .session
            .import_eagle_library(EagleImportRequest {
                eagle_path,
                create: Some(EagleLibraryTarget {
                    parent,
                    name: "Imported".into(),
                }),
                import_trash: false,
            })
            .unwrap();
        let done = finished(&f.session, ticket.id);
        let report = done.report.expect("the report");
        assert_eq!(report.imported, 5, "{report:?}");
        assert_eq!(report.duplicates, 1);
        assert_eq!(report.locked_folders, ["Secret"]);

        let info = f.session.current().expect("the new library is open");
        assert_eq!(info.name, "Imported");
        let counts = f.session.counts().unwrap();
        assert_eq!(counts.all, 4, "{counts:?}");
        let tree = f.session.folder_tree().unwrap();
        let names: Vec<&str> = tree.iter().map(|folder| folder.name.as_str()).collect();
        assert_eq!(names, ["Posters", "Secret", "Empty"]);
    }

    #[test]
    fn importing_into_the_open_library_refuses_read_only() {
        let f = fixture();
        let eagle = eagle_fixture(f.home.path());
        let eagle_path = display_path(&eagle);
        let error = f
            .session
            .import_eagle_library(EagleImportRequest {
                eagle_path,
                create: None,
                import_trash: false,
            })
            .unwrap_err();
        assert!(matches!(error, SourceError::Invalid(_)), "{error:?}");
    }

    /// Waits until import `id` has finished, and returns its final progress.
    fn finished(session: &Session, id: u64) -> ImportProgress {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(done) = session.imports().into_iter().find(|progress| {
                progress.id == id
                    && matches!(
                        progress.state,
                        library_core::ImportState::Done | library_core::ImportState::Cancelled
                    )
            }) {
                return done;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "import {id} did not finish"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Waits until import `id` asks about a duplicate.
    fn waiting(session: &Session, id: u64) {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !session.imports().iter().any(|progress| {
            progress.id == id && progress.state == library_core::ImportState::WaitingForDecision
        }) {
            assert!(
                std::time::Instant::now() < deadline,
                "import {id} never asked"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn images(dir: &Path, count: u32) -> Vec<String> {
        (0..count)
            .map(|n| {
                let path = dir.join(format!("image-{n}.png"));
                fs::write(
                    &path,
                    media::fixtures::raster(image::ImageFormat::Png, 32, 24, n),
                )
                .unwrap();
                path.to_string_lossy().into_owned()
            })
            .collect()
    }

    struct FakeClipboard {
        files: Option<Vec<PathBuf>>,
        image: Option<(u32, u32, Vec<u8>)>,
    }

    impl Clipboard for FakeClipboard {
        fn files(&self) -> Option<Vec<PathBuf>> {
            self.files.clone()
        }
        fn image(&self) -> Option<(u32, u32, Vec<u8>)> {
            self.image.clone()
        }
    }

    #[test]
    fn added_files_count_and_send_progress() {
        let events = Arc::new(Mutex::new(Vec::<ImportProgress>::new()));
        let sink = Arc::clone(&events);
        let data = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let session = Session::new(
            AppStore::new(data.path()),
            home.path().to_path_buf(),
            "0.1.0-test",
            SyncEnvironment::default(),
            Arc::new(move |event: Event| {
                if let Event::ImportProgress(progress) = event {
                    sink.lock().unwrap().push(progress);
                }
            }),
        );
        assert!(matches!(
            session.add_files(vec![], None),
            Err(SourceError::Invalid(_))
        ));
        session
            .create_library(&home.path().to_string_lossy(), "Imports")
            .unwrap();
        let ticket = session.add_files(images(home.path(), 3), None).unwrap();
        let done = finished(&session, ticket.id);
        assert_eq!((done.added, done.previews_done), (3, 3));
        assert_eq!(session.counts().unwrap().all, 3);
        assert!(!session.has_running_imports());
        let events = events.lock().unwrap();
        assert!(events.iter().any(|event| event.id == ticket.id));
        assert_eq!(
            events.last().unwrap().state,
            library_core::ImportState::Done
        );
    }

    #[test]
    fn the_duplicate_question_waits_for_an_answer() {
        let f = fixture();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Dupes")
            .unwrap();
        let files = images(f.home.path(), 2);
        let first = f.session.add_files(files.clone(), None).unwrap();
        finished(&f.session, first.id);

        let again = f.session.add_files(files.clone(), None).unwrap();
        waiting(&f.session, again.id);
        assert!(f.session.has_running_imports());
        f.session
            .answer_import(again.id, DuplicateChoice::Skip)
            .unwrap();
        let done = finished(&f.session, again.id);
        assert_eq!((done.added, done.duplicates), (0, 2));

        let third = f.session.add_files(files, None).unwrap();
        waiting(&f.session, third.id);
        f.session.cancel_import(third.id).unwrap();
        assert_eq!(
            finished(&f.session, third.id).state,
            library_core::ImportState::Cancelled
        );
        assert_eq!(f.session.counts().unwrap().all, 2);
    }

    #[test]
    fn closing_while_an_import_waits_stops_it() {
        let f = fixture();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Busy")
            .unwrap();
        let files = images(f.home.path(), 1);
        finished(
            &f.session,
            f.session.add_files(files.clone(), None).unwrap().id,
        );
        let waiting_import = f.session.add_files(files, None).unwrap();
        waiting(&f.session, waiting_import.id);
        f.session.close_library().unwrap();
        assert!(f.session.current().is_none());
        assert!(!f.session.has_running_imports());
    }

    #[test]
    fn pasting_adds_files_or_an_image() {
        let f = fixture();
        let empty = FakeClipboard {
            files: None,
            image: None,
        };
        assert!(matches!(
            f.session.paste(&empty, None),
            Err(SourceError::Invalid(_))
        ));
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Paste")
            .unwrap();
        assert_eq!(f.session.paste(&empty, None).unwrap(), None);

        let image = FakeClipboard {
            files: None,
            image: Some((4, 2, vec![200; 4 * 2 * 4])),
        };
        let ticket = f.session.paste(&image, None).unwrap().unwrap();
        assert_eq!(finished(&f.session, ticket.id).added, 1);

        let copied = FakeClipboard {
            files: Some(
                images(f.home.path(), 2)
                    .into_iter()
                    .map(PathBuf::from)
                    .collect(),
            ),
            image: None,
        };
        let ticket = f.session.paste(&copied, None).unwrap().unwrap();
        assert_eq!(finished(&f.session, ticket.id).added, 2);
        assert_eq!(f.session.counts().unwrap().all, 3);
    }

    #[test]
    fn a_read_only_library_refuses_new_files() {
        let f = fixture();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Shared")
            .unwrap();
        let path = f.session.recent_libraries()[0].path.clone();
        f.session.shutdown();
        let _other = Library::open(Path::new(&path), &OpenOptions::new("other")).unwrap();
        f.session.open_library(&path).unwrap();
        assert_eq!(
            f.session.add_files(images(f.home.path(), 1), None),
            Err(SourceError::ReadOnly)
        );
    }

    #[test]
    fn opening_a_library_makes_missing_previews() {
        let f = fixture();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Resume")
            .unwrap();
        let path = f.session.recent_libraries()[0].path.clone();
        f.session.shutdown();
        {
            let library = Library::open(Path::new(&path), &OpenOptions::new("test")).unwrap();
            for file in images(f.home.path(), 2) {
                library
                    .add_file(Path::new(&file), library_native::Duplicates::Report)
                    .unwrap();
            }
        }
        f.session.open_library(&path).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let previews = loop {
            if let Some(task) = f
                .session
                .imports()
                .into_iter()
                .find(|progress| progress.kind == ImportKind::Previews)
            {
                break task;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "no previews task started"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let done = finished(&f.session, previews.id);
        assert_eq!(done.previews_done, 2);
        assert!(
            !f.session.has_running_imports(),
            "previews do not hold back quitting"
        );
    }

    /// A session whose events are recorded.
    fn recording() -> (Fixture, Arc<Mutex<Vec<Event>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let data = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let session = Session::new(
            AppStore::new(data.path()),
            home.path().to_path_buf(),
            "0.1.0-test",
            SyncEnvironment::default(),
            Arc::new(move |event: Event| sink.lock().unwrap().push(event)),
        );
        (
            Fixture {
                _data: data,
                home,
                session,
            },
            events,
        )
    }

    #[test]
    fn views_follow_added_files_under_the_same_id() {
        let (f, events) = recording();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Views")
            .unwrap();
        let progress = Mutex::new(Vec::new());
        let opened = f
            .session
            .open_view(LibraryQuery::default(), &|loaded, total| {
                progress.lock().unwrap().push((loaded, total));
            })
            .unwrap();
        assert_eq!(opened.total, 0);
        assert_eq!(*progress.lock().unwrap(), [(0, 0)]);

        let ticket = f.session.add_files(images(f.home.path(), 3), None).unwrap();
        finished(&f.session, ticket.id);
        let invalidated: Vec<_> = events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                Event::ViewsInvalidated(views) => Some(views.views.clone()),
                Event::ImportProgress(_) | Event::ExportProgress(_) => None,
            })
            .collect();
        assert_eq!(
            invalidated.last().unwrap(),
            &[ViewOpened {
                view_id: opened.view_id,
                total: 3
            }]
        );
        let cells = f.session.view_range(opened.view_id, 0, 500).unwrap();
        assert_eq!(cells.len(), 3);
        assert!(
            cells
                .iter()
                .all(|cell| cell.has_thumbnail && cell.thumb_version > 0)
        );
        assert!(cells.iter().all(|cell| cell.palette0.is_some()));
        // Newest first: the last file added comes first.
        let ids = f.session.view_ids(opened.view_id, 0, 3).unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], cells[0].id);

        let detail = f.session.item_detail(&cells[0].id.0).unwrap().unwrap();
        assert_eq!(detail.id, cells[0].id);
        assert_eq!(f.session.item_detail("01NOTANITEM").unwrap(), None);

        f.session.close_view(opened.view_id);
        assert_eq!(
            f.session.view_range(opened.view_id, 0, 10),
            Err(SourceError::NotFound)
        );
    }

    #[test]
    fn views_need_a_library_and_go_with_it() {
        let f = fixture();
        assert!(matches!(
            f.session.open_view(LibraryQuery::default(), &|_, _| {}),
            Err(SourceError::Invalid(_))
        ));
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Gone")
            .unwrap();
        let opened = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        f.session.close_library().unwrap();
        assert_eq!(
            f.session.view_range(opened.view_id, 0, 10),
            Err(SourceError::NotFound)
        );
    }

    #[test]
    fn the_thumb_scheme_serves_only_items_of_the_open_library() {
        let f = fixture();
        assert_eq!(f.session.image("/01ANYTHING", None), None, "no library");
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Thumbs")
            .unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 1), None).unwrap();
        finished(&f.session, ticket.id);
        let opened = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        let id = f.session.view_ids(opened.view_id, 0, 1).unwrap()[0]
            .0
            .clone();

        let thumbnail = f.session.image(&format!("/{id}"), None).unwrap();
        assert_eq!(thumbnail.content_type, "image/webp");
        let original = f.session.image(&format!("/{id}/original"), None).unwrap();
        assert_eq!(original.content_type, "image/png");
        assert_eq!(
            original.bytes,
            fs::read(f.home.path().join("image-0.png")).unwrap()
        );
        for path in [
            "/",
            "/../library.db",
            &format!("/{id}/other"),
            "/01NOTANITEM",
            "/a%2Fb",
        ] {
            assert_eq!(f.session.image(path, None), None, "{path}");
        }
    }

    fn last_totals(events: &Mutex<Vec<Event>>) -> Vec<ViewOpened> {
        events
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::ViewsInvalidated(views) => Some(views.views.clone()),
                Event::ImportProgress(_) | Event::ExportProgress(_) => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn locking_and_unlocking_update_the_open_views() {
        let (f, events) = recording();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Locked")
            .unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 3), None).unwrap();
        finished(&f.session, ticket.id);
        let tree = f
            .session
            .save_folder(FolderChange::Create {
                name: "Private".into(),
                parent: None,
                description: String::new(),
                icon_color: None,
            })
            .unwrap();
        let folder = tree[0].id.clone();
        let all = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        let first = f.session.view_ids(all.view_id, 0, 1).unwrap();
        f.session
            .apply_item_edit(&first[0].0, ItemEdit::AddToFolders(vec![folder.clone()]))
            .unwrap();
        assert!(
            f.session
                .image(&format!("/{}", first[0].0), None)
                .is_some_and(|image| !image.private)
        );

        let steps = Mutex::new(Vec::new());
        let key = f
            .session
            .lock_folder(
                &folder,
                "pw".into(),
                Some("Taxes".into()),
                None,
                &|done, total| {
                    steps.lock().unwrap().push((done, total));
                },
            )
            .unwrap();
        assert_eq!(steps.into_inner().unwrap().last(), Some(&(1, 1)));
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: all.view_id,
            total: 2
        }));
        assert_eq!(f.session.counts().unwrap().all, 2);
        assert!(f.session.image(&format!("/{}", first[0].0), None).is_none());
        assert_eq!(f.session.item_detail(&first[0].0).unwrap(), None);

        assert_eq!(
            f.session.unlock_folder(&folder, "nope".into()),
            Err(SourceError::WrongPassword)
        );
        f.session.unlock_folder(&folder, "pw".into()).unwrap();
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: all.view_id,
            total: 3
        }));
        // Decrypted images are marked private: the WebView must not cache them.
        let image = f.session.image(&format!("/{}", first[0].0), None).unwrap();
        assert!(image.private);
        f.session.relock_folder(&folder).unwrap();
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: all.view_id,
            total: 2
        }));

        // The recovery key file goes outside the library folder only.
        let library_root = PathBuf::from(f.session.current().unwrap().path.unwrap());
        assert!(matches!(
            f.session.save_recovery_file(
                &library_root.join("key.txt").to_string_lossy(),
                "Private",
                &key
            ),
            Err(SourceError::Invalid(_))
        ));
        let outside = f.home.path().join("Private recovery key.txt");
        f.session
            .save_recovery_file(&outside.to_string_lossy(), "Private", &key)
            .unwrap();
        let text = fs::read_to_string(&outside).unwrap();
        assert!(text.contains(&key.key) && text.contains(&key.phrase));
        assert!(text.contains("Locked folder: Private"));
    }

    #[test]
    fn folder_edits_refresh_the_open_views() {
        let (f, events) = recording();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Folders")
            .unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 3), None).unwrap();
        finished(&f.session, ticket.id);
        let tree = f
            .session
            .save_folder(FolderChange::Create {
                name: "Posters".into(),
                parent: None,
                description: String::new(),
                icon_color: None,
            })
            .unwrap();
        let folder = tree[0].id.clone();
        let folder_view = f
            .session
            .open_view(
                LibraryQuery {
                    scope: library_core::Scope::Folder {
                        id: folder.clone(),
                        include_descendants: false,
                    },
                    ..LibraryQuery::default()
                },
                &|_, _| {},
            )
            .unwrap();
        let all = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        assert_eq!(folder_view.total, 0);

        // Select All in the All view, except one item.
        let first = f.session.view_ids(all.view_id, 0, 1).unwrap();
        let edited = f
            .session
            .apply_batch_edit(
                ItemsTarget {
                    view_id: Some(all.view_id),
                    all: true,
                    ids: first.clone(),
                },
                ItemEdit::AddToFolders(vec![folder.clone()]),
            )
            .unwrap();
        assert_eq!(edited, 2);
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: folder_view.view_id,
            total: 2
        }));
        let tree = f.session.folder_tree().unwrap();
        assert_eq!(tree[0].image_count, Some(2));

        let item = f
            .session
            .apply_item_edit(&first[0].0, ItemEdit::AddToFolders(vec![folder.clone()]))
            .unwrap();
        assert_eq!(item.folders, std::slice::from_ref(&folder));
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: folder_view.view_id,
            total: 3
        }));

        // Deleting the open folder empties its view.
        let tree = f.session.delete_folder(&folder, false).unwrap();
        assert!(tree.is_empty());
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: folder_view.view_id,
            total: 0
        }));
    }

    #[test]
    fn tag_edits_refresh_the_views_and_follow_the_filter_modes() {
        let (f, events) = recording();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Tags")
            .unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 4), None).unwrap();
        finished(&f.session, ticket.id);
        let all = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        let ids = f.session.view_ids(all.view_id, 0, 4).unwrap();
        let target = |items: &[ItemId]| ItemsTarget {
            view_id: Some(all.view_id),
            all: false,
            ids: items.to_vec(),
        };
        let tag = |items: &[ItemId], names: &[&str]| {
            f.session
                .apply_batch_edit(
                    target(items),
                    ItemEdit::AddTags(names.iter().map(|name| (*name).to_owned()).collect()),
                )
                .unwrap()
        };
        tag(&ids[0..2], &["Poster", "Swiss"]);
        tag(&ids[1..2], &["Print"]);
        tag(&ids[2..3], &["poster"]);
        let query = |names: &[&str], mode: TagMode| LibraryQuery {
            tags: names.iter().map(|name| (*name).to_owned()).collect(),
            tag_mode: mode,
            ..LibraryQuery::default()
        };
        // Views are kept a few at a time: each filter view closes after its total is read.
        let total = |names: &[&str], mode: TagMode| {
            let view = f.session.open_view(query(names, mode), &|_, _| {}).unwrap();
            f.session.close_view(view.view_id);
            view.total
        };
        assert_eq!(total(&["Poster", "Swiss"], TagMode::Any), 2);
        assert_eq!(total(&["Poster", "Print"], TagMode::All), 1);
        assert_eq!(total(&["Poster", "Swiss"], TagMode::Exact), 1);
        assert_eq!(total(&["Poster", "Swiss"], TagMode::None), 2);
        let lower = f
            .session
            .open_view(query(&["poster"], TagMode::Any), &|_, _| {})
            .unwrap();
        assert_eq!(lower.total, 1);

        // Select All but the last item: the counts come from the view.
        let selected = f
            .session
            .selection_tags(ItemsTarget {
                view_id: Some(all.view_id),
                all: true,
                ids: ids[3..].to_vec(),
            })
            .unwrap();
        assert_eq!(selected.items, 3);
        let counts: Vec<(&str, Option<u32>)> = selected
            .tags
            .iter()
            .map(|tag| (tag.name.as_str(), tag.image_count))
            .collect();
        assert_eq!(
            counts,
            [
                ("Poster", Some(2)),
                ("poster", Some(1)),
                ("Print", Some(1)),
                ("Swiss", Some(2))
            ]
        );

        // A rename reaches every item and the open views.
        f.session.rename_tag("Poster", "Posters").unwrap();
        let cells = f.session.view_range(all.view_id, 0, 4).unwrap();
        let renamed = cells
            .iter()
            .filter(|cell| cell.tags.contains(&"Posters".to_owned()))
            .count();
        assert_eq!(renamed, 2);
        assert!(
            cells
                .iter()
                .all(|cell| !cell.tags.contains(&"Poster".to_owned()))
        );
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: lower.view_id,
            total: 1
        }));
        let vocabulary: Vec<String> = f
            .session
            .tag_vocabulary()
            .unwrap()
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        assert_eq!(vocabulary, ["poster", "Posters", "Print", "Swiss"]);

        // Removing one tag leaves the others.
        f.session
            .apply_batch_edit(
                target(&ids[0..1]),
                ItemEdit::RemoveTags(vec!["Swiss".into()]),
            )
            .unwrap();
        let first = f.session.item_detail(&ids[0].0).unwrap().unwrap();
        assert_eq!(first.tags, ["Posters"]);

        f.session.merge_tags("Print", "Posters").unwrap();
        f.session.delete_tag("poster").unwrap();
        let groups = f
            .session
            .save_tag_group(TagGroupChange::Create {
                name: "Kinds".into(),
                color: None,
                tags: vec!["Posters".into()],
            })
            .unwrap();
        assert_eq!(groups[0].tags, ["Posters"]);
        assert!(
            f.session
                .delete_tag_group(&groups[0].id)
                .unwrap()
                .is_empty()
        );
        let vocabulary: Vec<String> = f
            .session
            .tag_vocabulary()
            .unwrap()
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        assert_eq!(vocabulary, ["Posters", "Swiss"]);
    }

    #[test]
    fn search_stays_in_the_scope_and_smart_folders_keep_what_the_chips_showed() {
        let (f, events) = recording();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Search")
            .unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 4), None).unwrap();
        finished(&f.session, ticket.id);
        let tree = f
            .session
            .save_folder(FolderChange::Create {
                name: "Posters".into(),
                parent: None,
                description: String::new(),
                icon_color: None,
            })
            .unwrap();
        let folder = tree[0].id.clone();
        let all = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        let ids = f.session.view_ids(all.view_id, 0, 4).unwrap();
        f.session
            .apply_batch_edit(
                ItemsTarget {
                    view_id: Some(all.view_id),
                    all: false,
                    ids: ids[0..1].to_vec(),
                },
                ItemEdit::AddToFolders(vec![folder.clone()]),
            )
            .unwrap();
        f.session
            .apply_batch_edit(
                ItemsTarget {
                    view_id: Some(all.view_id),
                    all: false,
                    ids: ids[0..2].to_vec(),
                },
                ItemEdit::AddTags(vec!["Swiss".into()]),
            )
            .unwrap();
        f.session.close_view(all.view_id);
        let total = |query: LibraryQuery| {
            let view = f.session.open_view(query, &|_, _| {}).unwrap();
            f.session.close_view(view.view_id);
            view.total
        };
        // Every file is named image-N: the search matches all four, the folder holds one.
        let search = |text: &str| LibraryQuery {
            text: Some(text.into()),
            ..LibraryQuery::default()
        };
        assert_eq!(total(search("image")), 4);
        assert_eq!(total(search("swiss")), 2);
        assert_eq!(total(search("image -swiss")), 2);
        assert_eq!(total(search("ima")), 4);
        assert_eq!(
            total(LibraryQuery {
                scope: library_core::Scope::Folder {
                    id: folder.clone(),
                    include_descendants: false,
                },
                ..search("image")
            }),
            1
        );
        // One character searches nothing yet.
        assert_eq!(total(search("x")), 4);

        let chips = LibraryQuery {
            tags: vec!["Swiss".into()],
            ext: Some("png".into()),
            ..LibraryQuery::default()
        };
        let shown = total(chips.clone());
        let draft = f.session.smart_rules(&chips).unwrap();
        assert_eq!(draft.parent, None);
        let smart = f
            .session
            .save_smart_folder(SmartFolderChange::Create {
                name: "Swiss".into(),
                parent: draft.parent,
                conditions: draft.conditions,
            })
            .unwrap();
        let smart_view = f
            .session
            .open_view(
                LibraryQuery {
                    scope: library_core::Scope::Smart {
                        id: smart[0].id.clone(),
                    },
                    ..LibraryQuery::default()
                },
                &|_, _| {},
            )
            .unwrap();
        assert_eq!(smart_view.total, shown);
        assert_eq!(shown, 2);
        // Editing the rules recomputes the open smart view.
        f.session
            .save_smart_folder(SmartFolderChange::Update {
                id: smart[0].id.clone(),
                name: "Everything".into(),
                conditions: serde_json::json!([{ "match": "AND", "rules": [
                    { "property": "type", "method": "equal", "value": "image" }
                ]}]),
            })
            .unwrap();
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: smart_view.view_id,
            total: 4
        }));
        assert!(matches!(
            f.session.smart_rules(&search("swiss")),
            Err(SourceError::Invalid(_))
        ));
        let counts = f.session.extensions().unwrap();
        assert_eq!(counts.len(), 1);
        assert_eq!((counts[0].ext.as_str(), counts[0].count), ("png", 4));
        assert!(
            f.session
                .delete_smart_folder(&smart[0].id)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn files_go_into_the_open_folder() {
        let f = fixture();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Into")
            .unwrap();
        let tree = f
            .session
            .save_folder(FolderChange::Create {
                name: "Inbox".into(),
                parent: None,
                description: String::new(),
                icon_color: None,
            })
            .unwrap();
        let folder = tree[0].id.clone();
        assert!(matches!(
            f.session
                .add_files(images(f.home.path(), 1), Some("01GONE".into())),
            Err(SourceError::Invalid(_))
        ));
        let ticket = f
            .session
            .add_files(images(f.home.path(), 2), Some(folder.clone()))
            .unwrap();
        finished(&f.session, ticket.id);
        assert_eq!(f.session.folder_tree().unwrap()[0].image_count, Some(2));
        assert_eq!(f.session.counts().unwrap().uncategorized, 0);
    }

    #[test]
    fn forgetting_a_recent_library_removes_it() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        f.session.create_library(&parent, "Temp").unwrap();
        let path = f.session.recent_libraries()[0].path.clone();
        f.session.forget_recent_library(&path).unwrap();
        assert!(f.session.recent_libraries().is_empty());
    }

    #[test]
    fn undo_takes_back_the_last_edits_one_by_one() {
        let (f, events) = recording();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Undo")
            .unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 2), None).unwrap();
        finished(&f.session, ticket.id);
        let all = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        let ids = f.session.view_ids(all.view_id, 0, 2).unwrap();
        let id = ids[0].0.clone();
        assert_eq!(f.session.undo_status(), None);

        f.session
            .apply_item_edit(&id, ItemEdit::Rename("Renamed".into()))
            .unwrap();
        f.session
            .apply_item_edit(&id, ItemEdit::SetStar(4))
            .unwrap();
        f.session
            .apply_batch_edit(
                ItemsTarget {
                    view_id: Some(all.view_id),
                    all: true,
                    ids: Vec::new(),
                },
                ItemEdit::Trash,
            )
            .unwrap();
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: all.view_id,
            total: 0
        }));
        assert_eq!(
            f.session.undo_status(),
            Some(UndoStatus {
                kind: library_core::EditKind::Trash,
                items: 2
            })
        );

        let undone = f.session.undo().unwrap().unwrap();
        assert_eq!(undone.kind, library_core::EditKind::Trash);
        assert!(last_totals(&events).contains(&ViewOpened {
            view_id: all.view_id,
            total: 2
        }));
        f.session.undo().unwrap();
        assert_eq!(f.session.item_detail(&id).unwrap().unwrap().star, 0);
        f.session.undo().unwrap();
        let item = f.session.item_detail(&id).unwrap().unwrap();
        assert_ne!(item.name, "Renamed");
        assert!(f.session.original_paths(&ids).unwrap()[0].is_file());
        assert_eq!(f.session.undo().unwrap(), None);
    }

    #[test]
    fn the_trash_empties_for_good_and_takes_no_undo() {
        let f = fixture();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Trash")
            .unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 3), None).unwrap();
        finished(&f.session, ticket.id);
        let all = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        let ids = f.session.view_ids(all.view_id, 0, 3).unwrap();
        let paths = f.session.original_paths(&ids).unwrap();
        let target = |ids: &[ItemId]| ItemsTarget {
            view_id: None,
            all: false,
            ids: ids.to_vec(),
        };
        f.session
            .apply_batch_edit(target(&ids[..2]), ItemEdit::Trash)
            .unwrap();
        assert_eq!(f.session.delete_permanently(target(&ids[..1])).unwrap(), 1);
        assert!(!paths[0].exists());
        assert_eq!(f.session.empty_trash(&|_, _| {}).unwrap(), 1);
        assert!(!paths[1].exists() && paths[2].exists());
        assert_eq!(f.session.counts().unwrap().trash, 0);
        // The trash edit is still the last undoable one; its items are gone, so nothing returns.
        f.session.undo().unwrap();
        assert_eq!(f.session.counts().unwrap().all, 1);
    }

    #[test]
    fn the_trash_menu_counts_sums_and_restores_everything() {
        let f = fixture();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Trash menu")
            .unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 3), None).unwrap();
        finished(&f.session, ticket.id);
        let all = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        let ids = f.session.view_ids(all.view_id, 0, 3).unwrap();
        let target = |ids: &[ItemId]| ItemsTarget {
            view_id: None,
            all: false,
            ids: ids.to_vec(),
        };
        f.session
            .apply_batch_edit(target(&ids[..2]), ItemEdit::Trash)
            .unwrap();
        let info = f.session.trash_info().unwrap();
        assert_eq!(info.items, 2);
        assert!(info.bytes > 0);

        assert_eq!(f.session.restore_trash().unwrap(), 2);
        assert_eq!(f.session.counts().unwrap().trash, 0);
        assert_eq!(f.session.counts().unwrap().all, 3);
        // Restore All is one undoable edit: Ctrl+Z trashes the items again.
        let undone = f.session.undo().unwrap().unwrap();
        assert_eq!(undone.kind, library_core::EditKind::Restore);
        assert_eq!(f.session.counts().unwrap().trash, 2);
        assert_eq!(f.session.trash_info().unwrap().items, 2);
        // An empty trash restores nothing and is not remembered for undo: the last undoable
        // edit is still the move to the trash, its items gone for good.
        f.session.empty_trash(&|_, _| {}).unwrap();
        assert_eq!(f.session.restore_trash().unwrap(), 0);
        assert_eq!(
            f.session.undo_status().unwrap().kind,
            library_core::EditKind::Trash
        );
    }

    #[test]
    fn a_reviewed_import_previews_then_takes_only_what_was_confirmed() {
        let f = fixture();
        let info = f
            .session
            .create_library(&f.home.path().to_string_lossy(), "Reviewed")
            .unwrap();
        // The picked folder: an image at the top, a text file, and a subfolder with both.
        let picked = f.home.path().join("incoming");
        let nested = picked.join("skip me");
        std::fs::create_dir_all(&nested).unwrap();
        for (path, bytes) in [
            (
                picked.join("a.png"),
                media::fixtures::raster(image::ImageFormat::Png, 8, 8, 1),
            ),
            (picked.join("b.txt"), b"notes".to_vec()),
            (
                nested.join("c.png"),
                media::fixtures::raster(image::ImageFormat::Png, 8, 8, 2),
            ),
            (nested.join("d.txt"), b"more".to_vec()),
        ] {
            std::fs::write(path, bytes).unwrap();
        }

        let preview = f.session.preview_import(&picked.to_string_lossy()).unwrap();
        assert_eq!(preview.files, 4);
        assert!(preview.bytes > 0);
        assert_eq!(preview.unlisted, 0);
        // The picked folder itself is the root of the tree, as it becomes the library folder.
        let names: Vec<_> = preview
            .tree
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, ["incoming"]);
        let root = &preview.tree[0];
        assert!(root.folder);
        assert_eq!(root.bytes, preview.bytes);
        let children: Vec<_> = root
            .children
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(children, ["a.png", "b.txt", "skip me"]);
        let nested = root.children.iter().find(|entry| entry.folder).unwrap();
        let deep: Vec<_> = nested
            .children
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(deep, ["c.png", "d.txt"]);
        assert!(root.children[0].image && !root.children[1].image);

        // The library itself refuses, like the import would.
        assert!(
            f.session
                .preview_import(&info.path.clone().unwrap())
                .is_err()
        );

        // Confirm: skip the subfolder, take the text file at the top anyway.
        let filter = ImportFilter {
            exclude: HashSet::from(["skip me".to_owned()]),
            include: HashSet::from(["b.txt".to_owned()]),
            reviewed: true,
        };
        let ticket = f
            .session
            .add_files_filtered(vec![picked.to_string_lossy().into_owned()], None, filter)
            .unwrap();
        finished(&f.session, ticket.id);
        assert_eq!(f.session.counts().unwrap().all, 2);
        let tree = f.session.folder_tree().unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "incoming");
        assert!(tree[0].children.is_empty());
    }

    #[test]
    fn restoring_a_backup_starts_the_library_over() {
        let f = fixture();
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Backups")
            .unwrap();
        let backup = f.session.back_up_now().unwrap();
        let ticket = f.session.add_files(images(f.home.path(), 2), None).unwrap();
        finished(&f.session, ticket.id);
        let all = f
            .session
            .open_view(LibraryQuery::default(), &|_, _| {})
            .unwrap();
        let ids = f.session.view_ids(all.view_id, 0, 1).unwrap();
        f.session
            .apply_item_edit(&ids[0].0, ItemEdit::SetStar(3))
            .unwrap();
        assert!(f.session.backups().unwrap().len() >= 2);

        let info = f.session.restore_backup(&backup.name).unwrap();
        assert_eq!(info.name, "Backups");
        assert_eq!(f.session.counts().unwrap().all, 0);
        assert_eq!(f.session.undo_status(), None);
        assert!(matches!(
            f.session.view_range(all.view_id, 0, 1),
            Err(SourceError::NotFound)
        ));
        let check = f.session.check_library().unwrap();
        assert_eq!(check.orphan_count, 2);
        let repaired = f
            .session
            .repair_library(RepairOptions {
                orphans: true,
                missing: true,
            })
            .unwrap();
        assert!(repaired.is_clean());
        assert!(f.session.library_stats().unwrap().backups > 0);
        let renamed = f.session.rename_library("Renamed").unwrap();
        assert_eq!(renamed.name, "Renamed");
        assert_eq!(f.session.recent_libraries()[0].name, "Renamed");
    }
}

#[cfg(test)]
#[path = "session_eagle_tests.rs"]
mod eagle_tests;
