//! Background tasks: imports (adding files, making previews, importing an Eagle library folder,
//! adding to a running Eagle) and exports. Each runs on its own thread with a cancel flag and
//! its latest progress; finished imports are kept for the Imports panel.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use library_core::{
    ExportProgress, ExportState, ImportKind, ImportProgress, ImportState, SourceError,
};
use library_native::DuplicateChoice;

use crate::events::{Emit, Event};
use crate::lock;

/// Finished imports kept for the Imports panel.
const FINISHED_KEPT: usize = 20;

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

/// What an import thread sees of its own task.
pub struct TaskContext {
    pub id: u64,
    cancel: Arc<AtomicBool>,
    decision: Arc<Decision>,
    latest: Arc<Mutex<Option<ImportProgress>>>,
    emit: Emit,
}

impl TaskContext {
    /// Set by Cancel, by closing the library and by quitting.
    pub fn cancel_flag(&self) -> &Arc<AtomicBool> {
        &self.cancel
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Records the progress and tells the app.
    pub fn report(&self, progress: &ImportProgress) {
        *lock(&self.latest) = Some(progress.clone());
        (self.emit)(Event::ImportProgress(progress.clone()));
    }

    /// Waits for the answer to the duplicate question; Cancel when the task is cancelled.
    pub fn decide(&self) -> DuplicateChoice {
        self.decision.wait(&self.cancel)
    }
}

struct Running {
    kind: ImportKind,
    cancel: Arc<AtomicBool>,
    decision: Arc<Decision>,
    latest: Arc<Mutex<Option<ImportProgress>>>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct Imports {
    next_id: u64,
    running: HashMap<u64, Running>,
    finished: Vec<ImportProgress>,
}

/// The import tasks of one service.
#[derive(Clone, Default)]
pub struct ImportTasks {
    inner: Arc<Mutex<Imports>>,
}

impl ImportTasks {
    /// Runs `work` on a new thread; what it returns is kept as the finished task. An error when
    /// the thread cannot start: nothing runs then.
    pub fn spawn<F>(&self, kind: ImportKind, emit: Emit, work: F) -> Result<u64, SourceError>
    where
        F: FnOnce(&TaskContext) -> ImportProgress + Send + 'static,
    {
        let mut imports = lock(&self.inner);
        let id = imports.next_id + 1;
        let context = TaskContext {
            id,
            cancel: Arc::new(AtomicBool::new(false)),
            decision: Arc::new(Decision::default()),
            latest: Arc::new(Mutex::new(None)),
            emit,
        };
        let running = Running {
            kind,
            cancel: Arc::clone(&context.cancel),
            decision: Arc::clone(&context.decision),
            latest: Arc::clone(&context.latest),
            thread: None,
        };
        let registry = Arc::clone(&self.inner);
        // The thread waits for the registry lock that is held here, so it is registered
        // before it can remove itself.
        let thread = std::thread::Builder::new()
            .name(format!("import-{id}"))
            .spawn(move || {
                let done = work(&context);
                drop(context);
                let mut imports = lock(&registry);
                imports.running.remove(&id);
                imports.finished.insert(0, done);
                imports.finished.truncate(FINISHED_KEPT);
            })
            .map_err(|error| SourceError::Storage(format!("start the import: {error}")))?;
        imports.next_id = id;
        imports.running.insert(
            id,
            Running {
                thread: Some(thread),
                ..running
            },
        );
        Ok(id)
    }

    /// The answer to the duplicate question of import `id`.
    pub fn answer(&self, id: u64, choice: DuplicateChoice) -> Result<(), SourceError> {
        let imports = lock(&self.inner);
        let running = imports.running.get(&id).ok_or(SourceError::NotFound)?;
        running.decision.answer(choice);
        Ok(())
    }

    /// Stops import `id` after the current file; added files stay.
    pub fn cancel(&self, id: u64) -> Result<(), SourceError> {
        let imports = lock(&self.inner);
        let running = imports.running.get(&id).ok_or(SourceError::NotFound)?;
        running.cancel.store(true, Ordering::Relaxed);
        running.decision.answer(DuplicateChoice::Cancel);
        Ok(())
    }

    /// Running imports first, then finished ones, newest first.
    pub fn list(&self) -> Vec<ImportProgress> {
        let imports = lock(&self.inner);
        let mut running: Vec<ImportProgress> = imports
            .running
            .values()
            .filter_map(|running| lock(&running.latest).clone())
            .collect();
        running.sort_by_key(|progress| std::cmp::Reverse(progress.id));
        running.extend(imports.finished.iter().cloned());
        running
    }

    /// Import `id`: `Some(None)` while it runs and has not reported yet, `Some(progress)` with
    /// its latest or final progress, `None` for an id the registry does not know (any more).
    pub fn status(&self, id: u64) -> Option<Option<ImportProgress>> {
        let imports = lock(&self.inner);
        if let Some(running) = imports.running.get(&id) {
            return Some(lock(&running.latest).clone());
        }
        imports
            .finished
            .iter()
            .find(|progress| progress.id == id)
            .map(|progress| Some(progress.clone()))
    }

    /// Adding files or importing an Eagle library is running (previews made after a restart
    /// do not count: they resume later). A task that sent its final progress counts as over
    /// even before its thread has ended.
    pub fn has_running(&self) -> bool {
        lock(&self.inner).running.values().any(|running| {
            running.kind != ImportKind::Previews
                && !lock(&running.latest).as_ref().is_some_and(|progress| {
                    matches!(progress.state, ImportState::Done | ImportState::Cancelled)
                })
        })
    }

