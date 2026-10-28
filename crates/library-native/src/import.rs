//! Adding many files: walking dropped folders, one duplicate decision per batch, cancelling,
//! progress, and previews on a small thread pool (docs/spec.md §4, docs/architecture.md §7.4).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use library_core::{
    DuplicateNotice, ImportFailure, ImportKind, ImportProgress, ImportState, SourceError,
};

use crate::add::{AddOutcome, Duplicates};
use crate::library::Library;
use crate::time::now_ms;

/// What to do with files whose content is already in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicatePolicy {
    /// Ask once, at the first duplicate; the answer holds for the rest of the batch.
    Ask,
    Skip,
    AddAnyway,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateChoice {
    Skip,
    AddAnyway,
    /// Stop the import; files already added stay.
    Cancel,
}

/// Receives progress and answers the duplicate question. `decide` may block until the user
/// chooses.
pub trait ImportObserver: Send + Sync {
    fn progress(&self, progress: &ImportProgress);
    fn decide(&self, duplicate: &DuplicateNotice) -> DuplicateChoice;
}

/// Failures and skipped duplicates listed in the report; the counters keep counting past this.
const LISTED: usize = 200;
/// Files that are never worth adding.
const JUNK: &[&str] = &["thumbs.db", "desktop.ini", ".ds_store"];
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// Threads for thumbnails: leave a core for the UI, and at most four big decodes at once.
pub fn preview_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(2, |cores| cores.get().saturating_sub(1))
        .clamp(1, 4)
}

/// Adds `sources` (files and folders, walked recursively) and makes their previews.
/// Returns the final progress. Cancelling stops after the current file; added files stay, and
/// previews not made yet are left for the next open.
pub fn import_files(
    library: &Library,
    id: u64,
    sources: &[PathBuf],
    policy: DuplicatePolicy,
    observer: &dyn ImportObserver,
    cancel: &AtomicBool,
) -> ImportProgress {
    let tracker = Tracker::new(id, ImportKind::Files, observer);
    tracker.update(true, |progress| progress.state = ImportState::Scanning);
    let (files, refused) = collect(library.root(), sources, cancel);
    tracker.update(true, |progress| {
        progress.total = u32::try_from(files.len() + refused.len()).unwrap_or(u32::MAX);
        progress.processed = u32::try_from(refused.len()).unwrap_or(u32::MAX);
        for failure in refused {
            note_failure(progress, failure);
        }
        progress.state = ImportState::Adding;
    });

    let mut policy = policy;
    let pool = match rayon::ThreadPoolBuilder::new()
        .num_threads(preview_threads())
        .build()
    {
        Ok(pool) => pool,
        Err(error) => {
            tracing::warn!("no thread pool for previews: {error}");
            return tracker.finish(cancel);
        }
    };
    pool.in_place_scope(|scope| {
        for file in &files {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            tracker.update(false, |progress| {
                progress.current = file.file_name().map(|name| name.to_string_lossy().into());
            });
            match add_one(library, file, &mut policy, &tracker, cancel) {
                Outcome::Added { id, stored } => {
                    let ext = extension(file);
                    let preview = media::has_preview(&ext);
                    tracker.update(false, |progress| {
                        progress.added += 1;
                        progress.processed += 1;
                        if preview {
                            progress.previews_total += 1;
                        }
                    });
                    if preview {
                        let tracker = &tracker;
                        scope.spawn(move |_| {
                            make_preview(library, &id, &stored, &ext, tracker, cancel);
                        });
                    }
                }
                Outcome::Skipped(notice) => tracker.update(false, |progress| {
                    progress.duplicates += 1;
                    progress.processed += 1;
                    if progress.skipped.len() < LISTED {
                        progress.skipped.push(notice);
                    }
                }),
                Outcome::Failed(reason) => tracker.update(false, |progress| {
                    progress.processed += 1;
                    note_failure(progress, failure_for(file, reason));
                }),
                Outcome::Cancelled => break,
            }
        }
        tracker.update(true, |progress| {
            progress.current = None;
            if !cancel.load(Ordering::Relaxed) {
                progress.state = ImportState::Previews;
            }
        });
    });
    tracker.finish(cancel)
}

