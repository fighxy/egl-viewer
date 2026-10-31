//! The open library, its import tasks and the app's files, without Tauri: what the commands
//! call, and what the tests drive (docs/architecture.md §7.4, §9, D-022, D-024).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use app_store::{
    AppStore, LoginProof, LoginStatus, Preferences, RecentLibrary, SyncEnvironment, check_location,
    default_library_parent, display_path,
};
use library_core::{
    Backup, DuplicateNotice, ExportPreview, ExportProgress, ExportRequest, ExportSelection,
    ExportState, ExtensionCount, Folder, FolderChange, FolderIndex, ImportKind, ImportPreview,
    ImportProgress, ImportState, ImportTicket, Item, ItemCell, ItemEdit, ItemId, LibraryCheck,
    LibraryCounts, LibraryInfo, LibraryQuery, LibrarySource, LibraryStats, LocationProblem,
    PivPolicy, PivPrompt, PivWrap, Progress, ReadOnlyReason, RecoveryKey, RepairOptions,
    SelectionTags, SmartFolder, SmartFolderChange, SourceError, Tag, TagGroup, TagGroupChange,
    Thumbnail, TrashInfo, UndoStack, UndoStatus, Views, rules_from_query,
};
use library_native::{
    DuplicateChoice, DuplicatePolicy, ImportFilter, ImportObserver, ImportTarget, Library,
    NativeSource, OpenOptions, check_enrollment, create_library, export_items, export_preview,
    import_files, make_pending_previews, utc_file_stamp,
};
use secrecy::SecretString;
use serde::Serialize;
use ts_rs::TS;

use crate::views::{
    Blocking, Emit, Event, ImageRequest, ItemsTarget, Refresher, ViewOpened, count, invalidated,
    ready,
};
use crate::webauthn::SystemWebAuthn;
use crate::yubikey;

/// "Save as smart folder": the rules that keep what a screen shows, and the smart folder the new
/// one goes under (the open smart folder, so its rules apply too).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SmartDraft {
    pub parent: Option<String>,
    /// Eagle's `conditions` array.
    pub conditions: serde_json::Value,
}

/// What the app does on start: the reopened library, or why it shows Welcome instead.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StartupResult {
    pub library: Option<LibraryInfo>,
    /// Shown on Welcome when the last library was not reopened for a reason worth saying.
    pub notice: Option<String>,
}

/// What is on the clipboard: copied files, or an image (RGBA pixels).
pub trait Clipboard {
    fn files(&self) -> Option<Vec<PathBuf>>;
    fn image(&self) -> Option<(u32, u32, Vec<u8>)>;
}

/// Finished imports kept for the Imports panel.
const FINISHED_KEPT: usize = 20;

pub struct Session {
    store: AppStore,
    home: PathBuf,
    app_version: String,
    sync: SyncEnvironment,
    library: Mutex<Option<Arc<Library>>>,
    imports: Arc<Mutex<Imports>>,
    /// Export tasks of the open library (docs/spec.md §7.4 item 12).
    exports: Arc<Mutex<Exports>>,
    /// Views of the open library; dropped when it closes.
    views: Arc<Mutex<Views>>,
    /// How to take back the last item edits of the open library (docs/spec.md §6).
    undo: Mutex<UndoStack>,
    emit: Emit,
}

#[derive(Default)]
struct Imports {
    next_id: u64,
    running: HashMap<u64, Running>,
    finished: Vec<ImportProgress>,
}

#[derive(Default)]
struct Exports {
    next_id: u64,
    running: HashMap<u64, ExportRunning>,
}

struct ExportRunning {
    cancel: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<ExportProgress>>>,
    thread: Option<JoinHandle<()>>,
}

struct Running {
    kind: ImportKind,
    cancel: Arc<AtomicBool>,
    decision: Arc<Decision>,
    latest: Arc<Mutex<Option<ImportProgress>>>,
    thread: Option<JoinHandle<()>>,
}

/// The answer to the duplicate question, handed from a command to the waiting import thread.
#[derive(Default)]
struct Decision {
    choice: Mutex<Option<DuplicateChoice>>,
    ready: Condvar,
}

impl Decision {
    fn answer(&self, choice: DuplicateChoice) {
        *lock(&self.choice) = Some(choice);
        self.ready.notify_all();
    }

