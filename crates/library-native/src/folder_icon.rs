//! The app's icon on the library folder in the file manager (D-117): Windows Explorer reads a
//! hidden `desktop.ini`, KDE's Dolphin a `.directory` file. The folder stays a folder: nothing
//! inside it changes, and the library opens without these files. Other platforms show their
//! own folder icon (macOS and iOS would need `.eglib` registered as a package; Android file
//! managers take no icon from the folder).
//!
//! The files are written once, when missing, and never replace a file the user made. A
//! failure is logged, not returned: an icon is not worth refusing to open a library.

use std::fs;
use std::io;
use std::path::Path;

/// The icon file next to `desktop.ini`; hidden like it.
#[cfg_attr(not(windows), allow(dead_code))]
const ICON_ICO: &str = ".egl-icon.ico";
/// The icon file `.directory` points to.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const ICON_PNG: &str = ".egl-icon.png";

/// What Windows Explorer reads.
#[cfg_attr(not(windows), allow(dead_code))]
const DESKTOP_INI: &str = "desktop.ini";
/// What KDE's Dolphin reads.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const DOT_DIRECTORY: &str = ".directory";

/// The icon in the two shapes the file managers read.
#[derive(Debug, Clone, Copy)]
pub struct FolderIcon<'a> {
    /// A Windows `.ico`.
    pub ico: &'a [u8],
    /// A PNG, 128 px is plenty.
    pub png: &'a [u8],
}

/// Gives the library folder `root` the app's icon on this platform; a no-op elsewhere. Call it
/// only on a library this instance may write (D-016): a read-only library is left as it is.
pub fn decorate_library_folder(root: &Path, icon: &FolderIcon) {
    if let Err(error) = decorate(root, icon) {
        tracing::warn!("cannot set the library folder's icon: {error}");
    }
}

/// `desktop.ini` for Explorer: the icon, by a path relative to the folder so a moved or
/// copied library keeps it.
#[cfg_attr(not(windows), allow(dead_code))]
fn desktop_ini() -> String {
    format!(
        "[.ShellClassInfo]\r\nIconResource={ICON_ICO},0\r\nIconFile={ICON_ICO}\r\nIconIndex=0\r\n"
    )
}

/// `.directory` for KDE: the icon, relative to the folder.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn dot_directory() -> String {
    format!("[Desktop Entry]\nIcon=./{ICON_PNG}\n")
}

/// Writes `name` in `root` only when nothing is there yet.
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
fn write_new(root: &Path, name: &str, bytes: &[u8]) -> io::Result<bool> {
    use std::io::Write;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(name))
    {
        Ok(mut file) => {
            file.write_all(bytes)?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn decorate(root: &Path, icon: &FolderIcon) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::process::CommandExt;

    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // `attrib`, so the attributes need no unsafe Win32 call (the crate forbids it).
    let attrib = |flags: &[&str], path: &Path| -> io::Result<()> {
        let status = std::process::Command::new("attrib")
            .args(flags)
            .arg(path)
            .creation_flags(CREATE_NO_WINDOW)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "attrib {flags:?} failed: {status}"
            )))
        }
    };
    if write_new(root, ICON_ICO, icon.ico)? {
        attrib(&["+h"], &root.join(ICON_ICO))?;
    }
    if write_new(root, DESKTOP_INI, desktop_ini().as_bytes())? {
        attrib(&["+h", "+s"], &root.join(DESKTOP_INI))?;
    }
    // Explorer reads `desktop.ini` only in a folder marked system (or read-only, which would
    // get in the way of deleting it).
    if fs::metadata(root)?.file_attributes() & FILE_ATTRIBUTE_SYSTEM == 0 {
        attrib(&["+s"], root)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn decorate(root: &Path, icon: &FolderIcon) -> io::Result<()> {
    write_new(root, ICON_PNG, icon.png)?;
    write_new(root, DOT_DIRECTORY, dot_directory().as_bytes())?;
    Ok(())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn decorate(_root: &Path, _icon: &FolderIcon) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: FolderIcon<'static> = FolderIcon {
        ico: b"ico bytes",
        png: b"png bytes",
    };

    #[test]
    fn the_icon_files_point_at_the_icon_by_a_relative_path() {
        assert!(desktop_ini().contains("IconResource=.egl-icon.ico,0"));
        assert!(dot_directory().contains("Icon=./.egl-icon.png"));
    }

    /// D-117: the library folder gets the icon its platform reads, once, and a file the user
    /// put there is never replaced.
    #[test]
    fn a_library_folder_gets_the_icon_and_keeps_what_is_there() {
        let dir = tempfile::tempdir().unwrap();
        let root = crate::create_library(dir.path(), "Иконка 📁", "0.0.0-test").unwrap();
        decorate_library_folder(&root, &ICON);
        if cfg!(windows) {
            assert_eq!(fs::read(root.join(ICON_ICO)).unwrap(), ICON.ico);
            assert_eq!(
                fs::read_to_string(root.join(DESKTOP_INI)).unwrap(),
                desktop_ini()
            );
        } else if cfg!(target_os = "linux") {
            assert_eq!(fs::read(root.join(ICON_PNG)).unwrap(), ICON.png);
            assert_eq!(
                fs::read_to_string(root.join(DOT_DIRECTORY)).unwrap(),
                dot_directory()
            );
            // A `.directory` the user wrote stays theirs.
            fs::write(
                root.join(DOT_DIRECTORY),
                "[Desktop Entry]\nIcon=folder-red\n",
            )
            .unwrap();
            decorate_library_folder(&root, &ICON);
            assert_eq!(
                fs::read_to_string(root.join(DOT_DIRECTORY)).unwrap(),
                "[Desktop Entry]\nIcon=folder-red\n"
            );
        }
        // Twice is the same as once, and the library still opens and checks clean.
        decorate_library_folder(&root, &ICON);
        let library = crate::Library::open(&root, &crate::OpenOptions::new("0.0.0-test")).unwrap();
        let check = library.check_library().unwrap();
        assert!(check.is_clean(), "{check:?}");
    }

    #[cfg(windows)]
    #[test]
    fn explorer_reads_the_icon_of_a_system_folder() {
        use std::os::windows::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let root = crate::create_library(dir.path(), "Icon", "0.0.0-test").unwrap();
        decorate_library_folder(&root, &ICON);
        assert_ne!(
            fs::metadata(&root).unwrap().file_attributes() & 0x4,
            0,
            "system folder"
        );
        let ini = fs::metadata(root.join(DESKTOP_INI))
            .unwrap()
            .file_attributes();
        assert_eq!(ini & 0x6, 0x6, "desktop.ini hidden and system");
        // A system folder can still be removed.
        drop(fs::remove_dir_all(&root));
        assert!(!root.exists());
    }
}