/// Makes the thumbnails that a crash or a cancelled import left undone.
pub fn make_pending_previews(
    library: &Library,
    id: u64,
    observer: &dyn ImportObserver,
    cancel: &AtomicBool,
) -> ImportProgress {
    let tracker = Tracker::new(id, ImportKind::Previews, observer);
    tracker.update(true, |progress| progress.state = ImportState::Previews);
    let Ok(pool) = rayon::ThreadPoolBuilder::new()
        .num_threads(preview_threads())
        .build()
    else {
        return tracker.finish(cancel);
    };
    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let batch = match library.pending_previews(64) {
            Ok(batch) if !batch.is_empty() => batch,
            Ok(_) => break,
            Err(error) => {
                tracing::warn!("cannot list items without previews: {error}");
                break;
            }
        };
        let first = batch[0].id.clone();
        tracker.update(true, |progress| {
            progress.previews_total += u32::try_from(batch.len()).unwrap_or(u32::MAX);
        });
        pool.in_place_scope(|scope| {
            for pending in &batch {
                let tracker = &tracker;
                scope.spawn(move |_| {
                    make_preview(
                        library,
                        &pending.id,
                        &pending.file,
                        &pending.ext,
                        tracker,
                        cancel,
                    );
                });
            }
        });
        // Every item of a batch leaves the pending list (stored or marked); if the first one
        // is still pending, the database refuses writes: stop instead of looping.
        if library
            .pending_previews(1)
            .is_ok_and(|next| next.first().is_some_and(|next| next.id == first))
        {
            break;
        }
    }
    tracker.finish(cancel)
}

enum Outcome {
    Added { id: String, stored: PathBuf },
    Skipped(DuplicateNotice),
    Failed(String),
    Cancelled,
}

fn add_one(
    library: &Library,
    file: &Path,
    policy: &mut DuplicatePolicy,
    tracker: &Tracker<'_>,
    cancel: &AtomicBool,
) -> Outcome {
    let duplicates = match policy {
        DuplicatePolicy::AddAnyway => Duplicates::AddAnyway,
        DuplicatePolicy::Ask | DuplicatePolicy::Skip => Duplicates::Report,
    };
    let (existing, in_trash) = match library.add_file(file, duplicates) {
        Ok(AddOutcome::Added { id, file }) => return Outcome::Added { id, stored: file },
        Ok(AddOutcome::Duplicate { existing, in_trash }) => (existing, in_trash),
        Err(error) => return Outcome::Failed(reason(&error)),
    };
    let notice = DuplicateNotice {
        path: file.to_string_lossy().into_owned(),
        existing,
        in_trash,
    };
    if *policy != DuplicatePolicy::Ask {
        return Outcome::Skipped(notice);
    }
    tracker.update(true, |progress| {
        progress.state = ImportState::WaitingForDecision;
        progress.duplicate = Some(notice.clone());
    });
    let choice = tracker.observer.decide(&notice);
    tracker.update(true, |progress| {
        progress.state = ImportState::Adding;
        progress.duplicate = None;
    });
    match choice {
        DuplicateChoice::Skip => {
            *policy = DuplicatePolicy::Skip;
            Outcome::Skipped(notice)
        }
        DuplicateChoice::AddAnyway => {
            *policy = DuplicatePolicy::AddAnyway;
            match library.add_file(file, Duplicates::AddAnyway) {
                Ok(AddOutcome::Added { id, file }) => Outcome::Added { id, stored: file },
                Ok(AddOutcome::Duplicate { .. }) => Outcome::Skipped(notice),
                Err(error) => Outcome::Failed(reason(&error)),
            }
        }
        DuplicateChoice::Cancel => {
            cancel.store(true, Ordering::Relaxed);
            Outcome::Cancelled
        }
    }
}

