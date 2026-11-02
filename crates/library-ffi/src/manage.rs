//! Management exports: adding files, originals, trash, folders, tags, multi-select edits,
//! undo, query windows and smart folders. Mirrors what `src-tauri` commands call on
//! `library-native`, without Tauri and without Eagle.
//!
//! Conventions match the rest of the crate: JSON in / JSON out as UTF-8 C strings (free with
//! `egl_string_free`), an [`EglStatus`] return, and the message in `egl_last_error(lib)`.

use std::collections::{HashMap, HashSet};
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use library_core::{
    DuplicateNotice, FolderChange, FolderIndex, ImportKind, ImportProgress, ImportTicket, ItemCell,
    ItemEdit, ItemId, ItemLite, LibraryQuery, MAX_WINDOW, Progress, Scope, ScopeHints, SearchMatch,
    SmartFolderChange, SourceError, TagGroupChange, UndoStack, View, ViewSource, Views,
};
use library_native::{
    DuplicateChoice, DuplicatePolicy, ImportFilter, ImportObserver, ImportTarget, Library,
    import_files, make_pending_previews,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{EglLibrary, EglStatus, c_str, fail, fail_source, with_lib, write_json, write_string};

/// Finished imports kept for `egl_imports`.
const FINISHED_KEPT: usize = 20;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Per-handle state the management exports need.
#[derive(Default)]
pub(crate) struct State {
    imports: Arc<Mutex<Imports>>,
    undo: Mutex<UndoStack>,
    empty_trash_cancel: AtomicBool,
}

#[derive(Default)]
struct Imports {
    next_id: u64,
    running: HashMap<u64, Running>,
    finished: Vec<ImportProgress>,
}

struct Running {
    kind: ImportKind,
    cancel: Arc<AtomicBool>,
    decision: Arc<Decision>,
    latest: Arc<Mutex<Option<ImportProgress>>>,
    thread: Option<JoinHandle<()>>,
}

/// The answer to the duplicate question, handed from `egl_answer_import` to the import thread.
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
    latest: Arc<Mutex<Option<ImportProgress>>>,
    decision: Arc<Decision>,
    cancel: Arc<AtomicBool>,
}

impl ImportObserver for Observer {
    fn progress(&self, progress: &ImportProgress) {
        *lock(&self.latest) = Some(progress.clone());
    }

    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        self.decision.wait(&self.cancel)
    }
}

impl State {
    fn start<F>(&self, library: Arc<Library>, kind: ImportKind, work: F) -> Result<u64, String>
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
            latest: Arc::clone(&latest),
            decision: Arc::clone(&decision),
            cancel: Arc::clone(&cancel),
        };
        let registry = Arc::clone(&self.imports);
        let task_cancel = Arc::clone(&cancel);
        // The thread waits for the registry lock held here, so it is registered before it
        // can remove itself.
        let thread = std::thread::Builder::new()
            .name(format!("egl-import-{id}"))
            .spawn(move || {
                let done = work(&library, id, &observer, &task_cancel);
                drop(observer);
                drop(library);
                let mut imports = lock(&registry);
                imports.running.remove(&id);
                imports.finished.insert(0, done);
                imports.finished.truncate(FINISHED_KEPT);
            })
            .map_err(|err| format!("cannot start an import: {err}"))?;
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
        Ok(id)
    }

    fn previewing(&self) -> bool {
        lock(&self.imports)
            .running
            .values()
            .any(|running| running.kind == ImportKind::Previews)
    }

    /// Cancels every import and waits for their threads; added files stay.
    pub(crate) fn stop_imports(&self) {
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
            let _ = thread.join();
        }
    }
}

/// `ViewSource` straight over the library (no async source layer).
struct LibView<'a>(&'a Library);

impl ViewSource for LibView<'_> {
    fn load_scope(
        &self,
        scope: &Scope,
        _hints: &ScopeHints,
        progress: Progress<'_>,
    ) -> Result<Vec<ItemLite>, SourceError> {
        let items = self.0.load_scope(scope)?;
        progress(items.len(), items.len());
        Ok(items)
    }

    fn folder_index(&self) -> Result<FolderIndex, SourceError> {
        Ok(FolderIndex::new(&self.0.folder_tree()?))
    }

    fn search_ids(&self, text: &str) -> Result<SearchMatch, SourceError> {
        self.0.search(text)
    }
}

