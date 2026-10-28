//! Every error leaves this crate as a `SourceError`; IO and SQLite errors become `Storage`.

use std::fmt::Display;

use library_core::SourceError;

pub(crate) type Result<T, E = SourceError> = std::result::Result<T, E>;

/// Adds what was being done to an IO or SQLite error.
pub(crate) trait Context<T> {
    fn context(self, what: &str) -> Result<T>;
}

impl<T, E: Display> Context<T> for std::result::Result<T, E> {
    fn context(self, what: &str) -> Result<T> {
        self.map_err(|error| SourceError::Storage(format!("{what}: {error}")))
    }
}
