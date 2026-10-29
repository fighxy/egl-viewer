//! Locked folders: encryption at rest, hiding, unlocking, recovery, removal, and every write
//! that takes items under a lock or out from under one (docs/age-locked-folders.md, spec §9
//! phase 4).

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use library_core::{
    DuplicateNotice, FolderChange, ImportProgress, ItemEdit, ItemId, PivPolicy, PivPrompt,
    RecoveryKey, Scope, SourceError,
};
use rusqlite::params;

use crate::add::Placement;
use crate::import::{
    DuplicateChoice, DuplicatePolicy, ImportObserver, ImportTarget, import_files,
    make_pending_previews,
};
use crate::piv::SoftwarePiv;
use crate::test_support::*;
use crate::vault::AGE_MAGIC;
use crate::{AddOutcome, Duplicates, Library};

struct Quiet;

impl ImportObserver for Quiet {
    fn progress(&self, _progress: &ImportProgress) {}
    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        DuplicateChoice::Skip
    }
}

fn previews(library: &Library) {
    make_pending_previews(library, 1, &Quiet, &AtomicBool::new(false));
}

fn lock(library: &Library, folder: &str, password: &str) -> RecoveryKey {
    library
        .lock_folder(folder, password.into(), None, None, &|_, _| {})
        .unwrap()
}

