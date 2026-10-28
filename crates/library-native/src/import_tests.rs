//! The importer on temporary folders: mixed formats, folders, duplicates, cancel, crashes.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use library_core::{DuplicateNotice, ImportProgress, ImportState};
use media::fixtures;
use rusqlite::params;

use crate::Library;
use crate::import::{
    DuplicateChoice, DuplicatePolicy, ImportObserver, ImportTarget, import_files,
    make_pending_previews,
};
use crate::preview::NO_PREVIEW;
use crate::test_support::*;

/// Records every progress update and answers the duplicate question from a script.
struct Script {
    answer: DuplicateChoice,
    asked: AtomicUsize,
    updates: Mutex<Vec<ImportProgress>>,
}

impl Script {
    fn answering(answer: DuplicateChoice) -> Self {
        Self {
            answer,
            asked: AtomicUsize::new(0),
            updates: Mutex::new(Vec::new()),
        }
    }
}

impl ImportObserver for Script {
    fn progress(&self, progress: &ImportProgress) {
        self.updates.lock().unwrap().push(progress.clone());
    }

    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        self.asked.fetch_add(1, Ordering::Relaxed);
        self.answer
    }
}

/// `count` files cycling through every v1 format, with distinct content per number.
fn mixed_files(dir: &Path, count: u32) -> Vec<PathBuf> {
    mixed_files_from(dir, 0, count)
}

fn mixed_files_from(dir: &Path, first: u32, count: u32) -> Vec<PathBuf> {
    let formats = fixtures::every_format(0).len() as u32;
    (first..first + count)
        .map(|n| {
            let (ext, bytes) = fixtures::every_format(n)
                .into_iter()
                .nth((n % formats) as usize)
                .unwrap();
            write_file(dir, &format!("file-{n:04}.{ext}"), &bytes)
        })
        .collect()
}

fn count(library: &Library, sql: &str) -> i64 {
    library.conn().query_row(sql, [], |row| row.get(0)).unwrap()
}

fn import(library: &Library, sources: &[PathBuf], observer: &Script) -> ImportProgress {
    import_files(
        library,
        1,
        sources,
        &ImportTarget::default(),
        DuplicatePolicy::Ask,
        observer,
        &AtomicBool::new(false),
    )
}

#[test]
fn a_mixed_folder_gets_items_thumbnails_and_palettes() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Mixed");
    let library = open(&root);
    let source = dir.path().join("drop");
    let nested = source.join("nested").join("deeper");
    fs::create_dir_all(&nested).unwrap();
    mixed_files(&source, 8);
    mixed_files_from(&nested, 8, 3);
    write_file(&source, "notes.txt", b"plain text");
    write_file(&source, "clip.avif", b"not decoded in v1");
    write_file(&source, "broken.jpg", b"\xFF\xD8\xFF not really");
    write_file(&source, ".hidden.png", b"skipped");
    write_file(&source, "Thumbs.db", b"skipped");

    let observer = Script::answering(DuplicateChoice::Skip);
    let done = import(&library, &[source], &observer);

    assert_eq!(done.state, ImportState::Done);
    assert_eq!(
        done.total, 14,
        "11 images, a text file, an AVIF, a broken JPEG"
    );
    assert_eq!((done.added, done.duplicates), (14, 0));
    assert_eq!(
        done.previews_total, 12,
        "the text file and the AVIF have no preview"
    );
    assert_eq!(done.previews_done, 12);
    assert_eq!(done.failed, 1);
    assert!(done.failures[0].path.ends_with("broken.jpg"));
    assert!(done.failures[0].reason.starts_with("No preview"));

    assert_eq!(count(&library, "SELECT count(*) FROM item"), 14);
    assert_eq!(
        count(
            &library,
            "SELECT count(*) FROM item WHERE thumb_version = 1"
        ),
        11
    );
    assert_eq!(
        count(
            &library,
            &format!("SELECT count(*) FROM item WHERE thumb_version = {NO_PREVIEW}")
        ),
        1
    );
    assert_eq!(
        count(
            &library,
            "SELECT count(*) FROM item WHERE thumb_version = 1
             AND width = 96 AND height = 64 AND palette LIKE '[{\"color\":%'"
        ),
        11
    );
    let thumbs = files_under(&root.join("thumbs"));
    assert_eq!(thumbs.len(), 11);
    assert!(thumbs.iter().all(|thumb| thumb.ends_with(".webp")));
    assert!(library.pending_previews(10).unwrap().is_empty());
    let counts = library.counts().unwrap();
    assert_eq!(
        (
            counts.all,
            counts.uncategorized,
            counts.untagged,
            counts.trash
        ),
        (14, 0, 14, 0),
        "the dropped folder became a library folder, so nothing is uncategorized"
    );

    let last = observer.updates.lock().unwrap().last().cloned().unwrap();
    assert_eq!(last, done, "the final state reaches the observer");
}

#[test]
fn duplicates_are_asked_once_per_batch() {
    for (answer, added, skipped) in [
        (DuplicateChoice::Skip, 0, 3),
        (DuplicateChoice::AddAnyway, 3, 0),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = new_library(dir.path(), "Dupes");
        let library = open(&root);
        let files = mixed_files(dir.path(), 3);
        import(&library, &files, &Script::answering(DuplicateChoice::Skip));

        let observer = Script::answering(answer);
        let again = import(&library, &files, &observer);
        assert_eq!(observer.asked.load(Ordering::Relaxed), 1, "{answer:?}");
        assert_eq!(
            (again.added, again.duplicates),
            (added, skipped),
            "{answer:?}"
        );
        assert_eq!(again.skipped.len(), skipped as usize);
        let waited = observer.updates.lock().unwrap().iter().any(|update| {
            update.state == ImportState::WaitingForDecision && update.duplicate.is_some()
        });
        assert!(waited, "{answer:?}");
    }
}

