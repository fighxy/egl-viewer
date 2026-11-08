//! Host helpers: data dirs, logs, reveal in Explorer, Shift-at-start (D-129 / D-032).
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

/// `--no-reopen` skips restoring the last library (same as Tauri).
pub fn no_reopen_requested() -> bool {
    std::env::args().any(|arg| arg == "--no-reopen")
}

/// Holding Shift at start skips reopening the last library (D-017 / D-022 / D-032).
/// Uses `device_query` so this crate never writes `unsafe` (D-032).
pub fn shift_held_at_start() -> bool {
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    {
        use device_query::{DeviceQuery, DeviceState, Keycode};
        let keys = DeviceState::new().get_keys();
        keys.contains(&Keycode::LShift) || keys.contains(&Keycode::RShift)
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

/// Skip reopen when `--no-reopen` or Shift is held at process start.
pub fn skip_reopen() -> bool {
    no_reopen_requested() || shift_held_at_start()
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

/// Opens a file with the app Windows (or the OS) associates with its type.
pub fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
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

/// Shows originals in Explorer / Finder / file manager.
pub fn reveal_paths(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Select the first file; additional files share its folder.
        let first = &paths[0];
        let arg = format!("/select,{}", first.display());
        std::process::Command::new("explorer")
            .raw_arg(arg)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-R");
        for path in paths {
            cmd.arg(path);
        }
        cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        open_in_file_manager(&paths[0])
    }
}

/// Puts file paths on the system clipboard (Explorer paste).
pub fn copy_files(paths: &[PathBuf]) -> Result<u32, String> {
    use std::sync::{Mutex, OnceLock, PoisonError};
    static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();
    let slot = CLIPBOARD.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().unwrap_or_else(PoisonError::into_inner);
    let clipboard = match guard.as_mut() {
        Some(clipboard) => clipboard,
        None => guard.insert(arboard::Clipboard::new().map_err(|e| e.to_string())?),
    };
    clipboard
        .set()
        .file_list(paths)
        .map_err(|e| e.to_string())?;
    Ok(u32::try_from(paths.len()).unwrap_or(u32::MAX))
}

/// Opens an http(s) URL in the system browser.
pub fn open_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Whether the Space key is currently held (for peek release / D-073).
pub fn space_key_down() -> bool {
    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    {
        use device_query::{DeviceQuery, DeviceState, Keycode};
        DeviceState::new().get_keys().contains(&Keycode::Space)
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

/// Best-effort reduced-motion: env override, or Windows desktop MinAnimate = 0.
pub fn reduced_motion() -> bool {
    if std::env::var_os("EGL_REDUCED_MOTION").is_some()
        || std::env::var_os("EGL_NO_MOTION").is_some()
    {
        return true;
    }
    #[cfg(windows)]
    {
        if let Ok(hkcu) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(r"Control Panel\Desktop\WindowMetrics")
        {
            if let Ok(value) = hkcu.get_value::<String, _>("MinAnimate") {
                return value.trim() == "0";
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_dir_is_non_empty() {
        assert!(!app_data_dir().as_os_str().is_empty());
    }

    #[test]
    fn shift_query_does_not_panic() {
        let _ = shift_held_at_start();
    }

    #[test]
    fn reduced_motion_is_callable() {
        let _ = reduced_motion();
    }
}