/// The stored row of an item: name, encrypted, file path, sealed fields.
fn row(library: &Library, item: &str) -> (String, bool, String, Option<Vec<u8>>) {
    library
        .conn()
        .query_row(
            "SELECT name, encrypted, file_path, sealed FROM item WHERE id = ?1",
            params![item],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap()
}

fn wraps(library: &Library, item: &str) -> Vec<String> {
    let conn = library.conn();
    let mut statement = conn
        .prepare("SELECT folder_id FROM item_key_wrap WHERE item_id = ?1 ORDER BY folder_id")
        .unwrap();
    statement
        .query_map(params![item], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn search_rows(library: &Library, item: &str) -> i64 {
    library
        .conn()
        .query_row(
            "SELECT count(*) FROM item_fts WHERE rowid = (SELECT seq FROM item WHERE id = ?1)",
            params![item],
            |row| row.get(0),
        )
        .unwrap()
}

fn all(library: &Library) -> Vec<String> {
    scope_ids(library, &Scope::All)
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

fn starts_with_age(path: &Path) -> bool {
    fs::read(path).is_ok_and(|bytes| bytes.starts_with(AGE_MAGIC))
}

fn stored(root: &Path, file_path: &str) -> std::path::PathBuf {
    root.join(file_path)
}

#[test]
fn a_lock_encrypts_the_folder_and_hides_it_until_unlocked() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Locks");
    let library = open(&root);
    let private = create(&library, "Private", None);
    let public = create(&library, "Public", None);
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    let c = add_png(&library, dir.path(), "c.png", 3);
    put(&library, &a, &[&private]);
    put(&library, &b, &[&private, &public]);
    put(&library, &c, &[&public]);
    previews(&library);
    let source_a = fs::read(dir.path().join("a.png")).unwrap();

    let key = lock(&library, &private, "correct horse");
    assert_eq!(key.key.len(), 34);
    assert_eq!(key.phrase.split(' ').count(), 12);

    // Hidden everywhere, an item in a second, open folder included.
    assert_eq!(all(&library), vec![c.clone()]);
    assert_eq!(
        scope_ids(&library, &folder_scope(&public, false)),
        vec![c.clone()]
    );
    assert_eq!(library.counts().unwrap().all, 1);
    assert_eq!(library.counts().unwrap().uncategorized, 0);
    let tree = library.folder_tree().unwrap();
    let locked = find(&tree, &private);
    assert!(locked.is_locked && !locked.is_unlocked);
    assert_eq!(locked.image_count, None);
    assert_eq!(find(&tree, &public).image_count, Some(1));
    assert_eq!(library.item(&a).unwrap(), None);
    assert_eq!(library.thumbnail(&ItemId(a.clone()).0).unwrap(), None);
    assert_eq!(library.original(&a).unwrap(), None);

    // At rest: age ciphertext, no names, no search rows, no plaintext copies.
    for item in [&a, &b] {
        let (name, encrypted, file_path, sealed) = row(&library, item);
        assert_eq!(name, "");
        assert!(encrypted);
        assert!(file_path.ends_with(".age"), "{file_path}");
        assert!(starts_with_age(&stored(&root, &file_path)));
        assert!(sealed.unwrap().starts_with(AGE_MAGIC));
        assert_eq!(wraps(&library, item), vec![private.clone()]);
        assert_eq!(search_rows(&library, item), 0);
        assert!(starts_with_age(
            &library.sealed_thumbnail_path(item).unwrap()
        ));
        assert!(!library.thumbnail_path(item).unwrap().exists());
    }
    let files = files_under(&root.join("files"));
    assert!(
        !files
            .iter()
            .any(|file| file.ends_with("a.png") || file.ends_with("b.png")),
        "{files:?}"
    );
    assert!(files.iter().any(|file| file.ends_with("c.png")));
    let (name, encrypted, _, _) = row(&library, &c);
    assert_eq!((name.as_str(), encrypted), ("c", false));
    // Format version 4 already keeps older apps out of a library with locks (D-047, D-056).
    let version: u32 = library
        .conn()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 4);

    // Unlocking: everything comes back, decrypted in memory only.
    assert!(matches!(
        library.unlock_folder(&private, "wrong".into()),
        Err(SourceError::WrongPassword)
    ));
    assert_eq!(all(&library), vec![c.clone()]);
    library
        .unlock_folder(&private, "correct horse".into())
        .unwrap();
    assert_eq!(all(&library), sorted(vec![a.clone(), b.clone(), c.clone()]));
    let lite = library.load_scope(&Scope::All).unwrap();
    let lite_a = lite.iter().find(|item| item.id.0 == a).unwrap();
    assert_eq!(lite_a.name, "a");
    assert!(lite_a.palette0.is_some());
    let item = library.item(&a).unwrap().unwrap();
    assert_eq!(item.name, "a");
    assert!(!item.palettes.is_empty());
    assert!(
        library
            .thumbnail(&a)
            .unwrap()
            .unwrap()
            .bytes
            .starts_with(b"RIFF")
    );
    assert_eq!(library.original(&a).unwrap().unwrap().bytes, source_a);
    let tree = library.folder_tree().unwrap();
    assert!(find(&tree, &private).is_unlocked);
    assert_eq!(find(&tree, &private).image_count, Some(2));
    // Still ciphertext on disk while open.
    assert!(starts_with_age(&stored(&root, &row(&library, &a).2)));

    library.relock_folder(&private);
    assert_eq!(all(&library), [c]);
}

#[test]
fn each_locked_folder_has_its_own_password_and_keys() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Two"));
    let first = create(&library, "First", None);
    let second = create(&library, "Second", None);
    let only_first = add_png(&library, dir.path(), "x.png", 1);
    let only_second = add_png(&library, dir.path(), "y.png", 2);
    let both = add_png(&library, dir.path(), "z.png", 3);
    put(&library, &only_first, &[&first]);
    put(&library, &only_second, &[&second]);
    put(&library, &both, &[&first, &second]);
    lock(&library, &first, "one");
    // The shared item is encrypted for the first lock: the second needs its key.
    assert_eq!(
        library
            .lock_folder(&second, "two".into(), None, None, &|_, _| {})
            .err(),
        Some(SourceError::Locked {
            folder: "First".into()
        })
    );
    library.unlock_folder(&first, "one".into()).unwrap();
    lock(&library, &second, "two");
    library.relock_folder(&first);
    assert_eq!(
        sorted(wraps(&library, &both)),
        sorted(vec![first.clone(), second.clone()])
    );

    // The other folder's password opens nothing here.
    assert!(matches!(
        library.unlock_folder(&first, "two".into()),
        Err(SourceError::WrongPassword)
    ));
    library.unlock_folder(&first, "one".into()).unwrap();
    // The shared item stays hidden while the second folder is locked.
    assert_eq!(all(&library), vec![only_first.clone()]);
    assert!(!library.is_unlocked(&second));
    library.relock_folder(&first);
    library.unlock_folder(&second, "two".into()).unwrap();
    assert_eq!(all(&library), vec![only_second.clone()]);
    library.unlock_folder(&first, "one".into()).unwrap();
    assert_eq!(all(&library), sorted(vec![only_first, only_second, both]));
}

#[test]
fn the_recovery_key_or_phrase_sets_a_new_password() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Recover"));
    let folder = create(&library, "Private", None);
    let item = add_png(&library, dir.path(), "a.png", 1);
    put(&library, &item, &[&folder]);
    let key = lock(&library, &folder, "forgotten");

    assert!(matches!(
        library.recover_folder(&folder, "AAAA-BBBB", "new".into()),
        Err(SourceError::WrongPassword)
    ));
    library
        .recover_folder(&folder, &key.key.to_lowercase(), "new".into())
        .unwrap();
    assert_eq!(all(&library), vec![item.clone()], "recovery also unlocks");
    library.relock_folder(&folder);
    assert!(matches!(
        library.unlock_folder(&folder, "forgotten".into()),
        Err(SourceError::WrongPassword)
    ));
    library.unlock_folder(&folder, "new".into()).unwrap();
    library.relock_folder(&folder);
    library
        .recover_folder(&folder, &key.phrase, "newer".into())
        .unwrap();
    library.relock_folder(&folder);
    library.unlock_folder(&folder, "newer".into()).unwrap();
    assert_eq!(all(&library), [item]);
}

