//! File logs under the app's local data folder, and Help → Open Log Folder (D-079).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use library_core::SourceError;
use tauri::{AppHandle, Manager};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;

const LOG_FILE: &str = "egl-viewer.log";

/// Shared append-only log file; kept for the process so tracing can write to it.
static FILE: OnceLock<Mutex<File>> = OnceLock::new();

/// `{app_local_data_dir}/logs`.
pub fn log_dir(app: &AppHandle) -> Result<PathBuf, SourceError> {
    app.path()
        .app_local_data_dir()
        .map(|dir| dir.join("logs"))
        .map_err(|error| SourceError::Storage(format!("app data folder: {error}")))
}

struct SharedFile;

impl Write for SharedFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let file = FILE
            .get()
            .ok_or_else(|| io::Error::other("log file was not opened"))?;
        let mut guard = file.lock().map_err(|_| io::Error::other("log file lock"))?;
        guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let file = FILE
            .get()
            .ok_or_else(|| io::Error::other("log file was not opened"))?;
        let mut guard = file.lock().map_err(|_| io::Error::other("log file lock"))?;
        guard.flush()
    }
}

impl<'a> MakeWriter<'a> for SharedFile {
    type Writer = SharedFile;

    fn make_writer(&'a self) -> Self::Writer {
        SharedFile
    }
}

/// Stderr plus an append-only file in `dir`. Safe to call once from `setup`.
///
/// Never log the Eagle token or raw bodies of `library/info` and `folder/get` (§10).
pub fn init(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LOG_FILE))
    {
        let _ = FILE.set(Mutex::new(file));
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr = tracing_subscriber::fmt::layer().with_writer(io::stderr);
    let file_layer = FILE.get().map(|_| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(SharedFile)
    });
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr)
        .with(file_layer)
        .try_init();
}

/// Stderr only, when the log folder cannot be created before the window exists.
pub fn init_stderr_only() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
