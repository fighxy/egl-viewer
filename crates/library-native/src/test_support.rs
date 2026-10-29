//! Helpers for the tests of this crate.

use std::fs;
use std::path::{Path, PathBuf};

use image::ImageFormat;
use library_core::{Folder, FolderChange, ItemEdit, Scope};
use media::fixtures;
use rusqlite::Connection;

use crate::{AddOutcome, Duplicates, Library, OpenOptions, create_library};

pub(crate) const APP: &str = "0.1.0-test";

pub(crate) fn options() -> OpenOptions {
    OpenOptions::new(APP)
}

pub(crate) fn new_library(parent: &Path, name: &str) -> PathBuf {
    create_library(parent, name, APP).expect("create a library")
}

pub(crate) fn open(root: &Path) -> Library {
    Library::open(root, &options()).expect("open the library")
}

pub(crate) fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

/// Every file under `dir`, relative, sorted.
pub(crate) fn files_under(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(
                    path.strip_prefix(dir)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    found.sort();
    found
}

pub(crate) fn create(library: &Library, name: &str, parent: Option<&str>) -> String {
    library
        .save_folder(&FolderChange::Create {
            name: name.into(),
            parent: parent.map(Into::into),
            description: String::new(),
            icon_color: None,
        })
        .unwrap()
}

pub(crate) fn add_png(library: &Library, dir: &Path, name: &str, seed: u32) -> String {
    let source = write_file(dir, name, &fixtures::raster(ImageFormat::Png, 8, 8, seed));
    match library.add_file(&source, Duplicates::Report).unwrap() {
        AddOutcome::Added { id, .. } => id,
        AddOutcome::Duplicate { .. } => panic!("{name} was a duplicate"),
    }
}

pub(crate) fn put(library: &Library, item: &str, folders: &[&str]) {
    let edit = ItemEdit::AddToFolders(folders.iter().map(|id| (*id).to_owned()).collect());
    library.apply_item_edit(item, &edit).unwrap();
}

pub(crate) fn find<'a>(tree: &'a [Folder], id: &str) -> &'a Folder {
    fn walk<'a>(tree: &'a [Folder], id: &str) -> Option<&'a Folder> {
        tree.iter().find_map(|folder| {
            (folder.id == id)
                .then_some(folder)
                .or_else(|| walk(&folder.children, id))
        })
    }
    walk(tree, id).unwrap_or_else(|| panic!("no folder {id}"))
}

pub(crate) fn names(tree: &[Folder]) -> Vec<String> {
    tree.iter().map(|folder| folder.name.clone()).collect()
}

pub(crate) fn scope_ids(library: &Library, scope: &Scope) -> Vec<String> {
    let mut ids: Vec<String> = library
        .load_scope(scope)
        .unwrap()
        .into_iter()
        .map(|item| item.id.0)
        .collect();
    ids.sort();
    ids
}

pub(crate) fn folder_scope(id: &str, include_descendants: bool) -> Scope {
    Scope::Folder {
        id: id.into(),
        include_descendants,
    }
}

/// Turns a library database back into format version 1: the old index, the old version.
pub(crate) fn make_version_one(db: &Path) {
    let conn = Connection::open(db).unwrap();
    conn.execute_batch(
        "DROP TABLE item_fts;
         CREATE VIRTUAL TABLE item_fts USING fts5(
           name, annotation, url, tags, folders,
           tokenize = 'unicode61 remove_diacritics 2'
         );
         INSERT INTO item_fts (rowid, name, annotation, url, tags, folders)
           SELECT seq, name, annotation, url, '', '' FROM item;
         PRAGMA user_version = 1;",
    )
    .unwrap();
}
