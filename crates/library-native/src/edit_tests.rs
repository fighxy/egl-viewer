//! Phase 7: name, note, link and rating; the trash; removing items for good; undo's state;
//! backups and restoring one; Check Library and its repair; Rebuild Thumbnails (docs/spec.md
//! §9, phase 7).

use std::fs;
use std::sync::atomic::AtomicBool;

use library_core::{
    BackupKind, DuplicateNotice, ImportProgress, ItemEdit, ItemId, RepairOptions, Scope,
    SourceError,
};
use rusqlite::params;

use crate::import::{DuplicateChoice, ImportObserver, make_pending_previews};
use crate::test_support::*;
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

fn ids(list: &[&str]) -> Vec<ItemId> {
    list.iter().map(|id| ItemId::from(*id)).collect()
}

fn stored_path(library: &Library, item: &str) -> String {
    library
        .conn()
        .query_row(
            "SELECT file_path FROM item WHERE id = ?1",
            params![item],
            |row| row.get(0),
        )
        .unwrap()
}

fn journal(library: &Library) -> Vec<String> {
    let conn = library.conn();
    let mut statement = conn.prepare("SELECT path FROM pending_file").unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn search_name(library: &Library, item: &str) -> String {
    library
        .conn()
        .query_row(
            "SELECT name FROM item_fts WHERE rowid = (SELECT seq FROM item WHERE id = ?1)",
            params![item],
            |row| row.get(0),
        )
        .unwrap()
}

fn thumb_version(library: &Library, item: &str) -> i64 {
    library
        .conn()
        .query_row(
            "SELECT thumb_version FROM item WHERE id = ?1",
            params![item],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn a_renamed_item_keeps_its_file_readable() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Rename");
    let library = open(&root);
    let id = add_png(&library, dir.path(), "IMG_1042.png", 1);
    let before = library.original(&id).unwrap().unwrap().bytes;
    let old = root.join(stored_path(&library, &id));

    let item = library
        .apply_item_edit(&id, &ItemEdit::Rename("Poster: final?".into()))
        .unwrap();
    assert!(
        library
            .original_path(&id)
            .unwrap()
            .ends_with("Poster_ final_.png")
    );
    assert_eq!(item.name, "Poster: final?");
    let stored = stored_path(&library, &id);
    assert!(stored.ends_with("/Poster_ final_.png"), "{stored}");
    assert!(!old.exists(), "the old name is gone");
    assert_eq!(library.original(&id).unwrap().unwrap().bytes, before);
    assert!(journal(&library).is_empty());
    assert_eq!(search_name(&library, &id), "Poster: final?");
    assert!(
        library
            .search("final")
            .unwrap()
            .contains(&ItemId(id.clone()))
    );

    // Reopening finds the same file under the same name.
    library.close().unwrap();
    let library = open(&root);
    assert_eq!(library.original(&id).unwrap().unwrap().bytes, before);
    assert_eq!(
        files_under(&root.join("files")),
        vec![stored.trim_start_matches("files/").to_owned()]
    );
}

#[test]
fn a_crash_during_a_rename_leaves_one_readable_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Crash");
    let library = open(&root);
    let id = add_png(&library, dir.path(), "a.png", 1);
    let old = stored_path(&library, &id);
    let folder = old.rsplit_once('/').unwrap().0.to_owned();

    // Before the row commits: the new name exists only in the journal.
    let new = format!("{folder}/b.png");
    fs::hard_link(root.join(&old), root.join(&new)).unwrap();
    library
        .conn()
        .execute(
            "INSERT INTO pending_file (path, created_at) VALUES (?1, 0)",
            params![new],
        )
        .unwrap();
    library.abandon();
    let library = open(&root);
    assert_eq!(stored_path(&library, &id), old);
    assert!(root.join(&old).is_file());
    assert!(!root.join(&new).exists());

    // After the row commits: the old name waits in the journal.
    library
        .apply_item_edit(&id, &ItemEdit::Rename("c".into()))
        .unwrap();
    let renamed = stored_path(&library, &id);
    fs::hard_link(root.join(&renamed), root.join(&old)).unwrap();
    library
        .conn()
        .execute(
            "INSERT INTO pending_file (path, created_at) VALUES (?1, 0)",
            params![old],
        )
        .unwrap();
    library.abandon();
    let library = open(&root);
    assert!(root.join(&renamed).is_file());
    assert!(!root.join(&old).exists());
    assert!(library.original(&id).unwrap().is_some());
}

#[test]
fn notes_links_and_ratings_are_stored_and_searchable() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Fields");
    let library = open(&root);
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);

    library
        .apply_item_edit(&a, &ItemEdit::SetAnnotation("Moodboard for spring".into()))
        .unwrap();
    let item = library
        .apply_item_edit(&a, &ItemEdit::SetUrl("https://example.com/a".into()))
        .unwrap();
    assert_eq!(item.annotation, "Moodboard for spring");
    assert_eq!(item.url, "https://example.com/a");
    assert!(
        library
            .search("spring")
            .unwrap()
            .contains(&ItemId(a.clone()))
    );
    assert!(
        library
            .search("example")
            .unwrap()
            .contains(&ItemId(a.clone()))
    );

    let edited = library
        .apply_batch_edit(&ids(&[&a, &b, "missing"]), &ItemEdit::SetStar(4))
        .unwrap();
    assert_eq!(edited, 2);
    assert_eq!(library.item(&b).unwrap().unwrap().star, 4);
    assert!(matches!(
        library.apply_item_edit(&a, &ItemEdit::SetUrl("file:///C:/x".into())),
        Err(SourceError::Invalid(_))
    ));
}

