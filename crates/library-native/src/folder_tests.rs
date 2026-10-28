//! Folders: the tree and its counts, edits, moves, deletes, membership, manual order, and
//! imports into folders (docs/spec.md §3, §9 phase 4).

use std::fs;
use std::sync::atomic::AtomicBool;

use image::ImageFormat;
use library_core::{
    DuplicateNotice, FolderChange, ImportProgress, ItemEdit, ItemId, ItemLite, Scope, SortKey,
    SourceError,
};
use media::fixtures;
use rusqlite::params;

use crate::import::{DuplicateChoice, DuplicatePolicy, ImportObserver, ImportTarget, import_files};
use crate::test_support::*;
use crate::{Library, OpenOptions};

fn changes(library: &Library) -> i64 {
    library
        .conn()
        .query_row("SELECT count(*) FROM change_log", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn a_parent_shows_its_childs_items_only_with_subfolders_and_counts_them_once() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tree"));
    let parent = create(&library, "Posters", None);
    let child = create(&library, "2024", Some(&parent));
    let sibling = create(&library, "2025", Some(&parent));
    let own = add_png(&library, dir.path(), "own.png", 1);
    let in_child = add_png(&library, dir.path(), "child.png", 2);
    let in_both = add_png(&library, dir.path(), "both.png", 3);
    put(&library, &own, &[&parent]);
    put(&library, &in_child, &[&child]);
    put(&library, &in_both, &[&child, &sibling]);

    assert_eq!(
        scope_ids(&library, &folder_scope(&parent, false)),
        std::slice::from_ref(&own)
    );
    let mut all = vec![own.clone(), in_child.clone(), in_both.clone()];
    all.sort();
    assert_eq!(scope_ids(&library, &folder_scope(&parent, true)), all);

    let tree = library.folder_tree().unwrap();
    assert_eq!(names(&tree), ["Posters"]);
    assert_eq!(names(&tree[0].children), ["2024", "2025"]);
    let posters = find(&tree, &parent);
    assert_eq!(posters.image_count, Some(1));
    assert_eq!(
        posters.descendant_image_count,
        Some(3),
        "in_both counts once"
    );
    assert_eq!(find(&tree, &child).image_count, Some(2));
    assert_eq!(find(&tree, &sibling).descendant_image_count, Some(1));

    // The trash does not count.
    library
        .conn()
        .execute(
            "UPDATE item SET deleted_at = 1 WHERE id = ?1",
            params![in_both],
        )
        .unwrap();
    let tree = library.folder_tree().unwrap();
    assert_eq!(find(&tree, &parent).descendant_image_count, Some(2));
}

#[test]
fn an_item_added_to_a_second_folder_stays_in_the_first() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Membership"));
    let first = create(&library, "First", None);
    let second = create(&library, "Second", None);
    let item = add_png(&library, dir.path(), "a.png", 1);
    put(&library, &item, &[&first]);
    let stored = library
        .apply_item_edit(&item, &ItemEdit::AddToFolders(vec![second.clone()]))
        .unwrap();
    assert_eq!(stored.folders.len(), 2);
    assert!(stored.folders.contains(&first) && stored.folders.contains(&second));

    let stored = library
        .apply_item_edit(&item, &ItemEdit::RemoveFromFolders(vec![first.clone()]))
        .unwrap();
    assert_eq!(stored.folders, std::slice::from_ref(&second));
    let stored = library
        .apply_item_edit(&item, &ItemEdit::SetFolders(vec![first.clone()]))
        .unwrap();
    assert_eq!(stored.folders, std::slice::from_ref(&first));
    assert!(scope_ids(&library, &Scope::Unfiled).is_empty());

    assert!(matches!(
        library.apply_item_edit(&item, &ItemEdit::AddToFolders(vec!["F404".into()])),
        Err(SourceError::Invalid(_))
    ));
    assert!(matches!(
        library.apply_item_edit(&item, &ItemEdit::SetStar(3)),
        Err(SourceError::NotSupported(_))
    ));
    let edited = library
        .apply_batch_edit(
            &[ItemId(item.clone()), ItemId("01MISSING".into())],
            &ItemEdit::AddToFolders(vec![second]),
        )
        .unwrap();
    assert_eq!(edited, 1, "missing items are left out");
}

