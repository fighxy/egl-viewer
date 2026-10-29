//! What the writer cleans up on open (docs/library-format.md §6, §7).

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use rusqlite::Connection;

use crate::error::{Context, Result};

/// Resolves `pending_file` rows left by a crash while adding files: keeps a file that its item
/// points at, deletes any other, and clears the journal.
pub(crate) fn pending_files(conn: &Connection, root: &Path) -> Result<usize> {
    let paths: Vec<String> = conn
        .prepare("SELECT path FROM pending_file")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()
        })
        .context("read the add journal")?;
    for path in &paths {
        let has_item: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM item WHERE file_path = ?1)",
                [path],
                |row| row.get(0),
            )
            .context("check the add journal")?;
        if !has_item {
            match journal_file(root, path) {
                Some(file) => remove_with_empty_parent(&file)?,
                None => tracing::warn!("ignoring a journal path outside files/: {path}"),
            }
        }
        conn.execute("DELETE FROM pending_file WHERE path = ?1", [path])
            .context("clear the add journal")?;
    }
    Ok(paths.len())
}

/// Deletes everything inside `files/.tmp/` and makes sure the folder exists.
pub(crate) fn empty_tmp(root: &Path) -> Result<()> {
    let tmp = root.join("files").join(".tmp");
    fs::create_dir_all(&tmp).context("create files/.tmp")?;
    for entry in fs::read_dir(&tmp).context("list files/.tmp")? {
        let path = entry.context("list files/.tmp")?.path();
        let removed = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        removed.context("empty files/.tmp")?;
    }
    Ok(())
}

/// A journal path: an original under `files/`, or a preview under `thumbs/` that encrypting
/// or decrypting an item replaced.
pub(crate) fn journal_file(root: &Path, stored: &str) -> Option<PathBuf> {
    match stored.split_once('/') {
        Some(("thumbs", rest)) => plain_parts(root.join("thumbs"), rest),
        _ => library_file(root, stored),
    }
}

/// A stored `files/…` path as a path on disk, or `None` if it would leave `files/`.
pub(crate) fn library_file(root: &Path, stored: &str) -> Option<PathBuf> {
    match stored.split_once('/') {
        Some(("files", rest)) => plain_parts(root.join("files"), rest),
        _ => None,
    }
}

/// `base` joined with the `/`-separated parts of `rest`, each one a plain name.
fn plain_parts(mut path: PathBuf, rest: &str) -> Option<PathBuf> {
    for part in rest.split('/') {
        // One plain name per part: no "", ".", "..", drive letters or separators.
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(name)), None) if name == part => path.push(part),
            _ => return None,
        }
    }
    Some(path)
}

/// Removes a file and then its folder if that is now empty. A missing file is fine.
pub(crate) fn remove_with_empty_parent(file: &Path) -> Result<()> {
    match fs::remove_file(file) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("remove an unfinished file"),
    }
    if let Some(parent) = file.parent() {
        // Fails while the folder still has files; that is the point.
        let _ = fs::remove_dir(parent);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_file_stays_inside_files() {
        let root = Path::new("/lib");
        assert_eq!(
            library_file(root, "files/7Q/ID/a.jpg"),
            Some(root.join("files").join("7Q").join("ID").join("a.jpg"))
        );
        for stored in [
            "../x",
            "files/../x",
            "thumbs/a.webp",
            "/etc/passwd",
            "files/./a",
            "files",
            "",
        ] {
            assert_eq!(library_file(root, stored), None, "{stored}");
        }
    }

    #[test]
    fn the_journal_also_names_previews() {
        let root = Path::new("/lib");
        assert_eq!(
            journal_file(root, "thumbs/7Q/ID.webp"),
            Some(root.join("thumbs").join("7Q").join("ID.webp"))
        );
        assert_eq!(
            journal_file(root, "files/7Q/ID/a.age"),
            Some(root.join("files").join("7Q").join("ID").join("a.age"))
        );
        for stored in ["thumbs/../x", "thumbs//a", "other/a", "thumbs"] {
            assert_eq!(journal_file(root, stored), None, "{stored}");
        }
    }
}
