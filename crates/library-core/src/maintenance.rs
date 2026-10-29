//! Library settings and upkeep of the native library (docs/spec.md §7.4, item 6): backups, the
//! library check and the size on disk.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::ItemId;

/// Why a backup was made; read from its file name (docs/library-format.md §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BackupKind {
    /// When the library opened, at most once a day.
    Daily,
    /// Back Up Now.
    Manual,
    /// Before a migration to a newer format.
    BeforeMigration,
    /// The state just before another backup was restored.
    BeforeRestore,
}

/// A copy of `library.db` in `backups/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Backup {
    /// The file name; restoring names the backup by it.
    pub name: String,
    pub kind: BackupKind,
    /// ms
    pub created_at: i64,
    pub size: u64,
}

/// An item whose original is not in `files/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MissingOriginal {
    pub id: ItemId,
    /// Empty for an item of a locked folder that is not unlocked.
    pub name: String,
    pub in_trash: bool,
}

/// What Check Library found (docs/spec.md §9, phase 7).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryCheck {
    /// Items checked, the trash and locked folders included.
    pub items: u32,
    /// `PRAGMA quick_check` said "ok".
    pub database_ok: bool,
    /// Its first findings otherwise.
    pub database_problems: Vec<String>,
    pub missing_count: u32,
    /// The first 200 items without their original.
    pub missing: Vec<MissingOriginal>,
    /// Files in `files/` that no item points at.
    pub orphan_count: u32,
    pub orphan_bytes: u64,
    /// The first 200 of them, as paths inside the library folder.
    pub orphans: Vec<String>,
    /// Thumbnails of items that no longer exist.
    pub orphan_thumbnails: u32,
}

impl LibraryCheck {
    /// Nothing to repair.
    pub fn is_clean(&self) -> bool {
        self.database_ok
            && self.missing_count == 0
            && self.orphan_count == 0
            && self.orphan_thumbnails == 0
    }
}

/// What Repair removes after a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct RepairOptions {
    /// Files and thumbnails no item points at.
    pub orphans: bool,
    /// Items whose original is gone, removed for good.
    pub missing: bool,
}

/// Bytes on disk, for Library Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryStats {
    pub originals: u64,
    pub thumbnails: u64,
    /// `library.db` with its WAL.
    pub database: u64,
    pub backups: u64,
}

impl LibraryStats {
    pub fn total(&self) -> u64 {
        self.originals + self.thumbnails + self.database + self.backups
    }
}
