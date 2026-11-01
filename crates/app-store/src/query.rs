//! The last place, filters, search and selection per library (docs/spec.md §9 phase 12, D-078).
//!
//! Browse preferences (sort, thumbnail size, grid/list) stay in `preferences.json` (D-041).
//! This file remembers where the user was inside one library so a restart reopens there.

use library_core::{SortKey, SourceError, TagMode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AppStore;
use crate::json_file::JsonFile;

const QUERIES: &str = "queries.json";
/// Cap how many selected ids we keep: Select All stores a flag, not every id.
const MAX_SELECTED_IDS: usize = 500;
const MAX_ENTRIES: usize = 40;

/// Where the library screen was: a fixed scope row, a folder or a smart folder.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SavedPlace {
    #[default]
    All,
    Uncategorized,
    Untagged,
    Trash,
    Folder {
        id: String,
    },
    Smart {
        id: String,
    },
}

/// Filters on top of the place (docs/spec.md §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SavedFilters {
    pub tags: Vec<String>,
    pub tag_mode: TagMode,
    pub folders: Vec<String>,
    pub folder_mode: TagMode,
    pub ext: Option<String>,
    pub rating: Option<u8>,
    pub shape: Option<String>,
}

impl Default for SavedFilters {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            tag_mode: TagMode::Any,
            folders: Vec::new(),
            folder_mode: TagMode::Any,
            ext: None,
            rating: None,
            shape: None,
        }
    }
}

/// The selection of the open view: ids, or "everything except these" (D-039).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SavedSelection {
    /// `true`: every item of the view except `ids`.
    pub all: bool,
    pub ids: Vec<String>,
}

/// One library's last screen: place, filters, search, selection, optional per-screen sort.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct LibrarySession {
    pub place: SavedPlace,
    pub filters: SavedFilters,
    pub search_text: String,
    pub selection: SavedSelection,
    /// A sort picked on a folder that has its own order; otherwise the global browse sort.
    pub screen_sort: Option<SortKey>,
    pub screen_ascending: Option<bool>,
}

impl Default for LibrarySession {
    fn default() -> Self {
        Self {
            place: SavedPlace::All,
            filters: SavedFilters::default(),
            search_text: String::new(),
            selection: SavedSelection::default(),
            screen_sort: None,
            screen_ascending: None,
        }
    }
}

impl LibrarySession {
    fn clamped(mut self) -> Self {
        if self.selection.ids.len() > MAX_SELECTED_IDS {
            self.selection.ids.truncate(MAX_SELECTED_IDS);
        }
        self.search_text = self.search_text.chars().take(500).collect();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct QueriesFile {
    /// Newest first; keyed by the canonical library path (same as recent libraries).
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    path: String,
    session: LibrarySession,
}

impl AppStore {
    /// The last place and selection for `path`, when one was saved.
    pub fn library_session(&self, path: &str) -> Option<LibrarySession> {
        let file: QueriesFile = JsonFile::new(self.dir(), QUERIES).load();
        file.entries
            .into_iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.session)
    }

    /// Remembers the screen for `path` so the next open restores it.
    pub fn save_library_session(
        &self,
        path: &str,
        session: LibrarySession,
    ) -> Result<(), SourceError> {
        let session = session.clamped();
        JsonFile::new(self.dir(), QUERIES).update(|file: &mut QueriesFile| {
            file.entries.retain(|entry| entry.path != path);
            file.entries.insert(
                0,
                Entry {
                    path: path.to_owned(),
                    session,
                },
            );
            file.entries.truncate(MAX_ENTRIES);
        })
    }

    /// Drops a library's saved screen when it leaves Recent.
    pub fn forget_library_session(&self, path: &str) -> Result<(), SourceError> {
        JsonFile::new(self.dir(), QUERIES).update(|file: &mut QueriesFile| {
            file.entries.retain(|entry| entry.path != path);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_means_no_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        assert_eq!(store.library_session("/libs/A.eglib"), None);
    }

    #[test]
    fn session_round_trips_per_library() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        let session = LibrarySession {
            place: SavedPlace::Folder { id: "F1".into() },
            filters: SavedFilters {
                tags: vec!["Poster".into()],
                tag_mode: TagMode::All,
                ..SavedFilters::default()
            },
            search_text: "sky".into(),
            selection: SavedSelection {
                all: false,
                ids: vec!["a".into(), "b".into()],
            },
            screen_sort: Some(SortKey::Title),
            screen_ascending: Some(true),
        };
        store
            .save_library_session("/libs/A.eglib", session.clone())
            .unwrap();
        store
            .save_library_session(
                "/libs/B.eglib",
                LibrarySession {
                    place: SavedPlace::Trash,
                    ..LibrarySession::default()
                },
            )
            .unwrap();
        assert_eq!(store.library_session("/libs/A.eglib"), Some(session));
        assert_eq!(
            store.library_session("/libs/B.eglib").map(|s| s.place),
            Some(SavedPlace::Trash)
        );
    }

    #[test]
    fn selected_ids_are_capped() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        let ids: Vec<_> = (0..600).map(|n| format!("id-{n}")).collect();
        store
            .save_library_session(
                "/libs/A.eglib",
                LibrarySession {
                    selection: SavedSelection {
                        all: false,
                        ids: ids.clone(),
                    },
                    ..LibrarySession::default()
                },
            )
            .unwrap();
        let saved = store.library_session("/libs/A.eglib").unwrap();
        assert_eq!(saved.selection.ids.len(), MAX_SELECTED_IDS);
        assert_eq!(saved.selection.ids[0], "id-0");
    }

    #[test]
    fn forget_drops_only_that_library() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        store
            .save_library_session("/libs/A.eglib", LibrarySession::default())
            .unwrap();
        store
            .save_library_session("/libs/B.eglib", LibrarySession::default())
            .unwrap();
        store.forget_library_session("/libs/A.eglib").unwrap();
        assert_eq!(store.library_session("/libs/A.eglib"), None);
        assert!(store.library_session("/libs/B.eglib").is_some());
    }

    #[test]
    fn entries_are_capped_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::new(dir.path());
        for n in 0..45 {
            store
                .save_library_session(
                    &format!("/libs/{n}.eglib"),
                    LibrarySession {
                        place: SavedPlace::Untagged,
                        ..LibrarySession::default()
                    },
                )
                .unwrap();
        }
        assert_eq!(store.library_session("/libs/0.eglib"), None);
        assert_eq!(
            store.library_session("/libs/44.eglib").map(|s| s.place),
            Some(SavedPlace::Untagged)
        );
    }
}
