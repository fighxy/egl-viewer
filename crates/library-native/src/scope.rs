//! Reads for the views and the detail panel: one SQL query per scope, the full item, and the
//! bytes the `thumb` scheme serves (docs/architecture.md §11, §15.3).

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use library_core::{Item, ItemId, ItemLite, Palette, Scope, SourceError, Thumbnail};
use rusqlite::{OptionalExtension, Row, named_params, params};

use crate::error::{Context, Result};
use crate::library::Library;
use crate::locks::VISIBLE;
use crate::recover::library_file;

/// The columns of [`ItemLite`], in the order `lite_from_row` reads them.
const LITE_COLUMNS: &str = "id, name, ext, width, height, size, star, \
     json_extract(palette, '$[0].color[0]'), json_extract(palette, '$[0].color[1]'), \
     json_extract(palette, '$[0].color[2]'), \
     added_at, file_created_at, file_modified_at, duration, thumb_version";

impl Library {
    /// The light items of one scope (docs/architecture.md §15.3). A folder scope also gives
    /// each direct member its manual position. Items under a lock that is not unlocked never
    /// leave this function; encrypted items come with their sealed name and palette opened.
    /// A smart folder evaluates its rules over All.
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
                     WHERE item_id = item.id AND folder_id = :folder)"
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
            Scope::Smart { id } => return self.load_smart(id),
        };
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let mut items = Vec::new();
        let mut positions = HashMap::new();
        let mut sealed: Vec<(usize, Vec<u8>)> = Vec::new();
        // The folder and its subfolders, for the scope with subfolders.
        let prefix = if folder.is_some()
            && matches!(
                scope,
                Scope::Folder {
                    include_descendants: true,
                    ..
                }
            ) {
            "WITH RECURSIVE sub(id) AS (SELECT :folder \
             UNION SELECT folder.id FROM folder JOIN sub ON folder.parent_id = sub.id) "
        } else {
            ""
        };
        let mut statement = conn
            .prepare(&format!(
                "{prefix}SELECT {LITE_COLUMNS}, encrypted, sealed FROM item \
                 WHERE {condition} AND {VISIBLE} ORDER BY seq"
            ))
            .context("load a scope")?;
        let mut rows = match folder {
            Some(id) => statement.query(named_params! {
                ":folder": id,
                ":hidden": visibility.hidden,
                ":unlocked": visibility.unlocked,
            }),
            None => statement.query(named_params! {
                ":hidden": visibility.hidden,
                ":unlocked": visibility.unlocked,
            }),
        }
        .context("load a scope")?;
        while let Some(row) = rows.next().context("load a scope")? {
            let item = lite_from_row(row).context("read an item")?;
            if row.get::<_, bool>(15).context("read an item")? {
                let blob: Option<Vec<u8>> = row.get(16).context("read an item")?;
                sealed.push((items.len(), blob.unwrap_or_default()));
            }
            positions.insert(item.id.0.clone(), items.len());
            items.push(item);
        }
        drop(rows);
        drop(statement);

        // Encrypted items: the name and the palette's first colour, opened in this session.
        let mut unreadable = Vec::new();
        for (at, blob) in &sealed {
            let id = items[*at].id.0.clone();
            match self.open_fields(&conn, &id, blob) {
                Ok(fields) => {
                    items[*at].palette0 = first_colour(fields.palette.as_deref());
                    items[*at].name = fields.name;
                }
                Err(error) => {
                    tracing::warn!("cannot open an encrypted item: {error}");
                    unreadable.push(*at);
                }
            }
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
        // An item whose key cannot be opened stays hidden, like the rest of a locked folder.
        for at in unreadable.into_iter().rev() {
            items.remove(at);
        }
        Ok(items)
    }

    /// The full item for the detail panel, also from the trash; `None` while it is hidden.
    pub fn item(&self, id: &str) -> Result<Option<Item>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let found = conn
            .query_row(
                &format!(
                    "SELECT id, name, ext, width, height, size, star, annotation, url, palette, \
                     added_at, file_created_at, file_modified_at, duration, bpm, exif, \
                     thumb_version, deleted_at IS NOT NULL, encrypted, sealed \
                     FROM item WHERE id = :id AND {VISIBLE}"
                ),
                named_params! {
                    ":id": id,
                    ":hidden": visibility.hidden,
                    ":unlocked": visibility.unlocked,
                },
                |row| {
                    let palette: Option<String> = row.get(9)?;
                    let exif: Option<String> = row.get(15)?;
                    let sealed: Option<Vec<u8>> = if row.get::<_, bool>(18)? {
                        Some(row.get::<_, Option<Vec<u8>>>(19)?.unwrap_or_default())
                    } else {
                        None
                    };
                    let item = Item {
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
                    };
                    Ok((item, sealed))
                },
            )
            .optional()
            .context("read an item")?;
        let Some((mut item, sealed)) = found else {
            return Ok(None);
        };
        if let Some(blob) = sealed {
            let fields = self.open_fields(&conn, id, &blob)?;
            item.palettes = fields
                .palette
                .as_deref()
                .and_then(|json| serde_json::from_str::<Vec<Palette>>(json).ok())
                .unwrap_or_default();
            item.exif = fields
                .exif
                .as_deref()
                .and_then(|json| serde_json::from_str(json).ok());
            item.name = fields.name;
            item.annotation = fields.annotation;
            item.url = fields.url;
        }
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

    /// The item's thumbnail, once the media pipeline made one; decrypted in memory for an item
    /// of an unlocked folder, `None` for a hidden one.
    pub fn thumbnail(&self, id: &str) -> Result<Option<Thumbnail>> {
        let Some((version, encrypted, _, _)) = self.visible_file(id)? else {
            return Ok(None);
        };
        if version <= 0 {
            return Ok(None);
        }
        let bytes = if encrypted {
            self.read_sealed(id, &self.sealed_thumbnail_path(id)?)?
        } else {
            self.read_inside(&self.thumbnail_path(id)?)?
        };
        Ok(bytes.map(|bytes| Thumbnail {
            bytes,
            content_type: "image/webp".into(),
            private: encrypted,
        }))
    }

    /// The stored original, for the full-size preview.
    pub fn original(&self, id: &str) -> Result<Option<Thumbnail>> {
        let Some((_, encrypted, file_path, ext)) = self.visible_file(id)? else {
            return Ok(None);
        };
        let bytes = if encrypted {
            let path = library_file(self.root(), &file_path)
                .ok_or_else(|| SourceError::Storage("bad stored path".into()))?;
            self.read_sealed(id, &path)?
        } else {
            self.read_inside(&self.root().join(file_path))?
        };
        Ok(bytes.map(|bytes| Thumbnail {
            bytes,
            content_type: content_type(&ext).into(),
            private: encrypted,
        }))
    }

    /// `thumb_version`, `encrypted`, `file_path` and `ext` of an item that may be shown.
    fn visible_file(&self, id: &str) -> Result<Option<(i64, bool, String, String)>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        conn.query_row(
            &format!(
                "SELECT thumb_version, encrypted, file_path, ext FROM item \
                 WHERE id = :id AND {VISIBLE}"
            ),
            named_params! {
                ":id": id,
                ":hidden": visibility.hidden,
                ":unlocked": visibility.unlocked,
            },
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .context("read an item")
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
        // A native item always has a folder list, perhaps empty; only Eagle can lack the key.
        folders: Some(Vec::new()),
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

/// The first colour of a palette stored as JSON.
fn first_colour(palette: Option<&str>) -> Option<[u8; 3]> {
    let palettes: Vec<Palette> = serde_json::from_str(palette?).ok()?;
    palettes.first().map(|entry| entry.color)
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
