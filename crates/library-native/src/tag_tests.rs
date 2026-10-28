//! Tags: item tag edits, the vocabulary, rename, merge and delete everywhere, tag groups
//! (docs/spec.md §9 phase 5).

use std::sync::atomic::AtomicBool;

use library_core::{
    DuplicateNotice, FolderChange, ImportProgress, ItemEdit, ItemId, LibraryQuery, Scope,
    SourceError, Tag, TagGroupChange, TagMode, matches_tags,
};
use rusqlite::params;

use crate::import::{DuplicateChoice, ImportObserver, make_pending_previews};
use crate::test_support::*;
use crate::{Library, OpenOptions};

fn ids(items: &[&str]) -> Vec<ItemId> {
    items.iter().map(|id| ItemId((*id).to_owned())).collect()
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn tag(library: &Library, items: &[&str], names: &[&str]) -> u32 {
    library
        .apply_batch_edit(&ids(items), &ItemEdit::AddTags(strings(names)))
        .unwrap()
}

fn untag(library: &Library, items: &[&str], names: &[&str]) -> u32 {
    library
        .apply_batch_edit(&ids(items), &ItemEdit::RemoveTags(strings(names)))
        .unwrap()
}

fn tags_of(library: &Library, item: &str) -> Vec<String> {
    library.item(item).unwrap().unwrap().tags
}

fn vocabulary(library: &Library) -> Vec<(String, u32)> {
    library
        .tags()
        .unwrap()
        .into_iter()
        .map(|Tag { name, image_count }| (name, image_count.unwrap()))
        .collect()
}

fn tag_rows(library: &Library) -> Vec<String> {
    let conn = library.conn();
    let mut statement = conn.prepare("SELECT name FROM tag ORDER BY name").unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// The items of All that a tag filter keeps, the way the views filter them.
fn filtered(library: &Library, selected: &[&str], mode: TagMode) -> Vec<String> {
    let query = LibraryQuery {
        tags: strings(selected),
        tag_mode: mode,
        ..LibraryQuery::default()
    };
    let mut kept: Vec<String> = library
        .load_scope(&Scope::All)
        .unwrap()
        .into_iter()
        .filter(|item| matches_tags(&item.tags, &query.tags, query.tag_mode))
        .map(|item| item.id.0)
        .collect();
    kept.sort();
    kept
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

#[test]
fn tag_modes_on_the_stored_tags_match_the_done_check() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let both = add_png(&library, dir.path(), "both.png", 1);
    let poster = add_png(&library, dir.path(), "poster.png", 2);
    let extra = add_png(&library, dir.path(), "extra.png", 3);
    let lower = add_png(&library, dir.path(), "lower.png", 4);
    let none = add_png(&library, dir.path(), "none.png", 5);
    assert_eq!(tag(&library, &[&both, &extra], &["Poster", "Swiss"]), 2);
    tag(&library, &[&poster], &["Poster"]);
    tag(&library, &[&extra], &["Print"]);
    tag(&library, &[&lower], &["poster"]);

    let any = filtered(&library, &["Poster", "Swiss"], TagMode::Any);
    assert_eq!(
        any,
        sorted(vec![both.clone(), poster.clone(), extra.clone()])
    );
    let all = filtered(&library, &["Poster", "Swiss"], TagMode::All);
    assert_eq!(all, sorted(vec![both.clone(), extra.clone()]));
    let exact = filtered(&library, &["Poster", "Swiss"], TagMode::Exact);
    assert_eq!(exact, vec![both.clone()]);
    let neither = filtered(&library, &["Poster", "Swiss"], TagMode::None);
    assert_eq!(neither, sorted(vec![lower.clone(), none.clone()]));
    // Exact strings: "Poster" is not "poster".
    assert_eq!(filtered(&library, &["poster"], TagMode::Any), vec![lower]);
}

#[test]
fn removing_one_tag_keeps_the_others_and_drops_unused_names() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    tag(&library, &[&a, &b], &["Poster", "Swiss", "Poster"]);
    tag(&library, &[&a], &["Logo"]);

    assert_eq!(untag(&library, &[&a, &b], &["Swiss", "Unknown"]), 2);
    assert_eq!(tags_of(&library, &a), ["Logo", "Poster"]);
    assert_eq!(tags_of(&library, &b), ["Poster"]);
    // Nothing refers to "Swiss" any more.
    assert_eq!(tag_rows(&library), ["Logo", "Poster"]);
    assert_eq!(
        vocabulary(&library),
        [("Logo".to_owned(), 1), ("Poster".to_owned(), 2)]
    );
}

#[test]
fn tag_edits_validate_and_skip_missing_items() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let a = add_png(&library, dir.path(), "a.png", 1);
    assert_eq!(tag(&library, &[&a, "01MISSING"], &["Poster"]), 1);
    assert!(matches!(
        library.apply_batch_edit(&ids(&[&a]), &ItemEdit::AddTags(vec![String::new()])),
        Err(SourceError::Invalid(_))
    ));
    let long = "x".repeat(10_001);
    assert!(matches!(
        library.apply_batch_edit(&ids(&[&a]), &ItemEdit::AddTags(vec![long])),
        Err(SourceError::Invalid(_))
    ));
}

