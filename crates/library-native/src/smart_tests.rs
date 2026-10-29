//! Smart folders in the native library (docs/spec.md §3, §9 phase 6).

use library_core::{
    FolderIndex, ItemEdit, LibraryQuery, Scope, SmartFolderChange, SourceError, TagMode,
    rules_from_query,
};
use rusqlite::params;
use serde_json::{Value, json};

use crate::test_support::*;
use crate::{Library, OpenOptions};

fn tag(library: &Library, item: &str, names: &[&str]) {
    library
        .apply_item_edit(
            item,
            &ItemEdit::AddTags(names.iter().map(|name| (*name).to_owned()).collect()),
        )
        .unwrap();
}

fn tagged_rule(tag: &str) -> Value {
    json!([{ "match": "AND", "boolean": "TRUE", "rules": [
        { "property": "tags", "method": "union", "value": [tag] }
    ]}])
}

fn smart(library: &Library, name: &str, parent: Option<&str>, conditions: Value) -> String {
    library
        .save_smart_folder(&SmartFolderChange::Create {
            name: name.into(),
            parent: parent.map(Into::into),
            conditions,
        })
        .unwrap()
}

fn smart_scope(id: &str) -> Scope {
    Scope::Smart { id: id.into() }
}

#[test]
fn smart_folders_form_their_own_tree() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Smart"));
    let posters = smart(&library, "Posters", None, tagged_rule("Poster"));
    let fonts = smart(&library, "Fonts", None, tagged_rule("Font"));
    let swiss = smart(&library, " Swiss ", Some(&posters), tagged_rule("Swiss"));
    let odd = smart(
        &library,
        "Colours",
        None,
        json!([{ "match": "AND", "rules": [{ "property": "color", "method": "similar", "value": "#f00" }] }]),
    );
    let tree = library.smart_folders().unwrap();
    assert_eq!(
        tree.iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>(),
        [posters.as_str(), fonts.as_str(), odd.as_str()]
    );
    assert_eq!(tree[0].children[0].id, swiss);
    assert_eq!(tree[0].children[0].name, "Swiss");
    assert_eq!(tree[0].conditions, tagged_rule("Poster"));
    assert!(tree[0].unsupported.is_empty());
    assert_eq!(tree[2].unsupported, ["color similar"]);

    library
        .save_smart_folder(&SmartFolderChange::Update {
            id: fonts.clone(),
            name: "Type".into(),
            conditions: tagged_rule("Type"),
        })
        .unwrap();
    assert_eq!(library.smart_folders().unwrap()[1].name, "Type");

    // Deleting a smart folder takes the ones under it; items do not change.
    library.delete_smart_folder(&posters).unwrap();
    let names: Vec<String> = library
        .smart_folders()
        .unwrap()
        .into_iter()
        .map(|folder| folder.name)
        .collect();
    assert_eq!(names, ["Type", "Colours"]);
    assert_eq!(
        library.delete_smart_folder(&posters),
        Err(SourceError::NotFound)
    );
    assert_eq!(
        library.load_scope(&smart_scope(&swiss)),
        Err(SourceError::NotFound)
    );
}

#[test]
fn bad_smart_folders_are_refused_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Smart"));
    let refused = |change: SmartFolderChange| library.save_smart_folder(&change).unwrap_err();
    assert!(matches!(
        refused(SmartFolderChange::Create {
            name: "x".into(),
            parent: None,
            conditions: json!({ "rules": [] }),
        }),
        SourceError::Invalid(_)
    ));
    assert!(matches!(
        refused(SmartFolderChange::Create {
            name: "x".into(),
            parent: Some("01MISSING".into()),
            conditions: tagged_rule("a"),
        }),
        SourceError::Invalid(_)
    ));
    assert_eq!(
        refused(SmartFolderChange::Update {
            id: "01MISSING".into(),
            name: "x".into(),
            conditions: tagged_rule("a"),
        }),
        SourceError::NotFound
    );
    assert!(library.smart_folders().unwrap().is_empty());
}

#[test]
fn a_smart_folder_shows_the_items_its_rules_and_its_parents_keep() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Smart"));
    let poster = add_png(&library, dir.path(), "poster.png", 1);
    let swiss = add_png(&library, dir.path(), "swiss.png", 2);
    let noted = add_png(&library, dir.path(), "noted.png", 3);
    let trashed = add_png(&library, dir.path(), "trashed.png", 4);
    tag(&library, &poster, &["Poster"]);
    tag(&library, &swiss, &["Poster", "Swiss"]);
    tag(&library, &trashed, &["Poster"]);
    library
        .conn()
        .execute(
            "UPDATE item SET annotation = 'Grid notes' WHERE id = ?1",
            params![noted],
        )
        .unwrap();
    library
        .conn()
        .execute(
            "UPDATE item SET deleted_at = 1 WHERE id = ?1",
            params![trashed],
        )
        .unwrap();
    let posters = smart(&library, "Posters", None, tagged_rule("Poster"));
    let swiss_only = smart(&library, "Swiss", Some(&posters), tagged_rule("Swiss"));
    let notes = smart(
        &library,
        "Notes",
        None,
        json!([{ "match": "AND", "rules": [{ "property": "annotation", "method": "contain", "value": "GRID" }] }]),
    );
    let nested_only_swiss = smart(
        &library,
        "Everything",
        Some(&swiss_only),
        json!([
            { "match": "AND", "rules": [{ "property": "tags", "method": "not-empty", "value": [] }] }
        ]),
    );
    // The trash stays out.
    assert_eq!(
        scope_ids(&library, &smart_scope(&posters)),
        sorted(vec![poster.clone(), swiss.clone()])
    );
    assert_eq!(
        scope_ids(&library, &smart_scope(&swiss_only)),
        vec![swiss.clone()]
    );
    // A nested smart folder keeps its parents' rules too.
    assert_eq!(
        scope_ids(&library, &smart_scope(&nested_only_swiss)),
        vec![swiss.clone()]
    );
    assert_eq!(scope_ids(&library, &smart_scope(&notes)), [noted]);
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

