//! Session bootstrap and browse helpers for the GPUI host (D-126 / D-129).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use app_store::{
    AppStore, BrowsePreferences, LibrarySession, SavedFilters, SavedPlace, SyncEnvironment, Theme,
};
use desktop_session::{Emit, Event, Session};
use library_core::{
    Folder, ItemCell, ItemId, LibraryCounts, LibraryInfo, LibraryQuery, Scope, SmartFolder,
    SortKey, TagMode, Thumbnail,
};

use crate::platform;
use crate::view_window::{PAGE, ViewCells};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_COMMIT: Option<&str> = option_env!("EGL_COMMIT");

/// Shared mailbox for Session → GPUI Emit events (views-invalidated, later import/export).
pub type EventMailbox = Arc<Mutex<Vec<Event>>>;

pub fn fixture_library_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/library/v5/Fixture.eglib")
}

pub fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn new_event_mailbox() -> EventMailbox {
    Arc::new(Mutex::new(Vec::new()))
}

/// Builds a Session with a stable AppStore under the GPUI host data dir (settings persist).
pub fn session_with_emit(emit: Emit) -> Session {
    session_at(platform::app_data_dir(), emit)
}

fn session_at(root: PathBuf, emit: Emit) -> Session {
    let data = root.join("data");
    let home = root.join("home");
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::create_dir_all(&home);
    Session::new(
        AppStore::new(data),
        home,
        APP_VERSION,
        SyncEnvironment::default(),
        emit,
    )
}

/// Session that pushes events into `mailbox` (Emit → Entity via RootApp poll).
pub fn session_with_mailbox(mailbox: EventMailbox) -> Session {
    let emit: Emit = Arc::new(move |event: Event| {
        if let Ok(mut queue) = mailbox.lock() {
            queue.push(event);
        }
    });
    session_with_emit(emit)
}

/// Builds a Session with a noop emit for tests.
///
/// Uses a disposable AppStore under the process temp dir so `open_library` never writes
/// Spike/Fixture paths into the user's persistent recent list (`%LOCALAPPDATA%\egl-viewer-gpui`).
#[cfg(test)]
pub fn session() -> Session {
    let root = std::env::temp_dir().join(format!(
        "egl-gpui-test-store-{}-{}",
        std::process::id(),
        now_stamp()
    ));
    session_at(root, Arc::new(|_event: Event| {}))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    All,
    Uncategorized,
    Untagged,
    Trash,
    Folder { id: String },
    Smart { id: String },
}

impl Place {
    pub fn from_saved(place: SavedPlace) -> Self {
        match place {
            SavedPlace::All => Self::All,
            SavedPlace::Uncategorized => Self::Uncategorized,
            SavedPlace::Untagged => Self::Untagged,
            SavedPlace::Trash => Self::Trash,
            SavedPlace::Folder { id } => Self::Folder { id },
            SavedPlace::Smart { id } => Self::Smart { id },
        }
    }

    pub fn to_saved(&self) -> SavedPlace {
        match self {
            Self::All => SavedPlace::All,
            Self::Uncategorized => SavedPlace::Uncategorized,
            Self::Untagged => SavedPlace::Untagged,
            Self::Trash => SavedPlace::Trash,
            Self::Folder { id } => SavedPlace::Folder { id: id.clone() },
            Self::Smart { id } => SavedPlace::Smart { id: id.clone() },
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
            Self::Smart { id } => Scope::Smart { id: id.clone() },
        }
    }

    pub fn open_folder_id(&self) -> Option<&str> {
        match self {
            Self::Folder { id } => Some(id.as_str()),
            _ => None,
        }
    }
}

/// Active filter chips (Phase 3.C) — mirrors `SavedFilters` plus search text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActiveFilters {
    pub tags: Vec<String>,
    pub tag_mode: TagMode,
    pub folders: Vec<String>,
    pub folder_mode: TagMode,
    pub ext: Option<String>,
    pub rating: Option<u8>,
    pub shape: Option<String>,
    /// The colour filter (D-176).
    pub color: Option<library_core::ColorFilter>,
    pub search_text: String,
}