    /// A task of `kind` is running.
    pub fn is_running(&self, kind: ImportKind) -> bool {
        lock(&self.inner)
            .running
            .values()
            .any(|running| running.kind == kind)
    }

    /// Cancels every import and waits for their threads; their added files stay.
    pub fn stop(&self) {
        let threads: Vec<(Arc<AtomicBool>, Arc<Decision>, JoinHandle<()>)> = {
            let mut imports = lock(&self.inner);
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
}

/// What an export thread sees of its own task.
pub struct ExportContext {
    pub id: u64,
    cancel: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<ExportProgress>>>,
    emit: Emit,
}

impl ExportContext {
    pub fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }

    /// Records the progress and tells the app.
    pub fn report(&self, progress: ExportProgress) {
        *lock(&self.latest) = Some(progress.clone());
        (self.emit)(Event::ExportProgress(progress));
    }

    pub fn latest(&self) -> Option<ExportProgress> {
        lock(&self.latest).clone()
    }
}

struct ExportRunning {
    cancel: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<ExportProgress>>>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct Exports {
    next_id: u64,
    running: HashMap<u64, ExportRunning>,
    /// What finished exports wrote: the only paths Show in Explorer may reveal.
    written: Vec<PathBuf>,
}

/// The export tasks of one service.
#[derive(Clone, Default)]
pub struct ExportTasks {
    inner: Arc<Mutex<Exports>>,
}

impl ExportTasks {
    /// Runs `work` on a new thread and answers with the first progress. `work` returns the
    /// folder or archive it wrote, which [`ExportTasks::may_reveal`] then allows.
    pub fn spawn<F>(&self, emit: Emit, work: F) -> Result<ExportProgress, SourceError>
    where
        F: FnOnce(&ExportContext) -> Option<PathBuf> + Send + 'static,
    {
        let mut exports = lock(&self.inner);
        let id = exports.next_id + 1;
        let context = ExportContext {
            id,
            cancel: Arc::new(AtomicBool::new(false)),
            latest: Arc::new(Mutex::new(None)),
            emit,
        };
        let cancel = Arc::clone(&context.cancel);
        let latest = Arc::clone(&context.latest);
        let registry = Arc::clone(&self.inner);
        // The thread waits for the registry lock that is held here, so it is registered before
        // it can remove itself.
        let thread = std::thread::Builder::new()
            .name(format!("export-{id}"))
            .spawn(move || {
                let written = work(&context);
                let mut exports = lock(&registry);
                if let Some(path) = written {
                    exports.written.push(path);
                }
                exports.running.remove(&id);
            })
            .map_err(|error| SourceError::Storage(format!("start the export: {error}")))?;
        exports.next_id = id;
        let first = lock(&latest).clone();
        exports.running.insert(
            id,
            ExportRunning {
                cancel,
                latest,
                thread: Some(thread),
            },
        );
        Ok(first.unwrap_or_else(|| preparing(id)))
    }

    /// Running exports, newest first, for the dialog to catch up with after it opens. One that
    /// has not reported yet is listed as preparing.
    pub fn list(&self) -> Vec<ExportProgress> {
        let exports = lock(&self.inner);
        let mut running: Vec<ExportProgress> = exports
            .running
            .iter()
            .map(|(id, running)| {
                lock(&running.latest)
                    .clone()
                    .unwrap_or_else(|| preparing(*id))
            })
            .collect();
        running.sort_by_key(|progress| std::cmp::Reverse(progress.id));
        running
    }

    /// An export is still running (its final event not sent yet).
    pub fn has_running(&self) -> bool {
        lock(&self.inner).running.values().any(|running| {
            !lock(&running.latest).as_ref().is_some_and(|progress| {
                matches!(
                    progress.state,
                    ExportState::Done | ExportState::Failed | ExportState::Cancelled
                )
            })
        })
    }

    /// Asks a running export to stop; its partial output is removed by the engine.
    pub fn cancel(&self, id: u64) -> Result<(), SourceError> {
        let cancel = lock(&self.inner)
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
    pub fn stop(&self) {
        let threads: Vec<JoinHandle<()>> = {
            let mut exports = lock(&self.inner);
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

    /// Show in Explorer after an export: only a path an export of this session wrote, so a
    /// UI cannot reveal arbitrary paths (D-089).
    pub fn may_reveal(&self, path: &str) -> Result<PathBuf, SourceError> {
        let path = PathBuf::from(path);
        if lock(&self.inner).written.contains(&path) {
            Ok(path)
        } else {
            Err(SourceError::Invalid(
                "Only what an export wrote can be shown.".into(),
            ))
        }
    }

    /// Allows `path` as if an export had written it; for tests of the callers.
    #[doc(hidden)]
    pub fn remember_written(&self, path: PathBuf) {
        lock(&self.inner).written.push(path);
    }
}

/// The progress of an export that has not reported yet.
pub(crate) fn preparing(id: u64) -> ExportProgress {
    ExportProgress {
        id,
        state: ExportState::Preparing,
        total: 0,
        done: 0,
        bytes_total: 0,
        bytes_done: 0,
        current: None,
        summary: None,
        error: None,
    }
}