#[test]
fn restore_brings_back_folders_and_tags() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Trash");
    let library = open(&root);
    let posters = create(&library, "Posters", None);
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    put(&library, &a, &[&posters]);
    library
        .apply_item_edit(&a, &ItemEdit::AddTags(vec!["Poster".into()]))
        .unwrap();

    library
        .apply_batch_edit(&ids(&[&a, &b]), &ItemEdit::Trash)
        .unwrap();
    assert!(scope_ids(&library, &Scope::All).is_empty());
    assert_eq!(scope_ids(&library, &Scope::Trash).len(), 2);
    let trashed = library.load_scope(&Scope::Trash).unwrap();
    assert!(trashed.iter().all(|item| item.deleted_at.is_some()));
    let since = library.item(&a).unwrap().unwrap().deleted_at;
    assert!(since.is_some());
    assert_eq!(library.counts().unwrap().trash, 2);

    // Trashing again keeps the first date.
    library.apply_item_edit(&a, &ItemEdit::Trash).unwrap();
    assert_eq!(library.item(&a).unwrap().unwrap().deleted_at, since);

    library.apply_item_edit(&a, &ItemEdit::Restore).unwrap();
    let item = library.item(&a).unwrap().unwrap();
    assert!(!item.is_deleted && item.deleted_at.is_none());
    assert_eq!(item.folders, vec![posters.clone()]);
    assert_eq!(item.tags, vec!["Poster".to_owned()]);
    assert_eq!(scope_ids(&library, &folder_scope(&posters, false)), vec![a]);
}

#[test]
fn emptying_the_trash_removes_the_files_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Empty");
    let library = open(&root);
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    let c = add_png(&library, dir.path(), "c.png", 3);
    library
        .apply_item_edit(&a, &ItemEdit::AddTags(vec!["Only on a".into()]))
        .unwrap();
    previews(&library);
    assert_eq!(files_under(&root.join("thumbs")).len(), 3);
    let a_file = root.join(stored_path(&library, &a));

    library
        .apply_batch_edit(&ids(&[&a, &b]), &ItemEdit::Trash)
        .unwrap();
    // Delete Permanently takes only what is in the trash.
    assert_eq!(library.delete_items(&ids(&[&a, &c])).unwrap(), 1);
    assert!(!a_file.exists());
    assert!(
        !a_file.parent().unwrap().exists(),
        "the item's folder goes too"
    );
    assert!(library.item(&a).unwrap().is_none());
    assert!(
        library
            .tags()
            .unwrap()
            .iter()
            .all(|tag| tag.name != "Only on a")
    );

    assert_eq!(library.empty_trash().unwrap(), 1);
    assert_eq!(library.counts().unwrap().trash, 0);
    assert_eq!(files_under(&root.join("files")).len(), 1);
    assert_eq!(files_under(&root.join("thumbs")).len(), 1);
    assert!(journal(&library).is_empty());
    assert_eq!(scope_ids(&library, &Scope::All), vec![c]);
}

#[test]
fn a_crash_after_removing_rows_deletes_the_files_on_the_next_open() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Crash");
    let library = open(&root);
    let a = add_png(&library, dir.path(), "a.png", 1);
    let stored = stored_path(&library, &a);
    // What the transaction leaves before the files go: no row, the paths in the journal.
    {
        let conn = library.conn();
        conn.execute(
            "INSERT INTO pending_file (path, created_at) VALUES (?1, 0)",
            params![stored],
        )
        .unwrap();
        conn.execute("DELETE FROM item WHERE id = ?1", params![a])
            .unwrap();
    }
    library.abandon();
    let library = open(&root);
    assert!(!root.join(&stored).exists());
    assert!(journal(&library).is_empty());
}