#[test]
fn cancelling_at_the_duplicate_question_keeps_what_was_added() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Cancel");
    let library = open(&root);
    let mut files = mixed_files(dir.path(), 2);
    import(
        &library,
        &files[..1],
        &Script::answering(DuplicateChoice::Skip),
    );
    files.rotate_left(1); // the new file first, then the duplicate
    files.push(write_file(
        dir.path(),
        "after.png",
        &fixtures::raster(image_png(), 10, 10, 99),
    ));

    let done = import(
        &library,
        &files,
        &Script::answering(DuplicateChoice::Cancel),
    );
    assert_eq!(done.state, ImportState::Cancelled);
    assert_eq!(done.added, 1, "the file before the question stays");
    assert_eq!(count(&library, "SELECT count(*) FROM item"), 2);
}

fn image_png() -> image::ImageFormat {
    image::ImageFormat::Png
}

#[test]
fn files_inside_the_library_and_missing_files_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Refuse");
    let library = open(&root);
    let inside = write_file(&root.join("thumbs"), "x.png", b"x");
    let done = import(
        &library,
        &[inside, dir.path().join("missing.jpg")],
        &Script::answering(DuplicateChoice::Skip),
    );
    assert_eq!((done.total, done.added, done.failed), (2, 0, 2));
    assert!(done.failures[0].reason.contains("inside this library"));
    assert!(done.failures[1].reason.contains("does not exist"));
}

#[test]
fn a_crash_during_an_import_leaves_a_consistent_library_and_previews_resume() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Crash");
    let library = open(&root);
    let source = dir.path().join("src");
    fs::create_dir_all(&source).unwrap();
    let files = mixed_files(&source, 12);

    // Stop after five files (a copy of the first one asks, the answer is Cancel), then end the
    // process without closing.
    let copy = dir
        .path()
        .join("copy")
        .with_extension(files[0].extension().unwrap());
    fs::copy(&files[0], &copy).unwrap();
    let mut first_run = files.clone();
    first_run.insert(5, copy);
    let partial = import(
        &library,
        &first_run,
        &Script::answering(DuplicateChoice::Cancel),
    );
    assert_eq!(partial.state, ImportState::Cancelled);
    assert_eq!(partial.added, 5);
    // Simulate previews that never ran.
    library
        .conn()
        .execute("UPDATE item SET thumb_version = 0", params![])
        .unwrap();
    let added = partial.added;
    library.abandon();

    let reopened = open(&root);
    assert!(reopened.was_closed_uncleanly());
    assert_eq!(count(&reopened, "SELECT count(*) FROM pending_file"), 0);
    let pending = reopened.pending_previews(100).unwrap();
    assert_eq!(pending.len() as u32, added);

    let previews = make_pending_previews(
        &reopened,
        2,
        &Script::answering(DuplicateChoice::Skip),
        &AtomicBool::new(false),
    );
    assert_eq!(previews.previews_done, added);
    assert!(reopened.pending_previews(10).unwrap().is_empty());

    // Adding the same files again reports the ones that made it as duplicates.
    let again = import_files(
        &reopened,
        3,
        &files,
        &ImportTarget::default(),
        DuplicatePolicy::Skip,
        &Script::answering(DuplicateChoice::Skip),
        &AtomicBool::new(false),
    );
    assert_eq!(again.duplicates, added);
    assert_eq!(again.added, 12 - added);
}

/// The done check of phase 2: 1 000 mixed files plus a few others. Slow in debug builds; run
/// with `cargo test --release -p library-native -- --ignored`.
#[test]
#[ignore]
fn a_thousand_mixed_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Thousand");
    let library = open(&root);
    let source = dir.path().join("src");
    fs::create_dir_all(&source).unwrap();
    mixed_files(&source, 1000);
    for n in 0..20 {
        write_file(
            &source,
            &format!("doc-{n}.pdf"),
            format!("%PDF-1.4 {n}").as_bytes(),
        );
    }
    let started = std::time::Instant::now();
    let done = import(
        &library,
        &[source],
        &Script::answering(DuplicateChoice::Skip),
    );
    eprintln!("1 020 files in {:?}", started.elapsed());
    assert_eq!((done.added, done.failed), (1020, 0));
    assert_eq!(done.previews_done, 1000);
    assert_eq!(
        count(
            &library,
            "SELECT count(*) FROM item WHERE thumb_version = 1 AND palette IS NOT NULL"
        ),
        1000
    );
    assert_eq!(files_under(&root.join("thumbs")).len(), 1000);
}

/// Makes a library for checking the app by hand: `Demo.eglib` with `EGL_DEMO_COUNT` (default
/// 10 000) mixed images inside `EGL_DEMO_PARENT`. Run with
/// `EGL_DEMO_PARENT=<folder> cargo test --release -p library-native -- --ignored demo_library`.
#[test]
#[ignore]
fn demo_library() {
    let Some(parent) = std::env::var_os("EGL_DEMO_PARENT") else {
        eprintln!("set EGL_DEMO_PARENT to the folder that gets Demo.eglib");
        return;
    };
    let count: u32 = std::env::var("EGL_DEMO_COUNT")
        .ok()
        .and_then(|count| count.parse().ok())
        .unwrap_or(10_000);
    let parent = std::path::PathBuf::from(parent);
    let root = new_library(&parent, "Demo");
    let library = open(&root);
    let source = tempfile::tempdir().unwrap();
    mixed_files(source.path(), count);
    let done = import(
        &library,
        &[source.path().to_path_buf()],
        &Script::answering(DuplicateChoice::Skip),
    );
    assert_eq!(done.added, count);
    library.close().unwrap();
    eprintln!("{} items in {}", count, root.display());
}
