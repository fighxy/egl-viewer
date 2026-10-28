//! Helpers for the tests of this crate.

use std::fs;
use std::path::{Path, PathBuf};

use crate::{Library, OpenOptions, create_library};

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
