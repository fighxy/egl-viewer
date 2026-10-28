//! An open native library: the writer with its lock, or a read-only reader
//! (docs/library-format.md §5, §7).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use library_core::{LibraryHolder, LibraryInfo, LibraryKind, ReadOnlyReason, SourceError};
use rusqlite::{Connection, OpenFlags};

use crate::DB_FILE;
use crate::backup;
use crate::detect::{looks_like_eagle_library, raw_header};
use crate::error::{Context, Result};
use crate::lock::{LibraryLock, LockAttempt};
use crate::locks::Keyring;
use crate::recover;
use crate::schema::{
    self, APPLICATION_ID, FORMAT_VERSION, Header, MIGRATIONS, Migration, SCHEMA_VERSION, meta_get,
    meta_set,
};
use crate::time::now_ms;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOptions {
    /// Written to `meta.last_written_with` and `meta.open_by`.
    pub app_version: String,
    /// The caller found the library inside a cloud-synced folder: open it read-only and
    /// immutable, so SQLite writes nothing there (D-021).
    pub sync_folder: bool,
}

impl OpenOptions {
    pub fn new(app_version: impl Into<String>) -> Self {
        Self {
            app_version: app_version.into(),
            sync_folder: false,
        }
    }
}

/// An open library. Dropping it closes it like [`Library::close`], ignoring errors.
#[derive(Debug)]
pub struct Library {
    root: PathBuf,
    // Declared before `lock`: the connection closes before the lock is released.
    conn: Mutex<Connection>,
    lock: Option<LibraryLock>,
    id: String,
    name: String,
    read_only: Option<ReadOnlyReason>,
    holder: Option<LibraryHolder>,
    unclean_close: bool,
    closed: bool,
    /// Keys of folders unlocked in this session (docs/age-locked-folders.md §4).
    keys: Mutex<Keyring>,
}

impl Library {
    /// Opens the library folder `root` (docs/library-format.md §7).
    pub fn open(root: &Path, options: &OpenOptions) -> Result<Self> {
        Self::open_with(root, options, MIGRATIONS)
    }

    pub(crate) fn open_with(
        root: &Path,
        options: &OpenOptions,
        migrations: &[Migration],
    ) -> Result<Self> {
        let newest = migrations
            .last()
            .map_or(FORMAT_VERSION, |step| step.version.max(FORMAT_VERSION));
        let schema = schema_target(migrations);
        let root = std::path::absolute(root).context("resolve the library path")?;
        if !root.is_dir() {
            return Err(SourceError::NotFound);
        }
        let db = root.join(DB_FILE);
        if !db.is_file() {
            return Err(SourceError::NotALibrary {
                eagle: looks_like_eagle_library(&root),
            });
        }
        // First from the file's bytes, so a refused file gets no -wal or -shm; then through
        // SQLite, which also sees a WAL.
        check_header(raw_header(&db), newest)?;
        let reader = open_reader(&db, options.sync_folder)?;
        let header = check_header(schema::read_header(&reader).ok(), newest)?;
        if options.sync_folder {
            return Self::reader(root, reader, header, schema, ReadOnlyReason::SyncFolder);
        }
        let lock = match LibraryLock::try_acquire(&root) {
            LockAttempt::Acquired(lock) => lock,
            LockAttempt::Held => {
                return Self::reader(root, reader, header, schema, ReadOnlyReason::OtherInstance);
            }
            LockAttempt::Unavailable => {
                return Self::reader(root, reader, header, schema, ReadOnlyReason::ReadOnlyMedium);
            }
        };
        drop(reader);
        Self::writer(root, &db, lock, header.version, options, migrations)
    }

    fn reader(
        root: PathBuf,
        conn: Connection,
        header: Header,
        schema: u32,
        reason: ReadOnlyReason,
    ) -> Result<Self> {
        if header.version < schema {
            return Err(SourceError::NotSupported(
                "this library needs an upgrade, which only an instance that can write it can do"
                    .into(),
            ));
        }
        let (id, name) = identity(&conn)?;
        let holder = if reason == ReadOnlyReason::OtherInstance {
            meta_get(&conn, "open_by")?
                .as_deref()
                .and_then(parse_holder)
        } else {
            None
        };
        Ok(Self {
            root,
            conn: Mutex::new(conn),
            lock: None,
            id,
            name,
            read_only: Some(reason),
            holder,
            unclean_close: false,
            closed: false,
            keys: Mutex::default(),
        })
    }

    fn writer(
        root: PathBuf,
        db: &Path,
        lock: LibraryLock,
        version: u32,
        options: &OpenOptions,
        migrations: &[Migration],
    ) -> Result<Self> {
        let mut conn = Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("open library.db")?;
        schema::configure_writer(&conn)?;

        let unclean_close = meta_get(&conn, "open_by")?.is_some();
        if unclean_close {
            tracing::warn!("the library was not closed properly last time; checking it");
            quick_check(&conn)?;
        }
        let now = now_ms();
        let open_by = serde_json::json!({
            "computer": computer_name(),
            "pid": std::process::id(),
            "since": now,
            "app": options.app_version,
        });
        meta_set(&conn, "open_by", &open_by.to_string())?;
        meta_set(&conn, "last_written_with", &options.app_version)?;

        if version < schema_target(migrations) {
            backup::before_migration(&conn, &root, version, now)?;
            schema::migrate(&mut conn, version, migrations)?;
        }
        let recovered = recover::pending_files(&conn, &root)?;
        if recovered > 0 {
            tracing::warn!("resolved {recovered} unfinished file additions");
        }
        recover::empty_tmp(&root)?;
        backup::daily(&conn, &root, now)?;
        let (id, name) = identity(&conn)?;
        Ok(Self {
            root,
            conn: Mutex::new(conn),
            lock: Some(lock),
            id,
            name,
            read_only: None,
            holder: None,
            unclean_close,
            closed: false,
            keys: Mutex::default(),
        })
    }

