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

/// Append target under [`log_dir`] (same idea as Tauri's D-079 `egl-viewer.log`).
pub fn log_file_path() -> PathBuf {
    log_dir().join("egl-viewer-gpui.log")
}

/// `MakeWriter` over a shared append-mode file (D-079; no `tracing-appender`).
struct MutexMakeWriter(std::sync::Arc<std::sync::Mutex<std::fs::File>>);

struct MutexWriter(std::sync::Arc<std::sync::Mutex<std::fs::File>>);

impl std::io::Write for MutexWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .flush()
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for MutexMakeWriter {
    type Writer = MutexWriter;

    fn make_writer(&'a self) -> Self::Writer {
        MutexWriter(std::sync::Arc::clone(&self.0))
    }
}

pub fn init_logging() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    // Always append to the log file. Also write stderr (useful for debug `cargo run`
    // and when a console is already attached). Release Windows GUI subsystem has no
    // console; we do not AllocConsole just to show logs.
    let file_layer = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path())
        .ok()
        .map(|file| {
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(MutexMakeWriter(std::sync::Arc::new(std::sync::Mutex::new(
                    file,
                ))))
        });

    let stderr_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);
    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer);
    let _ = match file_layer {
        Some(file_layer) => registry.with(file_layer).try_init(),
        None => registry.try_init(),
    };
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

/// Runs `work` on a fresh thread and returns once that thread has been started.
///
/// Windows `CreateProcess` / `SHOpenFolderAndSelectItems` pump the calling thread's
/// Win32 queue. GPUI is inside a mutable `App` borrow while handling the click that
/// starts them, so the pump panics and the process exits (D-139).
pub(crate) fn schedule_detached(
    name: &str,
    work: impl FnOnce() + Send + 'static,
) -> Result<(), String> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(work)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn spawn_command(mut command: std::process::Command) -> Result<(), String> {
    schedule_detached("shell-open", move || {
        if let Err(error) = command.spawn() {
            tracing::warn!("could not start a shell process: {error}");
        }
    })
}

pub fn open_in_file_manager(path: &Path) -> Result<(), String> {
    let path = require_absolute_existing(path)?;
    let folder = if path.is_dir() {
        path
    } else {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| "No parent folder to open".to_string())?
            .to_path_buf()
    };
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("explorer");
        command.arg(folder.as_os_str());
        spawn_command(command)
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        command.arg(folder.as_os_str());
        spawn_command(command)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(folder.as_os_str());
        spawn_command(command)
    }
}

/// Opens a file with the app Windows (or the OS) associates with its type.
pub fn open_path(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", &path.to_string_lossy()]);
        spawn_command(command)
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        command.arg(path.as_os_str());
        spawn_command(command)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(path.as_os_str());
        spawn_command(command)
    }
}

/// Shows originals in Explorer / Finder / file manager.
///
/// Paths must be absolute and exist (Session `original_paths` under the open
/// `.eglib`). Relative paths are refused: resolving them against the process cwd
/// (e.g. `target\debug` under `cargo run`) must never open Explorer there (D-134).
pub fn reveal_paths(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let absolute: Vec<PathBuf> = paths
        .iter()
        .map(|p| require_absolute_existing(p))
        .collect::<Result<Vec<_>, _>>()?;
    #[cfg(windows)]
    {
        crate::reveal_win::reveal_absolute_paths(&absolute)
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        command.arg("-R");
        for path in &absolute {
            command.arg(path);
        }
        spawn_command(command)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        open_in_file_manager(&absolute[0])
    }
}

/// Absolute, existing path only — never silently resolve against the process cwd.
fn require_absolute_existing(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("Show in Explorer: empty path".into());
    }
    if !path.is_absolute() {
        return Err(format!(
            "Show in Explorer needs an absolute library path, got {}",
            path.display()
        ));
    }
    if !path.exists() {
        return Err(format!(
            "Nothing to show in Explorer (missing): {}",
            path.display()
        ));
    }
    Ok(path.to_path_buf())
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
    let url = url.trim().to_string();
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "start", "", url.as_str()]);
        spawn_command(command)
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        command.arg(&url);
        spawn_command(command)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(&url);
        spawn_command(command)
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
            && let Ok(value) = hkcu.get_value::<String, _>("MinAnimate")
        {
            return value.trim() == "0";
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
    fn log_file_lives_under_log_dir() {
        let path = log_file_path();
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("egl-viewer-gpui.log"));
        assert_eq!(path.parent().map(Path::to_path_buf), Some(log_dir()));
    }

    #[test]
    fn shift_query_does_not_panic() {
        let _ = shift_held_at_start();
    }

    #[test]
    fn reduced_motion_is_callable() {
        let _ = reduced_motion();
    }

    #[test]
    fn reveal_refuses_relative_paths() {
        let err = reveal_paths(&[PathBuf::from("files/00/item.jpg")]).unwrap_err();
        assert!(
            err.contains("absolute"),
            "relative path must not open process cwd: {err}"
        );
    }

    #[test]
    fn reveal_refuses_missing_absolute() {
        #[cfg(windows)]
        let missing = PathBuf::from(r"C:\egl-viewer-missing-reveal-path-xyz\file.jpg");
        #[cfg(not(windows))]
        let missing = PathBuf::from("/egl-viewer-missing-reveal-path-xyz/file.jpg");
        let err = reveal_paths(&[missing]).unwrap_err();
        assert!(
            err.contains("missing") || err.contains("Nothing"),
            "missing absolute must error, not open cwd: {err}"
        );
    }

    #[test]
    fn detached_shell_work_is_not_on_the_calling_thread() {
        let (tx, rx) = std::sync::mpsc::channel();
        let caller = std::thread::current().id();
        schedule_detached("shell-open-test", move || {
            let _ = tx.send(std::thread::current().id());
        })
        .unwrap();
        let worker = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("shell thread");
        assert_ne!(worker, caller);
    }

    #[test]
    fn require_absolute_accepts_existing_dir() {
        let dir = std::env::temp_dir();
        assert!(dir.is_absolute());
        let got = require_absolute_existing(&dir).unwrap();
        assert_eq!(got, dir);
    }

    /// Live smoke: from `target\debug` cwd, reveal a real RSPECT original (D-134).
    /// Opt-in: `EGL_REVEAL_SMOKE=1 cargo test -p gpui-desktop reveal_rspect -- --ignored`.
    #[test]
    #[ignore = "opens Explorer; set EGL_REVEAL_SMOKE=1"]
    fn reveal_rspect_original_from_debug_cwd() {
        if std::env::var_os("EGL_REVEAL_SMOKE").is_none() {
            return;
        }
        let sample =
            PathBuf::from(r"D:\RSPECT.eglib\files\00\01M66R00V194J7YTDJD77PAZ00\PICT0145.JPG");
        assert!(
            sample.is_file(),
            "RSPECT sample missing: {}",
            sample.display()
        );
        let debug_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/debug")
            .canonicalize()
            .expect("target/debug");
        std::env::set_current_dir(&debug_dir).expect("chdir target/debug");
        assert_eq!(std::env::current_dir().unwrap(), debug_dir);
        #[cfg(windows)]
        let done = crate::reveal_win::install_shell_thread_probe();
        reveal_paths(&[sample]).expect("reveal must select the .eglib file, not cwd");
        #[cfg(windows)]
        done.recv_timeout(std::time::Duration::from_secs(15))
            .expect("explorer-reveal finished");
    }
}
