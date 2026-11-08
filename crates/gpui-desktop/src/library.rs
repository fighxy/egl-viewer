//! Session bootstrap and browse helpers for the GPUI host (D-126 / D-129).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use app_store::{
    AppStore, BrowsePreferences, LibrarySession, SavedPlace, SyncEnvironment, Theme,
};
use desktop_session::{Emit, Event, Session, ViewOpened};
use library_core::{
    Folder, ItemCell, ItemId, LibraryCounts, LibraryInfo, LibraryQuery, Scope, SortKey, Thumbnail,
};

use crate::platform;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn fixture_library_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/library/v5/Fixture.eglib")
}

#[allow(dead_code)]
pub fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Builds a Session with a stable AppStore under the GPUI host data dir (settings persist).
pub fn session() -> Session {
    let root = platform::app_data_dir();
    let data = root.join("data");
    let home = root.join("home");
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::create_dir_all(&home);
    let emit: Emit = Arc::new(|_event: Event| {});
    Session::new(
        AppStore::new(data),
        home,
        APP_VERSION,
        SyncEnvironment::default(),
        emit,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    All,
    Uncategorized,
    Untagged,
    Trash,
    Folder { id: String },
}

impl Place {
    pub fn from_saved(place: SavedPlace) -> Self {
        match place {
            SavedPlace::All => Self::All,
            SavedPlace::Uncategorized => Self::Uncategorized,
            SavedPlace::Untagged => Self::Untagged,
            SavedPlace::Trash => Self::Trash,
            SavedPlace::Folder { id } => Self::Folder { id },
            SavedPlace::Smart { .. } => Self::All,
        }
    }

    pub fn to_saved(&self) -> SavedPlace {
        match self {
            Self::All => SavedPlace::All,
            Self::Uncategorized => SavedPlace::Uncategorized,
            Self::Untagged => SavedPlace::Untagged,
            Self::Trash => SavedPlace::Trash,
            Self::Folder { id } => SavedPlace::Folder { id: id.clone() },
        }
    }

    pub fn to_scope(&self, include_subfolders: bool) -> Scope {
        match self {
            Self::All => Scope::All,
            Self::Uncategorized => Scope::Unfiled,
            Self::Untagged => Scope::Untagged,
            Self::Trash => Scope::Trash,
            Self::Folder { id } => Scope::Folder {
                id: id.clone(),
                include_descendants: include_subfolders,
            },
        }
    }
}

pub fn query_for(place: &Place, browse: &BrowsePreferences, sort: SortKey, ascending: bool) -> LibraryQuery {
    LibraryQuery {
        scope: place.to_scope(browse.include_subfolders),
        sort,
        ascending,
        ..LibraryQuery::default()
    }
}

pub fn open_view_window(
    session: &Session,
    query: LibraryQuery,
    start: u32,
    len: u32,
) -> Result<(ViewOpened, Vec<ItemCell>), library_core::SourceError> {
    let opened = session.open_view(query, &|_, _| {})?;
    let cells = session.view_range(opened.view_id, start, len)?;
    Ok((opened, cells))
}

pub fn cell_thumbnail(session: &Session, cell: &ItemCell) -> Option<Thumbnail> {
    if !cell.has_thumbnail {
        return None;
    }
    session.image(&format!("/{}", cell.id.0), Some(cell.thumb_version))
}

#[allow(dead_code)]
pub fn copy_fixture_library() -> std::io::Result<PathBuf> {
    let source = fixture_library_path();
    let scratch = std::env::temp_dir().join(format!(
        "egl-gpui-fixture-{}-{}",
        std::process::id(),
        now_stamp()
    ));
    let root = scratch.join("Fixture.eglib");
    copy_dir(&source, &root)?;
    Ok(root)
}

pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub ids: HashSet<String>,
    pub anchor: Option<usize>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchor = None;
    }

    pub fn select_only(&mut self, id: &str, index: usize) {
        self.ids.clear();
        self.ids.insert(id.to_string());
        self.anchor = Some(index);
    }

    pub fn toggle(&mut self, id: &str, index: usize) {
        if !self.ids.remove(id) {
            self.ids.insert(id.to_string());
        }
        self.anchor = Some(index);
    }

    pub fn extend_to(&mut self, cells: &[ItemCell], index: usize) {
        let Some(anchor) = self.anchor else {
            if let Some(cell) = cells.get(index) {
                self.select_only(&cell.id.0, index);
            }
            return;
        };
        let (lo, hi) = if anchor <= index {
            (anchor, index)
        } else {
            (index, anchor)
        };
        self.ids.clear();
        for cell in cells.iter().take(hi + 1).skip(lo) {
            self.ids.insert(cell.id.0.clone());
        }
    }

    pub fn select_all(&mut self, cells: &[ItemCell]) {
        self.ids = cells.iter().map(|c| c.id.0.clone()).collect();
        self.anchor = Some(0);
    }

    pub fn count(&self) -> usize {
        self.ids.len()
    }

    pub fn contains(&self, id: &ItemId) -> bool {
        self.ids.contains(&id.0)
    }

    pub fn to_saved(&self) -> app_store::SavedSelection {
        app_store::SavedSelection {
            all: false,
            ids: self.ids.iter().cloned().collect(),
        }
    }

    pub fn from_saved(saved: &app_store::SavedSelection) -> Self {
        Self {
            ids: saved.ids.iter().cloned().collect(),
            anchor: None,
        }
    }
}

