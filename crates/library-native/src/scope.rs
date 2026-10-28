//! Reads for the views and the detail panel: one SQL query per scope, the full item, the tag
//! vocabulary, and the bytes the `thumb` scheme serves (docs/architecture.md §11, §15.3).

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use library_core::{Item, ItemId, ItemLite, Palette, Scope, SourceError, Tag, Thumbnail};
use rusqlite::{OptionalExtension, Row, params};

use crate::error::{Context, Result};
use crate::library::Library;

/// The columns of [`ItemLite`], in the order `lite_from_row` reads them.
const LITE_COLUMNS: &str = "id, name, ext, width, height, size, star, \
     json_extract(palette, '$[0].color[0]'), json_extract(palette, '$[0].color[1]'), \
     json_extract(palette, '$[0].color[2]'), \
     added_at, file_created_at, file_modified_at, duration, thumb_version";

impl Library {
    /// The light items of one scope (docs/architecture.md §15.3). A folder scope also gives
    /// each direct member its manual position. Smart folders arrive with phase 6.
    pub fn load_scope(&self, scope: &Scope) -> Result<Vec<ItemLite>> {
        let mut folder: Option<&str> = None;
        let condition = match scope {
            Scope::All => "deleted_at IS NULL",
            Scope::Folder {
                id,
                include_descendants,
            } => {
                let known: Option<i64> = self
                    .conn()
                    .query_row("SELECT 1 FROM folder WHERE id = ?1", params![id], |row| {
                        row.get(0)
                    })
                    .optional()
                    .context("find the folder")?;
                if known.is_none() {
                    return Err(SourceError::NotFound);
                }
                folder = Some(id);
                if *include_descendants {
                    "deleted_at IS NULL AND EXISTS (SELECT 1 FROM item_folder \
                     WHERE item_id = item.id AND folder_id IN (SELECT id FROM sub))"
                } else {
                    "deleted_at IS NULL AND EXISTS (SELECT 1 FROM item_folder \
                     WHERE item_id = item.id AND folder_id = ?1)"
                }
            }
            Scope::Unfiled => {
                "deleted_at IS NULL \
                 AND NOT EXISTS (SELECT 1 FROM item_folder WHERE item_id = item.id)"
            }
            Scope::Untagged => {
                "deleted_at IS NULL \
                 AND NOT EXISTS (SELECT 1 FROM item_tag WHERE item_id = item.id)"
            }
            Scope::Trash => "deleted_at IS NOT NULL",
            Scope::Smart { .. } => return Err(SourceError::NotSupported("smart folders".into())),
        };
        let conn = self.conn();
        let mut items = Vec::new();
        let mut positions = HashMap::new();
        // The folder and its subfolders, for the scope with subfolders.
        let prefix = if folder.is_some() {
            "WITH RECURSIVE sub(id) AS (SELECT ?1 \
             UNION SELECT folder.id FROM folder JOIN sub ON folder.parent_id = sub.id) "
        } else {
            ""
        };
        let mut statement = conn
            .prepare(&format!(
                "{prefix}SELECT {LITE_COLUMNS} FROM item WHERE {condition} ORDER BY seq"
            ))
            .context("load a scope")?;
        let mut rows = match folder {
            Some(id) => statement.query(params![id]),
            None => statement.query([]),
        }
        .context("load a scope")?;
        while let Some(row) = rows.next().context("load a scope")? {
            let item = lite_from_row(row).context("read an item")?;
            positions.insert(item.id.0.clone(), items.len());
            items.push(item);
        }

        // Tags, interned: a large library repeats the same names many times.
        let mut names: HashMap<i64, Arc<str>> = HashMap::new();
        let mut statement = conn
            .prepare(
                "SELECT item_tag.item_id, tag.id, tag.name FROM item_tag \
                 JOIN tag ON tag.id = item_tag.tag_id ORDER BY tag.name",
            )
            .context("load tags")?;
        let mut rows = statement.query([]).context("load tags")?;
        while let Some(row) = rows.next().context("load tags")? {
            let item_id: String = row.get(0).context("read a tag")?;
            let Some(&position) = positions.get(&item_id) else {
                continue;
            };
            let tag_id: i64 = row.get(1).context("read a tag")?;
            let name = match names.get(&tag_id) {
                Some(name) => Arc::clone(name),
                None => {
                    let name: Arc<str> = row.get::<_, String>(2).context("read a tag")?.into();
                    names.insert(tag_id, Arc::clone(&name));
                    name
                }
            };
            items[position].tags.push(name);
        }

        let mut statement = conn
            .prepare("SELECT item_id, folder_id FROM item_folder")
            .context("load folders")?;
        let mut rows = statement.query([]).context("load folders")?;
        while let Some(row) = rows.next().context("load folders")? {
            let item_id: String = row.get(0).context("read a folder link")?;
            if let Some(&position) = positions.get(&item_id) {
                let folder: String = row.get(1).context("read a folder link")?;
                items[position].folders.get_or_insert_default().push(folder);
            }
        }

        if let Some(folder) = folder {
            let mut statement = conn
                .prepare(
                    "SELECT item_id, position FROM item_folder
                     WHERE folder_id = ?1 AND position IS NOT NULL",
                )
                .context("load positions")?;
            let mut rows = statement.query(params![folder]).context("load positions")?;
            while let Some(row) = rows.next().context("load positions")? {
                let item_id: String = row.get(0).context("read a position")?;
                if let Some(&at) = positions.get(&item_id) {
                    items[at].position = Some(row.get(1).context("read a position")?);
                }
            }
        }
        Ok(items)
    }

