//! [`LibraryService`]: the open native library with its views, undo stack and tasks
//! (docs/architecture.md §6.5, §7.4, §9, D-024, D-091).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use library_core::{
    Backup, DuplicateNotice, ExportPreview, ExportProgress, ExportRequest, ExportSelection,
    ExportState, ExtensionCount, Folder, FolderChange, FolderIndex, ImportKind, ImportPreview,
    ImportProgress, ImportTicket, Item, ItemCell, ItemEdit, ItemId, LibraryCheck, LibraryCounts,
    LibraryInfo, LibraryQuery, LibrarySource, LibraryStats, PivPrompt, PivWrap, Progress,
    RecoveryKey, RepairOptions, SelectionTags, SmartFolder, SmartFolderChange, SourceError, Tag,
    TagGroup, TagGroupChange, Thumbnail, TrashInfo, UndoStack, UndoStatus, View, Views,
    rules_from_query,
};
use library_native::{
    DuplicateChoice, DuplicatePolicy, ImportFilter, ImportObserver, ImportTarget, Library,
    NativeSource, OpenOptions, PivCard, create_library, decrypt_age_archive, export_items,
    export_preview, import_files, make_pending_previews, utc_file_stamp,
};
use serde::Serialize;
use ts_rs::TS;

use crate::events::{Emit, ItemsTarget, ViewOpened, count, invalidated};
use crate::source::{Blocking, ImageRequest, Refresher, ready};
use crate::tasks::{ExportTasks, ImportTasks, TaskContext};
use crate::{lock, now_ms};

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

/// The refusal for a command that needs an open library.
pub fn no_library() -> SourceError {
    SourceError::Invalid("Open a library first.".into())
}

/// The open native library as an app uses it. At most one library is open; opening another
/// keeps the first open until the second has opened (D-024).
pub struct LibraryService {
    app_version: String,
    library: Mutex<Option<Arc<Library>>>,
    /// Views of the open source; dropped when it closes. An app that shows another source (a
    /// running Eagle) keeps its views here too, so view ids stay unique.
    views: Arc<Mutex<Views>>,
    /// How to take back the last item edits of the open library (docs/spec.md §6).
    undo: Mutex<UndoStack>,
    /// Set by `cancel_empty_trash` while Empty Trash runs (D-074).
    empty_trash_cancel: AtomicBool,
    /// Set by `cancel_decrypt_age` while an in-app decrypt of a `.zip.age` runs (D-102).
    decrypt_age_cancel: AtomicBool,
    imports: ImportTasks,
    /// Export tasks of the open library (docs/spec.md §7.4 item 12).
    exports: ExportTasks,
    emit: Emit,
}

/// Hands import progress to the app and recomputes the views while the import writes.
struct Observer<'a> {
    context: &'a TaskContext,
    source: NativeSource,
    refresher: Refresher,
}

impl ImportObserver for Observer<'_> {
    /// The views are recomputed first, so they show the new items once the app hears "done".
    fn progress(&self, progress: &ImportProgress) {
        self.refresher.after(progress, &self.source);
        self.context.report(progress);
    }

    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        self.context.decide()
    }
}

impl LibraryService {
    /// `app_version` is written into the libraries this service creates and opens.
    pub fn new(app_version: impl Into<String>, emit: Emit) -> Self {
        Self {
            app_version: app_version.into(),
            library: Mutex::new(None),
            views: Arc::new(Mutex::new(Views::default())),
            undo: Mutex::new(UndoStack::default()),
            empty_trash_cancel: AtomicBool::new(false),
            decrypt_age_cancel: AtomicBool::new(false),
            imports: ImportTasks::default(),
            exports: ExportTasks::default(),
            emit,
        }
    }

    pub fn app_version(&self) -> &str {
        &self.app_version
    }

    pub fn emit(&self) -> &Emit {
        &self.emit
    }

    pub fn views(&self) -> &Arc<Mutex<Views>> {
        &self.views
    }

    pub fn imports(&self) -> &ImportTasks {
        &self.imports
    }

    pub fn exports(&self) -> &ExportTasks {
        &self.exports
    }

    // --- The library ---