impl ActiveFilters {
    pub fn from_saved(filters: &SavedFilters, search_text: String) -> Self {
        Self {
            tags: filters.tags.clone(),
            tag_mode: filters.tag_mode,
            folders: filters.folders.clone(),
            folder_mode: filters.folder_mode,
            ext: filters.ext.clone(),
            rating: filters.rating,
            shape: filters.shape.clone(),
            color: filters.color,
            search_text,
        }
    }

    pub fn to_saved(&self) -> SavedFilters {
        SavedFilters {
            tags: self.tags.clone(),
            tag_mode: self.tag_mode,
            folders: self.folders.clone(),
            folder_mode: self.folder_mode,
            ext: self.ext.clone(),
            rating: self.rating,
            shape: self.shape.clone(),
            color: self.color,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
            && self.folders.is_empty()
            && self.ext.is_none()
            && self.rating.is_none()
            && self.shape.is_none()
            && self.color.is_none()
            && self.search_text.trim().is_empty()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

pub fn query_for(
    place: &Place,
    browse: &BrowsePreferences,
    sort: SortKey,
    ascending: bool,
    filters: &ActiveFilters,
) -> LibraryQuery {
    LibraryQuery {
        scope: place.to_scope(browse.include_subfolders),
        tag_mode: filters.tag_mode,
        tags: filters.tags.clone(),
        extra_folder_ids: filters.folders.clone(),
        folder_mode: filters.folder_mode,
        ext: filters.ext.clone(),
        rating: filters.rating,
        shape: filters.shape.clone(),
        color: filters.color,
        text: {
            let trimmed = filters.search_text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        },
        sort,
        ascending,
    }
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

/// The selected items of the view. Select All is a flag (D-167): with `all` set, `picked` holds
/// the items taken out again, so selecting a view of any size stores nothing per item, and Rust
/// resolves the ids when a command needs them (`ItemsTarget { all: true, .. }`).
#[derive(Debug, Clone, Default)]
pub struct Selection {
    picked: HashSet<String>,
    all: bool,
    /// The view's total while `all` is set, for the count.
    total: usize,
    pub anchor: Option<usize>,
    /// Keyboard / rubber-band focus: a position in the view.
    pub focus: Option<usize>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.picked.clear();
        self.all = false;
        self.anchor = None;
        self.focus = None;
    }

    pub fn select_only(&mut self, id: &str, index: usize) {
        self.picked.clear();
        self.all = false;
        self.picked.insert(id.to_string());
        self.anchor = Some(index);
        self.focus = Some(index);
    }

    pub fn toggle(&mut self, id: &str, index: usize) {
        if !self.picked.remove(id) {
            self.picked.insert(id.to_string());
        }
        self.anchor = Some(index);
        self.focus = Some(index);
    }

    /// Shift-click: the run from the anchor to `index`. The caller loads that run first.
    pub fn extend_to(&mut self, cells: &ViewCells, index: usize) {
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
        self.picked.clear();
        self.all = false;
        for i in lo..=hi {
            if let Some(cell) = cells.get(i) {
                self.picked.insert(cell.id.0.clone());
            }
        }
        self.focus = Some(index);
    }

    pub fn select_indices(&mut self, cells: &ViewCells, indices: &[usize], focus: usize) {
        self.picked.clear();
        self.all = false;
        for &index in indices {
            if let Some(cell) = cells.get(index) {
                self.picked.insert(cell.id.0.clone());
            }
        }
        self.anchor = Some(focus);
        self.focus = Some(focus);
    }

    /// Every item of a view of `total`, without listing them.
    pub fn select_all(&mut self, total: usize) {
        self.picked.clear();
        self.all = total > 0;
        self.total = total;
        self.anchor = Some(0);
        self.focus = (total > 0).then_some(0);
    }

    /// The view changed size under a Select All.
    pub fn set_total(&mut self, total: usize) {
        self.total = total;
    }

    pub fn count(&self) -> usize {
        if self.all {
            self.total.saturating_sub(self.picked.len())
        } else {
            self.picked.len()
        }
    }

    pub fn contains(&self, id: &ItemId) -> bool {
        self.all != self.picked.contains(&id.0)
    }

    /// The selected ids when they are listed (no Select All); `None` under Select All.
    pub fn listed_ids(&self) -> Option<Vec<ItemId>> {
        (!self.all).then(|| self.picked.iter().cloned().map(ItemId).collect())
    }

    /// The selection as a command target on `view_id`; `None` when nothing is selected.
    pub fn target(&self, view_id: u64) -> Option<desktop_session::ItemsTarget> {
        if self.count() == 0 {
            return None;
        }
        Some(desktop_session::ItemsTarget {
            view_id: Some(view_id),
            all: self.all,
            ids: self.picked.iter().cloned().map(ItemId).collect(),
        })
    }

    pub fn to_saved(&self) -> app_store::SavedSelection {
        app_store::SavedSelection {
            all: self.all,
            ids: self.picked.iter().cloned().collect(),
        }
    }

    pub fn from_saved(saved: &app_store::SavedSelection) -> Self {
        Self {
            picked: saved.ids.iter().cloned().collect(),
            all: saved.all,
            total: 0,
            anchor: None,
            focus: None,
        }
    }
}

pub struct OpenLibraryState {
    pub info: LibraryInfo,
    pub place: Place,
    pub view_id: u64,
    pub total: u32,
    /// The view's positions, a page at a time (D-167).
    pub cells: ViewCells,
    /// The positions the grid drew last, so a refresh reloads what is on screen.
    pub visible: std::ops::Range<usize>,
    pub dimensions: Vec<[u32; 2]>,
    pub counts: LibraryCounts,
    pub folders: Vec<Folder>,
    pub smart_folders: Vec<SmartFolder>,
    pub expanded: HashSet<String>,
    pub selection: Selection,
    pub sort: SortKey,
    pub ascending: bool,
    pub browse: BrowsePreferences,
    pub filters: ActiveFilters,
    pub left_open: bool,
    pub right_open: bool,
    pub left_width: u32,
    pub right_width: u32,
    pub path: String,
}

impl OpenLibraryState {
    pub fn load(session: &Session, info: LibraryInfo) -> Result<Self, library_core::SourceError> {
        let path = info.path.clone().unwrap_or_else(|| {
            session
                .library_folder()
                .map(|p| p.to_string_lossy().into())
                .unwrap_or_default()
        });
        let prefs = session.preferences();
        let saved = session.library_session(&path).unwrap_or_default();
        let place = Place::from_saved(saved.place);
        let browse = prefs.browse;
        let sort = saved.screen_sort.unwrap_or(browse.sort);
        let ascending = saved.screen_ascending.unwrap_or(browse.ascending);
        let filters = ActiveFilters::from_saved(&saved.filters, saved.search_text);
        let query = query_for(&place, &browse, sort, ascending, &filters);
        let opened = session.open_view(query, &|_, _| {})?;
        let mut cells = ViewCells::new(opened.total as usize);
        cells.fill(0..PAGE, |start, len| {
            session.view_range(opened.view_id, start as u32, len as u32)
        })?;
        let dimensions = session.view_dimensions(opened.view_id).unwrap_or_default();
        let counts = session.counts().unwrap_or_default();
        let folders = session.folder_tree().unwrap_or_default();
        let smart_folders = session.smart_folders().unwrap_or_default();
        let mut selection = Selection::from_saved(&saved.selection);
        selection.set_total(opened.total as usize);
        Ok(Self {
            info,
            place,
            view_id: opened.view_id,
            total: opened.total,
            cells,
            visible: 0..0,
            dimensions,
            counts,
            folders,
            smart_folders,
            expanded: HashSet::new(),
            selection,
            sort,
            ascending,
            browse,
            filters,
            left_open: prefs.layout.left_open,
            right_open: prefs.layout.right_open,
            left_width: prefs.layout.left_width,
            right_width: prefs.layout.right_width,
            path,
        })
    }

    pub fn refresh_view(&mut self, session: &Session) -> Result<(), library_core::SourceError> {
        session.close_view(self.view_id);
        let query = query_for(
            &self.place,
            &self.browse,
            self.sort,
            self.ascending,
            &self.filters,
        );
        let opened = session.open_view(query, &|_, _| {})?;
        self.view_id = opened.view_id;
        self.total = opened.total;
        self.cells = ViewCells::new(opened.total as usize);
        self.selection.set_total(opened.total as usize);
        // The first page, and the one on screen when the view kept its scroll.
        self.fill_cells(session, 0..PAGE)?;
        self.fill_cells(session, self.visible.clone())?;
        self.dimensions = session.view_dimensions(opened.view_id).unwrap_or_default();
        self.counts = session.counts().unwrap_or_default();
        self.folders = session.folder_tree().unwrap_or_default();
        self.smart_folders = session.smart_folders().unwrap_or_default();
        Ok(())
    }

    /// The same view after a write changed its contents: same id, a new total; the pages on
    /// screen load again at once, the rest as they come into view.
    pub fn reload_cells(
        &mut self,
        session: &Session,
        total: usize,
    ) -> Result<(), library_core::SourceError> {
        self.total = total as u32;
        self.cells = ViewCells::new(total);
        self.selection.set_total(total);
        self.fill_cells(session, 0..PAGE)?;
        self.fill_cells(session, self.visible.clone())?;
        self.dimensions = session.view_dimensions(self.view_id).unwrap_or_default();
        Ok(())
    }

    /// Loads the cells of `range` that are not loaded yet (cells only: no thumbnails). For
    /// what needs ids beyond the screen: a shift-click run, a rubber band, the keyboard.
    pub fn fill_cells(
        &mut self,
        session: &Session,
        range: std::ops::Range<usize>,
    ) -> Result<(), library_core::SourceError> {
        let view_id = self.view_id;
        self.cells.fill(range, |start, len| {
            session.view_range(view_id, start as u32, len as u32)
        })
    }

    pub fn persist_session(&self, session: &Session) {
        let saved = LibrarySession {
            place: self.place.to_saved(),
            filters: self.filters.to_saved(),
            search_text: self.filters.search_text.clone(),
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

/// Display name for a folder id in the tree.
pub fn folder_name(folders: &[Folder], id: &str) -> Option<String> {
    for folder in folders {
        if folder.id == id {
            return Some(folder.name.clone());
        }
        if let Some(name) = folder_name(&folder.children, id) {
            return Some(name);
        }
    }
    None
}

/// Locked folders in tree order (D-059 Locked folders section).
pub fn locked_folders(folders: &[Folder]) -> Vec<Folder> {
    let mut out = Vec::new();
    collect_locked(folders, &mut out);
    out
}

fn collect_locked(folders: &[Folder], out: &mut Vec<Folder>) {
    for folder in folders {
        if folder.is_locked {
            out.push(folder.clone());
        }
        collect_locked(&folder.children, out);
    }
}

/// Folders tree with locked folders removed from the regular tree (they live in Locked).
pub fn unlocked_tree(folders: &[Folder]) -> Vec<Folder> {
    folders
        .iter()
        .filter(|f| !f.is_locked)
        .map(|f| Folder {
            children: unlocked_tree(&f.children),
            ..f.clone()
        })
        .collect()
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
    fn test_session_does_not_pollute_persistent_recent() {
        let persistent = platform::app_data_dir().join("data");
        let recent_path = persistent.join("recent.json");
        let before = std::fs::read(&recent_path).unwrap_or_default();

        let dir = tempdir().unwrap();
        let root = create_library(dir.path(), "Spike", APP_VERSION).unwrap();
        let opened = root.to_string_lossy().into_owned();
        let session = session();
        let info = session.open_library(&opened).unwrap();
        // `LibraryInfo.path` is the absolute form the store remembers; on Windows it can
        // differ from the path string we passed in (prefix / separators).
        let remembered = info
            .path
            .clone()
            .expect("native open must set LibraryInfo.path");
        assert!(
            session
                .recent_libraries()
                .iter()
                .any(|entry| entry.path == remembered),
            "ephemeral store should still remember within the test session"
        );
        session.close_library().unwrap();
        drop(session);

        let after = std::fs::read(&recent_path).unwrap_or_default();
        assert_eq!(
            before, after,
            "opening a temp Spike must not rewrite %LOCALAPPDATA%\\egl-viewer-gpui recent.json"
        );
        let after_text = String::from_utf8_lossy(&after);
        assert!(
            !after_text.contains(&opened) && !after_text.contains(&remembered),
            "persistent recent must not gain this test's path: {after_text}"
        );
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
        let folder = Place::Folder { id: "F1".into() };
        assert_eq!(Place::from_saved(folder.to_saved()), folder);
        let smart = Place::Smart { id: "S1".into() };
        assert_eq!(Place::from_saved(smart.to_saved()), smart);
    }

    #[test]
    fn query_for_includes_search_and_rating_filter() {
        let filters = ActiveFilters {
            search_text: "poster".into(),
            rating: Some(3),
            ..ActiveFilters::default()
        };
        let query = query_for(
            &Place::All,
            &BrowsePreferences::default(),
            SortKey::DateAdded,
            false,
            &filters,
        );
        assert_eq!(query.text.as_deref(), Some("poster"));
        assert_eq!(query.rating, Some(3));
    }

    #[test]
    fn apply_item_edit_rename_on_fixture() {
        use library_core::ItemEdit;

        let source = fixture_library_path();
        let dir = tempdir().unwrap();
        let root = dir.path().join("Fixture.eglib");
        copy_dir(&source, &root).unwrap();
        let session = session();
        let info = session.open_library(&root.to_string_lossy()).unwrap();
        let state = OpenLibraryState::load(&session, info).unwrap();
        let id = state.cells.get(0).unwrap().id.0.clone();
        let renamed = session
            .apply_item_edit(&id, ItemEdit::Rename("Phase3Rename".into()))
            .unwrap();
        assert_eq!(renamed.name, "Phase3Rename");
        let undone = session.undo().unwrap();
        assert!(undone.is_some());
        session.close_library().unwrap();
    }

    #[test]
    fn selection_ctrl_and_shift() {
        let mut cells = ViewCells::new(5);
        cells.insert_page(
            0,
            (0..5)
                .map(|i| ItemCell {
                    id: ItemId(format!("id{i}")),
                    name: format!("n{i}"),
                    ..Default::default()
                })
                .collect(),
        );
        let mut sel = Selection::default();
        sel.select_only("id0", 0);
        sel.toggle("id2", 2);
        assert_eq!(sel.count(), 2);
        sel.extend_to(&cells, 4);
        for id in ["id2", "id3", "id4"] {
            assert!(sel.contains(&ItemId(id.into())), "{id}");
        }
        assert!(!sel.contains(&ItemId("id0".into())));
        assert_eq!(sel.count(), 3);
    }

    #[test]
    fn select_all_is_a_flag_with_exceptions_and_targets_the_view() {
        let mut sel = Selection::default();
        sel.select_all(13_228);
        assert_eq!(sel.count(), 13_228);
        assert!(sel.contains(&ItemId("anything".into())));
        assert_eq!(sel.listed_ids(), None);
        // Ctrl-click takes one out again.
        sel.toggle("gone", 7);
        assert_eq!(sel.count(), 13_227);
        assert!(!sel.contains(&ItemId("gone".into())));
        let target = sel.target(42).unwrap();
        assert!(target.all);
        assert_eq!(target.view_id, Some(42));
        assert_eq!(target.ids, [ItemId("gone".into())]);
        let saved = sel.to_saved();
        assert!(saved.all);
        // A write shrank the view.
        sel.set_total(100);
        assert_eq!(sel.count(), 99);
        // A plain click ends it.
        sel.select_only("one", 3);
        assert_eq!(sel.count(), 1);
        assert_eq!(sel.listed_ids(), Some(vec![ItemId("one".into())]));
        assert!(!sel.target(42).unwrap().all);
        sel.select_all(0);
        assert_eq!(sel.count(), 0);
        assert!(sel.target(42).is_none());
    }

    #[test]
    fn fixture_item_detail_and_preview_payload() {
        use crate::browse::preview_image;

        let source = fixture_library_path();
        let dir = tempdir().unwrap();
        let root = dir.path().join("Fixture.eglib");
        copy_dir(&source, &root).unwrap();
        let session = session();
        let info = session.open_library(&root.to_string_lossy()).unwrap();
        let state = OpenLibraryState::load(&session, info).unwrap();
        assert!(!state.cells.is_empty());
        let cell = state.cells.get(0).unwrap();
        let detail = session.item_detail(&cell.id.0).unwrap().unwrap();
        assert_eq!(detail.id, cell.id);
        let payload = preview_image::preview_payload(&session, cell);
        // Fixture items should yield either an original or a thumb payload.
        assert!(
            payload.is_some() || !cell.has_thumbnail,
            "expected preview bytes for fixture cell {}",
            cell.id.0
        );
        let paths = session
            .original_paths(std::slice::from_ref(&cell.id))
            .unwrap();
        assert_eq!(paths.len(), 1);
        session.close_library().unwrap();
    }
}