/// The done check in the native library: a smart folder saved from a screen shows its items.
#[test]
fn a_smart_folder_saved_from_chips_shows_what_the_chips_showed() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Smart"));
    let items: Vec<String> = (0..6)
        .map(|n| add_png(&library, dir.path(), &format!("item-{n}.png"), n))
        .collect();
    let parent = create(&library, "Parent", None);
    let child = create(&library, "Child", Some(&parent));
    tag(&library, &items[0], &["Poster"]);
    tag(&library, &items[1], &["Poster", "Swiss"]);
    tag(&library, &items[2], &["poster"]);
    tag(&library, &items[4], &["Poster", "Swiss", "Print"]);
    put(&library, &items[0], &[&parent]);
    put(&library, &items[1], &[&child]);
    put(&library, &items[4], &[&parent]);
    library
        .conn()
        .execute("UPDATE item SET star = 4 WHERE id = ?1", params![items[3]])
        .unwrap();
    let screens = [
        LibraryQuery {
            tags: vec!["Poster".into(), "Swiss".into()],
            tag_mode: TagMode::All,
            ..LibraryQuery::default()
        },
        LibraryQuery {
            tags: vec!["Poster".into(), "Swiss".into()],
            tag_mode: TagMode::Exact,
            ..LibraryQuery::default()
        },
        LibraryQuery {
            scope: folder_scope(&parent, true),
            tags: vec!["Swiss".into()],
            tag_mode: TagMode::None,
            ..LibraryQuery::default()
        },
        LibraryQuery {
            scope: Scope::Unfiled,
            rating: Some(4),
            ..LibraryQuery::default()
        },
        LibraryQuery {
            scope: Scope::Untagged,
            ext: Some("png".into()),
            ..LibraryQuery::default()
        },
    ];
    let index = FolderIndex::new(&library.folder_tree().unwrap());
    for (n, query) in screens.into_iter().enumerate() {
        let snapshot = library.load_scope(&query.scope).unwrap();
        let mut shown: Vec<String> = library_core::query::filter::filter(
            &snapshot,
            &query,
            library_core::query::filter::Context {
                folders: &index,
                text: None,
            },
        )
        .into_iter()
        .map(|at| snapshot[at as usize].id.0.clone())
        .collect();
        shown.sort();
        let (parent, conditions) = rules_from_query(&query, &index).unwrap();
        let id = library
            .save_smart_folder(&SmartFolderChange::Create {
                name: format!("Saved {n}"),
                parent,
                conditions,
            })
            .unwrap();
        assert_eq!(scope_ids(&library, &smart_scope(&id)), shown, "{query:?}");
    }
}

#[test]
fn extensions_are_counted_outside_the_trash() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Types"));
    let a = add_png(&library, dir.path(), "a.png", 1);
    add_png(&library, dir.path(), "b.png", 2);
    let c = add_png(&library, dir.path(), "c.png", 3);
    library
        .conn()
        .execute("UPDATE item SET ext = 'jpg' WHERE id = ?1", params![a])
        .unwrap();
    library
        .conn()
        .execute("UPDATE item SET deleted_at = 1 WHERE id = ?1", params![c])
        .unwrap();
    let counts: Vec<(String, u32)> = library
        .extensions()
        .unwrap()
        .into_iter()
        .map(|count| (count.ext, count.count))
        .collect();
    assert_eq!(counts, [("jpg".to_owned(), 1), ("png".to_owned(), 1)]);
}

#[test]
fn a_read_only_library_refuses_smart_folder_writes() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Shared");
    let writer = open(&root);
    let id = smart(&writer, "Posters", None, tagged_rule("Poster"));
    let reader = Library::open(&root, &OpenOptions::new(APP)).unwrap();
    assert_eq!(reader.smart_folders().unwrap().len(), 1);
    assert_eq!(
        reader.save_smart_folder(&SmartFolderChange::Create {
            name: "x".into(),
            parent: None,
            conditions: tagged_rule("a"),
        }),
        Err(SourceError::ReadOnly)
    );
    assert_eq!(reader.delete_smart_folder(&id), Err(SourceError::ReadOnly));
}
