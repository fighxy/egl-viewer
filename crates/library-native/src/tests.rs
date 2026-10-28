//! Behaviour of the native library on temporary folders (docs/spec.md §10).

use std::fs;

use library_core::{LocationProblem, ReadOnlyReason, SourceError};
use rusqlite::Connection;

use crate::add::{Hooks, Step};
use crate::create::create_with;
use crate::library::{Library, OpenOptions};
use crate::schema::Migration;
use crate::test_support::*;
use crate::{APPLICATION_ID, AddOutcome, DB_FILE, Duplicates, create_library};

fn raw(root: &std::path::Path) -> Connection {
    Connection::open(root.join(DB_FILE)).unwrap()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn backups(root: &std::path::Path) -> Vec<String> {
    files_under(&root.join("backups"))
}

// Creating

#[test]
fn create_close_open_keeps_name_id_and_format() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "My Library");
    assert_eq!(root, dir.path().join("My Library.eglib"));
    for sub in ["files/.tmp", "thumbs", "backups"] {
        assert!(root.join(sub).is_dir(), "{sub}");
    }
    assert!(root.join("library.lock").is_file());

    let first = open(&root);
    let info = first.info();
    assert_eq!(info.name, "My Library");
    assert!(!info.read_only);
    first.close().unwrap();

    let again = open(&root);
    assert_eq!(again.info().id, info.id);
    assert!(!again.was_closed_uncleanly());
    let conn = again.conn();
    let app_id: i32 = conn
        .query_row("PRAGMA application_id", [], |r| r.get(0))
        .unwrap();
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    let journal: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    let auto_vacuum: i64 = conn
        .query_row("PRAGMA auto_vacuum", [], |r| r.get(0))
        .unwrap();
    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    let synchronous: i64 = conn
        .query_row("PRAGMA synchronous", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        (
            app_id,
            version,
            journal.as_str(),
            auto_vacuum,
            foreign_keys,
            synchronous
        ),
        (APPLICATION_ID, 1, "wal", 2, 1, 2)
    );
}

#[test]
fn reserved_locked_folder_storage_exists_and_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Lib");
    let library = open(&root);
    library
        .add_file(&write_file(dir.path(), "a.jpg", b"a"), Duplicates::Report)
        .unwrap();
    let conn = library.conn();
    for table in ["item_key_wrap", "folder_piv_wrap"] {
        assert_eq!(
            count(&conn, &format!("SELECT count(*) FROM {table}")),
            0,
            "{table}"
        );
    }
    assert_eq!(
        count(
            &conn,
            "SELECT count(*) FROM item WHERE encrypted <> 0 OR sealed IS NOT NULL"
        ),
        0
    );
    for column in [
        "locked",
        "label",
        "label_color",
        "kdf_salt",
        "kdf_params",
        "wrapped_identity",
        "recovery_wrap",
    ] {
        conn.prepare(&format!("SELECT {column} FROM folder"))
            .unwrap();
    }
}

#[test]
fn create_refuses_bad_names_existing_targets_and_nested_libraries() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["CON", "a<b", "name.", ""] {
        assert!(matches!(
            create_library(dir.path(), name, APP),
            Err(SourceError::Invalid(_))
        ));
    }
    let root = new_library(dir.path(), "Lib");
    assert_eq!(
        create_library(dir.path(), "Lib", APP),
        Err(SourceError::AlreadyExists)
    );
    assert_eq!(
        create_library(&root, "Inner", APP),
        Err(SourceError::UnsupportedLocation(
            LocationProblem::InsideLibrary
        ))
    );
    let nested = root.join("files");
    assert_eq!(
        create_library(&nested, "Inner", APP),
        Err(SourceError::UnsupportedLocation(
            LocationProblem::InsideLibrary
        ))
    );

    let eagle = dir.path().join("Old.library");
    fs::create_dir_all(eagle.join("images")).unwrap();
    fs::write(eagle.join("metadata.json"), "{}").unwrap();
    assert_eq!(
        create_library(&eagle, "Inside", APP),
        Err(SourceError::UnsupportedLocation(
            LocationProblem::InsideEagleLibrary
        ))
    );
    assert_eq!(
        fs::read_dir(&eagle).unwrap().count(),
        2,
        "nothing written in Eagle's folder"
    );

    assert!(matches!(
        create_library(&dir.path().join("missing"), "Lib", APP),
        Err(SourceError::Invalid(_))
    ));
}