    fn wait(&self, cancel: &AtomicBool) -> DuplicateChoice {
        let mut choice = lock(&self.choice);
        loop {
            if let Some(answer) = choice.take() {
                return answer;
            }
            if cancel.load(Ordering::Relaxed) {
                return DuplicateChoice::Cancel;
            }
            choice = self
                .ready
                .wait_timeout(choice, Duration::from_millis(200))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

struct Observer {
    emit: Emit,
    latest: Arc<Mutex<Option<ImportProgress>>>,
    decision: Arc<Decision>,
    cancel: Arc<AtomicBool>,
    /// The library the import writes to, for recomputing the views.
    source: NativeSource,
    refresher: Refresher,
}

impl ImportObserver for Observer {
    /// The views are recomputed first, so they show the new items once the UI hears "done".
    fn progress(&self, progress: &ImportProgress) {
        self.refresher.after(progress, &self.source);
        *lock(&self.latest) = Some(progress.clone());
        (self.emit)(Event::ImportProgress(progress.clone()));
    }

    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        self.decision.wait(&self.cancel)
    }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Session {
    pub fn new(
        store: AppStore,
        home: PathBuf,
        app_version: impl Into<String>,
        sync: SyncEnvironment,
        emit: Emit,
    ) -> Self {
        Self {
            store,
            home,
            app_version: app_version.into(),
            sync,
            library: Mutex::new(None),
            imports: Arc::new(Mutex::new(Imports::default())),
            exports: Arc::new(Mutex::new(Exports::default())),
            views: Arc::new(Mutex::new(Views::default())),
            undo: Mutex::new(UndoStack::default()),
            emit,
        }
    }

    pub fn store(&self) -> &AppStore {
        &self.store
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
        let root = create_library(&parent, name, &self.app_version)?;
        self.open_library(&root.to_string_lossy())
    }

    /// Opens a library folder. The library that is open stays open until this one has opened;
    /// then its imports stop and it closes (D-024). Opening the read-only library again
    /// retries its lock.
    pub fn open_library(&self, path: &str) -> Result<LibraryInfo, SourceError> {
        let path = PathBuf::from(path);
        let sync_folder = match check_location(&path, &self.sync) {
            Ok(()) => false,
            Err(LocationProblem::SyncFolder) => true,
            Err(problem) => return Err(SourceError::UnsupportedLocation(problem)),
        };
        let key = display_path(&path);
        if let Some(open) = self.current_library()
            && display_path(open.root()) == key
            && open.read_only_reason().is_none()
        {
            return Ok(open.info());
        }
        let options = OpenOptions {
            sync_folder,
            ..OpenOptions::new(self.app_version.clone())
        };
        let library = Arc::new(Library::open(Path::new(&key), &options)?);
        let info = library.info();
        self.stop_imports();
        // The previous library closes here, after the new one opened.
        let previous = lock(&self.library).replace(Arc::clone(&library));
        lock(&self.views).clear();
        lock(&self.undo).clear();
        drop(previous);
        if let Err(error) =
            self.store
                .remember_opened(&key, &info.name, info.id.as_deref(), now_ms())
        {
            tracing::warn!("cannot update recent libraries: {error}");
        }
        if !info.read_only
            && library
                .pending_previews(1)
                .is_ok_and(|next| !next.is_empty())
        {
            self.start(library, ImportKind::Previews, make_pending_previews);
        }
        Ok(info)
    }

    /// Close Library: imports stop, back to Welcome, and no reopen on the next start.
    pub fn close_library(&self) -> Result<(), SourceError> {
        if let Some(library) = self.take_library() {
            library.close()?;
        }
        self.store.set_closed_explicitly(true)
    }

    pub fn current(&self) -> Option<LibraryInfo> {
        self.current_library().map(|library| library.info())
    }

    pub fn counts(&self) -> Result<LibraryCounts, SourceError> {
        self.current_library().ok_or_else(no_library)?.counts()
    }

    /// Opens a view of the open library for `query` (docs/architecture.md §6.5).
    pub fn open_view(
        &self,
        query: LibraryQuery,
        progress: Progress<'_>,
    ) -> Result<ViewOpened, SourceError> {
        let source = self.source()?;
        let mut views = lock(&self.views);
        let view = views.open(query, &Blocking(&source), progress)?;
        Ok(ViewOpened {
            view_id: view.id(),
            total: count(view.total()),
        })
    }

    /// At most `MAX_WINDOW` cells of view `view_id`, from `start`.
    pub fn view_range(
        &self,
        view_id: u64,
        start: u32,
        len: u32,
    ) -> Result<Vec<ItemCell>, SourceError> {
        let mut views = lock(&self.views);
        let view = views.get(view_id).ok_or(SourceError::NotFound)?;
        Ok(view.range(start as usize, len as usize))
    }

    /// The ids of positions `start..start + len` of a view, for selections.
    pub fn view_ids(&self, view_id: u64, start: u32, len: u32) -> Result<Vec<ItemId>, SourceError> {
        let mut views = lock(&self.views);
        let view = views.get(view_id).ok_or(SourceError::NotFound)?;
        Ok(view.ids(start as usize, len as usize))
    }

    /// Width and height of every position in the view, for justified packing (D-067).
    pub fn view_dimensions(&self, view_id: u64) -> Result<Vec<[u32; 2]>, SourceError> {
        let mut views = lock(&self.views);
        let view = views.get(view_id).ok_or(SourceError::NotFound)?;
        Ok(view.dimensions())
    }

    pub fn close_view(&self, view_id: u64) {
        lock(&self.views).close(view_id);
    }

    /// The full item for the details panel.
    pub fn item_detail(&self, id: &str) -> Result<Option<Item>, SourceError> {
        ready(self.source()?.item(&ItemId(id.to_owned())))
    }

    /// What the `thumb` scheme serves for a URL path: a thumbnail or an original of the open
    /// library, or nothing (404).
    pub fn image(&self, path: &str) -> Option<Thumbnail> {
        let request = ImageRequest::parse(path)?;
        let source = self.source().ok()?;
        match request.read(&source) {
            Ok(image) => image,
            Err(error) => {
                tracing::debug!("no image for {path}: {error}");
                None
            }
        }
    }

    pub fn recent_libraries(&self) -> Vec<RecentLibrary> {
        self.store.recent_libraries()
    }

    pub fn forget_recent_library(&self, path: &str) -> Result<(), SourceError> {
        self.store.forget_recent(path)
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
                drop(self.take_library());
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
        let library = self.writable_library()?;
        let target = self.import_target(&library, folder)?;
        let sources: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let id = self.start(
            library,
            ImportKind::Files,
            move |library, id, observer, cancel| {
                import_files(
                    library,
                    id,
                    &sources,
                    &target,
                    &filter,
                    DuplicatePolicy::Ask,
                    observer,
                    cancel,
                )
            },
        );
        Ok(ImportTicket { id })
    }

    /// Walks `path` for a reviewed import and answers what is there; refuses like the import
    /// (a read-only library, or a folder already inside this one).
    pub fn preview_import(&self, path: &str) -> Result<ImportPreview, SourceError> {
        self.writable_library()?;
        ready(self.source()?.preview_import(Path::new(path)))
    }

    fn import_target(
        &self,
        library: &Library,
        folder: Option<String>,
    ) -> Result<ImportTarget, SourceError> {
        if let Some(folder) = &folder
            && !library.has_folder(folder)?
        {
            return Err(SourceError::Invalid("The folder no longer exists.".into()));
        }
        Ok(ImportTarget { folder })
    }

    /// The folder tree with counts.
    pub fn folder_tree(&self) -> Result<Vec<Folder>, SourceError> {
        ready(self.source()?.folder_tree())
    }

    /// Creates, edits or moves a folder; returns the new tree.
    pub fn save_folder(&self, change: FolderChange) -> Result<Vec<Folder>, SourceError> {
        let tree = ready(self.source()?.save_folder(change))?;
        self.refresh_views();
        Ok(tree)
    }

    /// Deletes a folder and its subfolders; returns the new tree.
    pub fn delete_folder(&self, id: &str, trash_items: bool) -> Result<Vec<Folder>, SourceError> {
        let tree = ready(self.source()?.delete_folder(id, trash_items))?;
        self.refresh_views();
        Ok(tree)
    }

    /// Edits one item; `Ctrl+Z` can take the edit back.
    pub fn apply_item_edit(&self, id: &str, edit: ItemEdit) -> Result<Item, SourceError> {
        let ids = [ItemId(id.to_owned())];
        let before = self.writable_library()?.item_states(&ids)?;
        let item = ready(self.source()?.apply_item_edit(&ids[0], edit.clone()))?;
        lock(&self.undo).push(&edit, &before);
        self.refresh_views();
        Ok(item)
    }

    /// One edit for a selection; returns how many items changed. `Ctrl+Z` can take it back.
    pub fn apply_batch_edit(
        &self,
        target: ItemsTarget,
        edit: ItemEdit,
    ) -> Result<u32, SourceError> {
        let ids = self.target_ids(target)?;
        let before = self.writable_library()?.item_states(&ids)?;
        let edited = ready(self.source()?.apply_batch_edit(&ids, edit.clone()))?;
        lock(&self.undo).push(&edit, &before);
        self.refresh_views();
        Ok(edited)
    }

    /// The ids of a selection: listed, or a whole view except some.
    fn target_ids(&self, target: ItemsTarget) -> Result<Vec<ItemId>, SourceError> {
        if !target.all {
            return Ok(target.ids);
        }
        let view_id = target
            .view_id
            .ok_or_else(|| SourceError::Invalid("Select All needs its view.".into()))?;
        let mut views = lock(&self.views);
        let view = views.get(view_id).ok_or(SourceError::NotFound)?;
        let except: HashSet<&ItemId> = target.ids.iter().collect();
        Ok(view
            .ids(0, view.total())
            .into_iter()
            .filter(|id| !except.contains(id))
            .collect())
    }

    /// `Ctrl+Z`: applies the inverse of the last item edit as new edits. Returns what was taken
    /// back, or `None` when there is nothing to undo.
    pub fn undo(&self) -> Result<Option<UndoStatus>, SourceError> {
        let source = self.source()?;
        let Some(entry) = lock(&self.undo).pop() else {
            return Ok(None);
        };
        let mut result = Ok(Some(entry.status()));
        for step in entry.steps {
            if let Err(error) = ready(source.apply_batch_edit(&step.ids, step.edit)) {
                result = Err(error);
                break;
            }
        }
        self.refresh_views();
        result
    }

    /// What `Ctrl+Z` would take back, for the Edit menu.
    pub fn undo_status(&self) -> Option<UndoStatus> {
        lock(&self.undo).status()
    }

    /// Delete Permanently: the selected items of the trash, with their files. No undo.
    pub fn delete_permanently(&self, target: ItemsTarget) -> Result<u32, SourceError> {
        let ids = self.target_ids(target)?;
        let removed = ready(self.source()?.delete_items(&ids))?;
        self.refresh_views();
        Ok(removed)
    }

    /// Empty Trash, with the files. No undo.
    pub fn empty_trash(&self) -> Result<u32, SourceError> {
        let removed = ready(self.source()?.empty_trash())?;
        self.refresh_views();
        Ok(removed)
    }

    /// Items and bytes of the trash, for the context menu of its row.
    pub fn trash_info(&self) -> Result<TrashInfo, SourceError> {
        ready(self.source()?.trash_info())
    }

    /// Restore All of the trash context menu: every visible trash item comes back with its
    /// folders and tags. `Ctrl+Z` can take it back.
    pub fn restore_trash(&self) -> Result<u32, SourceError> {
        let source = self.source()?;
        let ids = ready(source.trash_ids())?;
        if ids.is_empty() {
            return Ok(0);
        }
        let before = self.writable_library()?.item_states(&ids)?;
        let restored = ready(source.apply_batch_edit(&ids, ItemEdit::Restore))?;
        lock(&self.undo).push(&ItemEdit::Restore, &before);
        self.refresh_views();
        Ok(restored)
    }

    /// The originals of the selected items, for Open, Show in Explorer, Copy File and dragging
    /// out. Refused for an item of a locked folder: its file is encrypted.
    pub fn original_paths(&self, ids: &[ItemId]) -> Result<Vec<PathBuf>, SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        ids.iter().map(|id| library.original_path(&id.0)).collect()
    }

    /// The image under the cursor while an item is dragged out: its thumbnail, small.
    pub fn drag_image(&self, id: &ItemId) -> Option<Vec<u8>> {
        let thumbnail = self.current_library()?.thumbnail(&id.0).ok()??;
        media::drag_image(&thumbnail.bytes, 96).ok()
    }

    /// The library folder, for Library → Show in Explorer.
    pub fn library_folder(&self) -> Result<PathBuf, SourceError> {
        Ok(self
            .current_library()
            .ok_or_else(no_library)?
            .root()
            .to_path_buf())
    }

    pub fn backups(&self) -> Result<Vec<Backup>, SourceError> {
        self.current_library().ok_or_else(no_library)?.backups()
    }

    pub fn back_up_now(&self) -> Result<Backup, SourceError> {
        self.writable_library()?.back_up_now()
    }

    /// Restores a backup into the open library. Imports stop first; views, undo and unlocked
    /// folders start over, as after opening the library.
    pub fn restore_backup(&self, name: &str) -> Result<LibraryInfo, SourceError> {
        let library = self.writable_library()?;
        self.stop_imports();
        library.restore_backup(name)?;
        lock(&self.views).clear();
        lock(&self.undo).clear();
        if library
            .pending_previews(1)
            .is_ok_and(|next| !next.is_empty())
        {
            self.start(
                Arc::clone(&library),
                ImportKind::Previews,
                make_pending_previews,
            );
        }
        Ok(library.info())
    }

    pub fn rename_library(&self, name: &str) -> Result<LibraryInfo, SourceError> {
        let library = self.writable_library()?;
        library.rename_library(name)?;
        let info = library.info();
        if let Some(path) = &info.path
            && let Err(error) =
                self.store
                    .remember_opened(path, &info.name, info.id.as_deref(), now_ms())
        {
            tracing::warn!("cannot update recent libraries: {error}");
        }
        Ok(info)
    }

    pub fn check_library(&self) -> Result<LibraryCheck, SourceError> {
        self.current_library()
            .ok_or_else(no_library)?
            .check_library()
    }

    pub fn repair_library(&self, options: RepairOptions) -> Result<LibraryCheck, SourceError> {
        let check = self.writable_library()?.repair_library(options)?;
        self.refresh_views();
        Ok(check)
    }

    pub fn library_stats(&self) -> Result<LibraryStats, SourceError> {
        self.current_library().ok_or_else(no_library)?.stats()
    }

    /// What an export of `selection` would write, from the database alone (docs/spec.md §7.4 item 12).
    /// A read-only library exports fine.
    pub fn export_preview(
        &self,
        selection: ExportSelection,
        include_subfolders: bool,
    ) -> Result<ExportPreview, SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        export_preview(&library, &selection, include_subfolders)
    }

    /// Starts an export in the background and returns its first progress.
    pub fn start_export(&self, request: ExportRequest) -> Result<ExportProgress, SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        let mut exports = lock(&self.exports);
        exports.next_id += 1;
        let id = exports.next_id;
        let cancel = Arc::new(AtomicBool::new(false));
        let latest = Arc::new(Mutex::new(None));
        let emit = Arc::clone(&self.emit);
        let progress_latest = Arc::clone(&latest);
        let registry = Arc::clone(&self.exports);
        let task_cancel = Arc::clone(&cancel);
        // The thread waits for the registry lock that is held here, so it is registered before
        // it can remove itself.
        let thread = std::thread::Builder::new()
            .name(format!("export-{id}"))
            .spawn(move || {
                let mut on_progress = |progress: ExportProgress| {
                    *lock(&progress_latest) = Some(progress.clone());
                    emit(Event::ExportProgress(progress));
                };
                let result = export_items(&library, id, &request, &task_cancel, &mut on_progress);
                drop(on_progress);
                if let Err(error) = result {
                    // The engine cleans up after itself; the UI just hears that the run failed.
                    let latest = lock(&progress_latest);
                    let at = |take: fn(&ExportProgress) -> u64| latest.as_ref().map_or(0, take);
                    let failed = ExportProgress {
                        id,
                        state: ExportState::Failed,
                        total: at(|progress| progress.total),
                        done: at(|progress| progress.done),
                        bytes_total: at(|progress| progress.bytes_total),
                        bytes_done: at(|progress| progress.bytes_done),
                        current: None,
                        summary: None,
                        error: Some(error.to_string()),
                    };
                    drop(latest);
                    *lock(&progress_latest) = Some(failed.clone());
                    emit(Event::ExportProgress(failed));
                }
                drop(library);
                let mut exports = lock(&registry);
                exports.running.remove(&id);
            });
        match thread {
            Ok(thread) => {
                exports.running.insert(
                    id,
                    ExportRunning {
                        cancel,
                        latest,
                        thread: Some(thread),
                    },
                );
            }
            Err(error) => tracing::warn!("cannot start an export: {error}"),
        }
        let first = exports
            .running
            .get(&id)
            .and_then(|running| lock(&running.latest).clone());
        Ok(first.unwrap_or(ExportProgress {
            id,
            state: ExportState::Preparing,
            total: 0,
            done: 0,
            bytes_total: 0,
            bytes_done: 0,
            current: None,
            summary: None,
            error: None,
        }))
    }

