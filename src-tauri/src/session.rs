//! The open library and the app's files, without Tauri: what the commands call, and what the
//! tests drive (docs/architecture.md §9, D-022, D-024).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use app_store::{
    AppStore, Preferences, RecentLibrary, SyncEnvironment, check_location, default_library_parent,
    display_path,
};
use library_core::{LibraryInfo, LocationProblem, ReadOnlyReason, SourceError};
use library_native::{Library, OpenOptions, create_library};
use serde::Serialize;
use ts_rs::TS;

/// What the app does on start: the reopened library, or why it shows Welcome instead.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StartupResult {
    pub library: Option<LibraryInfo>,
    /// Shown on Welcome when the last library was not reopened for a reason worth saying.
    pub notice: Option<String>,
}

pub struct Session {
    store: AppStore,
    home: PathBuf,
    app_version: String,
    sync: SyncEnvironment,
    library: Mutex<Option<Library>>,
}

impl Session {
    pub fn new(
        store: AppStore,
        home: PathBuf,
        app_version: impl Into<String>,
        sync: SyncEnvironment,
    ) -> Self {
        Self {
            store,
            home,
            app_version: app_version.into(),
            sync,
            library: Mutex::new(None),
        }
    }

    pub fn store(&self) -> &AppStore {
        &self.store
    }

    /// The preference, or `%USERPROFILE%\egl-library`.
    pub fn default_library_location(&self) -> String {
        self.store
            .preferences()
            .default_library_parent
            .unwrap_or_else(|| display_path(&default_library_parent(&self.home)))
    }

    pub fn create_library(&self, parent: &str, name: &str) -> Result<LibraryInfo, SourceError> {
        let parent = PathBuf::from(parent);
        check_location(&parent, &self.sync).map_err(SourceError::UnsupportedLocation)?;
        if !parent.exists() && display_path(&parent) == self.default_library_location() {
            std::fs::create_dir_all(&parent)
                .map_err(|error| SourceError::Storage(format!("create the folder: {error}")))?;
        }
        let root = create_library(&parent, name, &self.app_version)?;
        self.open_library(&root.to_string_lossy())
    }

    /// Opens a library folder. Another library that is open stays open until this one has
    /// opened (D-024). Opening the read-only library again retries its lock.
    pub fn open_library(&self, path: &str) -> Result<LibraryInfo, SourceError> {
        let path = PathBuf::from(path);
        let sync_folder = match check_location(&path, &self.sync) {
            Ok(()) => false,
            Err(LocationProblem::SyncFolder) => true,
            Err(problem) => return Err(SourceError::UnsupportedLocation(problem)),
        };
        let key = display_path(&path);
        let mut current = self.current_library();
        if let Some(open) = current.as_ref()
            && display_path(open.root()) == key
            && open.read_only_reason().is_none()
        {
            return Ok(open.info());
        }
        let options = OpenOptions {
            sync_folder,
            ..OpenOptions::new(self.app_version.clone())
        };
        let library = Library::open(Path::new(&key), &options)?;
        let info = library.info();
        // The previous library closes here, after the new one opened.
        *current = Some(library);
        drop(current);
        if let Err(error) =
            self.store
                .remember_opened(&key, &info.name, info.id.as_deref(), now_ms())
        {
            tracing::warn!("cannot update recent libraries: {error}");
        }
        Ok(info)
    }

    /// Close Library: back to Welcome, and no reopen on the next start.
    pub fn close_library(&self) -> Result<(), SourceError> {
        let library = self.current_library().take();
        if let Some(library) = library {
            library.close()?;
        }
        self.store.set_closed_explicitly(true)
    }

    pub fn current(&self) -> Option<LibraryInfo> {
        self.current_library().as_ref().map(Library::info)
    }

    pub fn recent_libraries(&self) -> Vec<RecentLibrary> {
        self.store.recent_libraries()
    }

    pub fn forget_recent_library(&self, path: &str) -> Result<(), SourceError> {
        self.store.forget_recent(path)
    }

    pub fn preferences(&self) -> Preferences {
        self.store.preferences()
    }

    /// Stores preferences after checking the default folder like a library location.
    pub fn save_preferences(&self, preferences: Preferences) -> Result<Preferences, SourceError> {
        let mut preferences = preferences;
        if let Some(folder) = preferences.default_library_parent.take() {
            let folder = folder.trim();
            if !folder.is_empty() {
                check_location(Path::new(folder), &self.sync)
                    .map_err(SourceError::UnsupportedLocation)?;
                preferences.default_library_parent = Some(display_path(Path::new(folder)));
            }
        }
        self.store.save_preferences(&preferences)?;
        Ok(self.store.preferences())
    }

    /// Reopens the last library unless it was closed explicitly, the preference is off,
    /// `no_reopen` is set, or another instance has it open.
    pub fn startup(&self, no_reopen: bool) -> StartupResult {
        let nothing = StartupResult {
            library: None,
            notice: None,
        };
        if no_reopen || !self.store.preferences().reopen_last_library {
            return nothing;
        }
        let Some(path) = self.store.library_to_reopen() else {
            return nothing;
        };
        match self.open_library(&path) {
            Ok(info) if info.read_only_reason == Some(ReadOnlyReason::OtherInstance) => {
                let _ = self.current_library().take();
                StartupResult {
                    library: None,
                    notice: Some(format!(
                        "\"{}\" is open in another egl-viewer window.",
                        info.name
                    )),
                }
            }
            Ok(info) => StartupResult {
                library: Some(info),
                notice: None,
            },
            Err(error) => StartupResult {
                library: None,
                notice: Some(format!("The last library could not be opened: {error}.")),
            },
        }
    }