#[test]
fn a_rename_changes_the_tag_on_every_item_folder_and_group() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    tag(&library, &[&a, &b], &["poster"]);
    tag(&library, &[&b], &["Swiss"]);
    let folder = create(&library, "Posters", None);
    library
        .save_folder(&FolderChange::Update {
            id: folder.clone(),
            name: "Posters".into(),
            description: String::new(),
            tags: strings(&["poster"]),
            icon_color: None,
            order_by: None,
            sort_ascending: None,
        })
        .unwrap();
    let group = library
        .save_tag_group(&TagGroupChange::Create {
            name: "Kinds".into(),
            color: None,
            tags: strings(&["poster", "Logo"]),
        })
        .unwrap();
    let before: i64 = updated_at(&library, &a);

    // Only the case changes: still a rename, not a merge.
    library.rename_tag("poster", "Poster").unwrap();
    assert_eq!(tags_of(&library, &a), ["Poster"]);
    assert_eq!(tags_of(&library, &b), ["Poster", "Swiss"]);
    assert_eq!(library.inherited_folder_tags(&folder).unwrap(), ["Poster"]);
    assert_eq!(group_tags(&library, &group), ["Logo", "Poster"]);
    assert!(updated_at(&library, &a) >= before);
    assert_eq!(
        library.rename_tag("missing", "x"),
        Err(SourceError::NotFound)
    );
    assert!(matches!(
        library.rename_tag("Poster", ""),
        Err(SourceError::Invalid(_))
    ));
    library.rename_tag("Poster", "Poster").unwrap();
}

#[test]
fn renaming_onto_an_existing_tag_merges_the_two() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    tag(&library, &[&a], &["Posters", "Poster"]);
    tag(&library, &[&b], &["Posters"]);
    let group = library
        .save_tag_group(&TagGroupChange::Create {
            name: "Kinds".into(),
            color: Some("red".into()),
            tags: strings(&["Posters"]),
        })
        .unwrap();

    library.rename_tag("Posters", "Poster").unwrap();
    assert_eq!(tags_of(&library, &a), ["Poster"]);
    assert_eq!(tags_of(&library, &b), ["Poster"]);
    assert_eq!(group_tags(&library, &group), ["Poster"]);
    assert_eq!(tag_rows(&library), ["Poster"]);
    assert_eq!(vocabulary(&library), [("Poster".to_owned(), 2)]);
}

#[test]
fn a_merge_needs_both_tags() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    tag(&library, &[&a], &["Logo"]);
    tag(&library, &[&b], &["Logos"]);
    assert_eq!(
        library.merge_tags("Logos", "Mark"),
        Err(SourceError::NotFound)
    );
    library.merge_tags("Logos", "Logo").unwrap();
    assert_eq!(tags_of(&library, &b), ["Logo"]);
    assert_eq!(vocabulary(&library), [("Logo".to_owned(), 2)]);
}

#[test]
fn deleting_a_tag_removes_it_everywhere_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let a = add_png(&library, dir.path(), "a.png", 1);
    let b = add_png(&library, dir.path(), "b.png", 2);
    tag(&library, &[&a, &b], &["Draft", "Poster"]);
    let group = library
        .save_tag_group(&TagGroupChange::Create {
            name: "State".into(),
            color: None,
            tags: strings(&["Draft", "Final"]),
        })
        .unwrap();
    library.delete_tag("Draft").unwrap();
    assert_eq!(tags_of(&library, &a), ["Poster"]);
    assert_eq!(tags_of(&library, &b), ["Poster"]);
    assert_eq!(group_tags(&library, &group), ["Final"]);
    assert_eq!(library.delete_tag("Draft"), Err(SourceError::NotFound));
}

#[test]
fn the_vocabulary_has_item_tags_trash_and_group_tags_but_not_hidden_ones() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let open_item = add_png(&library, dir.path(), "open.png", 1);
    let trashed = add_png(&library, dir.path(), "trashed.png", 2);
    let secret = add_png(&library, dir.path(), "secret.png", 3);
    tag(&library, &[&open_item], &["Swiss", "poster"]);
    tag(&library, &[&trashed], &["Old"]);
    tag(&library, &[&secret], &["Taxes", "Swiss"]);
    library
        .save_tag_group(&TagGroupChange::Create {
            name: "Ideas".into(),
            color: None,
            tags: strings(&["Someday"]),
        })
        .unwrap();
    library
        .conn()
        .execute(
            "UPDATE item SET deleted_at = 1 WHERE id = ?1",
            params![trashed],
        )
        .unwrap();
    let private = create(&library, "Private", None);
    put(&library, &secret, &[&private]);
    make_pending_previews(&library, 1, &Quiet, &AtomicBool::new(false));
    library
        .lock_folder(&private, "correct horse".into(), None, None, &|_, _| {})
        .unwrap();

    // By name, case-insensitively first; counts leave out the trash and hidden items.
    assert_eq!(
        vocabulary(&library),
        [
            ("Old".to_owned(), 0),
            ("poster".to_owned(), 1),
            ("Someday".to_owned(), 0),
            ("Swiss".to_owned(), 1),
        ]
    );
    // A hidden item takes no tag edits.
    assert_eq!(tag(&library, &[&secret, &open_item], &["New"]), 1);
    assert_eq!(untag(&library, &[&secret], &["Taxes"]), 0);

    library
        .unlock_folder(&private, "correct horse".into())
        .unwrap();
    assert_eq!(tags_of(&library, &secret), ["Swiss", "Taxes"]);
    assert!(vocabulary(&library).contains(&("Taxes".to_owned(), 1)));
    // Tags are not sealed: tagging an item of an open lock writes no files.
    assert_eq!(tag(&library, &[&secret], &["Receipt"]), 1);
    assert_eq!(tags_of(&library, &secret), ["Receipt", "Swiss", "Taxes"]);
}