#[test]
fn a_move_under_its_own_descendant_is_refused_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Cycle"));
    let a = create(&library, "A", None);
    let b = create(&library, "B", Some(&a));
    let c = create(&library, "C", Some(&b));
    let before = changes(&library);
    for target in [&a, &b, &c] {
        let refused = library.save_folder(&FolderChange::Move {
            id: a.clone(),
            parent: Some(target.clone()),
            index: 0,
        });
        assert!(matches!(refused, Err(SourceError::Invalid(_))), "{target}");
    }
    assert_eq!(changes(&library), before, "nothing was written");
    let tree = library.folder_tree().unwrap();
    assert_eq!(names(&tree), ["A"]);

    // A real move: C to the root, then before A.
    library
        .save_folder(&FolderChange::Move {
            id: c.clone(),
            parent: None,
            index: 0,
        })
        .unwrap();
    let tree = library.folder_tree().unwrap();
    assert_eq!(names(&tree), ["C", "A"]);
    assert!(find(&tree, &b).children.is_empty());
}

#[test]
fn siblings_keep_the_order_they_are_moved_to() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Order"));
    let ids: Vec<String> = ["One", "Two", "Three", "Four"]
        .iter()
        .map(|name| create(&library, name, None))
        .collect();
    assert_eq!(
        names(&library.folder_tree().unwrap()),
        ["One", "Two", "Three", "Four"]
    );
    let to = |id: &String, index| {
        library
            .save_folder(&FolderChange::Move {
                id: id.clone(),
                parent: None,
                index,
            })
            .unwrap();
    };
    to(&ids[3], 0);
    assert_eq!(
        names(&library.folder_tree().unwrap()),
        ["Four", "One", "Two", "Three"]
    );
    to(&ids[0], 3);
    assert_eq!(
        names(&library.folder_tree().unwrap()),
        ["Four", "Two", "Three", "One"]
    );
    for _ in 0..30 {
        to(&ids[2], 1);
        to(&ids[1], 1);
    }
    assert_eq!(
        names(&library.folder_tree().unwrap()),
        ["Four", "Two", "Three", "One"]
    );

    // Equal keys (as an import could leave them) are numbered again on the next move.
    library
        .conn()
        .execute("UPDATE folder SET sort_key = 'V'", [])
        .unwrap();
    to(&ids[0], 0);
    assert_eq!(library.folder_tree().unwrap()[0].name, "One");
}

#[test]
fn deleting_a_folder_keeps_its_items_unless_asked_to_trash_them() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Delete"));
    let doomed = create(&library, "Doomed", None);
    let inner = create(&library, "Inner", Some(&doomed));
    let kept = create(&library, "Kept", None);
    let only_here = add_png(&library, dir.path(), "only.png", 1);
    let also_elsewhere = add_png(&library, dir.path(), "also.png", 2);
    let inner_only = add_png(&library, dir.path(), "inner.png", 3);
    put(&library, &only_here, &[&doomed]);
    put(&library, &also_elsewhere, &[&doomed, &kept]);
    put(&library, &inner_only, &[&inner]);

    assert_eq!(library.delete_folder(&doomed, false).unwrap(), 0);
    assert_eq!(names(&library.folder_tree().unwrap()), ["Kept"]);
    let mut uncategorized = vec![only_here.clone(), inner_only.clone()];
    uncategorized.sort();
    assert_eq!(scope_ids(&library, &Scope::Unfiled), uncategorized);
    assert_eq!(scope_ids(&library, &Scope::All).len(), 3);
    assert!(matches!(
        library.load_scope(&folder_scope(&doomed, false)),
        Err(SourceError::NotFound)
    ));

    // Again, with the trash option: only the items that were nowhere else go.
    let doomed = create(&library, "Doomed", None);
    put(&library, &only_here, &[&doomed]);
    put(&library, &also_elsewhere, &[&doomed]);
    assert_eq!(library.delete_folder(&doomed, true).unwrap(), 1);
    assert_eq!(scope_ids(&library, &Scope::Trash), [only_here]);
    assert_eq!(
        scope_ids(&library, &folder_scope(&kept, false)),
        [also_elsewhere]
    );
}