pub struct OpenLibraryState {
    pub info: LibraryInfo,
    pub place: Place,
    pub view_id: u64,
    pub total: u32,
    pub cells: Vec<ItemCell>,
    pub dimensions: Vec<[u32; 2]>,
    pub counts: LibraryCounts,
    pub folders: Vec<Folder>,
    pub expanded: HashSet<String>,
    pub selection: Selection,
    pub sort: SortKey,
    pub ascending: bool,
    pub browse: BrowsePreferences,
    pub left_open: bool,
    pub right_open: bool,
    pub left_width: u32,
    pub right_width: u32,
    pub path: String,
}

impl OpenLibraryState {
    pub fn load(session: &Session, info: LibraryInfo) -> Result<Self, library_core::SourceError> {
        let path = info
            .path
            .clone()
            .unwrap_or_else(|| session.library_folder().map(|p| p.to_string_lossy().into()).unwrap_or_default());
        let prefs = session.preferences();
        let saved = session
            .library_session(&path)
            .unwrap_or_default();
        let place = Place::from_saved(saved.place);
        let browse = prefs.browse;
        let sort = saved.screen_sort.unwrap_or(browse.sort);
        let ascending = saved.screen_ascending.unwrap_or(browse.ascending);
        let query = query_for(&place, &browse, sort, ascending);
        let (opened, cells) = open_view_window(session, query, 0, 500)?;
        let dimensions = session.view_dimensions(opened.view_id).unwrap_or_default();
        let counts = session.counts().unwrap_or_default();
        let folders = session.folder_tree().unwrap_or_default();
        let selection = Selection::from_saved(&saved.selection);
        Ok(Self {
            info,
            place,
            view_id: opened.view_id,
            total: opened.total,
            cells,
            dimensions,
            counts,
            folders,
            expanded: HashSet::new(),
            selection,
            sort,
            ascending,
            browse,
            left_open: prefs.layout.left_open,
            right_open: prefs.layout.right_open,
            left_width: prefs.layout.left_width,
            right_width: prefs.layout.right_width,
            path,
        })
    }

    pub fn refresh_view(&mut self, session: &Session) -> Result<(), library_core::SourceError> {
        session.close_view(self.view_id);
        let query = query_for(&self.place, &self.browse, self.sort, self.ascending);
        let (opened, cells) = open_view_window(session, query, 0, 500)?;
        self.view_id = opened.view_id;
        self.total = opened.total;
        self.cells = cells;
        self.dimensions = session.view_dimensions(opened.view_id).unwrap_or_default();
        self.counts = session.counts().unwrap_or_default();
        self.folders = session.folder_tree().unwrap_or_default();
        Ok(())
    }

    pub fn persist_session(&self, session: &Session) {
        let saved = LibrarySession {
            place: self.place.to_saved(),
            filters: Default::default(),
            search_text: String::new(),
            selection: self.selection.to_saved(),
            screen_sort: Some(self.sort),
            screen_ascending: Some(self.ascending),
        };
        let _ = session.save_library_session(&self.path, saved);
    }

    pub fn persist_browse_prefs(&self, session: &Session) {
        let mut prefs = session.preferences();
        prefs.browse = self.browse;
        prefs.layout.left_open = self.left_open;
        prefs.layout.right_open = self.right_open;
        prefs.layout.left_width = self.left_width;
        prefs.layout.right_width = self.right_width;
        let _ = session.save_preferences(prefs);
    }
}

pub fn window_title(info: Option<&LibraryInfo>) -> String {
    match info {
        Some(info) => format!("{} — egl-viewer", info.name),
        None => "egl-viewer".into(),
    }
}

#[allow(dead_code)]
pub fn next_theme(current: Theme) -> Theme {
    match current {
        Theme::System => Theme::Light,
        Theme::Light => Theme::Dark,
        Theme::Dark => Theme::System,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use library_native::create_library;
    use tempfile::tempdir;

    #[test]
    fn opens_a_fresh_library_and_lists_zero_items() {
        let dir = tempdir().unwrap();
        let root = create_library(dir.path(), "Spike", APP_VERSION).unwrap();
        let session = session();
        let info = session.open_library(&root.to_string_lossy()).unwrap();
        let state = OpenLibraryState::load(&session, info).unwrap();
        assert_eq!(state.info.name, "Spike");
        assert_eq!(state.total, 0);
        assert!(state.cells.is_empty());
        session.close_library().unwrap();
    }

    #[test]
    fn opens_the_v5_fixture_and_lists_its_items() {
        let source = fixture_library_path();
        assert!(source.is_dir(), "missing fixture at {}", source.display());
        let dir = tempdir().unwrap();
        let root = dir.path().join("Fixture.eglib");
        copy_dir(&source, &root).unwrap();
        let session = session();
        let info = session.open_library(&root.to_string_lossy()).unwrap();
        let state = OpenLibraryState::load(&session, info).unwrap();
        assert_eq!(state.info.name, "Fixture");
        assert!(state.total >= 1);
        session.close_library().unwrap();
    }

    #[test]
    fn place_round_trips_saved() {
        let folder = Place::Folder {
            id: "F1".into(),
        };
        assert_eq!(Place::from_saved(folder.to_saved()), folder);
    }

    #[test]
    fn selection_ctrl_and_shift() {
        let cells: Vec<ItemCell> = (0..5)
            .map(|i| ItemCell {
                id: ItemId(format!("id{i}")),
                name: format!("n{i}"),
                ..Default::default()
            })
            .collect();
        let mut sel = Selection::default();
        sel.select_only("id0", 0);
        sel.toggle("id2", 2);
        assert_eq!(sel.count(), 2);
        sel.extend_to(&cells, 4);
        assert!(sel.ids.contains("id2"));
        assert!(sel.ids.contains("id3"));
        assert!(sel.ids.contains("id4"));
    }
}
