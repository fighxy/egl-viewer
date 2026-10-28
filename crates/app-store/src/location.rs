//! Where libraries may live: local disks, outside cloud-synced folders (docs/spec.md §3, D-021).
//! Detection is best effort.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use library_core::LocationProblem;

/// The folder new libraries go to unless the user picks another: `%USERPROFILE%\egl-library`.
/// OneDrive's folder backup moves Desktop, Documents and Pictures, not the profile root.
pub fn default_library_parent(home: &Path) -> PathBuf {
    home.join("egl-library")
}

/// The roots of folders that cloud sync tools copy on this computer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncEnvironment {
    pub roots: Vec<PathBuf>,
}

impl SyncEnvironment {
    /// OneDrive from its environment variables, plus the sync roots registered with the
    /// Windows Cloud Files API (OneDrive, recent Dropbox, iCloud, Box).
    pub fn from_system() -> Self {
        let mut roots = onedrive_roots(|key| std::env::var_os(key));
        roots.extend(registered_sync_roots());
        Self { roots }
    }
}

pub(crate) fn onedrive_roots(var: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .iter()
        .filter_map(|key| var(key))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(windows)]
fn registered_sync_roots() -> Vec<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};

    const MANAGER: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager";
    let Ok(manager) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(MANAGER, KEY_READ)
    else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    for provider in manager.enum_keys().flatten() {
        let Ok(user_roots) =
            manager.open_subkey_with_flags(format!(r"{provider}\UserSyncRoots"), KEY_READ)
        else {
            continue;
        };
        // One value per user SID; the data is the root folder.
        for name in user_roots.enum_values().flatten().map(|(name, _)| name) {
            if let Ok(path) = user_roots.get_value::<String, _>(&name) {
                roots.push(PathBuf::from(path));
            }
        }
    }
    roots
}

#[cfg(not(windows))]
fn registered_sync_roots() -> Vec<PathBuf> {
    Vec::new()
}

/// Files that sync tools put at the root of the folders they copy.
const SYNC_MARKERS: &[&str] = &[".dropbox.cache", ".stfolder"];

/// Refuses network paths and folders inside a sync root or under a sync marker. `path` need not
/// exist yet: the nearest existing ancestor is resolved.
pub fn check_location(path: &Path, sync: &SyncEnvironment) -> Result<(), LocationProblem> {
    let resolved = resolve(path);
    if is_network_path(&resolved) {
        return Err(LocationProblem::NetworkPath);
    }
    let in_root = sync
        .roots
        .iter()
        .any(|root| starts_with_ignoring_case(&resolved, &resolve(root)));
    let marked = resolved.ancestors().any(|folder| {
        SYNC_MARKERS
            .iter()
            .any(|marker| folder.join(marker).exists())
    });
    if in_root || marked {
        return Err(LocationProblem::SyncFolder);
    }
    Ok(())
}

/// A path for people and for the recent list: canonical where it exists, without Windows'
/// `\\?\` prefix.
pub fn display_path(path: &Path) -> String {
    resolve(path).to_string_lossy().into_owned()
}

/// Canonicalizes the nearest existing ancestor and appends the rest.
fn resolve(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Ok(canonical) = ancestor.canonicalize() {
            let rest = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            let joined = if rest.as_os_str().is_empty() {
                canonical
            } else {
                canonical.join(rest)
            };
            return strip_verbatim(&joined);
        }
    }
    strip_verbatim(path)
}

/// `\\?\C:\x` → `C:\x` and `\\?\UNC\server\share` → `\\server\share`.
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

/// A UNC path. Canonicalizing turns mapped network drives into UNC paths on Windows.
fn is_network_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.starts_with(r"\\") && !text.starts_with(r"\\.\")
}