/// Decodes the stored copy; a file that cannot be decoded keeps its placeholder and gets a line
/// in the report.
fn make_preview(
    library: &Library,
    id: &str,
    stored: &Path,
    ext: &str,
    tracker: &Tracker<'_>,
    cancel: &AtomicBool,
) {
    if cancel.load(Ordering::Relaxed) {
        return;
    }
    let failure = match media::render(stored, ext) {
        Ok(rendered) => library
            .store_preview(id, &rendered)
            .err()
            .map(|error| reason(&error)),
        Err(media::MediaError::Unsupported) => None,
        Err(error) => {
            let _ = library.mark_no_preview(id);
            Some(format!("No preview: {error}"))
        }
    };
    tracker.update(false, |progress| {
        progress.previews_done += 1;
        if let Some(reason) = failure {
            note_failure(progress, failure_for(stored, reason));
        }
    });
}

/// Files to add, and sources refused before copying (missing, inside this library).
fn collect(
    root: &Path,
    sources: &[PathBuf],
    cancel: &AtomicBool,
) -> (Vec<PathBuf>, Vec<ImportFailure>) {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut files = Vec::new();
    let mut refused = Vec::new();
    for source in sources {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(metadata) = fs::metadata(source) else {
            refused.push(failure_for(
                source,
                "The file or folder does not exist.".into(),
            ));
            continue;
        };
        if source
            .canonicalize()
            .is_ok_and(|canonical| canonical.starts_with(&root))
        {
            refused.push(failure_for(
                source,
                "It is already inside this library.".into(),
            ));
            continue;
        }
        if metadata.is_dir() {
            walk(source, &mut files, cancel);
        } else if !is_junk(source) {
            files.push(source.clone());
        }
    }
    (files, refused)
}

/// Every file under `dir`, in name order; hidden and system files and linked folders are
/// skipped.
fn walk(dir: &Path, files: &mut Vec<PathBuf>, cancel: &AtomicBool) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if is_junk(&path) || is_hidden(&entry) {
            continue;
        }
        if kind.is_dir() {
            walk(&path, files, cancel);
        } else if kind.is_file() || (kind.is_symlink() && path.is_file()) {
            files.push(path);
        }
    }
}

fn is_junk(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.starts_with('.') || JUNK.contains(&name.to_lowercase().as_str())
    })
}

#[cfg(windows)]
fn is_hidden(entry: &fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    entry
        .metadata()
        .is_ok_and(|metadata| metadata.file_attributes() & (HIDDEN | SYSTEM) != 0)
}

#[cfg(not(windows))]
fn is_hidden(_entry: &fs::DirEntry) -> bool {
    false
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn reason(error: &SourceError) -> String {
    match error {
        SourceError::Storage(detail) => detail.clone(),
        other => other.to_string(),
    }
}

fn failure_for(path: &Path, reason: String) -> ImportFailure {
    ImportFailure {
        path: path.to_string_lossy().into_owned(),
        reason,
    }
}

fn note_failure(progress: &mut ImportProgress, failure: ImportFailure) {
    progress.failed += 1;
    if progress.failures.len() < LISTED {
        progress.failures.push(failure);
    }
}

/// The shared progress of one task, sent to the observer at most every 50 ms unless a change
/// matters (state changes, the end).
struct Tracker<'a> {
    progress: Mutex<(ImportProgress, Instant)>,
    observer: &'a dyn ImportObserver,
}

impl<'a> Tracker<'a> {
    fn new(id: u64, kind: ImportKind, observer: &'a dyn ImportObserver) -> Self {
        let progress = ImportProgress {
            id,
            kind,
            state: ImportState::Scanning,
            total: 0,
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
            started_at: now_ms(),
            finished_at: None,
        };
        Self {
            progress: Mutex::new((progress, Instant::now())),
            observer,
        }
    }

    fn update(&self, force: bool, change: impl FnOnce(&mut ImportProgress)) {
        let mut guard = self.progress.lock().unwrap_or_else(PoisonError::into_inner);
        let (progress, sent) = &mut *guard;
        change(progress);
        if force || sent.elapsed() >= PROGRESS_INTERVAL {
            *sent = Instant::now();
            self.observer.progress(progress);
        }
    }

    fn finish(self, cancel: &AtomicBool) -> ImportProgress {
        self.update(true, |progress| {
            progress.current = None;
            progress.duplicate = None;
            progress.state = if cancel.load(Ordering::Relaxed) {
                ImportState::Cancelled
            } else {
                ImportState::Done
            };
            progress.finished_at = Some(now_ms());
        });
        self.progress
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .0
    }
}
