//! Importing the synthetic Eagle library into a native library (docs/spec.md §10 "Eagle
//! import"). The committed fixture has placeholder bytes for the originals and thumbnails;
//! these tests copy it into a temporary folder and overwrite the placeholders with real
//! generated images (fixtures/eagle-library/README.md, D-034).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use eagle_folder::EagleLibrary;
use image::ImageFormat;
use library_core::{ImportProgress, ImportState, Scope};
use media::fixtures;

use crate::eagle_import::{EagleImportOptions, import_eagle};
use crate::import::ImportObserver;
use crate::test_support::{self, open, scope_ids};

/// The fixture with real images, in `root`.
fn eagle_fixture(root: &Path) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/eagle-library/Synthetic.library");
    let target = root.join("Synthetic.library");
    copy(&source, &target);
    let images = target.join("images");
    let jpg = |seed: u32| fixtures::jpeg(96, 64, seed);
    let png = |seed: u32| fixtures::raster(ImageFormat::Png, 96, 64, seed);
    let thumb = |seed: u32| fixtures::raster(ImageFormat::Png, 32, 32, seed);
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

fn copy(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap().flatten() {
        let to = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

struct Silent;

impl ImportObserver for Silent {
    fn progress(&self, _progress: &ImportProgress) {}
    fn decide(&self, _duplicate: &library_core::DuplicateNotice) -> crate::import::DuplicateChoice {
        crate::import::DuplicateChoice::Skip
    }
}

/// Cancels the import once `added` items are in.
struct CancelAfter {
    at: u32,
    cancel: Arc<AtomicBool>,
}

impl ImportObserver for CancelAfter {
    fn progress(&self, progress: &ImportProgress) {
        if progress.added >= self.at {
            self.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn decide(&self, _duplicate: &library_core::DuplicateNotice) -> crate::import::DuplicateChoice {
        crate::import::DuplicateChoice::Cancel
    }
}

fn import(
    library: &crate::Library,
    eagle: &EagleLibrary,
    options: &EagleImportOptions,
    cancel: &AtomicBool,
) -> ImportProgress {
    import_eagle(library, eagle, options, 7, &Silent, cancel)
}

#[test]
fn imports_everything_with_the_mapping_of_arch_15_5() {
    let dir = tempfile::tempdir().unwrap();
    let eagle = EagleLibrary::open(&eagle_fixture(dir.path())).unwrap();
    let root = test_support::new_library(dir.path(), "Imported");
    let library = open(&root);

    let progress = import(
        &library,
        &eagle,
        &EagleImportOptions::default(),
        &AtomicBool::new(false),
    );
    assert_eq!(progress.state, ImportState::Done);
    let report = progress.report.expect("the report");
    assert_eq!(report.imported, 5, "{report:?}");
    assert_eq!(report.duplicates, 1, "{report:?}");
    assert_eq!(report.trash_skipped, 1, "{report:?}");
    assert_eq!(report.skipped.len(), 1, "{report:?}");
    assert!(
        report.skipped[0].reason.contains("missing"),
        "{:?}",
        report.skipped[0]
    );
    assert_eq!(report.locked_folders, ["Secret"]);

    // Folders: the same tree and order as Eagle.
    let tree = library.folder_tree().unwrap();
    let names = test_support::names(&tree);
    assert_eq!(names, ["Posters", "Secret", "Empty"]);
    let posters = &tree[0];
    assert_eq!(posters.icon_color.as_deref(), Some("red"));
    assert_eq!(posters.description, "Poster designs");
    assert_eq!(posters.tags, ["design"]);
    assert_eq!(test_support::names(&posters.children), ["2024"]);

    // Tags (case preserved), the tag group and the smart folder.
    // Item tags (case preserved) and tag-group tags; a folder's own labels are not item
    // tags, so "design" stays out of the vocabulary on purpose.
    let vocabulary = library
        .tags()
        .unwrap()
        .into_iter()
        .map(|tag| tag.name)
        .collect::<Vec<_>>();
    for expected in ["poster", "warm", "icon", "cold"] {
        assert!(
            vocabulary.iter().any(|tag| tag == expected),
            "{vocabulary:?}"
        );
    }
    assert!(
        !vocabulary.iter().any(|tag| tag == "design"),
        "a folder label is not an item tag: {vocabulary:?}"
    );
    let groups = library.tag_groups().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Moods");
    assert_eq!(groups[0].tags, ["cold", "warm"]);
    let smart = library.smart_folders().unwrap();
    assert_eq!(smart.len(), 1);
    assert_eq!(smart[0].name, "Rated");

    // Items: fields, membership, the duplicate pair kept separate, the comment.
    let counts = library.counts().unwrap();
    assert_eq!(counts.all, 4, "{counts:?}"); // the locked folder's item stays hidden
    assert_eq!(counts.trash, 0);
    let native: String = library
        .conn()
        .query_row(
            "SELECT id FROM item WHERE eagle_id = 'EITEMPOSTER1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let poster = library.item(&native).unwrap().expect("the poster item");
    assert_eq!(poster.name, "sunset-poster");
    assert_eq!(poster.star, 3);
    assert_eq!(poster.tags, ["poster", "warm"]);
    assert_eq!(poster.annotation, "A sunset poster");
    assert_eq!(poster.url, "https://example.com/sunset");
    assert_eq!(poster.palettes.len(), 2);
    assert_eq!(poster.width, Some(96));
    assert_eq!(poster.added_at, Some(1792000000000));
    assert_eq!(poster.folders.len(), 2);
    let content_same = library
        .conn()
        .query_row(
            "SELECT count(*) FROM item WHERE content_hash = (
                 SELECT content_hash FROM item WHERE eagle_id = 'EITEMPOSTER1')",
            [],
            |row| row.get::<_, u32>(0),
        )
        .unwrap();
    assert_eq!(content_same, 2, "the Eagle duplicates stay separate items");
    let comments: u32 = library
        .conn()
        .query_row(
            "SELECT count(*) FROM comment WHERE item_id = (
                 SELECT id FROM item WHERE eagle_id = 'EITEMLOOSE01')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(comments, 1);

    // The Eagle thumbnail is the first thumbnail; the item without one got a generated
    // preview in the background.
    for eagle_id in ["EITEMPOSTER1", "EITEMNOTHUMB1"] {
        let version: i64 = library
            .conn()
            .query_row(
                "SELECT thumb_version FROM item WHERE eagle_id = ?1",
                [eagle_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(version > 0, "{eagle_id} has no thumbnail: {version}");
    }

    // Manual order from Eagle's `order` (correction 39).
    let position: Option<f64> = library
        .conn()
        .query_row(
            "SELECT item_folder.position FROM item_folder
             JOIN item ON item.id = item_folder.item_id
             JOIN folder ON folder.id = item_folder.folder_id
             WHERE item.eagle_id = 'EITEMPOSTER1' AND folder.eagle_id = 'FPOSTERS'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(position, Some(0.0));
    library.close().unwrap();
}

#[test]
fn locked_folders_import_with_no_password_and_plaintext_files() {
    let dir = tempfile::tempdir().unwrap();
    let eagle = EagleLibrary::open(&eagle_fixture(dir.path())).unwrap();
    let root = test_support::new_library(dir.path(), "Imported");
    let library = open(&root);
    import(
        &library,
        &eagle,
        &EagleImportOptions::default(),
        &AtomicBool::new(false),
    );

    let (locked, salt, identity): (bool, Option<Vec<u8>>, Option<Vec<u8>>) = library
        .conn()
        .query_row(
            "SELECT locked, kdf_salt, wrapped_identity FROM folder WHERE eagle_id = 'FSECRET'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert!(locked);
    assert!(salt.is_none() && identity.is_none());

    // The item exists but is hidden from every scope until a password is set.
    let hidden: u32 = library
        .conn()
        .query_row(
            "SELECT count(*) FROM item WHERE eagle_id = 'EITEMLOCKED1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(hidden, 1);
    let all = scope_ids(&library, &Scope::All);
    let native: String = library
        .conn()
        .query_row(
            "SELECT id FROM item WHERE eagle_id = 'EITEMLOCKED1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!all.contains(&native), "the locked item shows in All");

    // The file stays plaintext: the stored bytes are the original bytes.
    let stored: String = library
        .conn()
        .query_row(
            "SELECT file_path FROM item WHERE eagle_id = 'EITEMLOCKED1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let bytes = fs::read(library.root().join(stored)).unwrap();
    assert_eq!(bytes, fixtures::raster(ImageFormat::Png, 96, 64, 2));

    // The Eagle password never reaches the library.
    let db = fs::read(root.join("library.db")).unwrap();
    assert!(
        !db.windows(b"synthetic-password".len())
            .any(|window| window == b"synthetic-password"),
        "the folder password leaked into library.db"
    );
    library.close().unwrap();
}

#[test]
fn trash_imports_into_the_trash_only_when_asked() {
    let dir = tempfile::tempdir().unwrap();
    let eagle_path = eagle_fixture(dir.path());
    let root = test_support::new_library(dir.path(), "Imported");

    let eagle = EagleLibrary::open(&eagle_path).unwrap();
    let library = open(&root);
    let progress = import(
        &library,
        &eagle,
        &EagleImportOptions::default(),
        &AtomicBool::new(false),
    );
    assert_eq!(progress.report.expect("the report").trash_skipped, 1);
    assert_eq!(library.counts().unwrap().trash, 0);
    library.close().unwrap();

    let eagle = EagleLibrary::open(&eagle_path).unwrap();
    let library = open(&root);
    let progress = import(
        &library,
        &eagle,
        &EagleImportOptions {
            import_trash: true,
            root_folder: None,
        },
        &AtomicBool::new(false),
    );
    let report = progress.report.expect("the report");
    assert_eq!(report.trash_skipped, 0);
    assert_eq!(report.imported, 1, "{report:?}");
    assert_eq!(library.counts().unwrap().trash, 1);
    assert_eq!(scope_ids(&library, &Scope::Trash).len(), 1);
    library.close().unwrap();
}

#[test]
fn an_interrupted_import_resumes_without_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let eagle = EagleLibrary::open(&eagle_fixture(dir.path())).unwrap();
    let root = test_support::new_library(dir.path(), "Imported");
    let library = open(&root);

    let cancel = Arc::new(AtomicBool::new(false));
    let interrupted = import_eagle(
        &library,
        &eagle,
        &EagleImportOptions::default(),
        7,
        &CancelAfter {
            at: 2,
            cancel: Arc::clone(&cancel),
        },
        &cancel,
    );
    assert_eq!(interrupted.state, ImportState::Cancelled);

    let resumed = import(
        &library,
        &eagle,
        &EagleImportOptions::default(),
        &AtomicBool::new(false),
    );
    assert_eq!(resumed.state, ImportState::Done);
    let imported: u32 = library
        .conn()
        .query_row("SELECT count(*) FROM eagle_import", [], |row| row.get(0))
        .unwrap();
    assert_eq!(imported, 5, "no item imported twice");
    let items: u32 = library
        .conn()
        .query_row("SELECT count(*) FROM item", [], |row| row.get(0))
        .unwrap();
    assert_eq!(items, 5);

    // The folder tree was not duplicated by the resume.
    let tree = library.folder_tree().unwrap();
    assert_eq!(test_support::names(&tree), ["Posters", "Secret", "Empty"]);
    library.close().unwrap();
}

#[test]
fn unreadable_item_metadata_becomes_a_report_line() {
    let dir = tempfile::tempdir().unwrap();
    let eagle_path = eagle_fixture(dir.path());
    fs::write(
        eagle_path.join("images/EITEMLOOSE01.info/metadata.json"),
        "this is not json",
    )
    .unwrap();
    let eagle = EagleLibrary::open(&eagle_path).unwrap();
    let root = test_support::new_library(dir.path(), "Imported");
    let library = open(&root);

    let progress = import(
        &library,
        &eagle,
        &EagleImportOptions::default(),
        &AtomicBool::new(false),
    );
    let report = progress.report.expect("the report");
    assert_eq!(report.imported, 4, "{report:?}");
    assert_eq!(report.skipped.len(), 2, "{report:?}");
    assert!(
        report
            .skipped
            .iter()
            .any(|line| line.reason.contains("not valid JSON")),
        "{:?}",
        report.skipped
    );
    library.close().unwrap();
}

#[test]
fn importing_into_the_open_library_goes_under_a_root_folder() {
    let dir = tempfile::tempdir().unwrap();
    let eagle = EagleLibrary::open(&eagle_fixture(dir.path())).unwrap();
    let root = test_support::new_library(dir.path(), "Open");
    let library = open(&root);

    let progress = import(
        &library,
        &eagle,
        &EagleImportOptions {
            import_trash: false,
            root_folder: Some("Synthetic".into()),
        },
        &AtomicBool::new(false),
    );
    assert_eq!(progress.report.expect("the report").imported, 5);
    let tree = library.folder_tree().unwrap();
    assert_eq!(test_support::names(&tree), ["Synthetic"]);
    assert_eq!(
        test_support::names(&tree[0].children),
        ["Posters", "Secret", "Empty"]
    );
    library.close().unwrap();
}
