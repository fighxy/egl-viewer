//! The open native library as an app uses it, without a UI toolkit (D-091): the views of the
//! open library, the undo stack, item, folder, tag and lock edits, and the background tasks
//! (imports, previews, exports). The desktop Session (`desktop-session`, D-127) and the mobile
//! C ABI (`library-ffi`) both drive a [`LibraryService`]; what is particular to the desktop
//! (a running Eagle, the app's settings files, the clipboard, YubiKey and WebAuthn) lives in
//! `desktop-session`, not here.
//!
//! Where a library may live is the caller's question: the desktop checks cloud-synced and
//! network folders (`app_store::check_location`), a phone checks its own containers. The
//! service opens what it is given, read-only when told it is synced.

mod eagle;
mod events;
mod service;
mod source;
mod tasks;

#[cfg(test)]
mod tests;

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

pub use eagle::{EagleLibrarySummary, eagle_error, eagle_library_info};
pub use events::{
    Emit, Event, ItemsTarget, ScopeProgress, ViewOpened, ViewsInvalidated, count, invalidated,
};
pub use service::{LibraryService, SmartDraft, no_library};
pub use source::{Blocking, ImageRequest, Refresher, ready};
pub use tasks::{ExportContext, ExportTasks, ImportTasks, TaskContext};

/// Locks `mutex`, going on after a panic in another thread: the guarded state stays usable.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Milliseconds since the Unix epoch, UTC.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0)
}