fn open_view(library: &Library, query: LibraryQuery) -> Result<View, SourceError> {
    let mut views = Views::default();
    let view = views.open(query, &LibView(library), &|_, _| {})?;
    Ok(view.clone())
}

/// A selection: listed `ids`, or with `all` every item of `query` except `ids`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Target {
    ids: Vec<ItemId>,
    all: bool,
    query: Option<LibraryQuery>,
}

fn target_ids(library: &Library, target: Target) -> Result<Vec<ItemId>, SourceError> {
    if !target.all {
        return Ok(target.ids);
    }
    let query = target
        .query
        .ok_or_else(|| SourceError::Invalid("Select All needs its query.".into()))?;
    let view = open_view(library, query)?;
    let except: HashSet<&ItemId> = target.ids.iter().collect();
    Ok(view
        .ids(0, view.total())
        .into_iter()
        .filter(|id| !except.contains(id))
        .collect())
}

fn parse<T: DeserializeOwned>(
    handle: &EglLibrary,
    raw: *const c_char,
    what: &str,
) -> Result<T, EglStatus> {
    let text = c_str(raw).map_err(|status| fail(handle, status, format!("{what} is required")))?;
    serde_json::from_str(text)
        .map_err(|err| fail(handle, EglStatus::Invalid, format!("invalid {what}: {err}")))
}

fn arg<'a>(handle: &EglLibrary, raw: *const c_char, what: &str) -> Result<&'a str, EglStatus> {
    c_str(raw).map_err(|status| fail(handle, status, format!("{what} is required")))
}

fn reply<T: Serialize>(
    handle: &EglLibrary,
    out: *mut *mut c_char,
    result: Result<T, SourceError>,
    what: &str,
) -> EglStatus {
    match result {
        Ok(value) => match write_json(out, &value) {
            Ok(()) => EglStatus::Ok,
            Err(status) => fail(handle, status, format!("failed to serialize {what}")),
        },
        Err(err) => fail_source(handle, err),
    }
}

fn reply_count(handle: &EglLibrary, out: *mut u32, result: Result<u32, SourceError>) -> EglStatus {
    match result {
        Ok(count) => {
            if !out.is_null() {
                // SAFETY: out is a non-null pointer provided by the caller.
                unsafe { *out = count };
            }
            EglStatus::Ok
        }
        Err(err) => fail_source(handle, err),
    }
}

fn reply_unit(handle: &EglLibrary, result: Result<(), SourceError>) -> EglStatus {
    match result {
        Ok(()) => EglStatus::Ok,
        Err(err) => fail_source(handle, err),
    }
}

/// `try`-style early return for `Result<_, EglStatus>` inside `with_lib` bodies.
macro_rules! tri {
    ($expr:expr) => {
        match $expr {
            Ok(value) => value,
            Err(status) => return status,
        }
    };
}

fn writable(library: &Library) -> Result<(), SourceError> {
    if library.read_only_reason().is_some() {
        return Err(SourceError::ReadOnly);
    }
    Ok(())
}

/// An edit for `ids` that undo can take back.
fn edit_with_undo(
    handle: &EglLibrary,
    ids: &[ItemId],
    edit: &ItemEdit,
) -> Result<u32, SourceError> {
    edit.validate()?;
    writable(&handle.library)?;
    let before = handle.library.item_states(ids)?;
    let edited = handle.library.apply_batch_edit(ids, edit)?;
    lock(&handle.state.undo).push(edit, &before);
    Ok(edited)
}

// ---------------------------------------------------------------------------------------------
// 1. Adding files

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct AddRequest {
    paths: Vec<String>,
    folder: Option<String>,
    exclude: Vec<String>,
    include: Vec<String>,
    reviewed: bool,
}

