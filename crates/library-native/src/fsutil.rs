//! File system helpers shared by creating a library and adding files.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

/// Antivirus scanners hold new files briefly on Windows, so a rename can fail for up to about a
/// second (docs/library-format.md §6).
pub(crate) fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    const ATTEMPTS: u32 = 10;
    let mut attempt = 1;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < ATTEMPTS && is_transient(&error) => {
                attempt += 1;
                sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error),
        }
    }
}

fn is_transient(error: &std::io::Error) -> bool {
    // 32 is ERROR_SHARING_VIOLATION on Windows.
    error.kind() == ErrorKind::PermissionDenied || error.raw_os_error() == Some(32)
}
