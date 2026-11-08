//! Windows Show-in-Explorer via the shell (`SHOpenFolderAndSelectItems`).
//!
//! `explorer.exe /select,…` via `CreateProcess` often ignores a bad or relative
//! path and opens the process cwd instead (seen as `target\debug` under
//! `cargo run`). The shell API selects the real absolute item path (D-134).

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Com::CoInitialize;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ILCreateFromPathW, ILFree, SHELLEXECUTEINFOW, SHOpenFolderAndSelectItems, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

/// Reveal absolute existing paths in Explorer, selecting each file in its folder.
///
/// The shell call runs on `explorer-reveal`, not the caller. `SHOpenFolderAndSelectItems`
/// pumps the Win32 queue (`SHOpenOrGetFolderView` → the GPUI window procedure). From a
/// click, `App` is already mutably borrowed, so that re-entry panics and the process
/// exits after Explorer has opened (D-139).
pub fn reveal_absolute_paths(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let grouped = group_by_parent(paths)?;
    crate::platform::schedule_detached("explorer-reveal", move || {
        let result = reveal_grouped(grouped);
        #[cfg(test)]
        note_shell_thread();
        if let Err(error) = result {
            tracing::warn!("Show in Explorer: {error}");
        }
    })
}

fn group_by_parent(paths: &[PathBuf]) -> Result<HashMap<PathBuf, Vec<PathBuf>>, String> {
    let mut grouped: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for path in paths {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| format!("No parent folder for {}", path.display()))?
            .to_path_buf();
        grouped.entry(parent).or_default().push(path.clone());
    }
    Ok(grouped)
}

fn reveal_grouped(grouped: HashMap<PathBuf, Vec<PathBuf>>) -> Result<(), String> {
    // Unit tests must not open Explorer. The opt-in smoke test sets EGL_REVEAL_SMOKE.
    #[cfg(test)]
    if std::env::var_os("EGL_REVEAL_SMOKE").is_none() {
        return Ok(());
    }

    // Idempotent; already-initialized COM returns an error we ignore (same as opener).
    let _ = unsafe { CoInitialize(None) };

    for (parent, items) in grouped {
        let parent_id = OwnedItemIdList::new(&parent)?;
        let item_ids = items
            .iter()
            .map(|p| OwnedItemIdList::new(p))
            .collect::<Result<Vec<_>, _>>()?;
        let ptrs: Vec<*const ITEMIDLIST> = item_ids.iter().map(|item| item.item).collect();

        if let Err(error) =
            unsafe { SHOpenFolderAndSelectItems(parent_id.item, Some(ptrs.as_slice()), 0) }
        {
            // Electron / opener fallback: some hosts return "file not found" even when
            // the path exists; open the parent folder without selection.
            if error.code().0 == ERROR_FILE_NOT_FOUND.0 as i32 {
                let first = &items[0];
                let is_dir = first.is_dir();
                let mut info = SHELLEXECUTEINFOW {
                    cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                    nShow: SW_SHOWNORMAL.0,
                    lpFile: PCWSTR(parent_id.hstring.as_ptr()),
                    lpClass: if is_dir { w!("folder") } else { PCWSTR::null() },
                    lpVerb: if is_dir {
                        w!("explore")
                    } else {
                        PCWSTR::null()
                    },
                    ..Default::default()
                };
                unsafe { ShellExecuteExW(&mut info) }.map_err(|e| e.to_string())?;
            } else {
                return Err(error.to_string());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
fn note_shell_thread() {
    if let Some(tx) = SHELL_THREAD.lock().unwrap().as_ref() {
        let _ = tx.send(std::thread::current().id());
    }
}

#[cfg(test)]
static SHELL_THREAD: std::sync::Mutex<Option<std::sync::mpsc::Sender<std::thread::ThreadId>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
pub(crate) fn install_shell_thread_probe() -> std::sync::mpsc::Receiver<std::thread::ThreadId> {
    let (tx, rx) = std::sync::mpsc::channel();
    *SHELL_THREAD.lock().unwrap() = Some(tx);
    rx
}

struct OwnedItemIdList {
    hstring: HSTRING,
    item: *const ITEMIDLIST,
}

impl OwnedItemIdList {
    fn new(path: &Path) -> Result<Self, String> {
        let hstring = HSTRING::from(path);
        let item = unsafe { ILCreateFromPathW(&hstring) };
        if item.is_null() {
            Err(format!(
                "Could not prepare Explorer path: {}",
                path.display()
            ))
        } else {
            Ok(Self { hstring, item })
        }
    }
}

impl Drop for OwnedItemIdList {
    fn drop(&mut self) {
        if !self.item.is_null() {
            unsafe { ILFree(Some(self.item)) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_shell_is_not_on_the_calling_thread() {
        let rx = install_shell_thread_probe();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("item.txt");
        std::fs::write(&file, b"x").unwrap();
        let caller = std::thread::current().id();
        reveal_absolute_paths(std::slice::from_ref(&file)).unwrap();
        let worker = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("explorer-reveal thread");
        assert_ne!(worker, caller);
        *SHELL_THREAD.lock().unwrap() = None;
    }
}