#[test]
fn a_new_password_keeps_the_recovery_key_and_a_new_key_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Change"));
    let folder = create(&library, "Private", None);
    let first_key = lock(&library, &folder, "pw");
    assert!(matches!(
        library.change_folder_password(&folder, "wrong".into(), "pw2".into()),
        Err(SourceError::WrongPassword)
    ));
    library
        .change_folder_password(&folder, "pw".into(), "pw2".into())
        .unwrap();
    assert!(library.unlock_folder(&folder, "pw".into()).is_err());
    library.unlock_folder(&folder, "pw2".into()).unwrap();
    library.relock_folder(&folder);

    let second_key = library.renew_recovery_key(&folder, "pw2".into()).unwrap();
    assert_ne!(second_key.key, first_key.key);
    assert!(matches!(
        library.recover_folder(&folder, &first_key.key, "x".into()),
        Err(SourceError::WrongPassword)
    ));
    library
        .recover_folder(&folder, &second_key.key, "pw3".into())
        .unwrap();
}

#[test]
fn removing_a_lock_decrypts_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Remove");
    let library = open(&root);
    let folder = create(&library, "Private", None);
    let item = add_png(&library, dir.path(), "Ünïcode name.png", 1);
    put(&library, &item, &[&folder]);
    previews(&library);
    let source = fs::read(dir.path().join("Ünïcode name.png")).unwrap();
    lock(&library, &folder, "pw");

    assert!(matches!(
        library.remove_folder_lock(&folder, "wrong".into(), &|_, _| {}),
        Err(SourceError::WrongPassword)
    ));
    assert!(row(&library, &item).1);
    library
        .remove_folder_lock(&folder, "pw".into(), &|_, _| {})
        .unwrap();
    let (name, encrypted, file_path, sealed) = row(&library, &item);
    assert_eq!(name, "Ünïcode name");
    assert!(!encrypted);
    assert_eq!(sealed, None);
    assert!(file_path.ends_with("Ünïcode name.png"), "{file_path}");
    assert_eq!(fs::read(stored(&root, &file_path)).unwrap(), source);
    assert!(wraps(&library, &item).is_empty());
    assert_eq!(search_rows(&library, &item), 1);
    assert!(library.thumbnail_path(&item).unwrap().exists());
    assert!(!library.sealed_thumbnail_path(&item).unwrap().exists());
    let tree = library.folder_tree().unwrap();
    assert!(!find(&tree, &folder).is_locked);
    assert_eq!(all(&library), [item]);
    assert!(
        !files_under(&root.join("files"))
            .iter()
            .any(|file| file.ends_with(".age"))
    );
}

