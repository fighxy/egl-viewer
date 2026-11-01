//! Reading an Eagle `.library` folder on disk, read-only (docs/spec.md §8.2,
//! docs/architecture.md §15.6). Eagle may have the library open while it is read: its
//! `metadata.json` files are written in place, so every JSON read retries on a parse error
//! (correction 37). Eagle's cache in `%APPDATA%\Eagle` is never touched (correction 36), and
//! folder passwords are dropped while parsing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

/// What can go wrong while reading an Eagle library folder. Everything is an I/O or a parse
/// problem of one file; the import reports the file and goes on.
#[derive(Debug, thiserror::Error)]
pub enum EagleError {
    #[error("the folder does not exist: {0}")]
    NotFound(PathBuf),
    #[error("not an Eagle library: {0}")]
    NotAnEagleLibrary(String),
    #[error("cannot read {path}: {reason}")]
    Unreadable { path: PathBuf, reason: String },
    #[error("{path}: {reason}")]
    Invalid { path: PathBuf, reason: String },
}

impl EagleError {
    fn unreadable(path: &Path, error: impl std::fmt::Display) -> Self {
        Self::Unreadable {
            path: path.to_path_buf(),
            reason: error.to_string(),
        }
    }

    fn invalid(path: &Path, reason: impl Into<String>) -> Self {
        Self::Invalid {
            path: path.to_path_buf(),
            reason: reason.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, EagleError>;

/// An open Eagle `.library` folder. Reading only: the crate never writes anything.
#[derive(Debug, Clone)]
pub struct EagleLibrary {
    root: PathBuf,
    /// The folder name without `.library`.
    name: String,
}

/// What the import wizard shows for a picked folder (docs/spec.md §8.2 step 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySummary {
    pub name: String,
    /// `mtime.json` `all`: items including the trash.
    pub item_count: u32,
    /// Bytes of everything under `images/`.
    pub bytes: u64,
}

/// The library-level `metadata.json`: the folder tree, smart folders and tag groups.
#[derive(Debug, Clone, Default)]
pub struct LibraryMetadata {
    pub folders: Vec<FolderDto>,
    pub smart_folders: Vec<SmartFolderDto>,
    pub tag_groups: Vec<TagGroupDto>,
}

/// One entry of `mtime.json`, the manifest of what to read: the item id and its
/// `lastModified` on disk (kept for a later incremental re-import).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub id: String,
    pub last_modified: i64,
}

/// `mtime.json`: one entry per item plus `all`, the count including the trash.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    pub items: Vec<ManifestEntry>,
    pub total: u32,
}

/// A folder of the Eagle library, as stored in `metadata.json`. The `password` string is
/// turned into `is_locked` while parsing and is never kept (docs/spec.md §8.2, §8.6).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FolderDto {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub children: Vec<FolderDto>,
    /// The folder's own labels, not item tags.
    #[serde(default)]
    pub tags: Vec<String>,
    pub icon_color: Option<String>,
    pub cover_id: Option<String>,
    pub order_by: Option<String>,
    pub sort_increase: Option<bool>,
    #[serde(rename = "isUnLock", default)]
    pub is_unlock: Option<bool>,
    #[serde(
        rename = "password",
        default,
        deserialize_with = "non_empty",
        skip_serializing
    )]
    pub is_locked: bool,
}

/// A smart folder of the Eagle library; `conditions` is Eagle's rule JSON, kept unchanged
/// (docs/architecture.md §16).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SmartFolderDto {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub children: Vec<SmartFolderDto>,
    pub conditions: serde_json::Value,
    pub order_by: Option<String>,
    pub sort_increase: Option<bool>,
}

/// A tag group of the Eagle library; the array order is the display order.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TagGroupDto {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// One comment of an item (a rect on the image or a video time).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommentDto {
    pub id: String,
    #[serde(default)]
    pub annotation: String,
    pub x: Option<i64>,
    pub y: Option<i64>,
    pub w: Option<i64>,
    pub h: Option<i64>,
    pub at_seconds: Option<f64>,
    pub updated_at: Option<i64>,
}

/// An item of the Eagle library, as stored in `images/<id>.info/metadata.json`. Missing
/// optional fields stay `None`; Eagle omits keys until they have a value.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemDto {
    pub id: String,
    pub name: String,
    pub ext: String,
    pub size: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Absent until the item is rated.
    pub star: Option<u8>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Eagle folder ids; missing and `[]` are different cases for the live API, but the
    /// on-disk import only needs the ids.
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub palettes: Vec<serde_json::Value>,
    pub annotation: Option<String>,
    pub url: Option<String>,
    /// Date added, ms.
    pub modification_time: Option<i64>,
    pub btime: Option<i64>,
    pub mtime: Option<i64>,
    pub duration: Option<f64>,
    pub bpm: Option<f64>,
    /// EXIF strings (correction 38); kept as JSON, stored unchanged.
    pub raw_metas: Option<serde_json::Value>,
    /// Manual position per folder id (correction 39).
    pub order: Option<serde_json::Value>,
    #[serde(default)]
    pub comments: Vec<CommentDto>,
    pub no_thumbnail: Option<bool>,
    pub is_deleted: Option<bool>,
    /// On disk only: when Eagle last wrote this file, ms. Kept for a later re-import.
    pub last_modified: Option<i64>,
}

