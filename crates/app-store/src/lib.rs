//! Settings, recent libraries, library locations and secrets (docs/architecture.md §3).
//!
//! Small JSON files in the app's local data folder, written with a lock and an atomic replace
//! so that several app processes do not overwrite each other (D-022). The secret store for the
//! Eagle token (Windows Credential Manager) arrives in phase 9 (docs/spec.md §9).

mod json_file;
pub mod location;
pub mod login;
pub mod preferences;
pub mod recent;
pub mod secret;
pub mod settings;

use std::path::PathBuf;

pub use location::{SyncEnvironment, check_location, default_library_parent, display_path};
pub use login::{
    Authenticator, LoginMethod, LoginProof, LoginStatus, SoftwareAuthenticator, WebAuthnAssertion,
    WebAuthnCredential,
};
pub use preferences::{
    BrowsePreferences, Locale, Preferences, Theme, UiLayout, ViewMode, WindowState,
};
pub use recent::RecentLibrary;
pub use secret::{KeyringStore, MemoryStore, SecretStore, eagle_token_name};
pub use settings::ConnectionSettings;

/// The app's files in one folder: `%LOCALAPPDATA%\com.fighxy.eglviewer` on Windows.
#[derive(Debug, Clone)]
pub struct AppStore {
    dir: PathBuf,
}

impl AppStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &std::path::Path {
        &self.dir
    }
}