#[test]
fn folder_properties_and_inherited_tags() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Props"));
    let parent = create(&library, "Posters", None);
    let child = create(&library, "2024", Some(&parent));
    let update = |id: &str, name: &str, tags: &[&str]| FolderChange::Update {
        id: id.into(),
        name: name.into(),
        description: "for print".into(),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        icon_color: Some("red".into()),
        order_by: Some(SortKey::Title),
        sort_ascending: Some(true),
    };
    library
        .save_folder(&update(&parent, " Print ", &["Poster", "A3"]))
        .unwrap();
    library
        .save_folder(&update(&child, "2024", &["poster"]))
        .unwrap();
    let tree = library.folder_tree().unwrap();
    let posters = find(&tree, &parent);
    assert_eq!(posters.name, "Print", "names are trimmed");
    assert_eq!(posters.description, "for print");
    assert_eq!(posters.icon_color.as_deref(), Some("red"));
    assert_eq!(posters.order_by, Some(SortKey::Title));
    assert_eq!(posters.sort_ascending, Some(true));
    assert_eq!(posters.tags, ["A3", "Poster"]);
    assert_eq!(find(&tree, &child).extend_tags, ["A3", "Poster", "poster"]);
    let mut inherited = library.inherited_folder_tags(&child).unwrap();
    inherited.sort();
    assert_eq!(inherited, ["A3", "Poster", "poster"]);
}

#[test]
fn manual_order_numbers_the_folder_again() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Manual"));
    let folder = create(&library, "Board", None);
    let items: Vec<String> = (0..4)
        .map(|n| add_png(&library, dir.path(), &format!("{n}.png"), n))
        .collect();
    for item in &items {
        put(&library, item, &[&folder]);
    }
    let order = |library: &Library| {
        let mut loaded = library.load_scope(&folder_scope(&folder, false)).unwrap();
        // As the Manual sort orders: positions first, then the date added.
        let key = |item: &ItemLite| (item.position.unwrap_or(f64::INFINITY), item.added_at);
        loaded.sort_by(|a, b| {
            let (a, b) = (key(a), key(b));
            a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))
        });
        loaded.into_iter().map(|item| item.id.0).collect::<Vec<_>>()
    };
    assert_eq!(order(&library), items);
    library
        .reorder_items(
            &folder,
            &[ItemId(items[3].clone()), ItemId(items[2].clone())],
            Some(&ItemId(items[0].clone())),
        )
        .unwrap();
    assert_eq!(
        order(&library),
        [&items[2], &items[3], &items[0], &items[1]].map(String::clone),
        "the moved items keep their own order"
    );
    library
        .reorder_items(&folder, &[ItemId(items[3].clone())], None)
        .unwrap();
    assert_eq!(
        order(&library),
        [&items[2], &items[0], &items[1], &items[3]].map(String::clone)
    );
    // Dropped before one of the moved items: they land before the next item that stays.
    library
        .reorder_items(
            &folder,
            &[ItemId(items[0].clone()), ItemId(items[1].clone())],
            Some(&ItemId(items[0].clone())),
        )
        .unwrap();
    assert_eq!(
        order(&library),
        [&items[2], &items[0], &items[1], &items[3]].map(String::clone)
    );
    // An item that joins later has no position: it comes after the ordered ones.
    let late = add_png(&library, dir.path(), "late.png", 9);
    put(&library, &late, &[&folder]);
    let loaded = library.load_scope(&folder_scope(&folder, false)).unwrap();
    let joined = loaded.iter().find(|item| item.id.0 == late).unwrap();
    assert_eq!(joined.position, None);
    assert_eq!(order(&library).last(), Some(&late));
}