#[test]
fn files_added_to_a_locked_folder_are_encrypted_as_they_are_copied() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Add");
    let library = open(&root);
    let folder = create(&library, "Private", None);
    lock(&library, &folder, "pw");
    let source = write_file(
        dir.path(),
        "secret.png",
        &media::fixtures::raster(image::ImageFormat::Png, 12, 8, 4),
    );
    let placement = Placement {
        folder: Some(folder.clone()),
        tags: Vec::new(),
    };
    assert_eq!(
        library.add_file_to(&source, Duplicates::Report, &placement),
        Err(SourceError::Locked {
            folder: "Private".into()
        })
    );

    library.unlock_folder(&folder, "pw".into()).unwrap();
    let progress = import_files(
        &library,
        1,
        std::slice::from_ref(&source),
        &ImportTarget {
            folder: Some(folder.clone()),
        },
        DuplicatePolicy::Ask,
        &Quiet,
        &AtomicBool::new(false),
    );
    assert_eq!(progress.added, 1);
    let ids = all(&library);
    let item = &ids[0];
    let (name, encrypted, file_path, _) = row(&library, item);
    assert_eq!((name.as_str(), encrypted), ("", true));
    assert!(starts_with_age(&stored(&root, &file_path)));
    assert!(files_under(&root.join("files").join(".tmp")).is_empty());
    assert_eq!(wraps(&library, item), vec![folder.clone()]);
    let full = library.item(item).unwrap().unwrap();
    assert_eq!(full.name, "secret");
    assert_eq!((full.width, full.height), (Some(12), Some(8)));
    assert!(
        !full.palettes.is_empty(),
        "the preview's palette is sealed with the item"
    );
    assert!(starts_with_age(
        &library.sealed_thumbnail_path(item).unwrap()
    ));
    assert_eq!(
        library.original(item).unwrap().unwrap().bytes,
        fs::read(&source).unwrap()
    );

    library.relock_folder(&folder);
    assert!(all(&library).is_empty());
    // A file that is only in a locked folder is not a duplicate the question may reveal.
    assert!(matches!(
        library.add_file(&source, Duplicates::Report).unwrap(),
        AddOutcome::Added { .. }
    ));
}

#[test]
fn joining_or_leaving_a_locked_folder_encrypts_or_decrypts() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Membership");
    let library = open(&root);
    let folder = create(&library, "Private", None);
    lock(&library, &folder, "pw");
    let item = add_png(&library, dir.path(), "a.png", 1);
    let source = fs::read(dir.path().join("a.png")).unwrap();

    assert_eq!(
        library.apply_item_edit(&item, &ItemEdit::AddToFolders(vec![folder.clone()])),
        Err(SourceError::Locked {
            folder: "Private".into()
        })
    );
    assert!(!row(&library, &item).1);
    assert_eq!(
        library.item(&item).unwrap().unwrap().folders,
        Vec::<String>::new()
    );

    library.unlock_folder(&folder, "pw".into()).unwrap();
    let joined = library
        .apply_item_edit(&item, &ItemEdit::AddToFolders(vec![folder.clone()]))
        .unwrap();
    assert_eq!(joined.name, "a");
    assert!(row(&library, &item).1);
    assert!(starts_with_age(&stored(&root, &row(&library, &item).2)));

    library
        .apply_item_edit(&item, &ItemEdit::RemoveFromFolders(vec![folder.clone()]))
        .unwrap();
    let (name, encrypted, file_path, _) = row(&library, &item);
    assert_eq!((name.as_str(), encrypted), ("a", false));
    assert_eq!(fs::read(stored(&root, &file_path)).unwrap(), source);
    assert!(wraps(&library, &item).is_empty());
}