    /// Export tasks for the dialog to catch up with after it opens.
    pub fn exports(&self) -> Vec<ExportProgress> {
        let exports = lock(&self.exports);
        let mut running: Vec<ExportProgress> = exports
            .running
            .values()
            .filter_map(|running| lock(&running.latest).clone())
            .collect();
        running.sort_by_key(|progress| std::cmp::Reverse(progress.id));
        running
    }

    /// An export is still running (its final event not sent yet).
    pub fn has_running_exports(&self) -> bool {
        lock(&self.exports).running.values().any(|running| {
            !lock(&running.latest).as_ref().is_some_and(|progress| {
                matches!(
                    progress.state,
                    ExportState::Done | ExportState::Failed | ExportState::Cancelled
                )
            })
        })
    }

    /// Asks a running export to stop; its partial output is removed by the engine.
    pub fn cancel_export(&self, id: u64) -> Result<(), SourceError> {
        let cancel = lock(&self.exports)
            .running
            .get(&id)
            .map(|running| Arc::clone(&running.cancel));
        match cancel {
            Some(cancel) => {
                cancel.store(true, Ordering::Relaxed);
                Ok(())
            }
            None => Err(SourceError::NotFound),
        }
    }

    /// Cancels every export and waits for its thread; the partial output is removed.
    fn stop_exports(&self) {
        let threads: Vec<JoinHandle<()>> = {
            let mut exports = lock(&self.exports);
            exports
                .running
                .values_mut()
                .filter_map(|running| {
                    running.cancel.store(true, Ordering::Relaxed);
                    running.thread.take()
                })
                .collect()
        };
        for thread in threads {
            if thread.join().is_err() {
                tracing::warn!("an export thread panicked");
            }
        }
    }