/// Starts adding files and folders (walked recursively) in a background task; thumbnails
/// are made by the same task. `request_json`:
/// `{"paths":["/abs/a.png","/abs/dir"],"folder":null,"exclude":[],"include":[],"reviewed":false}`.
/// `*out_json` is `{"id":N}`; poll with `egl_import_status`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_add_files(
    lib: *mut EglLibrary,
    request_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let request: AddRequest = tri!(parse(handle, request_json, "add request JSON"));
        if request.paths.is_empty() {
            return fail(handle, EglStatus::Invalid, "paths must not be empty");
        }
        if let Err(err) = writable(&handle.library) {
            return fail_source(handle, err);
        }
        if let Some(folder) = &request.folder {
            match handle.library.has_folder(folder) {
                Ok(true) => {}
                Ok(false) => {
                    return fail(handle, EglStatus::Invalid, "The folder no longer exists.");
                }
                Err(err) => return fail_source(handle, err),
            }
        }
        let target = ImportTarget {
            folder: request.folder,
        };
        let filter = ImportFilter {
            exclude: request.exclude.into_iter().collect(),
            include: request.include.into_iter().collect(),
            reviewed: request.reviewed,
        };
        let sources: Vec<PathBuf> = request.paths.into_iter().map(PathBuf::from).collect();
        let started = handle.state.start(
            Arc::clone(&handle.library),
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
        match started {
            Ok(id) => reply(handle, out_json, Ok(ImportTicket { id }), "import ticket"),
            Err(message) => fail(handle, EglStatus::Other, message),
        }
    })
}

/// Answers the duplicate question of import `id` (state `waitingForDecision`).
/// `answer_utf8`: `skip` | `addAnyway` | `addToFolder` | `cancel`. The first answer holds for
/// the rest of the batch.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_answer_import(
    lib: *mut EglLibrary,
    id: u64,
    answer_utf8: *const c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let answer = tri!(arg(handle, answer_utf8, "answer"));
        let choice = match answer {
            "skip" => DuplicateChoice::Skip,
            "addAnyway" => DuplicateChoice::AddAnyway,
            "addToFolder" => DuplicateChoice::AddToFolder,
            "cancel" => DuplicateChoice::Cancel,
            other => {
                return fail(
                    handle,
                    EglStatus::Invalid,
                    format!("unknown duplicate answer: {other}"),
                );
            }
        };
        let imports = lock(&handle.state.imports);
        match imports.running.get(&id) {
            Some(running) => {
                running.decision.answer(choice);
                EglStatus::Ok
            }
            None => fail(
                handle,
                EglStatus::NotFound,
                "no running import with this id",
            ),
        }
    })
}

/// Stops import `id` after the current file; added files stay, missing thumbnails are made
/// by a later `egl_generate_previews`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_cancel_import(lib: *mut EglLibrary, id: u64) -> EglStatus {
    with_lib(lib, |handle| {
        let imports = lock(&handle.state.imports);
        match imports.running.get(&id) {
            Some(running) => {
                running.cancel.store(true, Ordering::Relaxed);
                running.decision.answer(DuplicateChoice::Cancel);
                EglStatus::Ok
            }
            None => fail(
                handle,
                EglStatus::NotFound,
                "no running import with this id",
            ),
        }
    })
}

/// Every import: running first (newest first), then the last finished ones.
/// `*out_json` is an `ImportProgress[]`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_imports(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let imports = lock(&handle.state.imports);
        let mut list: Vec<ImportProgress> = imports
            .running
            .values()
            .filter_map(|running| lock(&running.latest).clone())
            .collect();
        list.sort_by_key(|progress| std::cmp::Reverse(progress.id));
        list.extend(imports.finished.iter().cloned());
        drop(imports);
        reply(handle, out_json, Ok(list), "imports")
    })
}

/// The progress of import `id`: an `ImportProgress` object, or `null` when it started but has
/// not reported yet. `EGL_ERR_NOT_FOUND` for an unknown id.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_import_status(
    lib: *mut EglLibrary,
    id: u64,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let imports = lock(&handle.state.imports);
        let progress = if let Some(running) = imports.running.get(&id) {
            lock(&running.latest).clone()
        } else if let Some(done) = imports.finished.iter().find(|p| p.id == id) {
            Some(done.clone())
        } else {
            return fail(handle, EglStatus::NotFound, "no import with this id");
        };
        drop(imports);
        reply(handle, out_json, Ok(progress), "import progress")
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewTicket {
    /// The preview task, or `null` when nothing waits or one already runs.
    id: Option<u64>,
    /// Items waiting for a thumbnail after a rebuild (0 without `rebuild`).
    waiting: u32,
}