#[test]
fn moving_a_folder_under_a_lock_or_out_of_it_follows_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Moves"));
    let locked = create(&library, "Private", None);
    let moving = create(&library, "Drafts", None);
    let item = add_png(&library, dir.path(), "a.png", 1);
    put(&library, &item, &[&moving]);
    lock(&library, &locked, "pw");

    let into = FolderChange::Move {
        id: moving.clone(),
        parent: Some(locked.clone()),
        index: 0,
    };
    assert_eq!(
        library.save_folder(&into),
        Err(SourceError::Locked {
            folder: "Private".into()
        })
    );
    assert!(!row(&library, &item).1);

    library.unlock_folder(&locked, "pw".into()).unwrap();
    library.save_folder(&into).unwrap();
    assert!(row(&library, &item).1);
    assert_eq!(wraps(&library, &item), vec![locked.clone()]);
    library.relock_folder(&locked);
    assert!(
        all(&library).is_empty(),
        "a subfolder of a locked folder is hidden too"
    );
    let tree = library.folder_tree().unwrap();
    assert!(find(&tree, &locked).children.is_empty());

    library.unlock_folder(&locked, "pw".into()).unwrap();
    library
        .save_folder(&FolderChange::Move {
            id: moving,
            parent: None,
            index: 0,
        })
        .unwrap();
    assert!(!row(&library, &item).1);
    assert!(wraps(&library, &item).is_empty());
}

#[test]
fn deleting_a_locked_folder_needs_its_key_and_decrypts_first() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Delete"));
    let folder = create(&library, "Private", None);
    let item = add_png(&library, dir.path(), "a.png", 1);
    put(&library, &item, &[&folder]);
    lock(&library, &folder, "pw");
    assert_eq!(
        library.delete_folder(&folder, false),
        Err(SourceError::Locked {
            folder: "Private".into()
        })
    );
    library.unlock_folder(&folder, "pw".into()).unwrap();
    library.delete_folder(&folder, false).unwrap();
    let (name, encrypted, _, _) = row(&library, &item);
    assert_eq!((name.as_str(), encrypted), ("a", false));
    assert!(wraps(&library, &item).is_empty());
    assert_eq!(all(&library), [item]);
}

#[test]
fn an_unfinished_lock_is_finished_by_the_next_unlock() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Repair"));
    let folder = create(&library, "Private", None);
    let early = add_png(&library, dir.path(), "a.png", 1);
    put(&library, &early, &[&folder]);
    lock(&library, &folder, "pw");
    // A crash between a write and its encryption: an item in the folder still in plaintext.
    let late = add_png(&library, dir.path(), "b.png", 2);
    library
        .conn()
        .execute(
            "INSERT INTO item_folder (item_id, folder_id) VALUES (?1, ?2)",
            params![late, folder],
        )
        .unwrap();
    assert!(!row(&library, &late).1);
    assert!(
        all(&library).is_empty(),
        "hidden even before it is encrypted"
    );
    library.unlock_folder(&folder, "pw".into()).unwrap();
    assert!(row(&library, &late).1);
    assert_eq!(wraps(&library, &late), [folder]);
}

