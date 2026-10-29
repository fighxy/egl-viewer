//! Library data shared by every source (docs/architecture.md §5, §15).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::query::SortKey;

/// Opaque item id: an Eagle id for the Eagle sources, a ULID for the native library.
/// A newtype, so serde writes it as a plain string.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize, TS,
)]
#[ts(export)]
pub struct ItemId(pub String);

impl From<&str> for ItemId {
    fn from(id: &str) -> Self {
        Self(id.to_owned())
    }
}

/// Which kind of library a source serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LibraryKind {
    /// A running Eagle, through its HTTP API.
    EagleApi,
    /// An Eagle `.library` folder on disk, read only.
    EagleFolder,
    /// The app's own library format.
    Native,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryInfo {
    pub kind: LibraryKind,
    /// The native library's `meta.library_id`; a copy of the folder keeps it.
    pub id: Option<String>,
    pub name: String,
    /// Path of the library on the computer that holds it, when known.
    pub path: Option<String>,
    /// Changes when folder or smart folder metadata changes (Eagle `modificationTime`, ms).
    pub modification_time: Option<i64>,
    pub read_only: bool,
    /// Why a native library opened read-only; the UI explains it in a banner.
    pub read_only_reason: Option<ReadOnlyReason>,
    /// Who has the library open for writing, when another instance does.
    pub holder: Option<LibraryHolder>,
}

/// Why a native library is open read-only (docs/library-format.md §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReadOnlyReason {
    /// Another egl-viewer holds the library's lock.
    OtherInstance,
    /// The library sits inside a cloud-synced folder (D-021).
    SyncFolder,
    /// The lock file cannot be opened for writing: read-only medium or no permission.
    ReadOnlyMedium,
}

/// The writer named in the native library's `meta.open_by`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryHolder {
    pub computer: String,
    /// When that instance opened the library, ms.
    pub since: i64,
}

/// A folder node. The parent is implied by nesting: Eagle's `folder/get` has no `parent` field
/// (correction 8). Passwords never reach this type, only `is_locked` (§5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon_color: Option<String>,
    /// The folder's own labels, not item tags.
    pub tags: Vec<String>,
    /// Folder tags plus ancestor tags, computed by the source; display only.
    pub extend_tags: Vec<String>,
    pub children: Vec<Folder>,
    /// Items directly in the folder; `None` when the source has not counted yet (correction 32).
    pub image_count: Option<u32>,
    /// Items in the folder and all descendants, own items included.
    pub descendant_image_count: Option<u32>,
    pub is_locked: bool,
    pub is_unlocked: bool,
    /// The lock's tag of a locked folder: a caption and a colour, plaintext (D-018). Not an
    /// item tag.
    pub lock_label: Option<String>,
    pub lock_color: Option<String>,
    pub cover_item_id: Option<String>,
    /// The folder's own sort (docs/spec.md §5); `None` follows the global sort.
    pub order_by: Option<SortKey>,
    pub sort_ascending: Option<bool>,
}

/// The recovery secret of a new lock, shown once and never stored (docs/age-locked-folders.md
/// §7): the key, and the same bits as 12 words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecoveryKey {
    pub key: String,
    pub phrase: String,
}

/// One colour of an item's palette, as Eagle stores it: `{ color: [r, g, b], ratio }`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Palette {
    pub color: [u8; 3],
    /// Share of the image in percent; can be fractional.
    pub ratio: f64,
}

/// What snapshots keep and the grid needs. Never crosses IPC as a whole list (§6.5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemLite {
    pub id: ItemId,
    pub name: String,
    pub ext: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size: Option<u64>,
    /// 0 when unrated: Eagle omits the key until a rating is set.
    pub star: u8,
    /// Interned: large libraries repeat the same tag strings many times.
    pub tags: Vec<Arc<str>>,
    /// `None` and an empty list are different cases in Eagle (correction 15).
    pub folders: Option<Vec<String>>,
    pub palette0: Option<[u8; 3]>,
    /// Date added, ms (Eagle `modificationTime`).
    pub added_at: Option<i64>,
    /// Eagle `btime`, ms.
    pub file_created_at: Option<i64>,
    /// Eagle `mtime`, ms.
    pub file_modified_at: Option<i64>,
    /// When the item went to the trash, ms; `None` outside the trash. Eagle has only a flag
    /// and gives `Some(0)`.
    pub deleted_at: Option<i64>,
    pub duration: Option<f64>,
    /// No thumbnail to show: the native library has not made one (yet), Eagle stored none.
    pub no_thumbnail: bool,
    /// Changes when the thumbnail changes; part of its URL (native: `item.thumb_version`; Eagle:
    /// `modificationTime`).
    pub thumb_version: i64,
    /// The position in the open folder, for the Manual sort (native: `item_folder.position`;
    /// Eagle: `order[folderId]`); `None` outside a folder.
    pub position: Option<f64>,
}

