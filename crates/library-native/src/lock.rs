//! The library lock: an OS file lock on `library.lock`, released by the OS when the process
//! ends (docs/library-format.md §5, D-020).

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

use crate::LOCK_FILE;

/// Held while this process writes the library. Dropping it releases the lock.
#[derive(Debug)]
pub(crate) struct LibraryLock {
    file: File,
}

pub(crate) enum LockAttempt {
    Acquired(LibraryLock),
    /// Another process (or another open in this process) holds it.
    Held,
    /// The file cannot be opened for writing or locked: read-only medium, no permission.
    Unavailable,
}

impl LibraryLock {
    pub(crate) fn try_acquire(root: &Path) -> LockAttempt {
        // Created if a copied library lost it; never truncated, never deleted.
        let opened = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(LOCK_FILE));
        let file = match opened {
            Ok(file) => file,
            Err(error) => {
                tracing::info!("library lock cannot be opened for writing: {error}");
                return LockAttempt::Unavailable;
            }
        };
        match file.try_lock() {
            Ok(()) => LockAttempt::Acquired(Self { file }),
            Err(TryLockError::WouldBlock) => LockAttempt::Held,
            Err(TryLockError::Error(error)) => {
                tracing::info!("library lock cannot be taken: {error}");
                LockAttempt::Unavailable
            }
        }
    }
}

impl Drop for LibraryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_on_the_same_folder_is_refused_until_the_first_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let first = LibraryLock::try_acquire(dir.path());
        assert!(matches!(first, LockAttempt::Acquired(_)));
        assert!(matches!(
            LibraryLock::try_acquire(dir.path()),
            LockAttempt::Held
        ));
        drop(first);
        assert!(matches!(
            LibraryLock::try_acquire(dir.path()),
            LockAttempt::Acquired(_)
        ));
        assert!(dir.path().join(LOCK_FILE).is_file(), "the lock file stays");
    }
}