    /// Rebuild Thumbnails: every preview is made again by the preview task. Returns how many
    /// items wait.
    pub fn rebuild_thumbnails(&self) -> Result<u32, SourceError> {
        let library = self.writable_library()?;
        let waiting = library.rebuild_previews()?;
        self.refresh_views();
        let previewing = lock(&self.imports)
            .running
            .values()
            .any(|running| running.kind == ImportKind::Previews);
        if waiting > 0 && !previewing {
            self.start(library, ImportKind::Previews, make_pending_previews);
        }
        Ok(waiting)
    }

    /// The tag vocabulary with item counts (trash excluded).
    pub fn tag_vocabulary(&self) -> Result<Vec<Tag>, SourceError> {
        ready(self.source()?.tags())
    }

    pub fn tag_groups(&self) -> Result<Vec<TagGroup>, SourceError> {
        ready(self.source()?.tag_groups())
    }

    /// Creates or changes a tag group; returns every group. Item tags do not change.
    pub fn save_tag_group(&self, change: TagGroupChange) -> Result<Vec<TagGroup>, SourceError> {
        ready(self.source()?.save_tag_group(change))
    }

    pub fn delete_tag_group(&self, id: &str) -> Result<Vec<TagGroup>, SourceError> {
        ready(self.source()?.delete_tag_group(id))
    }