    /// Quitting: closes the library without marking it closed explicitly.
    pub fn shutdown(&self) {
        if let Some(library) = self.current_library().take()
            && let Err(error) = library.close()
        {
            tracing::warn!("closing the library on exit failed: {error}");
        }
    }

    fn current_library(&self) -> MutexGuard<'_, Option<Library>> {
        self.library.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    struct Fixture {
        _data: tempfile::TempDir,
        home: tempfile::TempDir,
        session: Session,
    }

    fn fixture() -> Fixture {
        fixture_with(SyncEnvironment::default())
    }

    fn fixture_with(sync: SyncEnvironment) -> Fixture {
        let data = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let session = Session::new(
            AppStore::new(data.path()),
            home.path().to_path_buf(),
            "0.1.0-test",
            sync,
        );
        Fixture {
            _data: data,
            home,
            session,
        }
    }

    #[test]
    fn create_in_the_default_folder_makes_it_and_remembers_the_library() {
        let f = fixture();
        let parent = f.session.default_library_location();
        assert!(parent.ends_with("egl-library"));
        let info = f.session.create_library(&parent, "Refs").unwrap();
        assert_eq!(info.name, "Refs");
        assert!(!info.read_only);
        let recent = f.session.recent_libraries();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].name, "Refs");
        assert!(recent[0].path.ends_with("Refs.eglib"));
        assert_eq!(f.session.current().unwrap().id, info.id);
    }

    #[test]
    fn a_library_reopens_on_start_unless_closed_explicitly() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        let info = f.session.create_library(&parent, "Daily").unwrap();
        f.session.shutdown();
        assert!(f.session.current().is_none());

        let start = f.session.startup(false);
        assert_eq!(start.library.unwrap().id, info.id);
        assert_eq!(start.notice, None);

        f.session.close_library().unwrap();
        assert_eq!(f.session.startup(false).library, None);

        f.session
            .open_library(&f.session.recent_libraries()[0].path)
            .unwrap();
        f.session.shutdown();
        assert_eq!(f.session.startup(true).library, None, "--no-reopen");
    }

    #[test]
    fn the_last_library_open_elsewhere_is_not_reopened_but_mentioned() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        f.session.create_library(&parent, "Busy").unwrap();
        let path = f.session.recent_libraries()[0].path.clone();
        f.session.shutdown();

        let other = Library::open(Path::new(&path), &OpenOptions::new("other")).unwrap();
        let start = f.session.startup(false);
        assert_eq!(start.library, None);
        assert!(start.notice.unwrap().contains("Busy"));
        assert!(f.session.current().is_none());

        // Opening it on purpose is read-only, and works for editing once the other closes.
        let info = f.session.open_library(&path).unwrap();
        assert_eq!(info.read_only_reason, Some(ReadOnlyReason::OtherInstance));
        other.close().unwrap();
        let info = f.session.open_library(&path).unwrap();
        assert!(!info.read_only);
    }

    #[test]
    fn switching_keeps_the_current_library_when_the_new_one_fails() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        let first = f.session.create_library(&parent, "First").unwrap();
        let not_a_library = f.home.path().join("plain");
        fs::create_dir(&not_a_library).unwrap();
        assert_eq!(
            f.session.open_library(&not_a_library.to_string_lossy()),
            Err(SourceError::NotALibrary { eagle: false })
        );
        assert_eq!(f.session.current().unwrap().id, first.id);
    }

    #[test]
    fn sync_folders_are_refused_for_create_and_read_only_for_open() {
        let home = tempfile::tempdir().unwrap();
        let cloud = home.path().join("OneDrive");
        fs::create_dir(&cloud).unwrap();
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        f.session.create_library(&parent, "Moved").unwrap();
        f.session.shutdown();
        let moved = cloud.join("Moved.eglib");
        fs::rename(f.home.path().join("Moved.eglib"), &moved).unwrap();

        let g = fixture_with(SyncEnvironment {
            roots: vec![cloud.clone()],
        });
        assert_eq!(
            g.session.create_library(&cloud.to_string_lossy(), "New"),
            Err(SourceError::UnsupportedLocation(
                LocationProblem::SyncFolder
            ))
        );
        let info = g.session.open_library(&moved.to_string_lossy()).unwrap();
        assert_eq!(info.read_only_reason, Some(ReadOnlyReason::SyncFolder));
        assert_eq!(
            g.session.open_library(r"\\nas\share\Lib.eglib"),
            Err(SourceError::UnsupportedLocation(
                LocationProblem::NetworkPath
            ))
        );
        let preferences = Preferences {
            default_library_parent: Some(cloud.to_string_lossy().into_owned()),
            ..Preferences::default()
        };
        assert_eq!(
            g.session.save_preferences(preferences),
            Err(SourceError::UnsupportedLocation(
                LocationProblem::SyncFolder
            ))
        );
    }

    #[test]
    fn the_default_folder_can_be_changed() {
        let f = fixture();
        let elsewhere = f.home.path().join("Libraries");
        fs::create_dir(&elsewhere).unwrap();
        let saved = f
            .session
            .save_preferences(Preferences {
                default_library_parent: Some(elsewhere.to_string_lossy().into_owned()),
                ..Preferences::default()
            })
            .unwrap();
        assert_eq!(
            saved.default_library_parent.as_deref(),
            Some(display_path(&elsewhere).as_str())
        );
        assert_eq!(
            f.session.default_library_location(),
            display_path(&elsewhere)
        );
    }

    #[test]
    fn forgetting_a_recent_library_removes_it() {
        let f = fixture();
        let parent = f.home.path().to_string_lossy().into_owned();
        f.session.create_library(&parent, "Temp").unwrap();
        let path = f.session.recent_libraries()[0].path.clone();
        f.session.forget_recent_library(&path).unwrap();
        assert!(f.session.recent_libraries().is_empty());
    }
}