    pub fn info(&self) -> LibraryInfo {
        LibraryInfo {
            kind: LibraryKind::Native,
            id: Some(self.id.clone()),
            name: self.name.clone(),
            path: Some(self.root.to_string_lossy().into_owned()),
            modification_time: None,
            read_only: self.read_only.is_some(),
            read_only_reason: self.read_only,
            holder: self.holder.clone(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn read_only_reason(&self) -> Option<ReadOnlyReason> {
        self.read_only
    }

    /// The previous writer did not close the library (a crash or a kill).
    pub fn was_closed_uncleanly(&self) -> bool {
        self.unclean_close
    }

    /// Checkpoints the WAL, removes `meta.open_by` and releases the lock.
    pub fn close(mut self) -> Result<()> {
        self.shutdown()
    }

    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Lock order: the connection first, then the keys; never the other way round.
    pub(crate) fn keyring(&self) -> &Mutex<Keyring> {
        &self.keys
    }

    pub(crate) fn check_writable(&self) -> Result<()> {
        match self.read_only {
            Some(_) => Err(SourceError::ReadOnly),
            None => Ok(()),
        }
    }

    fn shutdown(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.keys
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        if self.lock.is_some() {
            let conn = self.conn.get_mut().unwrap_or_else(PoisonError::into_inner);
            conn.execute("DELETE FROM meta WHERE key = 'open_by'", [])
                .context("clear meta.open_by")?;
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
                .context("checkpoint the WAL")?;
        }
        Ok(())
    }

    /// Ends this instance like a crash: the lock goes, `meta.open_by` stays.
    #[cfg(test)]
    pub(crate) fn abandon(mut self) {
        self.closed = true;
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            tracing::warn!("closing the library failed: {error}");
        }
    }
}

/// The schema version opening must reach: the last migration, or version 1.
fn schema_target(migrations: &[Migration]) -> u32 {
    migrations
        .last()
        .map_or(SCHEMA_VERSION, |step| step.version)
}

/// A read-only connection for the header check, and for read-only instances.
fn open_reader(db: &Path, immutable: bool) -> Result<Connection> {
    let query = if immutable {
        "mode=ro&immutable=1"
    } else {
        "mode=ro"
    };
    Connection::open_with_flags(
        sqlite_uri(db, query),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(not_a_library_or_storage)
}

/// A `file:` URI for SQLite, with the characters that URIs reserve escaped.
fn sqlite_uri(path: &Path, query: &str) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from(if raw.starts_with('/') {
        "file://"
    } else {
        "file:///"
    });
    for c in raw.chars() {
        match c {
            '%' => uri.push_str("%25"),
            '?' => uri.push_str("%3f"),
            '#' => uri.push_str("%23"),
            ' ' => uri.push_str("%20"),
            c => uri.push(c),
        }
    }
    uri.push('?');
    uri.push_str(query);
    uri
}

fn check_header(header: Option<Header>, newest: u32) -> Result<Header> {
    match header {
        Some(header) if header.application_id == APPLICATION_ID && header.version > 0 => {
            if header.version > newest {
                Err(SourceError::NewerFormat {
                    version: header.version,
                })
            } else {
                Ok(header)
            }
        }
        _ => Err(SourceError::NotALibrary { eagle: false }),
    }
}

fn not_a_library_or_storage(error: rusqlite::Error) -> SourceError {
    match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::NotADatabase) => SourceError::NotALibrary { eagle: false },
        _ => SourceError::Storage(format!("read library.db: {error}")),
    }
}

fn identity(conn: &Connection) -> Result<(String, String)> {
    let id = meta_get(conn, "library_id")?;
    let name = meta_get(conn, "name")?;
    match (id, name) {
        (Some(id), Some(name)) => Ok((id, name)),
        _ => Err(SourceError::Storage(
            "library.db has no library id or name".into(),
        )),
    }
}

fn quick_check(conn: &Connection) -> Result<()> {
    let result: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .context("check library.db")?;
    if result == "ok" {
        Ok(())
    } else {
        Err(SourceError::Storage(format!(
            "library.db is damaged ({result}); restore a copy from the backups folder"
        )))
    }
}

fn parse_holder(open_by: &str) -> Option<LibraryHolder> {
    let value: serde_json::Value = serde_json::from_str(open_by).ok()?;
    Some(LibraryHolder {
        computer: value.get("computer")?.as_str()?.to_owned(),
        since: value.get("since")?.as_i64()?,
    })
}

fn computer_name() -> String {
    ["COMPUTERNAME", "HOSTNAME"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|name| !name.is_empty()))
        .unwrap_or_else(|| "this computer".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_uris_escape_reserved_characters() {
        assert_eq!(
            sqlite_uri(Path::new("/home/a b/lib#1?.eglib/library.db"), "mode=ro"),
            "file:///home/a%20b/lib%231%3f.eglib/library.db?mode=ro"
        );
        assert_eq!(
            sqlite_uri(Path::new(r"C:\Users\x\100% refs\library.db"), "mode=ro"),
            "file:///C:/Users/x/100%25%20refs/library.db?mode=ro"
        );
    }

    #[test]
    fn parses_the_holder() {
        assert_eq!(
            parse_holder(r#"{"computer":"DESKTOP-1","pid":7,"since":1700000000000,"app":"0.1.0"}"#),
            Some(LibraryHolder {
                computer: "DESKTOP-1".into(),
                since: 1_700_000_000_000
            })
        );
        assert_eq!(parse_holder("not json"), None);
    }
}
