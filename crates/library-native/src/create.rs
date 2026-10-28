//! Creating a library in a temporary folder that is renamed when complete
//! (docs/library-format.md §9).

use std::fs;
use std::path::{Path, PathBuf};

use library_core::{LocationProblem, SourceError};
use rusqlite::Connection;
use ulid::Ulid;

use crate::detect::{is_native_library, looks_like_eagle_library};
use crate::error::{Context, Result};
use crate::fsutil::rename_with_retry;
use crate::name::validate_library_name;
use crate::schema::{self, meta_set};
use crate::time::now_ms;
use crate::{DB_FILE, LIBRARY_SUFFIX, LOCK_FILE};

const CREATING: &str = ".eglib.creating-";

/// Creates `<parent>/<name>.eglib` and returns its path. The caller checks sync folders and
/// network paths (`app-store`); this refuses names, existing targets, and folders inside
/// another library or an Eagle library. The new library is closed; open it next.
pub fn create_library(parent: &Path, name: &str, app_version: &str) -> Result<PathBuf> {
    create_with(parent, name, app_version, false)
}

pub(crate) fn create_with(
    parent: &Path,
    name: &str,
    app_version: &str,
    fail_after_database: bool,
) -> Result<PathBuf> {
    let name = validate_library_name(name)?;
    if !parent.is_dir() {
        return Err(SourceError::Invalid("The folder does not exist.".into()));
    }
    check_not_inside_a_library(parent)?;
    let target = parent.join(format!("{name}.{LIBRARY_SUFFIX}"));
    if target.exists() {
        return Err(SourceError::AlreadyExists);
    }
    remove_leftovers(parent);
    let temp = parent.join(format!(".{name}{CREATING}{}", Ulid::generate()));
    let built = build(&temp, &name, app_version).and_then(|()| {
        if fail_after_database {
            Err(SourceError::Storage("simulated failure".into()))
        } else {
            Ok(())
        }
    });
    if let Err(error) = built {
        let _ = fs::remove_dir_all(&temp);
        return Err(error);
    }
    if let Err(error) = rename_with_retry(&temp, &target) {
        let _ = fs::remove_dir_all(&temp);
        return Err(if target.exists() {
            SourceError::AlreadyExists
        } else {
            SourceError::Storage(format!("move the new library into place: {error}"))
        });
    }
    Ok(target)
}

fn check_not_inside_a_library(parent: &Path) -> Result<()> {
    for folder in parent.ancestors() {
        if is_native_library(folder) {
            return Err(SourceError::UnsupportedLocation(
                LocationProblem::InsideLibrary,
            ));
        }
        if looks_like_eagle_library(folder) {
            return Err(SourceError::UnsupportedLocation(
                LocationProblem::InsideEagleLibrary,
            ));
        }
    }
    Ok(())
}

/// Removes temporary folders that an interrupted create left in `parent`.
fn remove_leftovers(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') && name.contains(CREATING) && entry.path().is_dir() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn build(dir: &Path, name: &str, app_version: &str) -> Result<()> {
    fs::create_dir(dir).context("create the library folder")?;
    fs::create_dir_all(dir.join("files").join(".tmp")).context("create files/")?;
    fs::create_dir(dir.join("thumbs")).context("create thumbs/")?;
    fs::create_dir(dir.join("backups")).context("create backups/")?;
    fs::File::create(dir.join(LOCK_FILE)).context("create library.lock")?;

    let mut conn = Connection::open(dir.join(DB_FILE)).context("create library.db")?;
    schema::create(&conn)?;
    let now = now_ms().to_string();
    let id = Ulid::generate().to_string();
    let tx = conn.transaction().context("write meta")?;
    for (key, value) in [
        ("library_id", id.as_str()),
        ("name", name),
        ("created_at", now.as_str()),
        ("created_with", app_version),
        ("last_written_with", app_version),
    ] {
        meta_set(&tx, key, value)?;
    }
    tx.commit().context("write meta")?;
    // Closing the only connection checkpoints and removes the WAL files, so the folder can be
    // renamed on Windows.
    conn.close()
        .map_err(|(_, error)| error)
        .context("close library.db")
}
