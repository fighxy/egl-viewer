//! Recent libraries and what to reopen on start (docs/spec.md §7.2, D-022).

use std::path::Path;

use library_core::SourceError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AppStore;
use crate::json_file::JsonFile;

const RECENT: &str = "recent.json";
const MAX_ENTRIES: usize = 20;

/// One row of Recent libraries on the Welcome screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentLibrary {
    pub path: String,
    pub name: String,
    pub library_id: Option<String>,
    /// ms
    pub last_opened_at: i64,
    /// The folder or its `library.db` is gone (for example a USB drive that is not plugged in).
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RecentFile {
    last_library: Option<String>,
    closed_explicitly: bool,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    path: String,
    name: String,
    library_id: Option<String>,
    last_opened_at: i64,
}

impl AppStore {
    /// Newest first. Entries are keyed by the canonical path, because a copied library keeps
    /// its library id.
    pub fn recent_libraries(&self) -> Vec<RecentLibrary> {
        let file: RecentFile = JsonFile::new(self.dir(), RECENT).load();
        file.entries
            .into_iter()
            .map(|entry| RecentLibrary {
                missing: !Path::new(&entry.path).join("library.db").is_file(),
                path: entry.path,
                name: entry.name,
                library_id: entry.library_id,
                last_opened_at: entry.last_opened_at,
            })
            .collect()
    }

    /// Moves the library to the top and makes it the one to reopen on start.
    pub fn remember_opened(
        &self,
        path: &str,
        name: &str,
        library_id: Option<&str>,
        now: i64,
    ) -> Result<(), SourceError> {
        JsonFile::new(self.dir(), RECENT).update(|file: &mut RecentFile| {
            file.entries.retain(|entry| entry.path != path);
            file.entries.insert(
                0,
                Entry {
                    path: path.to_owned(),
                    name: name.to_owned(),
                    library_id: library_id.map(str::to_owned),
                    last_opened_at: now,
                },
            );
            file.entries.truncate(MAX_ENTRIES);
            file.last_library = Some(path.to_owned());
            file.closed_explicitly = false;
        })
    }

    pub fn forget_recent(&self, path: &str) -> Result<(), SourceError> {
        JsonFile::new(self.dir(), RECENT).update(|file: &mut RecentFile| {
            file.entries.retain(|entry| entry.path != path);
            if file.last_library.as_deref() == Some(path) {
                file.last_library = None;
            }
        })?;
        // The last place for that path is useless once it leaves Recent.
        self.forget_library_session(path)
    }

    /// Close Library sets it; opening a library clears it. Quitting is not an explicit close.
    pub fn set_closed_explicitly(&self, closed: bool) -> Result<(), SourceError> {
        JsonFile::new(self.dir(), RECENT).update(|file: &mut RecentFile| {
            file.closed_explicitly = closed;
        })
    }

    /// The library to reopen on start, unless it was closed explicitly.
    pub fn library_to_reopen(&self) -> Option<String> {
        let file: RecentFile = JsonFile::new(self.dir(), RECENT).load();
        if file.closed_explicitly {
            None
        } else {
            file.last_library
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn library(dir: &Path, name: &str) -> String {
        let path = dir.join(format!("{name}.eglib"));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("library.db"), b"").unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn two_processes_keep_both_entries() {
        let data = tempfile::tempdir().unwrap();
        let libs = tempfile::tempdir().unwrap();
        let a = library(libs.path(), "A");
        let b = library(libs.path(), "B");
        AppStore::new(data.path())
            .remember_opened(&a, "A", Some("1"), 1)
            .unwrap();
        AppStore::new(data.path())
            .remember_opened(&b, "B", Some("2"), 2)
            .unwrap();
        let names: Vec<_> = AppStore::new(data.path())
            .recent_libraries()
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(names, ["B", "A"]);
    }

    #[test]
    fn reopening_moves_to_the_top_and_the_list_is_capped() {
        let data = tempfile::tempdir().unwrap();
        let store = AppStore::new(data.path());
        for n in 0..25 {
            store
                .remember_opened(&format!("/libs/{n}.eglib"), &n.to_string(), None, n)
                .unwrap();
        }
        store
            .remember_opened("/libs/10.eglib", "10", None, 99)
            .unwrap();
        let recent = store.recent_libraries();
        assert_eq!(recent.len(), 20);
        assert_eq!(recent[0].name, "10");
        assert_eq!(recent.iter().filter(|entry| entry.name == "10").count(), 1);
        assert!(
            !recent.iter().any(|entry| entry.name == "4"),
            "the oldest are dropped"
        );
    }

    #[test]
    fn a_library_that_is_gone_is_listed_as_missing() {
        let data = tempfile::tempdir().unwrap();
        let libs = tempfile::tempdir().unwrap();
        let here = library(libs.path(), "Here");
        let store = AppStore::new(data.path());
        store
            .remember_opened("/nowhere/Gone.eglib", "Gone", None, 1)
            .unwrap();
        store.remember_opened(&here, "Here", None, 2).unwrap();
        let recent = store.recent_libraries();
        assert_eq!(recent.len(), 2);
        assert!(!recent[0].missing);
        assert!(recent[1].missing);
    }

    #[test]
    fn close_library_stops_the_reopen_and_opening_resumes_it() {
        let data = tempfile::tempdir().unwrap();
        let store = AppStore::new(data.path());
        assert_eq!(store.library_to_reopen(), None);
        store
            .remember_opened("/libs/A.eglib", "A", None, 1)
            .unwrap();
        assert_eq!(store.library_to_reopen().as_deref(), Some("/libs/A.eglib"));
        store.set_closed_explicitly(true).unwrap();
        assert_eq!(store.library_to_reopen(), None);
        store
            .remember_opened("/libs/A.eglib", "A", None, 2)
            .unwrap();
        assert_eq!(store.library_to_reopen().as_deref(), Some("/libs/A.eglib"));
        store.forget_recent("/libs/A.eglib").unwrap();
        assert_eq!(store.library_to_reopen(), None);
        assert!(store.recent_libraries().is_empty());
    }
}
