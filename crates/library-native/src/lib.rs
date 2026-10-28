//! The native egl-viewer library: a folder `Name.eglib` with `library.db`, the original files
//! and the library lock (docs/library-format.md). No Tauri: the tests run on temporary folders.

mod add;
mod backup;
mod create;
mod detect;
mod error;
mod fsutil;
mod import;
mod library;
mod lock;
mod name;
mod preview;
mod recover;
mod schema;
mod time;

pub use time::utc_file_stamp;

#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use add::{AddOutcome, Duplicates};
pub use create::create_library;
pub use detect::{is_native_library, looks_like_eagle_library};
pub use import::{
    DuplicateChoice, DuplicatePolicy, ImportObserver, import_files, make_pending_previews,
    preview_threads,
};
pub use library::{Library, OpenOptions};
pub use name::validate_library_name;
pub use preview::{NO_PREVIEW, PendingPreview};
pub use schema::{APPLICATION_ID, FORMAT_VERSION};

/// Library folders end in `.eglib` (D-016).
pub const LIBRARY_SUFFIX: &str = "eglib";

pub(crate) const DB_FILE: &str = "library.db";
pub(crate) const LOCK_FILE: &str = "library.lock";
