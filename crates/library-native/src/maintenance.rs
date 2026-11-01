//! Library Settings: backups and restoring one, Check Library and its repair, the size on disk
//! and the library's name (docs/spec.md §7.4 item 6, docs/library-format.md §8).

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use library_core::{
    Backup, LibraryCheck, LibraryStats, MissingOriginal, RepairOptions, SourceError,
};
use rusqlite::Connection;
use rusqlite::backup::StepResult;

use crate::DB_FILE;
use crate::backup;
use crate::edits::Removable;
use crate::error::{Context, Result};
use crate::library::{
    Library, check_header, identity, mark_open, migrate_after_backup, open_reader, quick_check,
};
use crate::name::validate_library_name;
use crate::recover::{library_file, remove_with_empty_parent};
use crate::schema::{self, FORMAT_VERSION, MIGRATIONS, meta_set};
use crate::time::now_ms;

/// Lists of findings stop here; the counts go on.
const LISTED: usize = 200;

impl Library {
    /// The backups in `backups/`, newest first.
    pub fn backups(&self) -> Result<Vec<Backup>> {
        backup::list(self.root())
    }

    /// Back Up Now: a copy of `library.db` in `backups/`, counted among the 10 kept.
    pub fn back_up_now(&self) -> Result<Backup> {
        self.check_writable()?;
        let name = backup::manual(&self.conn(), self.root(), now_ms())?;
        self.backups()?
            .into_iter()
            .find(|backup| backup.name == name)
            .ok_or_else(|| SourceError::Storage("the backup was not written".into()))
    }

    /// Replaces the library's metadata with a backup of this library. The current state is
    /// backed up first, so the restore can be taken back. The lock stays held: the backup's
    /// pages are copied into the open connection (SQLite's backup API). An older format is
    /// migrated after its own backup. Locks start locked again, and the add journal of the
    /// backup is dropped without deleting files: Check Library shows files no row points at.
    pub fn restore_backup(&self, name: &str) -> Result<()> {
        self.check_writable()?;
        if backup::kind_of(name).is_none() {
            return Err(SourceError::Invalid("Not a backup of this library.".into()));
        }
        let path = self.root().join("backups").join(name);
        if !path.is_file() {
            return Err(SourceError::NotFound);
        }
        let source = open_reader(&path, true)?;
        let header = check_header(schema::read_header(&source).ok(), FORMAT_VERSION)?;
        let (backup_id, _) = identity(&source)?;
        if backup_id != self.id() {
            return Err(SourceError::Invalid(
                "This backup belongs to another library.".into(),
            ));
        }
        quick_check(&source).map_err(|_| SourceError::Storage("The backup is damaged.".into()))?;

        let now = now_ms();
        let mut conn = self.conn();
        backup::before_restore(&conn, self.root(), now)?;
        copy_database(&source, &mut conn)?;
        drop(source);
        schema::configure_writer(&conn)?;
        mark_open(&conn, self.app_version(), now)?;
        migrate_after_backup(&mut conn, self.root(), header.version, MIGRATIONS, now)?;
        conn.execute("DELETE FROM pending_file", [])
            .context("clear the add journal")?;
        let (_, restored_name) = identity(&conn)?;
        *self.name() = restored_name;
        self.keys().clear();
        drop(conn);
        backup::prune(&self.root().join("backups"))
    }

    /// Library Settings → Name: `meta.name`. The folder keeps its name.
    pub fn rename_library(&self, name: &str) -> Result<()> {
        self.check_writable()?;
        let name = validate_library_name(name)?;
        meta_set(&self.conn(), "name", &name)?;
        *self.name() = name;
        Ok(())
    }

    /// Check Library: the database's own check, items whose original is missing, and files
    /// and thumbnails no item points at. The connection is held throughout, so no file can be
    /// added meanwhile (a new file is journaled first, which needs the connection).
    pub fn check_library(&self) -> Result<LibraryCheck> {
        let conn = self.conn();
        self.check_with(&conn)
    }

