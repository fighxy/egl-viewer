//! Copies of `library.db` in `backups/` (docs/library-format.md §8).

use std::fs;
use std::path::Path;

use rusqlite::Connection;

use crate::error::{Context, Result};
use crate::time::utc_date;

const KEEP: usize = 10;

/// At most one backup per UTC day, made on open.
pub(crate) fn daily(conn: &Connection, root: &Path, now: i64) -> Result<()> {
    write(conn, root, &format!("library-{}.db", utc_date(now)))
}

/// Before migrating from `from`. An existing file is the state before an earlier attempt that
/// failed, so it is kept.
pub(crate) fn before_migration(conn: &Connection, root: &Path, from: u32, now: i64) -> Result<()> {
    write(conn, root, &format!("library-{}-v{from}.db", utc_date(now)))
}

fn write(conn: &Connection, root: &Path, file_name: &str) -> Result<()> {
    let dir = root.join("backups");
    let target = dir.join(file_name);
    if target.exists() {
        return Ok(());
    }
    fs::create_dir_all(&dir).context("create the backups folder")?;
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy()])
        .context("write a backup")?;
    prune(&dir)
}

/// Keeps the newest `KEEP` backups. Names start with the date, so name order is age order.
fn prune(dir: &Path) -> Result<()> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .context("list backups")?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.starts_with("library-") && name.ends_with(".db"))
        .collect();
    names.sort();
    let excess = names.len().saturating_sub(KEEP);
    for name in &names[..excess] {
        fs::remove_file(dir.join(name)).context("remove an old backup")?;
    }
    Ok(())
}
