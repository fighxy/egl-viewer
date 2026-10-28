//! The open library, its import tasks and the app's files, without Tauri: what the commands
//! call, and what the tests drive (docs/architecture.md §7.4, §9, D-022, D-024).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use app_store::{
    AppStore, Preferences, RecentLibrary, SyncEnvironment, check_location, default_library_parent,
    display_path,
};
use library_core::{
    DuplicateNotice, ImportKind, ImportProgress, ImportState, ImportTicket, LibraryCounts,
    LibraryInfo, LocationProblem, ReadOnlyReason, SourceError,
};
use library_native::{
    DuplicateChoice, DuplicatePolicy, ImportObserver, Library, OpenOptions, create_library,
    import_files, make_pending_previews, utc_file_stamp,
};
use serde::Serialize;
use ts_rs::TS;

/// What the app does on start: the reopened library, or why it shows Welcome instead.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StartupResult {
    pub library: Option<LibraryInfo>,
    /// Shown on Welcome when the last library was not reopened for a reason worth saying.
    pub notice: Option<String>,
}

/// Sends import progress to the UI (an `import-progress` event in the app).
pub type Emit = Arc<dyn Fn(&ImportProgress) + Send + Sync>;

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
    emit: Emit,
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
}

impl ImportObserver for Observer {
    fn progress(&self, progress: &ImportProgress) {
        *lock(&self.latest) = Some(progress.clone());
        (self.emit)(progress);
    }

    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        self.decision.wait(&self.cancel)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
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

    /// Adds files and folders to the open library in a background task (docs/spec.md §4).
    pub fn add_files(&self, paths: Vec<String>) -> Result<ImportTicket, SourceError> {
        let library = self.writable_library()?;
        let sources: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let id = self.start(
            library,
            ImportKind::Files,
            move |library, id, observer, cancel| {
                import_files(
                    library,
                    id,
                    &sources,
                    DuplicatePolicy::Ask,
                    observer,
                    cancel,
                )
            },
        );
        Ok(ImportTicket { id })
    }

    /// Adds what is on the clipboard: copied files, or a pasted image as
    /// `Pasted YYYY-MM-DD HH-MM-SS.png` (UTC). `None` when there is nothing to add.
    pub fn paste(&self, clipboard: &dyn Clipboard) -> Result<Option<ImportTicket>, SourceError> {
        self.writable_library()?;
        if let Some(files) = clipboard.files().filter(|files| !files.is_empty()) {
            let paths = files
                .iter()
                .map(|file| file.to_string_lossy().into_owned())
                .collect();
            return self.add_files(paths).map(Some);
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
                let done =
                    import_files(library, id, &[file], DuplicatePolicy::Ask, observer, cancel);
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
        };
        let registry = Arc::clone(&self.imports);
        let task_cancel = Arc::clone(&cancel);
        // The thread waits for the registry lock that is held here, so it is registered
        // before it can remove itself.
        let thread = std::thread::Builder::new()
            .name(format!("import-{id}"))
            .spawn(move || {
                let done = work(&library, id, &observer, &task_cancel);
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

    /// Stops imports and takes the library out of the session.
    fn take_library(&self) -> Option<Library> {
        self.stop_imports();
        let library = lock(&self.library).take()?;
        // Import threads hold their clones only while they run.
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
            Arc::new(|_: &ImportProgress| {}),
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
            Arc::new(move |progress: &ImportProgress| sink.lock().unwrap().push(progress.clone())),
        );
        assert!(matches!(
            session.add_files(vec![]),
            Err(SourceError::Invalid(_))
        ));
        session
            .create_library(&home.path().to_string_lossy(), "Imports")
            .unwrap();
        let ticket = session.add_files(images(home.path(), 3)).unwrap();
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
        let first = f.session.add_files(files.clone()).unwrap();
        finished(&f.session, first.id);

        let again = f.session.add_files(files.clone()).unwrap();
        waiting(&f.session, again.id);
        assert!(f.session.has_running_imports());
        f.session
            .answer_import(again.id, DuplicateChoice::Skip)
            .unwrap();
        let done = finished(&f.session, again.id);
        assert_eq!((done.added, done.duplicates), (0, 2));

        let third = f.session.add_files(files).unwrap();
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
        finished(&f.session, f.session.add_files(files.clone()).unwrap().id);
        let waiting_import = f.session.add_files(files).unwrap();
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
            f.session.paste(&empty),
            Err(SourceError::Invalid(_))
        ));
        f.session
            .create_library(&f.home.path().to_string_lossy(), "Paste")
            .unwrap();
        assert_eq!(f.session.paste(&empty).unwrap(), None);

        let image = FakeClipboard {
            files: None,
            image: Some((4, 2, vec![200; 4 * 2 * 4])),
        };
        let ticket = f.session.paste(&image).unwrap().unwrap();
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
        let ticket = f.session.paste(&copied).unwrap().unwrap();
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
            f.session.add_files(images(f.home.path(), 1)),
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

    #[test]
    fn forgetting_a_recent_library_removes_it() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        f.session.create_library(&parent, "Temp").unwrap();
        let path = f.session.recent_libraries()[0].path.clone();
        f.session.forget_recent_library(&path).unwrap();
        assert!(f.session.recent_libraries().is_empty());
    }
}