/// Turns a `password` string into a flag while parsing; the string itself is dropped here.
fn non_empty<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<bool, D::Error> {
    Ok(Option::<String>::deserialize(d)?.is_some_and(|s| !s.is_empty()))
}

/// How often a JSON file is read again after a parse error: Eagle writes these files in
/// place, so a reader can catch half-written JSON (correction 37).
const READ_ATTEMPTS: u32 = 5;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(150);

/// Reads and parses a JSON file, retrying on a parse error a few times with a short pause.
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let mut last = String::new();
    for attempt in 1..=READ_ATTEMPTS {
        let text = fs::read_to_string(path).map_err(|error| EagleError::unreadable(path, error))?;
        match serde_json::from_str(&text) {
            Ok(value) => return Ok(value),
            Err(error) => {
                last = error.to_string();
                if attempt < READ_ATTEMPTS {
                    std::thread::sleep(RETRY_DELAY);
                }
            }
        }
    }
    Err(EagleError::invalid(
        path,
        format!("not valid JSON after {READ_ATTEMPTS} reads: {last}"),
    ))
}

impl EagleLibrary {
    /// Opens the folder for reading after checking that it looks like an Eagle library:
    /// `metadata.json`, `mtime.json` and `images/` (docs/spec.md §8.2 step 1).
    pub fn open(root: &Path) -> Result<Self> {
        let root =
            std::path::absolute(root).map_err(|error| EagleError::unreadable(root, error))?;
        if !root.is_dir() {
            return Err(EagleError::NotFound(root));
        }
        let mut problems = Vec::new();
        if !root.join("metadata.json").is_file() {
            problems.push("metadata.json is missing");
        }
        if !root.join("mtime.json").is_file() {
            problems.push("mtime.json is missing");
        }
        if !root.join("images").is_dir() {
            problems.push("the images folder is missing");
        }
        if !problems.is_empty() {
            return Err(EagleError::NotAnEagleLibrary(format!(
                "{} ({})",
                root.display(),
                problems.join(", ")
            )));
        }
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Eagle library".into());
        let name = name.strip_suffix(".library").unwrap_or(&name).to_owned();
        Ok(Self { root, name })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// What the wizard shows: the library name, the item count from `mtime.json` and the
    /// bytes under `images/`. A missing or broken `mtime.json` is an error; a folder that
    /// cannot be walked counts its files as zero bytes.
    pub fn summary(&self) -> Result<LibrarySummary> {
        let manifest = self.manifest()?;
        let bytes = walk_bytes(&self.root.join("images"));
        Ok(LibrarySummary {
            name: self.name.clone(),
            item_count: manifest.total,
            bytes,
        })
    }

    /// The folder tree, smart folders and tag groups of `metadata.json`. Folder passwords
    /// are dropped while parsing: only `is_locked` survives.
    pub fn metadata(&self) -> Result<LibraryMetadata> {
        #[derive(Deserialize, Default)]
        #[serde(rename_all = "camelCase", default)]
        struct FileShape {
            #[serde(default)]
            folders: Vec<FolderDto>,
            #[serde(default, rename = "smartFolders")]
            smart_folders: Vec<SmartFolderDto>,
            #[serde(default, rename = "tagsGroups")]
            tag_groups: Vec<TagGroupDto>,
        }
        let path = self.root.join("metadata.json");
        let file: FileShape = read_json(&path)?;
        Ok(LibraryMetadata {
            folders: file.folders,
            smart_folders: file.smart_folders,
            tag_groups: file.tag_groups,
        })
    }

    /// `mtime.json`, the manifest of what to read: every item id with its `lastModified`.
    /// Entries without a numeric `lastModified` are skipped (the importer treats them as 0).
    pub fn manifest(&self) -> Result<Manifest> {
        let path = self.root.join("mtime.json");
        let raw: HashMap<String, serde_json::Value> = read_json(&path)?;
        let mut items = Vec::new();
        let mut total = 0;
        for (id, value) in raw {
            if id == "all" {
                total = value.as_u64().unwrap_or(0) as u32;
                continue;
            }
            let Some(last_modified) = value.as_i64() else {
                continue;
            };
            items.push(ManifestEntry { id, last_modified });
        }
        items.sort_by(|a, b| a.id.cmp(&b.id));
        if total == 0 {
            total = items.len() as u32;
        }
        Ok(Manifest { items, total })
    }

    /// Reads one item's `images/<id>.info/metadata.json`, with retries on a parse error.
    pub fn item(&self, id: &str) -> Result<ItemDto> {
        let path = self.item_dir(id).join("metadata.json");
        let mut item: ItemDto = read_json(&path)?;
        item.id = id.to_owned();
        Ok(item)
    }

    /// The item's original inside its `.info` directory, found by extension: the file name
    /// is not rebuilt, Eagle may have renamed the file (docs/spec.md §8.2). The thumbnail
    /// (`<name>_thumbnail.png`) never matches: its name always ends with `_thumbnail.png`.
    pub fn original(&self, item: &ItemDto) -> Option<PathBuf> {
        let wanted = item.ext.to_lowercase();
        let entries = fs::read_dir(self.item_dir(&item.id)).ok()?;
        entries.flatten().map(|entry| entry.path()).find(|path| {
            path.is_file()
                && path
                    .extension()
                    .map(|ext| ext.to_string_lossy().to_lowercase() == wanted)
                    .unwrap_or(false)
        })
    }

    /// The item's Eagle thumbnail (`_thumbnail.png`), when Eagle made one; small images
    /// often have none (`noThumbnail`, correction 29).
    pub fn thumbnail(&self, id: &str) -> Option<PathBuf> {
        let entries = fs::read_dir(self.item_dir(id)).ok()?;
        entries.flatten().map(|entry| entry.path()).find(|path| {
            path.is_file()
                && path
                    .file_name()
                    .map(|name| name.to_string_lossy().ends_with("_thumbnail.png"))
                    .unwrap_or(false)
        })
    }

    fn item_dir(&self, id: &str) -> PathBuf {
        self.root.join("images").join(format!("{id}.info"))
    }
}

/// The sum of every file's bytes under `dir`, best effort: folders that cannot be read
/// count as empty.
fn walk_bytes(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut bytes = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            bytes += walk_bytes(&path);
        } else {
            bytes += metadata.len();
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn folder_password_becomes_a_flag_and_is_not_kept() {
        let folder: FolderDto = serde_json::from_value(json!({
            "id": "F1",
            "name": "Secret",
            "password": "hunter2",
            "children": []
        }))
        .unwrap();
        assert!(folder.is_locked);
        let serialized = serde_json::to_string(&folder).unwrap();
        assert!(!serialized.contains("hunter2"));
        assert!(!serialized.contains("password"));
        assert!(!format!("{folder:?}").contains("hunter2"));
    }

    #[test]
    fn folder_without_password_is_not_locked() {
        let folder: FolderDto = serde_json::from_value(json!({
            "id": "F1",
            "name": "Open"
        }))
        .unwrap();
        assert!(!folder.is_locked);
        let empty: FolderDto = serde_json::from_value(json!({
            "id": "F2",
            "name": "Empty",
            "password": ""
        }))
        .unwrap();
        assert!(!empty.is_locked);
    }

    #[test]
    fn item_defaults_missing_fields() {
        let item: ItemDto = serde_json::from_value(json!({
            "id": "MVQ1",
            "name": "photo",
            "ext": "jpg",
            "size": 16228,
            "modificationTime": 1793113379762i64
        }))
        .unwrap();
        assert_eq!(item.name, "photo");
        assert_eq!(item.size, Some(16228));
        assert_eq!(item.star, None);
        assert!(item.tags.is_empty());
        assert!(item.folders.is_empty());
        assert!(item.comments.is_empty());
        assert!(!item.is_deleted.unwrap_or(false));
        assert_eq!(item.last_modified, None);
    }

    #[test]
    fn retrying_reader_recovers_from_a_truncated_write()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("metadata.json");
        fs::write(&path, "{\"name\": \"cut off")?;
        // Eagle writes these files in place: the truncated read is what a reader can catch,
        // and the finished write is what the retry must see (correction 37).
        let repair_path = path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            fs::write(repair_path, "{\"name\": \"whole\"}").unwrap();
        });
        #[derive(Deserialize)]
        struct Shape {
            name: String,
        }
        let parsed: Shape = read_json(&path)?;
        assert_eq!(parsed.name, "whole");
        Ok(())
    }

    #[test]
    fn a_file_that_stays_broken_is_an_invalid_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.json");
        fs::write(&path, "not json at all").unwrap();
        let result: std::result::Result<serde_json::Value, _> = read_json(&path);
        let Err(EagleError::Invalid { reason, .. }) = result else {
            panic!("expected an invalid error, got {result:?}");
        };
        assert!(reason.contains("not valid JSON"), "{reason}");
    }
}
