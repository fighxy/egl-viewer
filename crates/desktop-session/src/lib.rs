//! The desktop session: a native library through [`app_service::LibraryService`], a running
//! Eagle as the other source, and the app's own files (recent libraries, preferences, login),
//! without a UI toolkit (D-127). Tauri commands and the GPUI shell both drive a [`Session`].

mod export_eagle;
mod session;
#[cfg(windows)]
mod webauthn;
#[cfg(not(windows))]
#[path = "webauthn_unsupported.rs"]
mod webauthn;
pub mod yubikey;

pub use app_service::{
    Blocking, Emit, Event, ImageRequest, ItemsTarget, ScopeProgress, SmartDraft, ViewOpened, count,
    invalidated, ready,
};
pub use session::{
    Clipboard, EagleImportRequest, EagleLibrarySummary, EagleLibraryTarget, EagleSessionInfo,
    Session, StartupResult, lock, now_ms,
};