#[test]
fn encrypted_items_keep_their_sealed_name_and_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Sealed");
    let library = open(&root);
    let private = create(&library, "Private", None);
    let id = add_png(&library, dir.path(), "secret.png", 1);
    put(&library, &id, &[&private]);
    library
        .lock_folder(&private, "correct horse".into(), None, None, &|_, _| {})
        .unwrap();
    library
        .unlock_folder(&private, "correct horse".into())
        .unwrap();
    let path = stored_path(&library, &id);

    library
        .apply_item_edit(&id, &ItemEdit::Rename("still secret".into()))
        .unwrap();
    library
        .apply_item_edit(&id, &ItemEdit::SetAnnotation("hidden note".into()))
        .unwrap();
    assert_eq!(
        stored_path(&library, &id),
        path,
        "the sealed file keeps its name"
    );
    assert!(matches!(
        library.original_path(&id),
        Err(SourceError::Invalid(_))
    ));
    let (name, annotation): (String, String) = library
        .conn()
        .query_row(
            "SELECT name, annotation FROM item WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((name.as_str(), annotation.as_str()), ("", ""));
    let item = library.item(&id).unwrap().unwrap();
    assert_eq!(item.name, "still secret");
    assert_eq!(item.annotation, "hidden note");

    // A new session opens the sealed fields again.
    library.close().unwrap();
    let library = open(&root);
    library
        .unlock_folder(&private, "correct horse".into())
        .unwrap();
    assert_eq!(library.item(&id).unwrap().unwrap().name, "still secret");
}

#[test]
fn item_states_are_what_undo_needs() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Undo");
    let library = open(&root);
    let posters = create(&library, "Posters", None);
    let a = add_png(&library, dir.path(), "a.png", 1);
    put(&library, &a, &[&posters]);
    library
        .apply_item_edit(&a, &ItemEdit::AddTags(vec!["Poster".into()]))
        .unwrap();
    library.apply_item_edit(&a, &ItemEdit::SetStar(2)).unwrap();
    let states = library.item_states(&ids(&[&a, "missing"])).unwrap();
    assert_eq!(states.len(), 1);
    let state = &states[0];
    assert_eq!(state.name, "a");
    assert_eq!(state.star, 2);
    assert_eq!(state.tags, vec!["Poster".to_owned()]);
    assert_eq!(state.folders, vec![posters]);
    assert!(!state.in_trash);
}

#[test]
fn a_backup_restores_the_library_to_its_earlier_state() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Restore");
    let library = open(&root);
    let a = add_png(&library, dir.path(), "a.png", 1);
    let posters = create(&library, "Posters", None);
    put(&library, &a, &[&posters]);
    let backup = library.back_up_now().unwrap();
    assert_eq!(backup.kind, BackupKind::Manual);

    // Later changes: a rename, a new item, a deleted folder.
    library
        .apply_item_edit(&a, &ItemEdit::Rename("renamed".into()))
        .unwrap();
    let b = add_png(&library, dir.path(), "b.png", 2);
    library.delete_folder(&posters, false).unwrap();

    library.restore_backup(&backup.name).unwrap();
    assert_eq!(scope_ids(&library, &Scope::All), vec![a.clone()]);
    let item = library.item(&a).unwrap().unwrap();
    assert_eq!(item.name, "a");
    assert_eq!(item.folders, vec![posters]);
    assert!(library.item(&b).unwrap().is_none());
    assert!(
        library
            .backups()
            .unwrap()
            .iter()
            .any(|backup| backup.kind == BackupKind::BeforeRestore)
    );
    // The restored row names the file before the rename; b's file has no row. Check Library
    // says both, and nothing is deleted on its own.
    let check = library.check_library().unwrap();
    assert!(check.database_ok);
    assert_eq!(check.missing_count, 1);
    assert_eq!(check.orphan_count, 2);
    assert!(journal(&library).is_empty());

    // The library stays open for writing, and reopens normally.
    library.apply_item_edit(&a, &ItemEdit::SetStar(5)).unwrap();
    library.close().unwrap();
    let library = open(&root);
    assert_eq!(library.item(&a).unwrap().unwrap().star, 5);
}