    /// Renames a tag everywhere; onto an existing name, the two merge.
    pub fn rename_tag(&self, from: &str, to: &str) -> Result<(), SourceError> {
        ready(self.source()?.rename_tag(from, to))?;
        self.refresh_views();
        Ok(())
    }

    pub fn merge_tags(&self, source: &str, target: &str) -> Result<(), SourceError> {
        ready(self.source()?.merge_tags(source, target))?;
        self.refresh_views();
        Ok(())
    }

    /// Removes a tag from every item.
    pub fn delete_tag(&self, name: &str) -> Result<(), SourceError> {
        ready(self.source()?.delete_tag(name))?;
        self.refresh_views();
        Ok(())
    }

    /// The tags of a selection, each with the number of selected items that carry it, read
    /// from the view so that Select All needs no id list.
    pub fn selection_tags(&self, target: ItemsTarget) -> Result<SelectionTags, SourceError> {
        let view_id = target
            .view_id
            .ok_or_else(|| SourceError::Invalid("The selection needs its view.".into()))?;
        let mut views = lock(&self.views);
        let view = views.get(view_id).ok_or(SourceError::NotFound)?;
        let ids = target.ids.into_iter().collect();
        Ok(view.selection_tags(target.all, &ids))
    }

    /// The smart folder tree, each with the rules this build cannot evaluate.
    pub fn smart_folders(&self) -> Result<Vec<SmartFolder>, SourceError> {
        ready(self.source()?.smart_folders())
    }

