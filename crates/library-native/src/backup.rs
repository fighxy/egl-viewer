//! Copies of `library.db` in `backups/` (docs/library-format.md §8).

use std::fs;
use std::path::Path;

use library_core::{Backup, BackupKind};
use rusqlite::Connection;

use crate::error::{Context, Result};
use crate::time::{system_ms, utc_date};

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

/// Back Up Now. Returns the file name.
pub(crate) fn manual(conn: &Connection, root: &Path, now: i64) -> Result<String> {
    let name = format!("library-{}.db", stamp(now));
    write(conn, root, &name)?;
    Ok(name)
}

/// The state just before a backup is restored, so the restore can be taken back. Not pruned
/// here: pruning could remove the backup about to be restored.
pub(crate) fn before_restore(conn: &Connection, root: &Path, now: i64) -> Result<String> {
    let name = format!("library-{}-restore.db", stamp(now));
    write_only(conn, root, &name)?;
    Ok(name)
}

/// `YYYY-MM-DD-HHMMSS` in UTC: sorts after the daily backup's date only within the same day.
fn stamp(now: i64) -> String {
    let seconds = now.div_euclid(1000).rem_euclid(86_400);
    format!(
        "{}-{:02}{:02}{:02}",
        utc_date(now),
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

fn write(conn: &Connection, root: &Path, file_name: &str) -> Result<()> {
    write_only(conn, root, file_name)?;
    prune(&root.join("backups"))
}

fn write_only(conn: &Connection, root: &Path, file_name: &str) -> Result<()> {
    let dir = root.join("backups");
    let target = dir.join(file_name);
    if target.exists() {
        return Ok(());
    }
    fs::create_dir_all(&dir).context("create the backups folder")?;
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy()])
        .context("write a backup")?;
    Ok(())
}

/// The backups in `backups/`, newest first.
pub(crate) fn list(root: &Path) -> Result<Vec<Backup>> {
    let dir = root.join("backups");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("list backups"),
    };
    let mut backups = Vec::new();
    for entry in entries {
        let entry = entry.context("list backups")?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Some(kind) = kind_of(&name) else {
            continue;
        };
        let metadata = entry.metadata().context("list backups")?;
        if !metadata.is_file() {
            continue;
        }
        backups.push(Backup {
            name,
            kind,
            created_at: metadata.modified().ok().and_then(system_ms).unwrap_or(0),
            size: metadata.len(),
        });
    }
    backups.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.name.cmp(&a.name))
    });
    Ok(backups)
}

/// What a backup's file name says about it; `None` for files that are not backups.
pub(crate) fn kind_of(name: &str) -> Option<BackupKind> {
    let rest = name.strip_prefix("library-")?.strip_suffix(".db")?;
    let (date, tail) = rest.split_at_checked(10)?;
    let is_date = date.bytes().enumerate().all(|(at, byte)| match at {
        4 | 7 => byte == b'-',
        _ => byte.is_ascii_digit(),
    });
    if !is_date || name.contains(['/', '\\']) {
        return None;
    }
    Some(if tail.is_empty() {
        BackupKind::Daily
    } else if tail.starts_with("-v") {
        BackupKind::BeforeMigration
    } else if tail.ends_with("-restore") {
        BackupKind::BeforeRestore
    } else {
        BackupKind::Manual
    })
}

/// Keeps the newest `KEEP` backups. Names start with the date, so name order is age order.
pub(crate) fn prune(dir: &Path) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_tell_the_kind() {
        assert_eq!(kind_of("library-2026-10-29.db"), Some(BackupKind::Daily));
        assert_eq!(
            kind_of("library-2026-10-29-v2.db"),
            Some(BackupKind::BeforeMigration)
        );
        assert_eq!(
            kind_of("library-2026-10-29-143012.db"),
            Some(BackupKind::Manual)
        );
        assert_eq!(
            kind_of("library-2026-10-29-143012-restore.db"),
            Some(BackupKind::BeforeRestore)
        );
        for name in [
            "library.db",
            "library-x.db",
            "library-2026-10-29.txt",
            "notes.db",
        ] {
            assert_eq!(kind_of(name), None, "{name}");
        }
    }

    #[test]
    fn stamps_are_utc_times_safe_in_file_names() {
        // 2026-10-29 14:30:12 UTC
        assert_eq!(stamp(1_793_284_212_000), "2026-10-29-143012");
    }
}