#[test]
fn restore_refuses_files_that_are_not_backups_of_this_library() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Mine");
    let other = new_library(dir.path(), "Other");
    let library = open(&root);
    let theirs = open(&other).back_up_now().unwrap();
    fs::copy(
        other.join("backups").join(&theirs.name),
        root.join("backups").join("library-2020-01-01-000000.db"),
    )
    .unwrap();
    assert!(matches!(
        library.restore_backup("library-2020-01-01-000000.db"),
        Err(SourceError::Invalid(_))
    ));
    for name in ["../library.db", "library.db", "notes.txt"] {
        assert!(library.restore_backup(name).is_err(), "{name}");
    }
    assert!(matches!(
        library.restore_backup("library-2020-01-02.db"),
        Err(SourceError::NotFound)
    ));
}

#[test]
fn an_older_backup_is_migrated_after_restoring() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Old");
    let library = open(&root);
    let a = add_png(&library, dir.path(), "Poster.png", 1);
    let backup = library.back_up_now().unwrap();
    make_version_one(&root.join("backups").join(&backup.name));
    library.restore_backup(&backup.name).unwrap();
    let version: u32 = library
        .conn()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 4);
    assert!(library.search("ster").unwrap().contains(&ItemId(a)));
}

#[test]
fn check_library_finds_missing_originals_and_orphans_and_repairs_them() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Check");
    let library = open(&root);
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    previews(&library);
    let clean = library.check_library().unwrap();
    assert!(clean.is_clean(), "{clean:?}");
    assert_eq!(clean.items, 2);

    fs::remove_file(root.join(stored_path(&library, &a))).unwrap();
    fs::create_dir_all(root.join("files/zz/STRAY")).unwrap();
    fs::write(root.join("files/zz/STRAY/stray.jpg"), b"12345").unwrap();
    fs::write(root.join("files/.tmp/in-flight"), b"x").unwrap();
    fs::create_dir_all(root.join("thumbs/zz")).unwrap();
    fs::write(root.join("thumbs/zz/GONE.webp"), b"x").unwrap();

    let check = library.check_library().unwrap();
    assert_eq!(check.missing_count, 1);
    assert_eq!(check.missing[0].id, ItemId(a.clone()));
    assert_eq!(check.missing[0].name, "a");
    assert_eq!(check.orphan_count, 1);
    assert_eq!(check.orphan_bytes, 5);
    assert_eq!(check.orphans, vec!["files/zz/STRAY/stray.jpg".to_owned()]);
    assert_eq!(check.orphan_thumbnails, 1);

    let repaired = library
        .repair_library(RepairOptions {
            orphans: true,
            missing: true,
        })
        .unwrap();
    assert!(repaired.is_clean(), "{repaired:?}");
    assert!(library.item(&a).unwrap().is_none());
    assert!(library.item(&b).unwrap().is_some());
    assert!(!root.join("files/zz/STRAY").exists());
    assert!(!root.join("thumbs/zz/GONE.webp").exists());
    let stats = library.stats().unwrap();
    assert!(stats.originals > 0 && stats.thumbnails > 0 && stats.database > 0);
    assert!(stats.backups > 0);
}

#[test]
fn rebuilt_thumbnails_get_new_versions() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Rebuild");
    let library = open(&root);
    let a = add_png(&library, dir.path(), "a.png", 1);
    let text = write_file(dir.path(), "notes.txt", b"text");
    let AddOutcome::Added { id: notes, .. } = library.add_file(&text, Duplicates::Report).unwrap()
    else {
        panic!("added");
    };
    previews(&library);
    assert_eq!(thumb_version(&library, &a), 1);

    assert_eq!(library.rebuild_previews().unwrap(), 1);
    assert_eq!(thumb_version(&library, &a), -2);
    // The old thumbnail shows under a URL never used before, until the new one is made.
    let waiting = library.load_scope(&Scope::All).unwrap();
    assert!(
        waiting
            .iter()
            .any(|item| item.thumb_version == -2 && !item.no_thumbnail)
    );
    assert!(library.thumbnail(&a).unwrap().is_some());
    assert_eq!(thumb_version(&library, &notes), 0);
    previews(&library);
    assert_eq!(
        thumb_version(&library, &a),
        2,
        "never the URL of the old thumbnail"
    );
    assert!(library.thumbnail(&a).unwrap().is_some());
}

#[test]
fn the_library_can_be_renamed_in_its_settings() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Before");
    let library = open(&root);
    library.rename_library("  After  ").unwrap();
    assert_eq!(library.info().name, "After");
    assert!(library.rename_library("a/b").is_err());
    library.close().unwrap();
    assert_eq!(open(&root).info().name, "After");
    assert!(root.ends_with("Before.eglib"));
}
