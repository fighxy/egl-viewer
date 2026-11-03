//! One library per released format version under `fixtures/library/vN/`, and a test that opens
//! a copy of each with this build: it migrates forward after a backup and keeps every item,
//! folder, tag and file (docs/library-format.md §8).

use std::fs;
use std::path::{Path, PathBuf};

use library_core::{ItemEdit, Scope, TagGroupChange};
use rusqlite::params;

use crate::schema::FORMAT_VERSION;
use crate::test_support::*;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/library")
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

/// Writes `fixtures/library/v<FORMAT_VERSION>/Fixture.eglib` with this build: three items (one
/// in a folder with tags and a note, one in the trash), a tag group, an item and a folder that
/// a past export into Eagle marked, and an item imported from an Eagle library. Run once per
/// released format version with
/// `cargo test -p library-native -- --ignored make_format_fixture`.
#[test]
#[ignore]
fn make_format_fixture() {
    let target = fixtures().join(format!("v{FORMAT_VERSION}"));
    let _ = fs::remove_dir_all(&target);
    fs::create_dir_all(&target).unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let root = new_library(&target, "Fixture");
    let library = open(&root);
    let posters = create(&library, "Posters", None);
    let a = add_png(&library, scratch.path(), "poster.png", 1);
    let b = add_png(&library, scratch.path(), "imported.png", 2);
    let c = add_png(&library, scratch.path(), "trashed.png", 3);
    put(&library, &a, &[&posters]);
    library
        .apply_item_edit(
            &a,
            &ItemEdit::AddTags(vec!["Poster".into(), "poster".into()]),
        )
        .unwrap();
    library
        .apply_item_edit(&a, &ItemEdit::SetAnnotation("Spring campaign".into()))
        .unwrap();
    library.apply_item_edit(&c, &ItemEdit::Trash).unwrap();
    library
        .save_tag_group(&TagGroupChange::Create {
            name: "Kinds".into(),
            color: None,
            tags: vec!["Poster".into()],
        })
        .unwrap();
    {
        let conn = library.conn();
        conn.execute(
            "UPDATE item SET eagle_id = 'EXPORTEDITEM1' WHERE id = ?1",
            params![a],
        )
        .unwrap();
        conn.execute(
            "UPDATE folder SET eagle_id = 'EXPORTEDFOLDER1' WHERE id = ?1",
            params![posters],
        )
        .unwrap();
        conn.execute(
            "UPDATE item SET eagle_id = 'IMPORTEDITEM1' WHERE id = ?1",
            params![b],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO eagle_import (eagle_id, last_modified, imported_at)
             VALUES ('IMPORTEDITEM1', 1, 1)",
            [],
        )
        .unwrap();
    }
    library.close().unwrap();
    // Backups and the lock file are made again by whoever opens the copy.
    let _ = fs::remove_dir_all(root.join("backups"));
    let _ = fs::remove_file(root.join("library.lock"));
}

#[test]
fn every_released_format_migrates_from_its_fixture() {
    let mut versions: Vec<u32> = fs::read_dir(fixtures())
        .unwrap()
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.strip_prefix('v')?.parse().ok())
        .collect();
    versions.sort_unstable();
    assert!(!versions.is_empty(), "no fixtures under fixtures/library");
    for version in versions {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Fixture.eglib");
        copy(
            &fixtures().join(format!("v{version}")).join("Fixture.eglib"),
            &root,
        );
        let library = open(&root);
        let now: u32 = library
            .conn()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(now, FORMAT_VERSION, "v{version}");
        if version < FORMAT_VERSION {
            let backups = files_under(&root.join("backups"));
            assert!(
                backups
                    .iter()
                    .any(|name| name.ends_with(&format!("-v{version}.db"))),
                "v{version}: {backups:?}"
            );
        }
        assert_eq!(scope_ids(&library, &Scope::All).len(), 2, "v{version}");
        assert_eq!(scope_ids(&library, &Scope::Trash).len(), 1, "v{version}");
        let tags: Vec<String> = library
            .tags()
            .unwrap()
            .into_iter()
            .map(|tag| tag.name)
            .collect();
        assert!(
            tags.contains(&"Poster".to_owned()) && tags.contains(&"poster".to_owned()),
            "v{version}: {tags:?}"
        );
        assert!(
            library
                .search("campaign")
                .unwrap()
                .contains(&library_core::ItemId(
                    library
                        .conn()
                        .query_row(
                            "SELECT id FROM item WHERE annotation = 'Spring campaign'",
                            [],
                            |row| row.get(0),
                        )
                        .unwrap()
                )),
            "v{version}"
        );
        let check = library.check_library().unwrap();
        assert!(check.is_clean(), "v{version}: {check:?}");
        if version <= 5 {
            // Version 6 moved the ids an export wrote out of `eagle_id`; an import's origin and
            // the folders of a library that imported stay where they were.
            let conn = library.conn();
            let exported: Vec<(String, String)> = conn
                .prepare("SELECT kind, eagle_id FROM eagle_export ORDER BY kind")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(exported, [("item".to_owned(), "EXPORTEDITEM1".to_owned())]);
            let origins: Vec<String> = conn
                .prepare("SELECT eagle_id FROM item WHERE eagle_id IS NOT NULL")
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(origins, ["IMPORTEDITEM1"]);
            let folder: Option<String> = conn
                .query_row("SELECT eagle_id FROM folder", [], |row| row.get(0))
                .unwrap();
            assert_eq!(folder.as_deref(), Some("EXPORTEDFOLDER1"));
        }
        library.close().unwrap();
    }
}