    /// The full item for the detail panel, also from the trash.
    pub fn item(&self, id: &str) -> Result<Option<Item>> {
        let conn = self.conn();
        let found = conn
            .query_row(
                "SELECT id, name, ext, width, height, size, star, annotation, url, palette, \
                 added_at, file_created_at, file_modified_at, duration, bpm, exif, \
                 thumb_version, deleted_at IS NOT NULL FROM item WHERE id = ?1",
                params![id],
                |row| {
                    let palette: Option<String> = row.get(9)?;
                    let exif: Option<String> = row.get(15)?;
                    Ok(Item {
                        id: ItemId(row.get(0)?),
                        name: row.get(1)?,
                        ext: row.get(2)?,
                        width: row.get(3)?,
                        height: row.get(4)?,
                        size: size(row, 5)?,
                        star: row.get(6)?,
                        annotation: row.get(7)?,
                        url: row.get(8)?,
                        palettes: palette
                            .and_then(|json| serde_json::from_str::<Vec<Palette>>(&json).ok())
                            .unwrap_or_default(),
                        added_at: row.get(10)?,
                        file_created_at: row.get(11)?,
                        file_modified_at: row.get(12)?,
                        duration: row.get(13)?,
                        bpm: row.get(14)?,
                        exif: exif.and_then(|json| serde_json::from_str(&json).ok()),
                        no_thumbnail: row.get::<_, i64>(16)? <= 0,
                        thumb_version: row.get(16)?,
                        is_deleted: row.get(17)?,
                        ..Item::default()
                    })
                },
            )
            .optional()
            .context("read an item")?;
        let Some(mut item) = found else {
            return Ok(None);
        };
        let mut statement = conn
            .prepare(
                "SELECT tag.name FROM item_tag JOIN tag ON tag.id = item_tag.tag_id \
                 WHERE item_tag.item_id = ?1 ORDER BY tag.name",
            )
            .context("read the item's tags")?;
        item.tags = statement
            .query_map(params![id], |row| row.get(0))
            .and_then(Iterator::collect)
            .context("read the item's tags")?;
        let mut statement = conn
            .prepare("SELECT folder_id FROM item_folder WHERE item_id = ?1")
            .context("read the item's folders")?;
        item.folders = statement
            .query_map(params![id], |row| row.get(0))
            .and_then(Iterator::collect)
            .context("read the item's folders")?;
        Ok(Some(item))
    }