/// Answers the duplicate question with a fixed choice.
struct Answer(DuplicateChoice);

impl ImportObserver for Answer {
    fn progress(&self, _progress: &ImportProgress) {}
    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        self.0
    }
}

#[test]
fn imports_go_into_the_open_folder_and_mirror_dropped_folders() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Into"));
    let target = create(&library, "Inbox", None);
    library
        .save_folder(&FolderChange::Update {
            id: target.clone(),
            name: "Inbox".into(),
            description: String::new(),
            tags: vec!["New".into()],
            icon_color: None,
            order_by: None,
            sort_ascending: None,
        })
        .unwrap();
    let dropped = dir.path().join("Trip");
    fs::create_dir_all(dropped.join("Day 1")).unwrap();
    write_file(
        &dropped,
        "cover.png",
        &fixtures::raster(ImageFormat::Png, 8, 8, 10),
    );
    write_file(
        &dropped.join("Day 1"),
        "beach.png",
        &fixtures::raster(ImageFormat::Png, 8, 8, 11),
    );
    let loose = write_file(
        dir.path(),
        "loose.png",
        &fixtures::raster(ImageFormat::Png, 8, 8, 12),
    );

    let into = ImportTarget {
        folder: Some(target.clone()),
    };
    let done = import_files(
        &library,
        1,
        &[dropped.clone(), loose.clone()],
        &into,
        DuplicatePolicy::Ask,
        &Answer(DuplicateChoice::Skip),
        &AtomicBool::new(false),
    );
    assert_eq!(done.added, 3);
    let tree = library.folder_tree().unwrap();
    let inbox = find(&tree, &target);
    assert_eq!(names(&inbox.children), ["Trip"]);
    let trip = &inbox.children[0];
    assert_eq!(names(&trip.children), ["Day 1"]);
    assert_eq!(inbox.image_count, Some(1), "the loose file");
    assert_eq!(trip.image_count, Some(1));
    assert_eq!(trip.children[0].image_count, Some(1));
    // Every new item carries the target folder's tag.
    let untagged = library.load_scope(&Scope::Untagged).unwrap();
    assert!(untagged.is_empty());

    // The same folder again: its folders are reused, and the duplicates join another folder.
    let elsewhere = create(&library, "Elsewhere", None);
    let again = import_files(
        &library,
        2,
        &[dropped],
        &ImportTarget {
            folder: Some(elsewhere.clone()),
        },
        DuplicatePolicy::Ask,
        &Answer(DuplicateChoice::AddToFolder),
        &AtomicBool::new(false),
    );
    assert_eq!((again.added, again.duplicates), (0, 2));
    let tree = library.folder_tree().unwrap();
    let mirrored = &find(&tree, &elsewhere).children[0];
    assert_eq!(mirrored.name, "Trip");
    assert_eq!(mirrored.descendant_image_count, Some(2));
    assert_eq!(
        find(&tree, &trip.id).descendant_image_count,
        Some(2),
        "still in the first"
    );
}

#[test]
fn a_read_only_library_refuses_folder_writes() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Shared");
    let writer = open(&root);
    let folder = create(&writer, "Posters", None);
    let reader = Library::open(&root, &OpenOptions::new(APP)).unwrap();
    assert!(reader.read_only_reason().is_some());
    assert_eq!(reader.folder_tree().unwrap().len(), 1);
    assert_eq!(
        reader.save_folder(&FolderChange::Create {
            name: "x".into(),
            parent: None,
            description: String::new(),
            icon_color: None,
        }),
        Err(SourceError::ReadOnly)
    );
    assert_eq!(
        reader.delete_folder(&folder, false),
        Err(SourceError::ReadOnly)
    );
}