#[test]
fn a_failed_create_leaves_nothing_and_leftovers_are_removed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(create_with(dir.path(), "Broken", APP, true).is_err());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);

    let leftover = dir.path().join(".Old.eglib.creating-01ABC");
    fs::create_dir(&leftover).unwrap();
    let unrelated = dir.path().join(".config");
    fs::create_dir(&unrelated).unwrap();
    new_library(dir.path(), "Fresh");
    assert!(!leftover.exists());
    assert!(unrelated.exists());
}

// Opening

#[test]
fn folders_that_are_not_libraries_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty");
    fs::create_dir(&empty).unwrap();
    assert_eq!(
        Library::open(&empty, &options()).err(),
        Some(SourceError::NotALibrary { eagle: false })
    );

    let other = dir.path().join("other");
    fs::create_dir(&other).unwrap();
    Connection::open(other.join(DB_FILE))
        .unwrap()
        .execute_batch("CREATE TABLE x (y); PRAGMA user_version = 1;")
        .unwrap();
    assert_eq!(
        Library::open(&other, &options()).err(),
        Some(SourceError::NotALibrary { eagle: false })
    );

    let garbage = dir.path().join("garbage");
    fs::create_dir(&garbage).unwrap();
    fs::write(garbage.join(DB_FILE), vec![7_u8; 4096]).unwrap();
    assert_eq!(
        Library::open(&garbage, &options()).err(),
        Some(SourceError::NotALibrary { eagle: false })
    );

    let eagle = dir.path().join("Refs.library");
    fs::create_dir_all(eagle.join("images")).unwrap();
    fs::write(eagle.join("metadata.json"), "{}").unwrap();
    assert_eq!(
        Library::open(&eagle, &options()).err(),
        Some(SourceError::NotALibrary { eagle: true })
    );

    assert_eq!(
        Library::open(&dir.path().join("missing"), &options()).err(),
        Some(SourceError::NotFound)
    );
}

#[test]
fn a_library_from_a_newer_format_is_refused_and_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Future");
    raw(&root)
        .execute_batch("PRAGMA user_version = 9;")
        .unwrap();
    let before = fs::read(root.join(DB_FILE)).unwrap();

    assert_eq!(
        Library::open(&root, &options()).err(),
        Some(SourceError::NewerFormat { version: 9 })
    );
    assert_eq!(fs::read(root.join(DB_FILE)).unwrap(), before);
    assert!(backups(&root).is_empty());
    assert!(!root.join("library.db-wal").exists());
}

#[test]
fn a_second_open_is_read_only_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Shared");
    let writer = open(&root);
    let backups_before = backups(&root);
    write_file(&root.join("files").join(".tmp"), "copying", b"in progress");

    let reader = open(&root);
    let info = reader.info();
    assert!(info.read_only);
    assert_eq!(info.read_only_reason, Some(ReadOnlyReason::OtherInstance));
    assert!(info.holder.is_some(), "names the writer");
    assert!(root.join("files").join(".tmp").join("copying").exists());
    assert_eq!(backups(&root), backups_before);
    assert_eq!(
        reader.add_file(&write_file(dir.path(), "a.png", b"x"), Duplicates::Report),
        Err(SourceError::ReadOnly)
    );

    // "Open for editing" once the writer has closed.
    writer.close().unwrap();
    let editor = open(&root);
    assert_eq!(editor.read_only_reason(), None);
    drop(reader);
}

#[test]
fn a_killed_writer_releases_the_lock_and_the_next_open_notices() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Crash");
    open(&root).abandon();
    assert!(
        count(
            &raw(&root),
            "SELECT count(*) FROM meta WHERE key = 'open_by'"
        ) == 1
    );

    let next = open(&root);
    assert_eq!(next.read_only_reason(), None, "the lock was free at once");
    assert!(next.was_closed_uncleanly());
    next.close().unwrap();

    assert!(!open(&root).was_closed_uncleanly());
    assert_eq!(
        count(
            &raw(&root),
            "SELECT count(*) FROM meta WHERE key = 'open_by'"
        ),
        0
    );
}

