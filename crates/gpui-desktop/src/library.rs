//! Thin helpers around [`app_service::LibraryService`] for the GPUI spike.
//! Opens a native `.eglib` and serves a window of the All-items view with no IPC.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use app_service::{Emit, Event, LibraryService, ViewOpened};
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

/// Builds a service that drops events on the floor (no WebView to notify).
pub fn service() -> LibraryService {
    let emit: Emit = Arc::new(|_event: Event| {});
    LibraryService::new(APP_VERSION, emit)
}

/// Opens `path`, then an All / newest-first view, and returns the first window of cells.
pub fn open_and_list(
    service: &LibraryService,
    path: &Path,
    sync_folder: bool,
    start: u32,
    len: u32,
) -> Result<(LibraryInfo, ViewOpened, Vec<ItemCell>), library_core::SourceError> {
    let info = service.open_library(path, sync_folder)?;
    let opened = service.open_view(LibraryQuery::default(), &|_, _| {})?;
    let cells = service.view_range(opened.view_id, start, len)?;
    Ok((info, opened, cells))
}

/// Thumbnail bytes for a grid cell, when the library has one.
pub fn cell_thumbnail(service: &LibraryService, cell: &ItemCell) -> Option<Thumbnail> {
    if !cell.has_thumbnail {
        return None;
    }
    service.image(&cell.id.0)
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
        let service = service();
        let (info, opened, cells) = open_and_list(&service, &root, false, 0, 50).unwrap();
        assert_eq!(info.name, "Spike");
        assert_eq!(opened.total, 0);
        assert!(cells.is_empty());
        service.close_library().unwrap();
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
        let service = service();
        let (info, opened, cells) = open_and_list(&service, &root, false, 0, 50).unwrap();
        assert_eq!(info.name, "Fixture");
        assert!(opened.total >= 1, "fixture should contain items");
        assert_eq!(cells.len(), opened.total as usize);
        assert!(cells.iter().all(|cell| !cell.name.is_empty()));
        service.close_library().unwrap();
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
