//! The search field in the native library (docs/spec.md §5 "Search", §9 phase 6, D-047).

use std::sync::atomic::AtomicBool;

use library_core::{DuplicateNotice, ImportProgress, ItemEdit, ItemId, SearchMatch};
use rusqlite::params;

use crate::import::{DuplicateChoice, ImportObserver, make_pending_previews};
use crate::test_support::*;
use crate::{DB_FILE, Library};

/// Sets a note and a link the way the phase 7 editor will: the row and its index entry.
fn describe(library: &Library, item: &str, annotation: &str, url: &str) {
    let conn = library.conn();
    conn.execute(
        "UPDATE item SET annotation = ?2, url = ?3 WHERE id = ?1",
        params![item, annotation, url],
    )
    .unwrap();
    conn.execute(
        "UPDATE item_fts SET annotation = ?2, url = ?3
         WHERE rowid = (SELECT seq FROM item WHERE id = ?1)",
        params![item, annotation, url],
    )
    .unwrap();
}

fn tag(library: &Library, item: &str, names: &[&str]) {
    library
        .apply_item_edit(
            item,
            &ItemEdit::AddTags(names.iter().map(|name| (*name).to_owned()).collect()),
        )
        .unwrap();
}

/// The ids a search keeps among `all`, sorted.
fn kept(library: &Library, text: &str, all: &[&String]) -> Vec<String> {
    let found = library.search(text).unwrap();
    let mut ids: Vec<String> = all
        .iter()
        .filter(|id| found.contains(&ItemId((**id).clone())))
        .map(|id| (*id).clone())
        .collect();
    ids.sort();
    ids
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

struct Sample {
    library: Library,
    poster: String,
    note: String,
    swiss: String,
    filed: String,
    cyrillic: String,
}

/// poster: named "Poster Final"; note: a note and a link; swiss: tagged "Swiss Style";
/// filed: in the folder "Плакаты"; cyrillic: named "ПЛАКАТ".
fn sample(dir: &std::path::Path) -> Sample {
    let library = open(&new_library(dir, "Search"));
    let poster = add_png(&library, dir, "Poster Final.png", 1);
    let note = add_png(&library, dir, "IMG_0001.png", 2);
    let swiss = add_png(&library, dir, "IMG_0002.png", 3);
    let filed = add_png(&library, dir, "IMG_0003.png", 4);
    let cyrillic = add_png(&library, dir, "ПЛАКАТ.png", 5);
    describe(
        &library,
        &note,
        "Grid notes for the brochure",
        "https://example.com/brochure",
    );
    tag(&library, &swiss, &["Swiss Style"]);
    let folder = create(&library, "Плакаты", None);
    put(&library, &filed, &[&folder]);
    Sample {
        library,
        poster,
        note,
        swiss,
        filed,
        cyrillic,
    }
}

#[test]
fn a_term_is_any_part_of_a_name_note_link_tag_or_folder_name() {
    let dir = tempfile::tempdir().unwrap();
    let s = sample(dir.path());
    let all = [&s.poster, &s.note, &s.swiss, &s.filed, &s.cyrillic];
    let search = |text: &str| kept(&s.library, text, &all);
    // The middle of a word, without case.
    assert_eq!(search("STER"), vec![s.poster.clone()]);
    assert_eq!(search("brochure"), vec![s.note.clone()]);
    assert_eq!(search("example.com"), vec![s.note.clone()]);
    assert_eq!(search("wiss sty"), vec![s.swiss.clone()]);
    // Cyrillic folds too: the folder "Плакаты" and the name "ПЛАКАТ".
    assert_eq!(
        search("плакат"),
        sorted(vec![s.filed.clone(), s.cyrillic.clone()])
    );
    // Two characters are searched without the index.
    assert_eq!(search("fi"), vec![s.poster.clone()]);
    assert_eq!(
        search("пл"),
        sorted(vec![s.filed.clone(), s.cyrillic.clone()])
    );
    assert_eq!(search("nothing like this"), Vec::<String>::new());
}

#[test]
fn or_minus_and_parentheses_combine_terms() {
    let dir = tempfile::tempdir().unwrap();
    let s = sample(dir.path());
    let all = [&s.poster, &s.note, &s.swiss, &s.filed, &s.cyrillic];
    let search = |text: &str| kept(&s.library, text, &all);
    let everything = sorted(all.iter().map(|id| (*id).clone()).collect());
    assert_eq!(
        search("poster OR brochure"),
        sorted(vec![s.poster.clone(), s.note.clone()])
    );
    assert_eq!(search("img grid"), vec![s.note.clone()]);
    let not_img = search("-img");
    assert_eq!(not_img, sorted(vec![s.poster.clone(), s.cyrillic.clone()]));
    assert_eq!(search("img -(grid OR swiss)"), vec![s.filed.clone()]);
    assert_eq!(
        search("(final OR плакат) -плакаты"),
        sorted(vec![s.poster.clone(), s.cyrillic.clone()])
    );
    // Only exclusions: everything else.
    let found = s.library.search("-poster").unwrap();
    assert!(matches!(found, SearchMatch::Except(_)));
    assert_eq!(
        search("-poster"),
        everything
            .iter()
            .filter(|id| **id != s.poster)
            .cloned()
            .collect::<Vec<_>>()
    );
    // `or` in lowercase is a word: no item has it.
    assert_eq!(search("poster or brochure"), Vec::<String>::new());
}

#[test]
fn search_inside_a_folder_keeps_only_that_folder() {
    let dir = tempfile::tempdir().unwrap();
    let s = sample(dir.path());
    let folder = s.library.folder_tree().unwrap()[0].id.clone();
    let scope = scope_ids(&s.library, &folder_scope(&folder, false));
    let found = s.library.search("img").unwrap();
    let inside: Vec<&String> = scope
        .iter()
        .filter(|id| found.contains(&ItemId((*id).clone())))
        .collect();
    // "img" matches three items; only the one in the folder is in its scope.
    assert_eq!(inside, [&s.filed]);
}

struct Quiet;

impl ImportObserver for Quiet {
    fn progress(&self, _progress: &ImportProgress) {}
    fn decide(&self, _duplicate: &DuplicateNotice) -> DuplicateChoice {
        DuplicateChoice::Skip
    }
}

#[test]
fn encrypted_items_are_found_only_while_their_folder_is_open() {
    let dir = tempfile::tempdir().unwrap();
    let library = open(&new_library(dir.path(), "Locked"));
    let secret = add_png(&library, dir.path(), "secret plan.png", 1);
    let private = create(&library, "Private", None);
    put(&library, &secret, &[&private]);
    make_pending_previews(&library, 1, &Quiet, &AtomicBool::new(false));
    library
        .lock_folder(&private, "correct horse".into(), None, None, &|_, _| {})
        .unwrap();
    let id = ItemId(secret.clone());
    assert!(!library.search("secret").unwrap().contains(&id));
    assert!(!library.search("se").unwrap().contains(&id));
    library
        .unlock_folder(&private, "correct horse".into())
        .unwrap();
    assert!(library.search("secret").unwrap().contains(&id));
    assert!(library.search("pl").unwrap().contains(&id));
}

#[test]
fn an_older_library_gets_the_new_index_after_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let root = new_library(dir.path(), "Old");
    let library = open(&root);
    let poster = add_png(&library, dir.path(), "Poster Final.png", 1);
    library.close().unwrap();
    make_version_one(&root.join(DB_FILE));

    let library = open(&root);
    let version: u32 = library
        .conn()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, crate::FORMAT_VERSION);
    assert!(
        files_under(&root.join("backups"))
            .iter()
            .any(|name| name.ends_with("-v1.db"))
    );
    assert!(library.search("ster").unwrap().contains(&ItemId(poster)));
}