#[test]
fn a_library_in_a_sync_folder_opens_immutable_and_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Cloud");
    let options = OpenOptions {
        sync_folder: true,
        ..options()
    };
    let library = Library::open(&root, &options).unwrap();
    assert_eq!(library.read_only_reason(), Some(ReadOnlyReason::SyncFolder));
    assert_eq!(library.info().holder, None);
    assert!(!root.join("library.db-wal").exists());
    assert!(!root.join("library.db-shm").exists());
    assert!(backups(&root).is_empty());
}

#[test]
fn migrations_run_after_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Old");
    let steps = [Migration {
        version: 2,
        sql: "CREATE TABLE added_in_v2 (x INTEGER) STRICT;",
    }];
    let library = Library::open_with(&root, &options(), &steps).unwrap();
    let names = backups(&root);
    assert!(
        names.iter().any(|name| name.ends_with("-v1.db")),
        "{names:?}"
    );
    let conn = library.conn();
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2);
    assert_eq!(count(&conn, "SELECT count(*) FROM added_in_v2"), 0);

    let backup = names.iter().find(|name| name.ends_with("-v1.db")).unwrap();
    let old = Connection::open(root.join("backups").join(backup)).unwrap();
    let old_version: u32 = old
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        old_version, 1,
        "the backup is the state before the migration"
    );
}

#[test]
fn a_read_only_open_refuses_a_library_that_needs_a_migration() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Old");
    let _writer = open(&root);
    let steps = [Migration {
        version: 2,
        sql: "SELECT 1;",
    }];
    assert!(matches!(
        Library::open_with(&root, &options(), &steps),
        Err(SourceError::NotSupported(_))
    ));
}

#[test]
fn backups_are_daily_and_only_ten_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Backups");
    open(&root).close().unwrap();
    open(&root).close().unwrap();
    assert_eq!(backups(&root).len(), 1, "one per day");

    let folder = root.join("backups");
    for day in 1..=12 {
        fs::write(folder.join(format!("library-2020-01-{day:02}.db")), b"old").unwrap();
    }
    fs::remove_file(folder.join(&backups(&root)[12])).unwrap();
    open(&root).close().unwrap();
    let kept = backups(&root);
    assert_eq!(kept.len(), 10);
    assert!(!kept.contains(&"library-2020-01-01.db".to_owned()));
}

// Adding a file

#[test]
fn an_added_file_has_a_row_and_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Add");
    let library = open(&root);
    let source = write_file(dir.path(), "Poster Final.JPG", b"jpeg bytes");

    let AddOutcome::Added { id } = library.add_file(&source, Duplicates::Report).unwrap() else {
        panic!("not added");
    };
    let conn = library.conn();
    let (name, ext, path, size, hash): (String, String, String, i64, Vec<u8>) = conn
        .query_row(
            "SELECT name, ext, file_path, size, content_hash FROM item WHERE id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        (name.as_str(), ext.as_str(), size),
        ("Poster Final", "jpg", 10)
    );
    assert_eq!(path, format!("files/{}/{id}/Poster Final.JPG", &id[24..]));
    assert_eq!(hash, blake3::hash(b"jpeg bytes").as_bytes().to_vec());
    assert_eq!(fs::read(root.join(&path)).unwrap(), b"jpeg bytes");
    assert!(source.exists(), "the source stays");
    assert_eq!(count(&conn, "SELECT count(*) FROM pending_file"), 0);
    assert_eq!(count(&conn, "SELECT count(*) FROM change_log"), 1);
    let indexed: String = conn
        .query_row(
            "SELECT name FROM item_fts WHERE item_fts MATCH 'poster'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(indexed, "Poster Final");
    assert!(files_under(&root.join("files").join(".tmp")).is_empty());
}