#[test]
fn a_second_instance_can_unlock_but_not_change_locks() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Reader");
    let writer = open(&root);
    let folder = create(&writer, "Private", None);
    let other = create(&writer, "Other", None);
    let item = add_png(&writer, dir.path(), "a.png", 1);
    put(&writer, &item, &[&folder]);
    lock(&writer, &folder, "pw");
    let source = fs::read(dir.path().join("a.png")).unwrap();

    let reader = open(&root);
    assert!(reader.read_only_reason().is_some());
    assert!(all(&reader).is_empty());
    reader.unlock_folder(&folder, "pw".into()).unwrap();
    assert_eq!(all(&reader), vec![item.clone()]);
    assert_eq!(reader.original(&item).unwrap().unwrap().bytes, source);
    assert_eq!(
        reader
            .lock_folder(&other, "x".into(), None, None, &|_, _| {})
            .err(),
        Some(SourceError::ReadOnly)
    );
    assert_eq!(
        reader.remove_folder_lock(&folder, "pw".into(), &|_, _| {}),
        Err(SourceError::ReadOnly)
    );
    // The writer's session is its own.
    assert!(all(&writer).is_empty());
}

#[test]
fn plaintext_left_by_a_crash_is_removed_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Crash");
    let library = open(&root);
    let folder = create(&library, "Private", None);
    let item = add_png(&library, dir.path(), "a.png", 1);
    put(&library, &item, &[&folder]);
    lock(&library, &folder, "pw");
    // As if the process died after the row changed and before the old copies were deleted.
    let shard = &item[item.len() - 2..];
    let plain = format!("files/{shard}/{item}/a.png");
    let thumb = format!("thumbs/{shard}/{item}.webp");
    fs::write(root.join(&plain), b"plaintext").unwrap();
    fs::create_dir_all(root.join("thumbs").join(shard)).unwrap();
    fs::write(root.join(&thumb), b"preview").unwrap();
    {
        let conn = library.conn();
        for path in [&plain, &thumb] {
            conn.execute(
                "INSERT INTO pending_file (path, created_at) VALUES (?1, 0)",
                params![path],
            )
            .unwrap();
        }
    }
    library.abandon();
    let library = open(&root);
    assert!(!root.join(&plain).exists());
    assert!(!root.join(&thumb).exists());
    assert!(starts_with_age(&stored(&root, &row(&library, &item).2)));
}

#[test]
fn the_lock_label_is_plaintext_and_changes_while_locked() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Label"));
    let folder = create(&library, "Private", None);
    library
        .lock_folder(
            &folder,
            "pw".into(),
            Some(" Taxes ".into()),
            Some("red".into()),
            &|_, _| {},
        )
        .unwrap();
    let tree = library.folder_tree().unwrap();
    assert_eq!(find(&tree, &folder).lock_label.as_deref(), Some("Taxes"));
    assert_eq!(find(&tree, &folder).lock_color.as_deref(), Some("red"));
    library
        .set_lock_label(&folder, Some("2026".into()), Some("blue".into()))
        .unwrap();
    let tree = library.folder_tree().unwrap();
    assert_eq!(find(&tree, &folder).lock_label.as_deref(), Some("2026"));
    assert!(matches!(
        library.set_lock_label(&folder, None, Some("teal".into())),
        Err(SourceError::Invalid(_))
    ));
    assert!(matches!(
        library.lock_folder(&folder, "again".into(), None, None, &|_, _| {}),
        Err(SourceError::Invalid(_))
    ));
    let plain = create(&library, "Plain", None);
    assert!(matches!(
        library.lock_folder(&plain, String::new(), None, None, &|_, _| {}),
        Err(SourceError::Invalid(_))
    ));
}