    /// Removes what a check found: files and thumbnails no item points at, and (visible)
    /// items whose original is gone. Returns a new check.
    pub fn repair_library(&self, options: RepairOptions) -> Result<LibraryCheck> {
        self.check_writable()?;
        if options.missing {
            let missing: Vec<String> = {
                let conn = self.conn();
                let (rows, _) = self.stored_rows(&conn)?;
                rows.into_iter()
                    .filter(|row| !self.original_exists(&row.file_path))
                    .map(|row| row.id)
                    .collect()
            };
            self.remove_items(
                &missing,
                Removable::Any,
                &|_, _| {},
                &std::sync::atomic::AtomicBool::new(false),
                &|| {},
            )?;
        }
        let conn = self.conn();
        if options.orphans {
            let (rows, pending) = self.stored_rows(&conn)?;
            let referenced = referenced(&rows, &pending);
            for path in walk_files(self.root())? {
                if !referenced.contains(&path.stored)
                    && let Some(file) = library_file(self.root(), &path.stored)
                {
                    remove_with_empty_parent(&file)?;
                }
            }
            let ids: HashSet<&str> = rows.iter().map(|row| row.id.as_str()).collect();
            for thumb in walk_thumbs(self.root())? {
                if !ids.contains(thumb.item.as_str()) {
                    match fs::remove_file(&thumb.path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == ErrorKind::NotFound => {}
                        Err(error) => return Err(error).context("remove a thumbnail"),
                    }
                }
            }
        }
        self.check_with(&conn)
    }

    fn check_with(&self, conn: &Connection) -> Result<LibraryCheck> {
        let mut check = LibraryCheck::default();
        let mut statement = conn
            .prepare("PRAGMA quick_check(20)")
            .context("check library.db")?;
        let findings: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .and_then(Iterator::collect)
            .context("check library.db")?;
        check.database_ok = findings.len() == 1 && findings[0] == "ok";
        if !check.database_ok {
            check.database_problems = findings;
        }

        let (rows, pending) = self.stored_rows(conn)?;
        check.items = u32::try_from(rows.len()).unwrap_or(u32::MAX);
        let keys = self.keys();
        for row in &rows {
            if self.original_exists(&row.file_path) {
                continue;
            }
            check.missing_count += 1;
            if check.missing.len() < LISTED {
                let name = if row.encrypted {
                    keys.cached_fields(&row.id)
                        .map(|fields| fields.name.clone())
                        .unwrap_or_default()
                } else {
                    row.name.clone()
                };
                check.missing.push(MissingOriginal {
                    id: library_core::ItemId(row.id.clone()),
                    name,
                    in_trash: row.in_trash,
                });
            }
        }
        drop(keys);

        let referenced = referenced(&rows, &pending);
        for file in walk_files(self.root())? {
            if referenced.contains(&file.stored) {
                continue;
            }
            check.orphan_count += 1;
            check.orphan_bytes += file.size;
            if check.orphans.len() < LISTED {
                check.orphans.push(file.stored);
            }
        }
        let ids: HashSet<&str> = rows.iter().map(|row| row.id.as_str()).collect();
        check.orphan_thumbnails = u32::try_from(
            walk_thumbs(self.root())?
                .iter()
                .filter(|thumb| !ids.contains(thumb.item.as_str()))
                .count(),
        )
        .unwrap_or(u32::MAX);
        Ok(check)
    }

    fn original_exists(&self, stored: &str) -> bool {
        library_file(self.root(), stored).is_some_and(|path| path.is_file())
    }