#[test]
fn the_same_content_is_reported_as_a_duplicate_also_from_the_trash() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Dupes");
    let library = open(&root);
    let first = write_file(dir.path(), "a.png", b"same");
    let second = write_file(dir.path(), "b.png", b"same");

    let AddOutcome::Added { id } = library.add_file(&first, Duplicates::Report).unwrap() else {
        panic!("not added");
    };
    assert_eq!(
        library.add_file(&second, Duplicates::Report).unwrap(),
        AddOutcome::Duplicate {
            existing: id.clone(),
            in_trash: false
        }
    );
    library
        .conn()
        .execute("UPDATE item SET deleted_at = 1 WHERE id = ?1", [&id])
        .unwrap();
    assert_eq!(
        library.add_file(&second, Duplicates::Report).unwrap(),
        AddOutcome::Duplicate {
            existing: id,
            in_trash: true
        }
    );
    assert!(matches!(
        library.add_file(&second, Duplicates::AddAnyway).unwrap(),
        AddOutcome::Added { .. }
    ));
    assert_eq!(count(&library.conn(), "SELECT count(*) FROM item"), 2);
    assert!(files_under(&root.join("files").join(".tmp")).is_empty());
}

#[test]
fn a_source_that_changes_during_the_copy_adds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Changing");
    let library = open(&root);
    let source = write_file(dir.path(), "growing.tif", b"first part");
    let grow = || {
        fs::write(&source, b"first part and more").unwrap();
    };
    let hooks = Hooks {
        after_copy: Some(&grow),
        ..Hooks::default()
    };
    assert!(
        library
            .add_file_with(&source, Duplicates::Report, &hooks)
            .is_err()
    );
    assert_eq!(count(&library.conn(), "SELECT count(*) FROM item"), 0);
    assert!(files_under(&root.join("files")).is_empty());
}

#[test]
fn a_crash_at_any_step_leaves_no_row_without_a_file_and_no_file_without_a_row() {
    for step in [Step::Copied, Step::Journaled, Step::Renamed] {
        let dir = tempfile::tempdir().unwrap();
        let root = new_library(dir.path(), "Crashes");
        let library = open(&root);
        let source = write_file(dir.path(), "photo.jpg", b"photo");
        let hooks = Hooks {
            crash_after: Some(step),
            ..Hooks::default()
        };
        assert_eq!(
            library
                .add_file_with(&source, Duplicates::Report, &hooks)
                .unwrap(),
            None
        );
        assert!(
            !files_under(&root.join("files")).is_empty(),
            "{step:?} left a file"
        );
        library.abandon();

        let reopened = open(&root);
        assert!(reopened.was_closed_uncleanly());
        let conn = reopened.conn();
        assert_eq!(count(&conn, "SELECT count(*) FROM item"), 0, "{step:?}");
        assert_eq!(
            count(&conn, "SELECT count(*) FROM pending_file"),
            0,
            "{step:?}"
        );
        drop(conn);
        assert!(files_under(&root.join("files")).is_empty(), "{step:?}");
        let shards: Vec<_> = fs::read_dir(root.join("files"))
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name() != ".tmp")
            .collect();
        for shard in shards {
            assert_eq!(
                fs::read_dir(shard.path()).unwrap().count(),
                0,
                "{step:?} left a folder"
            );
        }

        // Adding the same file again works and is not a duplicate of a ghost.
        assert!(matches!(
            reopened.add_file(&source, Duplicates::Report).unwrap(),
            AddOutcome::Added { .. }
        ));
    }
}

#[test]
fn recovery_keeps_files_whose_row_exists_and_ignores_paths_outside_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Journal");
    let library = open(&root);
    let source = write_file(dir.path(), "kept.png", b"kept");
    let AddOutcome::Added { id } = library.add_file(&source, Duplicates::Report).unwrap() else {
        panic!("not added");
    };
    let path: String = library
        .conn()
        .query_row("SELECT file_path FROM item WHERE id = ?1", [&id], |r| {
            r.get(0)
        })
        .unwrap();
    let outside = write_file(dir.path(), "outside.txt", b"keep me");
    library
        .conn()
        .execute(
            "INSERT INTO pending_file (path, created_at) VALUES (?1, 0), ('../outside.txt', 0)",
            [&path],
        )
        .unwrap();
    library.abandon();

    let reopened = open(&root);
    assert!(root.join(&path).exists());
    assert!(outside.exists());
    assert_eq!(
        count(&reopened.conn(), "SELECT count(*) FROM pending_file"),
        0
    );
}