    /// Creates `name.eglib` in `parent` and opens it. The caller has checked the location.
    pub fn create_library(&self, parent: &Path, name: &str) -> Result<LibraryInfo, SourceError> {
        let root = create_library(parent, name, &self.app_version)?;
        self.open_library(&root, false)
    }

    /// Opens a library folder; `sync_folder` opens it read-only (D-021). The library that is open
    /// stays open until this one has opened; then its imports stop and it closes (D-024).
    /// Opening the open library again returns it, unless it is read-only: that retries its lock.
    pub fn open_library(&self, path: &Path, sync_folder: bool) -> Result<LibraryInfo, SourceError> {
        if let Some(open) = self.current_library()
            && open.root() == path
            && open.read_only_reason().is_none()
        {
            return Ok(open.info());
        }
        let options = OpenOptions {
            sync_folder,
            ..OpenOptions::new(self.app_version.clone())
        };
        let library = Arc::new(Library::open(path, &options)?);
        let info = library.info();
        self.imports.stop();
        // The previous library closes here, after the new one opened.
        let previous = lock(&self.library).replace(Arc::clone(&library));
        lock(&self.views).clear();
        lock(&self.undo).clear();
        drop(previous);
        if !info.read_only {
            self.start_previews_if_waiting(library);
        }
        Ok(info)
    }

    /// Stops imports and exports and takes the library out of the service. `None` when none is
    /// open, or when a thread still held it (it closes when that thread lets go).
    pub fn take_library(&self) -> Option<Library> {
        self.imports.stop();
        self.exports.stop();
        lock(&self.views).clear();
        lock(&self.undo).clear();
        let library = lock(&self.library).take()?;
        // Import and export threads hold their clones only while they run.
        Arc::try_unwrap(library).ok()
    }

    /// Closes the open library, if any.
    pub fn close_library(&self) -> Result<(), SourceError> {
        match self.take_library() {
            Some(library) => library.close(),
            None => Ok(()),
        }
    }

    pub fn current_library(&self) -> Option<Arc<Library>> {
        lock(&self.library).clone()
    }