    /// The tag vocabulary with the number of items outside the trash that carry each tag.
    pub fn tags(&self) -> Result<Vec<Tag>> {
        let conn = self.conn();
        let mut statement = conn
            .prepare(
                "SELECT tag.name, count(item.id) FROM tag \
                 LEFT JOIN item_tag ON item_tag.tag_id = tag.id \
                 LEFT JOIN item ON item.id = item_tag.item_id AND item.deleted_at IS NULL \
                 GROUP BY tag.id ORDER BY tag.name",
            )
            .context("read tags")?;
        statement
            .query_map([], |row| {
                Ok(Tag {
                    name: row.get(0)?,
                    image_count: Some(row.get(1)?),
                })
            })
            .and_then(Iterator::collect)
            .context("read tags")
    }

    /// The item's thumbnail, once the media pipeline made one.
    pub fn thumbnail(&self, id: &str) -> Result<Option<Thumbnail>> {
        let version: Option<i64> = self
            .conn()
            .query_row(
                "SELECT thumb_version FROM item WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .context("read an item")?;
        if version.is_none_or(|version| version <= 0) {
            return Ok(None);
        }
        let path = self.thumbnail_path(id)?;
        Ok(self.read_inside(&path)?.map(|bytes| Thumbnail {
            bytes,
            content_type: "image/webp".into(),
        }))
    }

    /// The stored original, for the full-size preview.
    pub fn original(&self, id: &str) -> Result<Option<Thumbnail>> {
        let found: Option<(String, String)> = self
            .conn()
            .query_row(
                "SELECT file_path, ext FROM item WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .context("read an item")?;
        let Some((file_path, ext)) = found else {
            return Ok(None);
        };
        Ok(self
            .read_inside(&self.root().join(file_path))?
            .map(|bytes| Thumbnail {
                bytes,
                content_type: content_type(&ext).into(),
            }))
    }

    /// Reads `path` only when it really lies inside the library folder (docs/architecture.md
    /// §10, rule 9). A missing file is `None`.
    fn read_inside(&self, path: &Path) -> Result<Option<Vec<u8>>> {
        let Some(path) = canonical(path)? else {
            return Ok(None);
        };
        let root = canonical(self.root())?
            .ok_or_else(|| SourceError::Storage("the library folder is gone".into()))?;
        if !path.starts_with(&root) {
            return Err(SourceError::Invalid(
                "the file is outside the library folder".into(),
            ));
        }
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(SourceError::Storage(format!("read a file: {error}"))),
        }
    }
}

fn canonical(path: &Path) -> Result<Option<PathBuf>> {
    match fs::canonicalize(path) {
        Ok(path) => Ok(Some(path)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(SourceError::Storage(format!("resolve a path: {error}"))),
    }
}

/// Sizes are stored as SQLite integers (`i64`).
fn size(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    Ok(row
        .get::<_, Option<i64>>(index)?
        .and_then(|value| u64::try_from(value).ok()))
}

fn lite_from_row(row: &Row<'_>) -> rusqlite::Result<ItemLite> {
    let channel = |index: usize| -> rusqlite::Result<Option<u8>> {
        Ok(row
            .get::<_, Option<i64>>(index)?
            .and_then(|value| u8::try_from(value).ok()))
    };
    let palette0 = match (channel(7)?, channel(8)?, channel(9)?) {
        (Some(r), Some(g), Some(b)) => Some([r, g, b]),
        _ => None,
    };
    let thumb_version: i64 = row.get(14)?;
    Ok(ItemLite {
        id: ItemId(row.get(0)?),
        name: row.get(1)?,
        ext: row.get(2)?,
        width: row.get(3)?,
        height: row.get(4)?,
        size: size(row, 5)?,
        star: row.get(6)?,
        tags: Vec::new(),
        folders: None,
        palette0,
        added_at: row.get(10)?,
        file_created_at: row.get(11)?,
        file_modified_at: row.get(12)?,
        duration: row.get(13)?,
        no_thumbnail: thumb_version <= 0,
        thumb_version,
        position: None,
    })
}

/// What the WebView is told an original is.
fn content_type(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "psd" => "image/vnd.adobe.photoshop",
        _ => "application/octet-stream",
    }
}