    /// Creates or edits a smart folder; returns the tree. Open views of it follow.
    pub fn save_smart_folder(
        &self,
        change: SmartFolderChange,
    ) -> Result<Vec<SmartFolder>, SourceError> {
        let tree = ready(self.source()?.save_smart_folder(change))?;
        self.refresh_views();
        Ok(tree)
    }

    pub fn delete_smart_folder(&self, id: &str) -> Result<Vec<SmartFolder>, SourceError> {
        let tree = ready(self.source()?.delete_smart_folder(id))?;
        self.refresh_views();
        Ok(tree)
    }

    /// The rules for "Save as smart folder"; refused with the reason for what rules cannot hold
    /// (search text, the trash, nothing to keep).
    pub fn smart_rules(&self, query: &LibraryQuery) -> Result<SmartDraft, SourceError> {
        let folders = FolderIndex::new(&ready(self.source()?.folder_tree())?);
        let (parent, conditions) = rules_from_query(query, &folders)?;
        Ok(SmartDraft { parent, conditions })
    }

    /// File types with their item counts, for the Type filter.
    pub fn extensions(&self) -> Result<Vec<ExtensionCount>, SourceError> {
        ready(self.source()?.extensions())
    }

    /// Manual order inside `folder`.
    pub fn reorder_items(
        &self,
        folder: &str,
        ids: Vec<ItemId>,
        before: Option<ItemId>,
    ) -> Result<(), SourceError> {
        ready(self.source()?.reorder_items(folder, &ids, before.as_ref()))?;
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
        let library = self.writable_library()?;
        let key = library.lock_folder(id, password, label, label_color, progress);
        self.refresh_views();
        key
    }

