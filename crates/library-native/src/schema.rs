//! Format version, the schema of version 1 and forward migrations (docs/library-format.md §2–§4).

use library_core::SourceError;
use rusqlite::Connection;

use crate::error::{Context, Result};

/// ASCII `EGLB`: marks the SQLite file as an egl-viewer library (D-019).
pub const APPLICATION_ID: i32 = 0x4547_4C42;

/// The newest format version this build knows.
pub const FORMAT_VERSION: u32 = 1;

const SCHEMA_V1: &str = include_str!("schema/v1.sql");

/// One forward step, from `version - 1` to `version`.
pub(crate) struct Migration {
    pub version: u32,
    pub sql: &'static str,
}

/// Steps after version 1, oldest first. Empty until format version 2 exists.
pub(crate) const MIGRATIONS: &[Migration] = &[];

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

/// Creates version 1 in an empty database: the settings fixed at creation, then the schema.
pub(crate) fn create(conn: &Connection) -> Result<()> {
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
        "PRAGMA application_id = {APPLICATION_ID}; PRAGMA user_version = {FORMAT_VERSION};"
    ))
    .context("write the format version")?;
    conn.execute_batch(SCHEMA_V1).context("create the schema")
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

    fn normalized(sql: &str) -> String {
        sql.lines()
            .filter(|line| !line.trim_start().starts_with("--") || line.contains("CREATE"))
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned()
    }

    #[test]
    fn schema_matches_the_documented_format() {
        let doc = include_str!("../../../docs/library-format.md");
        let block = doc
            .split("```sql\n")
            .last()
            .and_then(|rest| rest.split("```").next())
            .expect("docs/library-format.md has a schema block");
        let code = SCHEMA_V1
            .lines()
            .skip_while(|line| line.starts_with("--") || line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            normalized(&code),
            normalized(block),
            "src/schema/v1.sql and docs/library-format.md §4 differ"
        );
    }

    #[test]
    fn creates_version_one_with_every_table() {
        let dir = tempfile::tempdir().unwrap();
        let conn = Connection::open(dir.path().join("library.db")).unwrap();
        create(&conn).unwrap();
        let header = read_header(&conn).unwrap();
        assert_eq!(header.application_id, APPLICATION_ID);
        assert_eq!(header.version, 1);
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