#[test]
fn a_yubikey_opens_only_its_folder_and_the_password_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Piv");
    let library = open(&root);
    let taxes = create(&library, "Taxes", None);
    let photos = create(&library, "Photos", None);
    lock(&library, &taxes, "taxes-pw");
    let photos_recovery = lock(&library, &photos, "photos-pw");
    let taxes_key = SoftwarePiv::generate("111", 0x82, PivPolicy::Touch, None);
    let photos_key = SoftwarePiv::generate("222", 0x95, PivPolicy::Pin, Some("654321"));
    library
        .bind_piv(&taxes, "taxes-pw".into(), taxes_key.card())
        .unwrap();
    library
        .bind_piv(&photos, "photos-pw".into(), photos_key.card())
        .unwrap();
    assert!(
        library
            .bind_piv(&taxes, "taxes-pw".into(), taxes_key.card())
            .is_err()
    );

    let called = std::cell::Cell::new(false);
    assert!(matches!(
        library.unlock_with_piv(
            &taxes,
            "111",
            &PivPrompt::Pin {
                pin: "654321".into()
            },
            |_| {
                called.set(true);
                Ok([0; 32])
            }
        ),
        Err(SourceError::Invalid(_))
    ));
    assert!(
        !called.get(),
        "a PIN prompt never starts ECDH on a touch wrap"
    );
    assert!(!library.is_unlocked(&taxes));

    library
        .unlock_with_piv(&taxes, "111", &PivPrompt::Touch, |epk| {
            taxes_key
                .agree(&PivPrompt::Touch, epk)
                .map_err(|_| SourceError::WrongPassword)
        })
        .unwrap();
    assert!(library.is_unlocked(&taxes));
    assert!(!library.is_unlocked(&photos));

    library.relock_folder(&taxes);
    assert!(matches!(
        library.unlock_with_piv(&photos, "222", &PivPrompt::Touch, |_| Ok([0; 32])),
        Err(SourceError::Invalid(_))
    ));
    assert!(matches!(
        library.unlock_with_piv(
            &photos,
            "222",
            &PivPrompt::Pin {
                pin: "000000".into()
            },
            |epk| photos_key
                .agree(
                    &PivPrompt::Pin {
                        pin: "000000".into()
                    },
                    epk
                )
                .map_err(|_| SourceError::WrongPassword)
        ),
        Err(SourceError::WrongPassword)
    ));
    assert!(!library.is_unlocked(&photos));
    library
        .unlock_with_piv(
            &photos,
            "222",
            &PivPrompt::Pin {
                pin: "654321".into(),
            },
            |epk| {
                photos_key
                    .agree(
                        &PivPrompt::Pin {
                            pin: "654321".into(),
                        },
                        epk,
                    )
                    .map_err(|_| SourceError::WrongPassword)
            },
        )
        .unwrap();
    assert!(library.is_unlocked(&photos));
    assert!(!library.is_unlocked(&taxes));

    // The card is not required: the password and the recovery secret still open the folder.
    library.relock_all();
    library.unlock_folder(&taxes, "taxes-pw".into()).unwrap();
    assert!(library.is_unlocked(&taxes));
    assert!(!library.is_unlocked(&photos));
    library.relock_all();
    library
        .recover_folder(&photos, &photos_recovery.key, "photos-new".into())
        .unwrap();
    assert!(library.is_unlocked(&photos));
    assert_eq!(library.piv_wraps(&photos).unwrap().len(), 1);
    library
        .change_folder_password(&photos, "photos-new".into(), "photos-newer".into())
        .unwrap();
    assert_eq!(
        library.piv_wraps(&photos).unwrap()[0].policy,
        PivPolicy::Pin
    );

    library
        .unbind_piv(&photos, "photos-newer".into(), "222")
        .unwrap();
    assert!(library.piv_wraps(&photos).unwrap().is_empty());
    library.relock_all();
    assert!(matches!(
        library.unlock_with_piv(
            &photos,
            "222",
            &PivPrompt::Pin {
                pin: "654321".into()
            },
            |_| { Ok([0; 32]) }
        ),
        Err(SourceError::NotFound)
    ));
    library
        .unlock_folder(&photos, "photos-newer".into())
        .unwrap();

    library.relock_all();
    library
        .remove_folder_lock(&taxes, "taxes-pw".into(), &|_, _| {})
        .unwrap();
    assert!(library.piv_wraps(&taxes).is_err());
    let left: i64 = library
        .conn()
        .query_row(
            "SELECT count(*) FROM folder_piv_wrap WHERE folder_id = ?1",
            rusqlite::params![taxes],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
}