    /// Opens a locked folder for this session; read-only instances may too.
    pub fn unlock_folder(&self, id: &str, password: String) -> Result<(), SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        library.unlock_folder(id, password)?;
        self.after_unlock(library);
        Ok(())
    }

    /// YubiKeys bound to one locked folder. Nothing secret is in the list.
    pub fn piv_wraps(&self, id: &str) -> Result<Vec<PivWrap>, SourceError> {
        self.current_library().ok_or_else(no_library)?.piv_wraps(id)
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
        self.writable_library()?.bind_piv(id, password, &card)
    }

    /// Drops one card's wrap. The password still opens the folder.
    pub fn unbind_yubikey(
        &self,
        id: &str,
        password: String,
        serial: String,
    ) -> Result<(), SourceError> {
        self.writable_library()?.unbind_piv(id, password, &serial)
    }

    /// Opens one folder with its YubiKey. A missing card does not lock the others out.
    pub fn unlock_with_yubikey(
        &self,
        id: &str,
        serial: String,
        prompt: PivPrompt,
    ) -> Result<(), SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        let prompt_for_card = prompt.clone();
        library.unlock_with_piv(id, &serial, &prompt, |slot, ephemeral| {
            yubikey::agree(&serial, slot, &prompt_for_card, ephemeral)
        })?;
        self.after_unlock(library);
        Ok(())
    }

    pub fn login_status(&self) -> LoginStatus {
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
        let library = self.writable_library()?;
        library.recover_folder(id, recovery, new_password)?;
        self.after_unlock(library);
        Ok(())
    }

    /// Locks an open folder again: its key leaves memory and its items leave the views.
    pub fn relock_folder(&self, id: &str) -> Result<(), SourceError> {
        self.current_library()
            .ok_or_else(no_library)?
            .relock_folder(id);
        self.refresh_views();
        Ok(())
    }

    pub fn change_folder_password(
        &self,
        id: &str,
        current: String,
        new_password: String,
    ) -> Result<(), SourceError> {
        self.writable_library()?
            .change_folder_password(id, current, new_password)
    }

    /// A new recovery key; the old one stops working. Shown once.
    pub fn renew_recovery_key(
        &self,
        id: &str,
        current: String,
    ) -> Result<RecoveryKey, SourceError> {
        self.writable_library()?.renew_recovery_key(id, current)
    }

    /// Takes the lock off and decrypts the folder (`progress(done, total)`).
    pub fn remove_folder_lock(
        &self,
        id: &str,
        password: String,
        progress: &dyn Fn(u32, u32),
    ) -> Result<(), SourceError> {
        let library = self.writable_library()?;
        let removed = library.remove_folder_lock(id, password, progress);
        self.refresh_views();
        removed
    }

    pub fn set_lock_label(
        &self,
        id: &str,
        label: Option<String>,
        label_color: Option<String>,
    ) -> Result<(), SourceError> {
        self.writable_library()?
            .set_lock_label(id, label, label_color)
    }

    /// Writes the recovery key to a text file the user chose, outside the library folder: the
    /// library never holds it (docs/age-locked-folders.md §7).
    pub fn save_recovery_file(
        &self,
        path: &str,
        folder: &str,
        key: &RecoveryKey,
    ) -> Result<(), SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        let path = PathBuf::from(path);
        let inside = |root: &Path| {
            let parent = path.parent().and_then(|parent| parent.canonicalize().ok());
            let root = root.canonicalize().ok();
            matches!((parent, root), (Some(parent), Some(root)) if parent.starts_with(&root))
        };
        if inside(library.root()) {
            return Err(SourceError::Invalid(
                "Save the recovery key outside the library folder.".into(),
            ));
        }
        let text = format!(
            "egl-viewer recovery key\r\n\r\nLibrary: {}\r\nLocked folder: {folder}\r\n\r\n\
             Key: {}\r\nPhrase: {}\r\n\r\nThe key or the phrase opens this folder when its \
             password is lost. Anyone who has them can open it: keep this file safe and away from \
             the library.\r\n",
            library.info().name,
            key.key,
            key.phrase,
        );
        std::fs::write(&path, text)
            .map_err(|error| SourceError::Storage(format!("save the recovery key: {error}")))
    }

    /// The views show the folder; previews that waited for its key are made now.
    fn after_unlock(&self, library: Arc<Library>) {
        self.refresh_views();
        let previewing = lock(&self.imports)
            .running
            .values()
            .any(|running| running.kind == ImportKind::Previews);
        if library.read_only_reason().is_none()
            && !previewing
            && library
                .pending_previews(1)
                .is_ok_and(|next| !next.is_empty())
        {
            self.start(library, ImportKind::Previews, make_pending_previews);
        }
    }

    /// Recomputes the open views after a write and tells the UI.
    fn refresh_views(&self) {
        let Ok(source) = self.source() else {
            return;
        };
        let totals = lock(&self.views).refresh(&Blocking(&source));
        match totals.map(invalidated) {
            Ok(Some(event)) => (self.emit)(event),
            Ok(None) => {}
            Err(error) => tracing::warn!("cannot refresh the views: {error}"),
        }
    }

    /// Adds what is on the clipboard: copied files, or a pasted image as
    /// `Pasted YYYY-MM-DD HH-MM-SS.png` (UTC). `None` when there is nothing to add.
    pub fn paste(
        &self,
        clipboard: &dyn Clipboard,
        folder: Option<String>,
    ) -> Result<Option<ImportTicket>, SourceError> {
        let library = self.writable_library()?;
        let target = self.import_target(&library, folder.clone())?;
        if let Some(files) = clipboard.files().filter(|files| !files.is_empty()) {
            let paths = files
                .iter()
                .map(|file| file.to_string_lossy().into_owned())
                .collect();
            return self.add_files(paths, folder).map(Some);
        }
        let Some((width, height, rgba)) = clipboard.image() else {
            return Ok(None);
        };
        let png = media::png_from_rgba(width, height, rgba)
            .map_err(|error| SourceError::Invalid(error.to_string()))?;
        let stamp = utc_file_stamp(now_ms());
        let dir = std::env::temp_dir().join(format!("egl-viewer-paste-{}", now_ms()));
        let file = dir.join(format!("Pasted {stamp}.png"));
        std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&file, png))
            .map_err(|error| SourceError::Storage(format!("keep the pasted image: {error}")))?;
        let library = self.writable_library()?;
        let id = self.start(
            library,
            ImportKind::Files,
            move |library, id, observer, cancel| {
                let done = import_files(
                    library,
                    id,
                    &[file],
                    &target,
                    &ImportFilter::default(),
                    DuplicatePolicy::Ask,
                    observer,
                    cancel,
                );
                let _ = std::fs::remove_dir_all(&dir);
                done
            },
        );
        Ok(Some(ImportTicket { id }))
    }

    /// The answer to the duplicate question of import `id`.
    pub fn answer_import(&self, id: u64, choice: DuplicateChoice) -> Result<(), SourceError> {
        let imports = lock(&self.imports);
        let running = imports.running.get(&id).ok_or(SourceError::NotFound)?;
        running.decision.answer(choice);
        Ok(())
    }

    /// Stops import `id` after the current file; added files stay.
    pub fn cancel_import(&self, id: u64) -> Result<(), SourceError> {
        let imports = lock(&self.imports);
        let running = imports.running.get(&id).ok_or(SourceError::NotFound)?;
        running.cancel.store(true, Ordering::Relaxed);
        running.decision.answer(DuplicateChoice::Cancel);
        Ok(())
    }

    /// Running imports first, then finished ones, newest first.
    pub fn imports(&self) -> Vec<ImportProgress> {
        let imports = lock(&self.imports);
        let mut running: Vec<ImportProgress> = imports
            .running
            .values()
            .filter_map(|running| lock(&running.latest).clone())
            .collect();
        running.sort_by_key(|progress| std::cmp::Reverse(progress.id));
        running.extend(imports.finished.iter().cloned());
        running
    }

    /// Files are being added (previews made after a restart do not count: they resume later).
    /// A task that sent its final progress counts as over even before its thread has ended.
    pub fn has_running_imports(&self) -> bool {
        lock(&self.imports).running.values().any(|running| {
            running.kind == ImportKind::Files
                && !lock(&running.latest).as_ref().is_some_and(|progress| {
                    matches!(progress.state, ImportState::Done | ImportState::Cancelled)
                })
        })
    }

    /// Quitting: imports stop and the library closes without being marked closed explicitly.
    pub fn shutdown(&self) {
        if let Some(library) = self.take_library()
            && let Err(error) = library.close()
        {
            tracing::warn!("closing the library on exit failed: {error}");
        }
    }

    /// Cancels every import and waits for their threads; their added files stay.
    pub fn stop_imports(&self) {
        let threads: Vec<(Arc<AtomicBool>, Arc<Decision>, JoinHandle<()>)> = {
            let mut imports = lock(&self.imports);
            imports
                .running
                .values_mut()
                .filter_map(|running| {
                    let thread = running.thread.take()?;
                    Some((
                        Arc::clone(&running.cancel),
                        Arc::clone(&running.decision),
                        thread,
                    ))
                })
                .collect()
        };
        for (cancel, decision, _) in &threads {
            cancel.store(true, Ordering::Relaxed);
            decision.answer(DuplicateChoice::Cancel);
        }
        for (_, _, thread) in threads {
            if thread.join().is_err() {
                tracing::warn!("an import thread panicked");
            }
        }
    }

    fn start<F>(&self, library: Arc<Library>, kind: ImportKind, work: F) -> u64
    where
        F: FnOnce(&Library, u64, &dyn ImportObserver, &AtomicBool) -> ImportProgress
            + Send
            + 'static,
    {
        let mut imports = lock(&self.imports);
        imports.next_id += 1;
        let id = imports.next_id;
        let cancel = Arc::new(AtomicBool::new(false));
        let decision = Arc::new(Decision::default());
        let latest = Arc::new(Mutex::new(None));
        let observer = Observer {
            emit: Arc::clone(&self.emit),
            latest: Arc::clone(&latest),
            decision: Arc::clone(&decision),
            cancel: Arc::clone(&cancel),
            source: NativeSource::new(Arc::clone(&library)),
            refresher: Refresher::new(Arc::clone(&self.views), Arc::clone(&self.emit)),
        };
        let registry = Arc::clone(&self.imports);
        let task_cancel = Arc::clone(&cancel);
        // The thread waits for the registry lock that is held here, so it is registered
        // before it can remove itself.
        let thread = std::thread::Builder::new()
            .name(format!("import-{id}"))
            .spawn(move || {
                let done = work(&library, id, &observer, &task_cancel);
                drop(observer);
                drop(library);
                let mut imports = lock(&registry);
                imports.running.remove(&id);
                imports.finished.insert(0, done);
                imports.finished.truncate(FINISHED_KEPT);
            });
        match thread {
            Ok(thread) => {
                imports.running.insert(
                    id,
                    Running {
                        kind,
                        cancel,
                        decision,
                        latest,
                        thread: Some(thread),
                    },
                );
            }
            Err(error) => tracing::warn!("cannot start an import: {error}"),
        }
        id
    }

    fn writable_library(&self) -> Result<Arc<Library>, SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        if library.read_only_reason().is_some() {
            return Err(SourceError::ReadOnly);
        }
        Ok(library)
    }

    fn current_library(&self) -> Option<Arc<Library>> {
        lock(&self.library).clone()
    }

    fn source(&self) -> Result<NativeSource, SourceError> {
        Ok(NativeSource::new(
            self.current_library().ok_or_else(no_library)?,
        ))
    }

    /// Stops imports and exports and takes the library out of the session.
    fn take_library(&self) -> Option<Library> {
        self.stop_imports();
        self.stop_exports();
        lock(&self.views).clear();
        lock(&self.undo).clear();
        let library = lock(&self.library).take()?;
        // Import and export threads hold their clones only while they run.
        Arc::try_unwrap(library).ok()
    }
}

fn no_library() -> SourceError {
    SourceError::Invalid("Open a library first.".into())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use library_core::TagMode;

    use super::*;

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
        let session = Session::new(
            AppStore::new(data.path()),
            home.path().to_path_buf(),
            "0.1.0-test",
            sync,
            Arc::new(|_: Event| {}),
        );
        Fixture {
            _data: data,
            home,
            session,
        }
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
                Event::ImportProgress(_) => None,
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
        assert_eq!(f.session.image("/01ANYTHING"), None, "no library");
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

        let thumbnail = f.session.image(&format!("/{id}")).unwrap();
        assert_eq!(thumbnail.content_type, "image/webp");
        let original = f.session.image(&format!("/{id}/original")).unwrap();
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
            assert_eq!(f.session.image(path), None, "{path}");
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
                Event::ImportProgress(_) => None,
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
                .image(&format!("/{}", first[0].0))
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
        assert!(f.session.image(&format!("/{}", first[0].0)).is_none());
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
        let image = f.session.image(&format!("/{}", first[0].0)).unwrap();
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
        assert_eq!(f.session.empty_trash().unwrap(), 1);
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
        f.session.empty_trash().unwrap();
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