/// Makes the thumbnails a crash or a cancelled import left undone, in the background (what
/// the desktop does on open). With `rebuild != 0` every thumbnail is made again (Rebuild
/// Thumbnails). `*out_json`: `{"id":N|null,"waiting":N}`; poll `id` with `egl_import_status`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_generate_previews(
    lib: *mut EglLibrary,
    rebuild: c_int,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        if let Err(err) = writable(&handle.library) {
            return fail_source(handle, err);
        }
        let waiting = if rebuild != 0 {
            match handle.library.rebuild_previews() {
                Ok(waiting) => waiting,
                Err(err) => return fail_source(handle, err),
            }
        } else {
            0
        };
        let id = if handle.state.previewing() {
            None
        } else {
            match handle.state.start(
                Arc::clone(&handle.library),
                ImportKind::Previews,
                make_pending_previews,
            ) {
                Ok(id) => Some(id),
                Err(message) => return fail(handle, EglStatus::Other, message),
            }
        };
        reply(
            handle,
            out_json,
            Ok(PreviewTicket { id, waiting }),
            "preview ticket",
        )
    })
}

// ---------------------------------------------------------------------------------------------
// 2. Originals

/// Absolute path of the stored original, for a full-size view or sharing. Refused
/// (`EGL_ERR_INVALID` / `EGL_ERR_UNSUPPORTED`) for an item of a locked folder: its file is
/// encrypted. Full item detail is `egl_item`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_original_path(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_path: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let id = tri!(arg(handle, id_utf8, "item id"));
        match handle.library.original_path(id) {
            Ok(path) => match write_string(out_path, path.to_string_lossy()) {
                Ok(()) => EglStatus::Ok,
                Err(status) => fail(handle, status, "failed to copy original path"),
            },
            Err(err) => fail_source(handle, err),
        }
    })
}

// ---------------------------------------------------------------------------------------------
// 3. Trash

/// Moves a selection to the trash (undoable). `target_json`: see header (Target).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_trash_items(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    out_count: *mut u32,
) -> EglStatus {
    with_lib(lib, |handle| {
        let target: Target = tri!(parse(handle, target_json, "target JSON"));
        let result = target_ids(&handle.library, target)
            .and_then(|ids| edit_with_undo(handle, &ids, &ItemEdit::Trash));
        reply_count(handle, out_count, result)
    })
}

/// Restore All: every visible trash item comes back with its folders and tags (undoable).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_restore_trash(lib: *mut EglLibrary, out_count: *mut u32) -> EglStatus {
    with_lib(lib, |handle| {
        let result = handle.library.trash_ids().and_then(|ids| {
            if ids.is_empty() {
                Ok(0)
            } else {
                edit_with_undo(handle, &ids, &ItemEdit::Restore)
            }
        });
        reply_count(handle, out_count, result)
    })
}

/// Removes the trashed items of a selection for good, with their files. No undo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_permanently(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    out_count: *mut u32,
) -> EglStatus {
    with_lib(lib, |handle| {
        let target: Target = tri!(parse(handle, target_json, "target JSON"));
        let result =
            target_ids(&handle.library, target).and_then(|ids| handle.library.delete_items(&ids));
        reply_count(handle, out_count, result)
    })
}

/// Empties the trash with the files; blocks until done. No undo. Another thread may call
/// `egl_cancel_empty_trash` to stop between items.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_empty_trash(lib: *mut EglLibrary, out_count: *mut u32) -> EglStatus {
    with_lib(lib, |handle| {
        handle
            .state
            .empty_trash_cancel
            .store(false, Ordering::SeqCst);
        let result =
            handle
                .library
                .empty_trash(&|_, _| {}, &handle.state.empty_trash_cancel, &|| {});
        reply_count(handle, out_count, result)
    })
}

/// Stops a running `egl_empty_trash` between items; remaining files go on the next open.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_cancel_empty_trash(lib: *mut EglLibrary) -> EglStatus {
    with_lib(lib, |handle| {
        handle
            .state
            .empty_trash_cancel
            .store(true, Ordering::SeqCst);
        EglStatus::Ok
    })
}

/// `{"items":N,"bytes":N}` of the visible trash.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_trash_info(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        reply(handle, out_json, handle.library.trash_info(), "trash info")
    })
}

// ---------------------------------------------------------------------------------------------
// 4. Folders

/// Creates, updates (rename etc.) or moves a folder; `*out_json` is the new folder tree.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_save_folder(
    lib: *mut EglLibrary,
    change_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let change: FolderChange = tri!(parse(handle, change_json, "folder change JSON"));
        let result = change
            .validate()
            .and_then(|()| handle.library.save_folder(&change))
            .and_then(|_| handle.library.folder_tree());
        reply(handle, out_json, result, "folder tree")
    })
}

