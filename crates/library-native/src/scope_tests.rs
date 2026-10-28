//! Reads for the views: scopes, the full item, tags, thumbnails and originals.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use image::ImageFormat;
use library_core::{ItemId, LibrarySource, Scope, ScopeHints, SourceError};
use media::fixtures;
use rusqlite::params;

use crate::test_support::*;
use crate::{AddOutcome, Duplicates, Library, NativeSource, OpenOptions};

/// Adds a PNG and makes its preview; returns the item id.
fn add_png(library: &Library, dir: &Path, name: &str, seed: u32, preview: bool) -> String {
    let source = write_file(dir, name, &fixtures::raster(ImageFormat::Png, 40, 20, seed));
    let AddOutcome::Added { id, file } = library.add_file(&source, Duplicates::Report).unwrap()
    else {
        panic!("{name} was a duplicate");
    };
    if preview {
        let rendered = media::render(&file, "png").unwrap();
        library.store_preview(&id, &rendered).unwrap();
    }
    id
}

/// a: tagged "Poster"; b: in folder F1, no preview yet; c: in the trash.
fn sample(dir: &Path) -> (Library, [String; 3]) {
    let root = new_library(dir, "Scopes");
    let library = open(&root);
    let a = add_png(&library, dir, "a.png", 1, true);
    let b = add_png(&library, dir, "b.png", 2, false);
    let c = add_png(&library, dir, "c.png", 3, true);
    let conn = library.conn();
    conn.execute("INSERT INTO tag (name) VALUES ('Poster'), ('poster')", [])
        .unwrap();
    conn.execute(
        "INSERT INTO item_tag (item_id, tag_id) VALUES (?1, 1), (?2, 2)",
        params![a, c],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO folder (id, name, sort_key, created_at, updated_at)
         VALUES ('F1', 'Posters', 'a', 0, 0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO item_folder (item_id, folder_id) VALUES (?1, 'F1')",
        params![b],
    )
    .unwrap();
    conn.execute("UPDATE item SET deleted_at = 1 WHERE id = ?1", params![c])
        .unwrap();
    drop(conn);
    (library, [a, b, c])
}

fn ids(library: &Library, scope: &Scope) -> Vec<String> {
    library
        .load_scope(scope)
        .unwrap()
        .into_iter()
        .map(|item| item.id.0)
        .collect()
}

#[test]
fn each_scope_is_one_query() {
    let dir = tempfile::tempdir().unwrap();
    let (library, [a, b, c]) = sample(dir.path());
    assert_eq!(ids(&library, &Scope::All), [a.clone(), b.clone()]);
    assert_eq!(ids(&library, &Scope::Unfiled), std::slice::from_ref(&a));
    assert_eq!(ids(&library, &Scope::Untagged), std::slice::from_ref(&b));
    assert_eq!(ids(&library, &Scope::Trash), [c]);
    assert_eq!(
        ids(
            &library,
            &Scope::Folder {
                id: "F1".into(),
                include_descendants: false
            }
        ),
        std::slice::from_ref(&b)
    );

    let all = library.load_scope(&Scope::All).unwrap();
    let (first, second) = (&all[0], &all[1]);
    assert_eq!(first.name, "a");
    assert_eq!(first.ext, "png");
    assert_eq!((first.width, first.height), (Some(40), Some(20)));
    assert!(first.size.is_some_and(|size| size > 0));
    assert_eq!(
        first.tags.iter().map(|tag| &**tag).collect::<Vec<_>>(),
        ["Poster"]
    );
    assert_eq!(first.folders, None);
    assert!(first.palette0.is_some());
    assert!(!first.no_thumbnail && first.thumb_version > 0);
    assert!(first.added_at.is_some());
    assert_eq!(second.folders.as_deref(), Some(&["F1".to_owned()][..]));
    assert!(second.no_thumbnail);
    assert_eq!(second.palette0, None);
}

#[test]
fn tags_are_shared_strings() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Interned");
    let library = open(&root);
    let a = add_png(&library, dir.path(), "a.png", 1, false);
    let b = add_png(&library, dir.path(), "b.png", 2, false);
    let conn = library.conn();
    conn.execute("INSERT INTO tag (name) VALUES ('Poster')", [])
        .unwrap();
    conn.execute(
        "INSERT INTO item_tag (item_id, tag_id) VALUES (?1, 1), (?2, 1)",
        params![a, b],
    )
    .unwrap();
    drop(conn);
    let items = library.load_scope(&Scope::All).unwrap();
    assert!(Arc::ptr_eq(&items[0].tags[0], &items[1].tags[0]));
}