/// The full item for the detail panel.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Item {
    pub id: ItemId,
    pub name: String,
    pub ext: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size: Option<u64>,
    pub star: u8,
    pub tags: Vec<String>,
    pub folders: Vec<String>,
    pub annotation: String,
    pub url: String,
    pub palettes: Vec<Palette>,
    pub added_at: Option<i64>,
    pub file_created_at: Option<i64>,
    pub file_modified_at: Option<i64>,
    pub duration: Option<f64>,
    pub bpm: Option<f64>,
    /// EXIF strings (Eagle `rawMetas`, correction 38).
    pub exif: Option<serde_json::Value>,
    pub no_thumbnail: bool,
    /// Part of the thumbnail URL, as in [`ItemLite::thumb_version`].
    pub thumb_version: i64,
    pub is_deleted: bool,
    /// When the item went to the trash, ms, when the source knows it.
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Tag {
    /// The tag's identity is the exact, case-sensitive string.
    pub name: String,
    /// Items carrying the tag, trash excluded, when the source knows it.
    pub image_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TagGroup {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SmartFolder {
    pub id: String,
    pub name: String,
    pub children: Vec<SmartFolder>,
    /// Eagle's rule language, kept unchanged (docs/architecture.md §16).
    pub conditions: serde_json::Value,
    pub order_by: Option<String>,
    pub sort_ascending: Option<bool>,
    /// Rules this source cannot evaluate, as `property method`: they never match, and the UI
    /// warns about them.
    #[serde(default)]
    pub unsupported: Vec<String>,
}

/// One file extension of the library and how many items outside the trash have it, for the
/// Type filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExtensionCount {
    /// Lowercase, without the dot.
    pub ext: String,
    pub count: u32,
}

/// Image bytes served to the WebView through the `thumb` URI scheme (§11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    pub bytes: Vec<u8>,
    pub content_type: String,
    /// Decrypted from a locked folder: the WebView must not keep it in its cache.
    pub private: bool,
}

/// Where a change feed continues from; opaque to everyone but its source.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeToken(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Changes {
    pub changed: Vec<ItemId>,
    pub removed: Vec<ItemId>,
    pub next: ChangeToken,
}

/// A file to add, by path on the computer that runs the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewFile {
    pub path: String,
    /// Defaults to the file name without extension.
    pub name: Option<String>,
}

/// Where and how added files land.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AddTarget {
    pub folders: Vec<String>,
    pub tags: Vec<String>,
    pub star: Option<u8>,
    pub annotation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ImportTicket {
    pub id: u64,
}

/// What an import task is doing (docs/spec.md §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ImportState {
    /// Walking dropped folders.
    Scanning,
    /// Copying files into the library.
    Adding,
    /// The first duplicate of the batch waits for the user's choice.
    WaitingForDecision,
    /// Every file is in; thumbnails and palettes are still being made.
    Previews,
    Done,
    Cancelled,
}

/// Files the user added, or previews that the library makes after a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ImportKind {
    Files,
    Previews,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportFailure {
    pub path: String,
    pub reason: String,
}

/// A file whose content is already in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicateNotice {
    pub path: String,
    /// The id of the item that has the same content.
    pub existing: String,
    pub in_trash: bool,
    /// The folder the file was going into; the existing item can join it instead.
    pub target_folder: Option<String>,
}

/// The state of one import task, sent to the UI as it changes (`import-progress` events).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportProgress {
    pub id: u64,
    pub kind: ImportKind,
    pub state: ImportState,
    /// Files found; known once scanning ends.
    pub total: u32,
    /// Files handled so far: added, skipped as duplicates or failed.
    pub processed: u32,
    pub added: u32,
    pub duplicates: u32,
    pub failed: u32,
    pub previews_total: u32,
    pub previews_done: u32,
    /// The file being copied.
    pub current: Option<String>,
    /// The duplicate that waits for a decision.
    pub duplicate: Option<DuplicateNotice>,
    /// Failures and files without a preview, at most the first 200.
    pub failures: Vec<ImportFailure>,
    /// Duplicates that were skipped, at most the first 200.
    pub skipped: Vec<DuplicateNotice>,
    /// ms
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

/// Items per fixed row of the left bar (docs/spec.md §7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryCounts {
    pub all: u32,
    pub uncategorized: u32,
    pub untagged: u32,
    pub trash: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn folder_uses_camel_case_and_fills_missing_fields() {
        let folder: Folder = serde_json::from_value(json!({
            "id": "F1",
            "name": "Posters",
            "iconColor": "red",
            "imageCount": 3,
            "children": [{ "id": "F2", "name": "Child" }]
        }))
        .unwrap();
        assert_eq!(folder.icon_color.as_deref(), Some("red"));
        assert_eq!(folder.image_count, Some(3));
        assert_eq!(folder.descendant_image_count, None);
        assert_eq!(folder.children[0].name, "Child");
        assert!(folder.children[0].children.is_empty());
        assert!(!folder.is_locked);
    }

    #[test]
    fn palette_matches_eagle_shape() {
        let palette: Palette =
            serde_json::from_value(json!({ "color": [142, 83, 55], "ratio": 3.25 })).unwrap();
        assert_eq!(palette.color, [142, 83, 55]);
        assert!((palette.ratio - 3.25).abs() < f64::EPSILON);
    }

    #[test]
    fn item_id_is_a_plain_string_on_the_wire() {
        assert_eq!(
            serde_json::to_value(ItemId::from("MVQ1")).unwrap(),
            json!("MVQ1")
        );
    }
}
