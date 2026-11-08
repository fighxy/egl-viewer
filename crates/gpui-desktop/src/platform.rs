//! Host helpers: data dirs, logs, reveal in Explorer (D-129).
//! File pickers use GPUI `App::prompt_for_paths` from the root entity (see `app.rs`).

use std::path::{Path, PathBuf};

/// Stable local-data root for the GPUI host (separate from the Tauri app id until cutover).
pub fn app_data_dir() -> PathBuf {
    local_data_base()
        .map(|base| base.join("egl-viewer-gpui"))
        .unwrap_or_else(|| std::env::temp_dir().join("egl-viewer-gpui"))
}

fn local_data_base() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
    }
}

pub fn log_dir() -> PathBuf {
    app_data_dir().join("logs")
}

pub fn init_logging() {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// `--no-reopen` skips restoring the last library (same as Tauri). Shift-at-start remains
/// unimplemented without a safe Win32 wrapper (D-032), same as the shipping host today.
pub fn no_reopen_requested() -> bool {
    std::env::args().any(|arg| arg == "--no-reopen")
}

pub fn open_in_file_manager(path: &Path) -> Result<(), String> {
    let path = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(path.as_os_str())
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path.as_os_str())
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path.as_os_str())
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_dir_is_non_empty() {
        assert!(!app_data_dir().as_os_str().is_empty());
    }
}
