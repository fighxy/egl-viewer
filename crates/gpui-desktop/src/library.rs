//! Thin helpers around [`desktop_session::Session`] for the GPUI spike.
//! Opens a native `.eglib` and serves a window of the All-items view with no IPC.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use app_store::{AppStore, SyncEnvironment};
use desktop_session::{Emit, Event, Session, ViewOpened};
use library_core::{ItemCell, LibraryInfo, LibraryQuery, Thumbnail};

/// App version string the native library records in meta (matches the workspace package).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Fixture shipped in the repo for smoke opens (format v5).
pub fn fixture_library_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/library/v5/Fixture.eglib")
}

/// A coarse stamp for unique temp fixture folders.
pub fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Builds a [`Session`] that drops events on the floor (no UI host to notify).
///
/// App-store data and home live under a unique temp tree for the process so the spike
/// never touches the shipping Tauri local-data folder.
pub fn session() -> Session {
    let root = std::env::temp_dir().join(format!("egl-gpui-desktop-{}", std::process::id()));
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

/// Opens `path` through Session (location checks included), then an All / newest-first view.
pub fn open_and_list(
    session: &Session,
    path: &Path,
    start: u32,
    len: u32,
) -> Result<(LibraryInfo, ViewOpened, Vec<ItemCell>), library_core::SourceError> {
    let _ = session.close_library();
    let info = session.open_library(&path.to_string_lossy())?;
    let opened = session.open_view(LibraryQuery::default(), &|_, _| {})?;
    let cells = session.view_range(opened.view_id, start, len)?;
    Ok((info, opened, cells))
}

/// Thumbnail bytes for a grid cell, when the library has one.
pub fn cell_thumbnail(session: &Session, cell: &ItemCell) -> Option<Thumbnail> {
    if !cell.has_thumbnail {
        return None;
    }
    session.image(&format!("/{}", cell.id.0), Some(cell.thumb_version))
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
        let (info, opened, cells) = open_and_list(&session, &root, 0, 50).unwrap();
        assert_eq!(info.name, "Spike");
        assert_eq!(opened.total, 0);
        assert!(cells.is_empty());
        session.close_library().unwrap();
    }

    #[test]
    fn opens_the_v5_fixture_and_lists_its_items() {
        let source = fixture_library_path();
        assert!(
            source.is_dir(),
            "missing fixture library at {}",
            source.display()
        );
        // Copy so open can migrate forward; the checked-in fixture stays untouched.
        let dir = tempdir().unwrap();
        let root = dir.path().join("Fixture.eglib");
        copy_dir(&source, &root);
        let session = session();
        let (info, opened, cells) = open_and_list(&session, &root, 0, 50).unwrap();
        assert_eq!(info.name, "Fixture");
        assert!(opened.total >= 1, "fixture should contain items");
        assert_eq!(cells.len(), opened.total as usize);
        assert!(cells.iter().all(|cell| !cell.name.is_empty()));
        session.close_library().unwrap();
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
}