#[test]
fn tag_groups_keep_their_order_and_never_touch_items() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Tags"));
    let a = add_png(&library, dir.path(), "a.png", 1);
    tag(&library, &[&a], &["Serif"]);
    let first = library
        .save_tag_group(&TagGroupChange::Create {
            name: "  Type ".into(),
            color: Some("blue".into()),
            tags: strings(&["sans", "Serif", "Mono"]),
        })
        .unwrap();
    let second = library
        .save_tag_group(&TagGroupChange::Create {
            name: "Colour".into(),
            color: None,
            tags: Vec::new(),
        })
        .unwrap();
    let groups = library.tag_groups().unwrap();
    assert_eq!(
        groups
            .iter()
            .map(|group| group.id.as_str())
            .collect::<Vec<_>>(),
        [first.as_str(), second.as_str()]
    );
    assert_eq!(groups[0].name, "Type");
    assert_eq!(groups[0].color.as_deref(), Some("blue"));
    assert_eq!(groups[0].tags, ["Mono", "sans", "Serif"]);

    library
        .save_tag_group(&TagGroupChange::Update {
            id: first.clone(),
            name: "Typefaces".into(),
            color: None,
            tags: strings(&["Serif"]),
        })
        .unwrap();
    let groups = library.tag_groups().unwrap();
    assert_eq!(groups[0].name, "Typefaces");
    assert_eq!(groups[0].color, None);
    assert_eq!(groups[0].tags, ["Serif"]);
    // "sans" and "Mono" were only in the group.
    assert_eq!(tag_rows(&library), ["Serif"]);

    library.delete_tag_group(&first).unwrap();
    assert_eq!(tags_of(&library, &a), ["Serif"]);
    assert_eq!(library.tag_groups().unwrap().len(), 1);
    assert_eq!(library.delete_tag_group(&first), Err(SourceError::NotFound));
    assert_eq!(
        library.save_tag_group(&TagGroupChange::Update {
            id: first,
            name: "Gone".into(),
            color: None,
            tags: Vec::new(),
        }),
        Err(SourceError::NotFound)
    );
}

#[test]
fn a_read_only_library_refuses_tag_writes() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Shared");
    let writer = open(&root);
    let a = add_png(&writer, dir.path(), "a.png", 1);
    tag(&writer, &[&a], &["Poster"]);
    let reader = Library::open(&root, &OpenOptions::new(APP)).unwrap();
    assert!(reader.read_only_reason().is_some());
    assert_eq!(vocabulary(&reader), [("Poster".to_owned(), 1)]);
    assert_eq!(
        reader.apply_batch_edit(&ids(&[&a]), &ItemEdit::AddTags(strings(&["x"]))),
        Err(SourceError::ReadOnly)
    );
    assert_eq!(reader.rename_tag("Poster", "x"), Err(SourceError::ReadOnly));
    assert_eq!(reader.merge_tags("Poster", "x"), Err(SourceError::ReadOnly));
    assert_eq!(reader.delete_tag("Poster"), Err(SourceError::ReadOnly));
    assert_eq!(
        reader.save_tag_group(&TagGroupChange::Create {
            name: "x".into(),
            color: None,
            tags: Vec::new(),
        }),
        Err(SourceError::ReadOnly)
    );
    assert_eq!(reader.delete_tag_group("G"), Err(SourceError::ReadOnly));
}

struct Quiet;

impl ImportObserver for Quiet {
    fn progress(&self, _progress: &ImportProgress) {}
    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        DuplicateChoice::Skip
    }
}

fn updated_at(library: &Library, item: &str) -> i64 {
    library
        .conn()
        .query_row(
            "SELECT updated_at FROM item WHERE id = ?1",
            params![item],
            |row| row.get(0),
        )
        .unwrap()
}

fn group_tags(library: &Library, group: &str) -> Vec<String> {
    library
        .tag_groups()
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == group)
        .unwrap()
        .tags
}