/// Deletes a folder and its subfolders; with `trash_items != 0` their items go to the trash.
/// `*out_json` is the new folder tree.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_folder(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    trash_items: c_int,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let id = tri!(arg(handle, id_utf8, "folder id"));
        let result = handle
            .library
            .delete_folder(id, trash_items != 0)
            .and_then(|_| handle.library.folder_tree());
        reply(handle, out_json, result, "folder tree")
    })
}

/// Manual order: `ids_json` (`["id",…]`) go before `before_utf8` (NULL: to the end) in
/// `folder_utf8`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_reorder_items(
    lib: *mut EglLibrary,
    folder_utf8: *const c_char,
    ids_json: *const c_char,
    before_utf8: *const c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let folder = tri!(arg(handle, folder_utf8, "folder id"));
        let ids: Vec<ItemId> = tri!(parse(handle, ids_json, "ids JSON"));
        let before = if before_utf8.is_null() {
            None
        } else {
            Some(ItemId(
                tri!(arg(handle, before_utf8, "before id")).to_owned(),
            ))
        };
        reply_unit(
            handle,
            handle.library.reorder_items(folder, &ids, before.as_ref()),
        )
    })
}

// ---------------------------------------------------------------------------------------------
// 5. Tags

/// Renames a tag on every item; onto an existing tag the two merge.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_rename_tag(
    lib: *mut EglLibrary,
    from_utf8: *const c_char,
    to_utf8: *const c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let from = tri!(arg(handle, from_utf8, "from tag"));
        let to = tri!(arg(handle, to_utf8, "to tag"));
        reply_unit(handle, handle.library.rename_tag(from, to))
    })
}

/// Merges tag `source` into `target` on every item.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_merge_tags(
    lib: *mut EglLibrary,
    source_utf8: *const c_char,
    target_utf8: *const c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let source = tri!(arg(handle, source_utf8, "source tag"));
        let target = tri!(arg(handle, target_utf8, "target tag"));
        reply_unit(handle, handle.library.merge_tags(source, target))
    })
}

/// Removes a tag from every item.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_tag(
    lib: *mut EglLibrary,
    name_utf8: *const c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let name = tri!(arg(handle, name_utf8, "tag name"));
        reply_unit(handle, handle.library.delete_tag(name))
    })
}

/// `TagGroup[]`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_tag_groups(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        reply(handle, out_json, handle.library.tag_groups(), "tag groups")
    })
}

/// Creates or updates a tag group; `*out_json` is every group.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_save_tag_group(
    lib: *mut EglLibrary,
    change_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let change: TagGroupChange = tri!(parse(handle, change_json, "tag group change JSON"));
        let result = change
            .validate()
            .and_then(|()| handle.library.save_tag_group(&change))
            .and_then(|_| handle.library.tag_groups());
        reply(handle, out_json, result, "tag groups")
    })
}

/// Deletes a tag group (item tags stay); `*out_json` is every group left.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_tag_group(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let id = tri!(arg(handle, id_utf8, "tag group id"));
        let result = handle
            .library
            .delete_tag_group(id)
            .and_then(|()| handle.library.tag_groups());
        reply(handle, out_json, result, "tag groups")
    })
}

// ---------------------------------------------------------------------------------------------
// 6. Multi-select and undo

/// One `ItemEdit` for a selection (undoable); `*out_count` is how many items changed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_apply_batch_edit(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    edit_json: *const c_char,
    out_count: *mut u32,
) -> EglStatus {
    with_lib(lib, |handle| {
        let target: Target = tri!(parse(handle, target_json, "target JSON"));
        let edit: ItemEdit = tri!(parse(handle, edit_json, "edit JSON"));
        let result =
            target_ids(&handle.library, target).and_then(|ids| edit_with_undo(handle, &ids, &edit));
        reply_count(handle, out_count, result)
    })
}

/// The tags of a selection with per-tag counts, size and the first cells:
/// `{"items":N,"tags":[{"name","imageCount"}],"size":N,"stack":[ItemCell…]}`. Counts come from
/// `query` (default All) so a trash selection needs `{"kind":"trash"}` as its scope.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_selection_tags(
    lib: *mut EglLibrary,
    target_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let target: Target = tri!(parse(handle, target_json, "target JSON"));
        let all = target.all;
        let ids: HashSet<ItemId> = target.ids.into_iter().collect();
        let result = open_view(&handle.library, target.query.unwrap_or_default())
            .map(|view| view.selection_tags(all, &ids));
        reply(handle, out_json, result, "selection tags")
    })
}