fn starts_with_ignoring_case(path: &Path, root: &Path) -> bool {
    let lower = |part: std::path::Component<'_>| part.as_os_str().to_string_lossy().to_lowercase();
    let mut path_parts = path.components().map(lower);
    let root_parts: Vec<String> = root.components().map(lower).collect();
    !root_parts.is_empty()
        && root_parts
            .iter()
            .all(|part| path_parts.next().as_ref() == Some(part))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn env(roots: &[&Path]) -> SyncEnvironment {
        SyncEnvironment {
            roots: roots.iter().map(|root| root.to_path_buf()).collect(),
        }
    }

    #[test]
    fn the_default_folder_is_outside_onedrive() {
        let home = tempfile::tempdir().unwrap();
        let onedrive = home.path().join("OneDrive");
        fs::create_dir_all(onedrive.join("Pictures")).unwrap();
        let sync = env(&[&onedrive]);
        let default = default_library_parent(home.path());
        assert_eq!(default, home.path().join("egl-library"));
        assert_eq!(check_location(&default, &sync), Ok(()));
        assert_eq!(
            check_location(&onedrive.join("Pictures").join("Refs.eglib"), &sync),
            Err(LocationProblem::SyncFolder)
        );
    }

    #[test]
    fn sync_roots_match_without_case_and_before_they_exist() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("Dropbox");
        fs::create_dir(&root).unwrap();
        let sync = env(&[&root]);
        let upper = PathBuf::from(root.to_string_lossy().to_uppercase())
            .join("new")
            .join("x");
        if upper.parent().unwrap().parent().unwrap().exists() {
            assert_eq!(
                check_location(&upper, &sync),
                Err(LocationProblem::SyncFolder)
            );
        }
        assert_eq!(
            check_location(&root.join("not").join("yet"), &sync),
            Err(LocationProblem::SyncFolder)
        );
        assert_eq!(
            check_location(&home.path().join("Dropboxed"), &sync),
            Ok(())
        );
    }

    #[test]
    fn markers_of_sync_tools_are_recognised() {
        let home = tempfile::tempdir().unwrap();
        for marker in SYNC_MARKERS {
            let synced = home.path().join(format!("synced{marker}"));
            fs::create_dir_all(synced.join(marker)).unwrap();
            fs::create_dir_all(synced.join("deep")).unwrap();
            assert_eq!(
                check_location(
                    &synced.join("deep").join("Lib.eglib"),
                    &SyncEnvironment::default()
                ),
                Err(LocationProblem::SyncFolder),
                "{marker}"
            );
        }
        assert_eq!(
            check_location(&home.path().join("plain"), &SyncEnvironment::default()),
            Ok(())
        );
    }

    #[test]
    fn onedrive_comes_from_its_variables() {
        let roots = onedrive_roots(|key| match key {
            "OneDrive" => Some(r"C:\Users\me\OneDrive".into()),
            "OneDriveCommercial" => Some(r"C:\Users\me\OneDrive - Work".into()),
            "OneDriveConsumer" => Some("".into()),
            _ => None,
        });
        assert_eq!(
            roots,
            vec![
                PathBuf::from(r"C:\Users\me\OneDrive"),
                PathBuf::from(r"C:\Users\me\OneDrive - Work")
            ]
        );
    }

    #[test]
    fn unc_paths_are_network_paths() {
        assert!(is_network_path(Path::new(r"\\nas\photos\Lib.eglib")));
        assert!(!is_network_path(Path::new(r"\\.\pipe\x")));
        assert!(!is_network_path(Path::new(r"C:\Users\me")));
        assert!(!is_network_path(Path::new("/home/me")));
        assert_eq!(
            check_location(
                Path::new(r"\\nas\photos\Lib.eglib"),
                &SyncEnvironment::default()
            ),
            Err(LocationProblem::NetworkPath)
        );
    }

    #[test]
    fn verbatim_prefixes_are_removed() {
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\C:\Users\me")),
            PathBuf::from(r"C:\Users\me")
        );
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\UNC\nas\photos")),
            PathBuf::from(r"\\nas\photos")
        );
        assert_eq!(
            strip_verbatim(Path::new("/home/me")),
            PathBuf::from("/home/me")
        );
    }
}