    pub fn writable_library(&self) -> Result<Arc<Library>, SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        if library.read_only_reason().is_some() {
            return Err(SourceError::ReadOnly);
        }
        Ok(library)
    }

    pub fn source(&self) -> Result<NativeSource, SourceError> {
        Ok(NativeSource::new(
            self.current_library().ok_or_else(no_library)?,
        ))
    }

    pub fn info(&self) -> Option<LibraryInfo> {
        self.current_library().map(|library| library.info())
    }

    pub fn counts(&self) -> Result<LibraryCounts, SourceError> {
        self.current_library().ok_or_else(no_library)?.counts()
    }

    // --- Views ---

    /// Opens a view of `source` for `query` (docs/architecture.md §6.5).
    pub fn open_view_of(
        &self,
        source: &dyn LibrarySource,
        query: LibraryQuery,
        progress: Progress<'_>,
    ) -> Result<ViewOpened, SourceError> {
        let mut views = lock(&self.views);
        let view = views.open(query, &Blocking(source), progress)?;
        Ok(ViewOpened {
            view_id: view.id(),
            total: count(view.total()),
        })
    }

    /// Opens a view of the open library for `query`.
    pub fn open_view(
        &self,
        query: LibraryQuery,
        progress: Progress<'_>,
    ) -> Result<ViewOpened, SourceError> {
        let source = self.source()?;
        self.open_view_of(&source, query, progress)
    }

    /// Runs `read` on view `view_id`.
    pub fn with_view<T>(
        &self,
        view_id: u64,
        read: impl FnOnce(&View) -> T,
    ) -> Result<T, SourceError> {
        let mut views = lock(&self.views);
        let view = views.get(view_id).ok_or(SourceError::NotFound)?;
        Ok(read(view))
    }

    /// At most `MAX_WINDOW` cells of view `view_id`, from `start`.
    pub fn view_range(
        &self,
        view_id: u64,
        start: u32,
        len: u32,
    ) -> Result<Vec<ItemCell>, SourceError> {
        self.with_view(view_id, |view| view.range(start as usize, len as usize))
    }

    /// The ids of positions `start..start + len` of a view, for selections.
    pub fn view_ids(&self, view_id: u64, start: u32, len: u32) -> Result<Vec<ItemId>, SourceError> {
        self.with_view(view_id, |view| view.ids(start as usize, len as usize))
    }

    /// Width and height of every position in the view, for justified packing (D-067).
    pub fn view_dimensions(&self, view_id: u64) -> Result<Vec<[u32; 2]>, SourceError> {
        self.with_view(view_id, View::dimensions)
    }

    pub fn close_view(&self, view_id: u64) {
        lock(&self.views).close(view_id);
    }

    /// Recomputes the open views from `source` and tells the app their totals, each plus
    /// `extra` positions the app appends (a running Eagle's "Importing…" cells).
    pub fn refresh_views_of(&self, source: &dyn LibrarySource, extra: usize) {
        let totals = lock(&self.views).refresh(&Blocking(source));
        let totals = totals.map(|totals| {
            totals
                .into_iter()
                .map(|(view_id, total)| (view_id, total + extra))
                .collect()
        });
        match totals.map(invalidated) {
            Ok(Some(event)) => (self.emit)(event),
            Ok(None) => {}
            Err(error) => tracing::warn!("cannot refresh the views: {error}"),
        }
    }

    /// Recomputes the open views after a write to the open library and tells the app.
    pub fn refresh_views(&self) {
        if let Ok(source) = self.source() {
            self.refresh_views_of(&source, 0);
        }
    }

    /// The ids of a selection: listed, or a whole view except some.
    pub fn target_ids(&self, target: ItemsTarget) -> Result<Vec<ItemId>, SourceError> {
        if !target.all {
            return Ok(target.ids);
        }
        let view_id = target
            .view_id
            .ok_or_else(|| SourceError::Invalid("Select All needs its view.".into()))?;
        self.with_view(view_id, |view| {
            let except: HashSet<&ItemId> = target.ids.iter().collect();
            view.ids(0, view.total())
                .into_iter()
                .filter(|id| !except.contains(id))
                .collect()
        })
    }

    /// The tags of a selection, each with the number of selected items that carry it, read
    /// from the view so that Select All needs no id list.
    pub fn selection_tags(&self, target: ItemsTarget) -> Result<SelectionTags, SourceError> {
        let view_id = target
            .view_id
            .ok_or_else(|| SourceError::Invalid("The selection needs its view.".into()))?;
        let ids = target.ids.into_iter().collect();
        self.with_view(view_id, |view| view.selection_tags(target.all, &ids))
    }

    // --- Items ---

    /// The full item for the details panel.
    pub fn item(&self, id: &str) -> Result<Option<Item>, SourceError> {
        ready(self.source()?.item(&ItemId(id.to_owned())))
    }

    /// A thumbnail or an original of the open library for a `thumb` URL path, or nothing.
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

    /// Edits one item; undo can take the edit back.
    pub fn apply_item_edit(&self, id: &str, edit: ItemEdit) -> Result<Item, SourceError> {
        let ids = [ItemId(id.to_owned())];
        let before = self.writable_library()?.item_states(&ids)?;
        let item = ready(self.source()?.apply_item_edit(&ids[0], edit.clone()))?;
        lock(&self.undo).push(&edit, &before);
        self.refresh_views();
        Ok(item)
    }

    /// One edit for a selection; returns how many items changed. Undo can take it back.
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

    /// Applies the inverse of the last item edit as new edits. Returns what was taken back, or
    /// `None` when there is nothing to undo. A failed undo stays on the stack.
    pub fn undo(&self) -> Result<Option<UndoStatus>, SourceError> {
        let source = self.source()?;
        let Some(entry) = lock(&self.undo).pop() else {
            return Ok(None);
        };
        let mut result = Ok(Some(entry.status()));
        for step in &entry.steps {
            if let Err(error) = ready(source.apply_batch_edit(&step.ids, step.edit.clone())) {
                result = Err(error);
                break;
            }
        }
        if result.is_err() {
            lock(&self.undo).restore(entry);
        }
        self.refresh_views();
        result
    }

    /// What undo would take back, for the Edit menu.
    pub fn undo_status(&self) -> Option<UndoStatus> {
        lock(&self.undo).status()
    }

    /// The selected items of the trash go for good, with their files. No undo.
    pub fn delete_permanently(&self, target: ItemsTarget) -> Result<u32, SourceError> {
        let ids = self.target_ids(target)?;
        let removed = ready(self.source()?.delete_items(&ids))?;
        lock(&self.undo).forget(&ids);
        self.refresh_views();
        Ok(removed)
    }

    /// Empty Trash, with the files. No undo. `progress` is `(done, total)` over removing rows
    /// then deleting files; views refresh as soon as the rows are gone (D-074).
    pub fn empty_trash(&self, progress: &dyn Fn(u32, u32)) -> Result<u32, SourceError> {
        self.empty_trash_cancel.store(false, Ordering::SeqCst);
        let library = self.writable_library()?;
        let removed = library.empty_trash(progress, &self.empty_trash_cancel, &|| {
            self.refresh_views();
        })?;
        self.refresh_views();
        Ok(removed)
    }

    /// Stops a running Empty Trash between items. Rows already removed stay gone; remaining
    /// files stay in the journal and go on the next open.
    pub fn cancel_empty_trash(&self) {
        self.empty_trash_cancel.store(true, Ordering::SeqCst);
    }

    /// Items and bytes of the trash.
    pub fn trash_info(&self) -> Result<TrashInfo, SourceError> {
        ready(self.source()?.trash_info())
    }

    /// Restore All: every visible trash item comes back with its folders and tags. Undo can
    /// take it back.
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

    /// The originals of items, for opening, sharing and dragging out. Refused for an item of a
    /// locked folder: its file is encrypted.
    pub fn original_paths(&self, ids: &[ItemId]) -> Result<Vec<PathBuf>, SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        ids.iter().map(|id| library.original_path(&id.0)).collect()
    }

    /// The image under the cursor while an item is dragged out: its thumbnail, small.
    pub fn drag_image(&self, id: &ItemId) -> Option<Vec<u8>> {
        let thumbnail = self.current_library()?.thumbnail(&id.0).ok()??;
        media::drag_image(&thumbnail.bytes, 96).ok()
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

    // --- Folders ---

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

    // --- Tags and smart folders ---

    /// The tag vocabulary with item counts (trash excluded).
    pub fn tags(&self) -> Result<Vec<Tag>, SourceError> {
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

    /// The rules for "Save as smart folder" over the folders of `source`; refused with the
    /// reason for what rules cannot hold (search text, the trash, nothing to keep).
    pub fn smart_rules_of(
        source: &dyn LibrarySource,
        query: &LibraryQuery,
    ) -> Result<SmartDraft, SourceError> {
        let folders = FolderIndex::new(&ready(source.folder_tree())?);
        let (parent, conditions) = rules_from_query(query, &folders)?;
        Ok(SmartDraft { parent, conditions })
    }

    pub fn smart_rules(&self, query: &LibraryQuery) -> Result<SmartDraft, SourceError> {
        Self::smart_rules_of(&self.source()?, query)
    }

    // --- Library settings and maintenance ---

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
        self.imports.stop();
        library.restore_backup(name)?;
        lock(&self.views).clear();
        lock(&self.undo).clear();
        self.start_previews_if_waiting(Arc::clone(&library));
        Ok(library.info())
    }

    pub fn rename_library(&self, name: &str) -> Result<LibraryInfo, SourceError> {
        let library = self.writable_library()?;
        library.rename_library(name)?;
        Ok(library.info())
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

    /// Rebuild Thumbnails: every preview is made again by the preview task. Returns how many
    /// items wait.
    pub fn rebuild_thumbnails(&self) -> Result<u32, SourceError> {
        let library = self.writable_library()?;
        let waiting = library.rebuild_previews()?;
        self.refresh_views();
        if waiting > 0 && !self.imports.is_running(ImportKind::Previews) {
            self.start_previews(library);
        }
        Ok(waiting)
    }

    // --- Export ---

    /// What an export of `selection` would write, from the database alone (docs/spec.md §7.4
    /// item 12). A read-only library exports fine.
    pub fn export_preview(
        &self,
        selection: ExportSelection,
        include_subfolders: bool,
    ) -> Result<ExportPreview, SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        export_preview(&library, &selection, include_subfolders)
    }

    /// Starts an export into a folder tree or an archive in the background and returns its
    /// first progress.
    pub fn start_export(&self, request: ExportRequest) -> Result<ExportProgress, SourceError> {
        if request.eagle.is_some() {
            return Err(SourceError::NotSupported(
                "an export into Eagle is the desktop app's".into(),
            ));
        }
        let library = self.current_library().ok_or_else(no_library)?;
        self.exports.spawn(Arc::clone(&self.emit), move |context| {
            let mut on_progress = |progress: ExportProgress| context.report(progress);
            let result = export_items(
                &library,
                context.id,
                &request,
                context.cancel_flag(),
                &mut on_progress,
            );
            drop(library);
            if let Err(error) = result {
                // The engine cleans up after itself; the app just hears that the run failed.
                let latest = context.latest();
                let at = |take: fn(&ExportProgress) -> u64| latest.as_ref().map_or(0, take);
                context.report(ExportProgress {
                    id: context.id,
                    state: ExportState::Failed,
                    total: at(|progress| progress.total),
                    done: at(|progress| progress.done),
                    bytes_total: at(|progress| progress.bytes_total),
                    bytes_done: at(|progress| progress.bytes_done),
                    current: None,
                    summary: None,
                    error: Some(error.to_string()),
                });
            }
            context
                .latest()
                .and_then(|progress| progress.summary)
                .map(|summary| PathBuf::from(summary.path))
        })
    }

    /// Decrypts a passphrase `.age` archive to a ZIP path. Needs no open library; `progress` is
    /// `(done, total)` over ciphertext bytes. Cancel with [`Self::cancel_decrypt_age`]. A
    /// finished path is remembered for Show in Explorer like an export (D-102).
    pub fn decrypt_age_archive(
        &self,
        source: &Path,
        destination: &Path,
        password: String,
        progress: &dyn Fn(u64, u64),
    ) -> Result<Option<PathBuf>, SourceError> {
        self.decrypt_age_cancel.store(false, Ordering::SeqCst);
        let written = decrypt_age_archive(
            source,
            destination,
            &password,
            &self.decrypt_age_cancel,
            &mut |done, total| progress(done, total),
        )?;
        if let Some(path) = &written {
            self.exports.remember_written(path.clone());
        }
        Ok(written)
    }

    /// Stops a running [`Self::decrypt_age_archive`] between buffers; the partial ZIP is removed.
    pub fn cancel_decrypt_age(&self) {
        self.decrypt_age_cancel.store(true, Ordering::SeqCst);
    }

    // --- Locked folders ---

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
        path: &Path,
        folder: &str,
        key: &RecoveryKey,
    ) -> Result<(), SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
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
        std::fs::write(path, text)
            .map_err(|error| SourceError::Storage(format!("save the recovery key: {error}")))
    }

    /// YubiKeys bound to one locked folder. Nothing secret is in the list.
    pub fn piv_wraps(&self, id: &str) -> Result<Vec<PivWrap>, SourceError> {
        self.current_library().ok_or_else(no_library)?.piv_wraps(id)
    }

    /// Adds a card the app enrolled as a third wrap of the folder.
    pub fn bind_piv(&self, id: &str, password: String, card: &PivCard) -> Result<(), SourceError> {
        self.writable_library()?.bind_piv(id, password, card)
    }

    /// Drops one card's wrap. The password still opens the folder.
    pub fn unbind_piv(&self, id: &str, password: String, serial: &str) -> Result<(), SourceError> {
        self.writable_library()?.unbind_piv(id, password, serial)
    }

    /// Opens one folder with its card; `agree` runs the key agreement on the card.
    pub fn unlock_with_piv(
        &self,
        id: &str,
        serial: &str,
        prompt: &PivPrompt,
        agree: impl Fn(u8, &[u8]) -> Result<[u8; 32], SourceError>,
    ) -> Result<(), SourceError> {
        let library = self.current_library().ok_or_else(no_library)?;
        library.unlock_with_piv(id, serial, prompt, agree)?;
        self.after_unlock(library);
        Ok(())
    }

    /// The views show the folder; previews that waited for its key are made now.
    fn after_unlock(&self, library: Arc<Library>) {
        self.refresh_views();
        if library.read_only_reason().is_none() && !self.imports.is_running(ImportKind::Previews) {
            self.start_previews_if_waiting(library);
        }
    }

    // --- Adding files ---

    /// Adds files and folders to the open library in a background task, into `folder` when
    /// one is open (docs/spec.md §4); `filter` is what a reviewed import confirmed.
    pub fn add_files(
        &self,
        paths: Vec<PathBuf>,
        folder: Option<String>,
        filter: ImportFilter,
    ) -> Result<ImportTicket, SourceError> {
        let library = self.writable_library()?;
        let target = import_target(&library, folder)?;
        let id = self.start(
            library,
            ImportKind::Files,
            move |library, id, observer, cancel| {
                import_files(
                    library,
                    id,
                    &paths,
                    &target,
                    &filter,
                    DuplicatePolicy::Ask,
                    observer,
                    cancel,
                )
            },
        )?;
        Ok(ImportTicket { id })
    }

    /// Adds a pasted image as `Pasted YYYY-MM-DD HH-MM-SS.png` (UTC).
    pub fn add_png(
        &self,
        png: Vec<u8>,
        folder: Option<String>,
    ) -> Result<ImportTicket, SourceError> {
        let library = self.writable_library()?;
        let target = import_target(&library, folder)?;
        let stamp = utc_file_stamp(now_ms());
        let dir = std::env::temp_dir().join(format!("egl-viewer-paste-{}", now_ms()));
        let file = dir.join(format!("Pasted {stamp}.png"));
        std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&file, png))
            .map_err(|error| SourceError::Storage(format!("keep the pasted image: {error}")))?;
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
        )?;
        Ok(ImportTicket { id })
    }

    /// Walks `path` for a reviewed import and answers what is there; refuses like the import
    /// (a read-only library, or a folder already inside this one).
    pub fn preview_import(&self, path: &Path) -> Result<ImportPreview, SourceError> {
        self.writable_library()?;
        ready(self.source()?.preview_import(path))
    }

    /// Runs an import of `kind` on `library` in the background; its progress reaches the app and
    /// the views follow its writes.
    pub fn start<F>(
        &self,
        library: Arc<Library>,
        kind: ImportKind,
        work: F,
    ) -> Result<u64, SourceError>
    where
        F: FnOnce(&Library, u64, &dyn ImportObserver, &AtomicBool) -> ImportProgress
            + Send
            + 'static,
    {
        let views = Arc::clone(&self.views);
        let emit = Arc::clone(&self.emit);
        self.imports
            .spawn(kind, Arc::clone(&self.emit), move |context| {
                let observer = Observer {
                    context,
                    source: NativeSource::new(Arc::clone(&library)),
                    refresher: Refresher::new(views, emit),
                };
                let done = work(&library, context.id, &observer, context.cancel_flag());
                drop(observer);
                drop(library);
                done
            })
    }

    /// Thumbnails and palettes still to make, in the background. Nobody asked for them, so a
    /// thread that cannot start is only logged; the next open tries again.
    pub fn start_previews(&self, library: Arc<Library>) {
        if let Err(error) = self.start(library, ImportKind::Previews, make_pending_previews) {
            tracing::warn!("cannot make the pending previews: {error}");
        }
    }

    fn start_previews_if_waiting(&self, library: Arc<Library>) {
        if library
            .pending_previews(1)
            .is_ok_and(|next| !next.is_empty())
        {
            self.start_previews(library);
        }
    }

    pub fn answer_import(&self, id: u64, choice: DuplicateChoice) -> Result<(), SourceError> {
        self.imports.answer(id, choice)
    }

    pub fn cancel_import(&self, id: u64) -> Result<(), SourceError> {
        self.imports.cancel(id)
    }
}

/// The folder an import goes into, which must still exist.
fn import_target(library: &Library, folder: Option<String>) -> Result<ImportTarget, SourceError> {
    if let Some(folder) = &folder
        && !library.has_folder(folder)?
    {
        return Err(SourceError::Invalid("The folder no longer exists.".into()));
    }
    Ok(ImportTarget { folder })
}