    /// Every item's stored file, and the journal.
    fn stored_rows(&self, conn: &Connection) -> Result<(Vec<StoredRow>, Vec<String>)> {
        let mut statement = conn
            .prepare("SELECT id, name, file_path, encrypted, deleted_at IS NOT NULL FROM item")
            .context("read items")?;
        let rows = statement
            .query_map([], |row| {
                Ok(StoredRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    file_path: row.get(2)?,
                    encrypted: row.get(3)?,
                    in_trash: row.get(4)?,
                })
            })
            .and_then(Iterator::collect)
            .context("read items")?;
        let mut statement = conn
            .prepare("SELECT path FROM pending_file")
            .context("read the journal")?;
        let pending = statement
            .query_map([], |row| row.get(0))
            .and_then(Iterator::collect)
            .context("read the journal")?;
        Ok((rows, pending))
    }

    /// Bytes on disk by part, for Library Settings.
    pub fn stats(&self) -> Result<LibraryStats> {
        let root = self.root();
        let database = [DB_FILE, "library.db-wal", "library.db-shm"]
            .iter()
            .filter_map(|name| fs::metadata(root.join(name)).ok())
            .map(|metadata| metadata.len())
            .sum();
        Ok(LibraryStats {
            originals: walk_files(root)?.iter().map(|file| file.size).sum(),
            thumbnails: folder_size(&root.join("thumbs"))?,
            database,
            backups: folder_size(&root.join("backups"))?,
        })
    }
}

struct StoredRow {
    id: String,
    name: String,
    file_path: String,
    encrypted: bool,
    in_trash: bool,
}

/// A file under `files/`, outside `files/.tmp`.
struct StoredFile {
    /// `files/…` with `/` separators, as rows store it.
    stored: String,
    size: u64,
}

/// A file under `thumbs/`, with the item id its name starts with.
struct ThumbFile {
    path: std::path::PathBuf,
    item: String,
}

fn referenced(rows: &[StoredRow], pending: &[String]) -> HashSet<String> {
    rows.iter()
        .map(|row| row.file_path.clone())
        .chain(pending.iter().cloned())
        .collect()
}

/// Every file under `files/` except the temp folder, which the next open empties.
fn walk_files(root: &Path) -> Result<Vec<StoredFile>> {
    let mut found = Vec::new();
    let base = root.join("files");
    let mut stack = vec![(base.clone(), "files".to_owned())];
    while let Some((dir, stored)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("list the library's files"),
        };
        for entry in entries {
            let entry = entry.context("list the library's files")?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let file_type = entry.file_type().context("list the library's files")?;
            if file_type.is_dir() {
                if !(dir == base && name == ".tmp") {
                    stack.push((path, format!("{stored}/{name}")));
                }
            } else if file_type.is_file() {
                let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
                found.push(StoredFile {
                    stored: format!("{stored}/{name}"),
                    size,
                });
            }
        }
    }
    Ok(found)
}

/// Thumbnails: `thumbs/<shard>/<id>.webp`, `<id>.age`, and leftovers of an interrupted write
/// (`.<id>.webp.tmp`).
fn walk_thumbs(root: &Path) -> Result<Vec<ThumbFile>> {
    let mut found = Vec::new();
    let shards = match fs::read_dir(root.join("thumbs")) {
        Ok(shards) => shards,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(found),
        Err(error) => return Err(error).context("list thumbnails"),
    };
    for shard in shards {
        let shard = shard.context("list thumbnails")?;
        if !shard.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        for entry in fs::read_dir(shard.path()).context("list thumbnails")? {
            let entry = entry.context("list thumbnails")?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let item = name
                .trim_start_matches('.')
                .split('.')
                .next()
                .unwrap_or_default()
                .to_owned();
            found.push(ThumbFile {
                path: entry.path(),
                item,
            });
        }
    }
    Ok(found)
}

fn folder_size(dir: &Path) -> Result<u64> {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("measure a folder"),
        };
        for entry in entries {
            let entry = entry.context("measure a folder")?;
            let file_type = entry.file_type().context("measure a folder")?;
            if file_type.is_dir() {
                stack.push(entry.path());
            } else {
                total += entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
            }
        }
    }
    Ok(total)
}

/// Copies every page of `source` into the open connection, in one step.
fn copy_database(source: &Connection, target: &mut Connection) -> Result<()> {
    let copy = rusqlite::backup::Backup::new(source, target).context("restore the backup")?;
    for _ in 0..50 {
        match copy.step(-1).context("restore the backup")? {
            StepResult::Done => return Ok(()),
            _ => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    Err(SourceError::Storage(
        "the library is busy; try the restore again".into(),
    ))
}
