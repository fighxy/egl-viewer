//! Format versions, the schema and forward migrations (docs/library-format.md §2–§4).

use library_core::SourceError;
use rusqlite::Connection;

use crate::error::{Context, Result};

/// ASCII `EGLB`: marks the SQLite file as an egl-viewer library (D-019).
pub const APPLICATION_ID: i32 = 0x4547_4C42;

/// The newest format version this build reads and writes.
pub const FORMAT_VERSION: u32 = 3;

/// The newest schema: new libraries start here, and opening migrates older ones to it.
pub(crate) const SCHEMA_VERSION: u32 = 3;

// Version 2 had the schema of version 1: the first locked folder raised a library to it, so that
// an older app would not add plaintext files to a locked folder (D-019, D-045). Every library is
// at version 3 now before anything is written, which keeps older apps out just as well (D-047).

/// Version 1, the start of every library; the migrations bring it to the newest schema.
const SCHEMA_V1: &str = include_str!("schema/v1.sql");

/// One forward step, from `version - 1` to `version`.
pub(crate) struct Migration {
    pub version: u32,
    pub sql: &'static str,
}

/// Steps after version 1, oldest first. Version 2 changed no schema (it marks locks); version 3
/// rebuilds the search index for substring search (D-047).
pub(crate) const MIGRATIONS: &[Migration] = &[Migration {
    version: 3,
    sql: include_str!("schema/v3.sql"),
}];

/// What the SQLite header says about a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Header {
    pub application_id: i32,
    pub version: u32,
}

pub(crate) fn read_header(conn: &Connection) -> rusqlite::Result<Header> {
    let application_id = conn.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    let version = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(Header {
        application_id,
        version,
    })
}

/// Creates the newest version in an empty database: the settings fixed at creation, version 1,
/// then every migration (no backup: there is nothing to keep yet).
pub(crate) fn create(conn: &mut Connection) -> Result<()> {
    conn.execute_batch("PRAGMA page_size = 4096; PRAGMA auto_vacuum = INCREMENTAL;")
        .context("set the page size")?;
    let mode: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .context("enable WAL")?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(SourceError::Storage(format!(
            "WAL mode is not available here (journal mode {mode})"
        )));
    }
    conn.execute_batch(&format!(
        "PRAGMA application_id = {APPLICATION_ID}; PRAGMA user_version = 1;"
    ))
    .context("write the format version")?;
    conn.execute_batch(SCHEMA_V1).context("create the schema")?;
    migrate(conn, 1, MIGRATIONS)
}

/// The settings of every writer connection (docs/library-format.md §3).
pub(crate) fn configure_writer(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = FULL;")
        .context("configure the connection")?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .context("set the busy timeout")
}

/// Applies every step above `from`, one transaction per step.
pub(crate) fn migrate(conn: &mut Connection, from: u32, steps: &[Migration]) -> Result<()> {
    for step in steps.iter().filter(|step| step.version > from) {
        let what = format!("migrate to format version {}", step.version);
        let tx = conn.transaction().context(&what)?;
        tx.execute_batch(step.sql).context(&what)?;
        tx.pragma_update(None, "user_version", step.version)
            .context(&what)?;
        tx.commit().context(&what)?;
    }
    Ok(())
}

pub(crate) fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .optional()
    .context("read meta")
}

pub(crate) fn meta_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [key, value],
    )
    .context("write meta")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every schema object's SQL, comments dropped and spaces collapsed, by name.
    fn objects(conn: &Connection) -> Vec<(String, String)> {
        let mut statement = conn
            .prepare(
                "SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL \
                 AND name NOT LIKE 'item_fts_%' ORDER BY name",
            )
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get::<_, String>(1)?)))
            .unwrap()
            .map(|row| {
                let (name, sql) = row.unwrap();
                let plain: Vec<&str> = sql
                    .lines()
                    .map(|line| line.split("--").next().unwrap_or_default())
                    .collect();
                (
                    name,
                    plain
                        .join(" ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            })
            .collect()
    }

    #[test]
    fn schema_matches_the_documented_format() {
        let doc = include_str!("../../../docs/library-format.md");
        let block = doc
            .split("```sql\n")
            .last()
            .and_then(|rest| rest.split("```").next())
            .expect("docs/library-format.md has a schema block");
        let documented = Connection::open_in_memory().unwrap();
        documented.execute_batch(block).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut created = Connection::open(dir.path().join("library.db")).unwrap();
        create(&mut created).unwrap();
        assert_eq!(
            objects(&created),
            objects(&documented),
            "the schema of a new library and docs/library-format.md §4 differ"
        );
    }

    #[test]
    fn creates_the_newest_version_with_every_table() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = Connection::open(dir.path().join("library.db")).unwrap();
        create(&mut conn).unwrap();
        let header = read_header(&conn).unwrap();
        assert_eq!(header.application_id, APPLICATION_ID);
        assert_eq!(header.version, SCHEMA_VERSION);
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for table in [
            "meta",
            "item",
            "folder",
            "item_folder",
            "tag",
            "item_tag",
            "folder_tag",
            "tag_group",
            "tag_group_tag",
            "smart_folder",
            "comment",
            "change_log",
            "pending_file",
            "item_key_wrap",
            "folder_piv_wrap",
            "item_fts",
        ] {
            assert!(tables.iter().any(|name| name == table), "missing {table}");
        }
    }

    #[test]
    fn application_id_spells_eglb() {
        assert_eq!(&APPLICATION_ID.to_be_bytes(), b"EGLB");
    }
}