#[test]
fn the_full_item_has_tags_folders_palette_and_trash_state() {
    let dir = tempfile::tempdir().unwrap();
    let (library, [a, b, c]) = sample(dir.path());
    let item = library.item(&a).unwrap().unwrap();
    assert_eq!(item.name, "a");
    assert_eq!(item.tags, ["Poster"]);
    assert!(item.folders.is_empty());
    assert!(!item.palettes.is_empty());
    assert!(!item.no_thumbnail && !item.is_deleted);
    assert_eq!(library.item(&b).unwrap().unwrap().folders, ["F1"]);
    let trashed = library.item(&c).unwrap().unwrap();
    assert!(trashed.is_deleted);
    assert_eq!(trashed.tags, ["poster"]);
    assert_eq!(library.item("01NOTANITEM").unwrap(), None);
}

#[test]
fn tag_counts_leave_out_the_trash() {
    let dir = tempfile::tempdir().unwrap();
    let (library, _) = sample(dir.path());
    let counts: Vec<_> = library
        .tags()
        .unwrap()
        .into_iter()
        .map(|tag| (tag.name, tag.image_count))
        .collect();
    assert_eq!(
        counts,
        [
            ("Poster".to_owned(), Some(1)),
            ("poster".to_owned(), Some(0))
        ]
    );
}

#[test]
fn thumbnails_and_originals_are_read_from_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let (library, [a, b, _]) = sample(dir.path());
    let thumbnail = library.thumbnail(&a).unwrap().unwrap();
    assert_eq!(thumbnail.content_type, "image/webp");
    assert_eq!(&thumbnail.bytes[..4], b"RIFF");
    assert_eq!(library.thumbnail(&b).unwrap(), None, "no preview yet");
    assert_eq!(library.thumbnail("01NOTANITEM").unwrap(), None);

    let original = library.original(&a).unwrap().unwrap();
    assert_eq!(original.content_type, "image/png");
    assert_eq!(original.bytes, fs::read(dir.path().join("a.png")).unwrap());
    assert_eq!(library.original("01NOTANITEM").unwrap(), None);
}

#[test]
fn a_file_path_outside_the_library_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (library, [a, ..]) = sample(dir.path());
    write_file(dir.path(), "outside.png", b"not yours");
    library
        .conn()
        .execute(
            "UPDATE item SET file_path = '../outside.png' WHERE id = ?1",
            params![a],
        )
        .unwrap();
    assert!(matches!(library.original(&a), Err(SourceError::Invalid(_))));
}

#[tokio::test]
async fn the_native_source_answers_through_the_trait() {
    let dir = tempfile::tempdir().unwrap();
    let (library, [a, ..]) = sample(dir.path());
    let root = library.root().to_path_buf();
    let source: Box<dyn LibrarySource> = Box::new(NativeSource::new(Arc::new(library)));
    assert!(source.capabilities().writable);
    let progress = std::sync::Mutex::new(Vec::new());
    let items = source
        .load_scope(&Scope::All, &ScopeHints::default(), &|loaded, total| {
            progress.lock().unwrap().push((loaded, total));
        })
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(*progress.lock().unwrap(), [(2, 2)]);
    let id = ItemId(a);
    assert!(source.item(&id).await.unwrap().is_some());
    assert!(source.thumbnail(&id).await.unwrap().is_some());
    assert!(source.original(&id).await.unwrap().is_some());
    assert!(matches!(
        source.search_ids("poster").await,
        Err(SourceError::NotSupported(_))
    ));
    assert_eq!(source.folder_tree().await.unwrap()[0].name, "Posters");

    let reader = Library::open(&root, &OpenOptions::new(APP)).unwrap();
    assert!(reader.read_only_reason().is_some());
    assert!(!NativeSource::new(Arc::new(reader)).capabilities().writable);
}