/// Takes back the last edit made through this handle (item edit, batch edit, trash, restore).
/// `*out_json` is the `UndoStatus` taken back, or `null` when there was nothing to undo.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_undo(lib: *mut EglLibrary, out_json: *mut *mut c_char) -> EglStatus {
    with_lib(lib, |handle| {
        let entry = lock(&handle.state.undo).pop();
        let result = match entry {
            None => Ok(None),
            Some(entry) => {
                let status = entry.status();
                entry
                    .steps
                    .into_iter()
                    .try_for_each(|step| {
                        handle
                            .library
                            .apply_batch_edit(&step.ids, &step.edit)
                            .map(|_| ())
                    })
                    .map(|()| Some(status))
            }
        };
        reply(handle, out_json, result, "undo status")
    })
}

/// What `egl_undo` would take back: `{"kind":"…","items":N}` or `null`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_undo_status(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let status = lock(&handle.state.undo).status();
        reply(handle, out_json, Ok(status), "undo status")
    })
}

/// `egl_apply_item_edit` with undo: like it, but the edit can be taken back by `egl_undo`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_apply_item_edit_undoable(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    edit_json: *const c_char,
    out_item_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let id = tri!(arg(handle, id_utf8, "item id"));
        let edit: ItemEdit = tri!(parse(handle, edit_json, "edit JSON"));
        let ids = [ItemId(id.to_owned())];
        let result = edit
            .validate()
            .and_then(|()| writable(&handle.library))
            .and_then(|()| handle.library.item_states(&ids))
            .and_then(|before| {
                let item = handle.library.apply_item_edit(id, &edit)?;
                lock(&handle.state.undo).push(&edit, &before);
                Ok(item)
            });
        reply(handle, out_item_json, result, "item")
    })
}

// ---------------------------------------------------------------------------------------------
// 7. Query windows and smart folders

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueryWindow {
    total: usize,
    start: u32,
    items: Vec<ItemCell>,
}

/// A window of a full `LibraryQuery` (scope, tag/folder chips, type, rating, shape, search
/// text, sort): `{"total":N,"start":S,"items":[ItemCell…]}`, at most 200 items. Empty / `{}`
/// is All, newest first. Stateless: every call evaluates the query again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_query_window(
    lib: *mut EglLibrary,
    query_json: *const c_char,
    start: u32,
    len: u32,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let raw = c_str(query_json).unwrap_or("{}");
        let query: LibraryQuery = if raw.trim().is_empty() {
            LibraryQuery::default()
        } else {
            match serde_json::from_str(raw) {
                Ok(query) => query,
                Err(err) => {
                    return fail(
                        handle,
                        EglStatus::Invalid,
                        format!("invalid query JSON: {err}"),
                    );
                }
            }
        };
        let result = open_view(&handle.library, query).map(|view| QueryWindow {
            total: view.total(),
            start,
            items: view.range(start as usize, (len as usize).min(MAX_WINDOW)),
        });
        reply(handle, out_json, result, "query window")
    })
}

/// `ExtensionCount[]` for the Type filter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_extensions(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        reply(handle, out_json, handle.library.extensions(), "extensions")
    })
}

/// `SmartFolder[]` tree.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_smart_folders(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        reply(
            handle,
            out_json,
            handle.library.smart_folders(),
            "smart folders",
        )
    })
}

/// Creates or updates a smart folder (Eagle-style `conditions`); `*out_json` is the tree.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_save_smart_folder(
    lib: *mut EglLibrary,
    change_json: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let change: SmartFolderChange = tri!(parse(handle, change_json, "smart folder JSON"));
        let result = handle
            .library
            .save_smart_folder(&change)
            .and_then(|_| handle.library.smart_folders());
        reply(handle, out_json, result, "smart folders")
    })
}

/// Deletes a smart folder (and its children); `*out_json` is the tree left.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_delete_smart_folder(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let id = tri!(arg(handle, id_utf8, "smart folder id"));
        let result = handle
            .library
            .delete_smart_folder(id)
            .and_then(|()| handle.library.smart_folders());
        reply(handle, out_json, result, "smart folders")
    })
}

#[cfg(test)]
#[path = "manage_tests.rs"]
mod tests;
