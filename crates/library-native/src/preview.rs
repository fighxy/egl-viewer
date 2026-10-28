//! Thumbnails, palettes and dimensions from the media pipeline, and the counts of the left bar
//! (docs/library-format.md §4, §6).

use std::fs;
use std::path::PathBuf;

use library_core::{LibraryCounts, SourceError};
use rusqlite::params;

use crate::error::{Context, Result};
use crate::fsutil::rename_with_retry;
use crate::library::Library;
use crate::recover::library_file;
use crate::time::now_ms;

/// `thumb_version` of an item whose preview cannot be made: keeps the placeholder and is not
/// tried again on the next open. 0 means "not made yet", above 0 a thumbnail exists.
pub const NO_PREVIEW: i64 = -1;

/// An item that still needs its thumbnail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPreview {
    pub id: String,
    pub file: PathBuf,
    pub ext: String,
}

impl Library {
    /// Items of a previewable type without a thumbnail yet, oldest first.
    pub fn pending_previews(&self, limit: usize) -> Result<Vec<PendingPreview>> {
        let exts: Vec<String> = media::preview_extensions()
            .map(|ext| format!("'{ext}'"))
            .collect();
        let sql = format!(
            "SELECT id, file_path, ext FROM item
             WHERE thumb_version = 0 AND encrypted = 0 AND ext IN ({})
             ORDER BY seq LIMIT ?1",
            exts.join(", ")
        );
        let conn = self.conn();
        let mut stmt = conn.prepare(&sql).context("find items without previews")?;
        let rows = stmt
            .query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get(2)?,
                ))
            })
            .context("find items without previews")?;
        let mut pending = Vec::new();
        for row in rows {
            let (id, stored, ext) = row.context("find items without previews")?;
            if let Some(file) = library_file(self.root(), &stored) {
                pending.push(PendingPreview { id, file, ext });
            }
        }
        Ok(pending)
    }

    /// Writes the thumbnail to `thumbs/<shard>/<id>.webp`, then the dimensions, palette and
    /// EXIF. The file is in place before the row says it exists.
    pub fn store_preview(&self, id: &str, rendered: &media::Rendered) -> Result<()> {
        self.check_writable()?;
        let thumb = self.thumbnail_path(id)?;
        let dir = thumb.parent().map(PathBuf::from).unwrap_or_default();
        fs::create_dir_all(&dir).context("create the thumbnail folder")?;
        let tmp = dir.join(format!(".{id}.webp.tmp"));
        fs::write(&tmp, &rendered.thumbnail.webp).context("write the thumbnail")?;
        if let Err(error) = rename_with_retry(&tmp, &thumb) {
            let _ = fs::remove_file(&tmp);
            return Err(error).context("move the thumbnail into place");
        }
        let palette = serde_json::to_string(&rendered.palette).context("encode the palette")?;
        let exif = rendered.exif.as_ref().map(serde_json::Value::to_string);
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("store the preview")?;
        let updated = tx
            .execute(
                "UPDATE item SET width = ?2, height = ?3, palette = ?4,
                     exif = COALESCE(?5, exif), thumb_version = MAX(thumb_version, 0) + 1,
                     updated_at = ?6
                 WHERE id = ?1",
                params![id, rendered.width, rendered.height, palette, exif, now],
            )
            .context("store the preview")?;
        if updated == 0 {
            drop(tx);
            let _ = fs::remove_file(&thumb);
            return Err(SourceError::NotFound);
        }
        tx.execute(
            "INSERT INTO change_log (entity, entity_id, op, at) VALUES ('item', ?1, 'update', ?2)",
            params![id, now],
        )
        .context("log the change")?;
        tx.commit().context("store the preview")
    }

    /// The file cannot be decoded: keep the placeholder and do not try again.
    pub fn mark_no_preview(&self, id: &str) -> Result<()> {
        self.check_writable()?;
        self.conn()
            .execute(
                "UPDATE item SET thumb_version = ?2 WHERE id = ?1 AND thumb_version = 0",
                params![id, NO_PREVIEW],
            )
            .context("mark the item without a preview")?;
        Ok(())
    }

    pub fn thumbnail_path(&self, id: &str) -> Result<PathBuf> {
        if id.len() < 2 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err(SourceError::Invalid(format!("not an item id: {id}")));
        }
        let shard = &id[id.len() - 2..];
        Ok(self
            .root()
            .join("thumbs")
            .join(shard)
            .join(format!("{id}.webp")))
    }

    /// Items per fixed row of the left bar; the trash counts only in Trash.
    pub fn counts(&self) -> Result<LibraryCounts> {
        let conn = self.conn();
        let count = |sql: &str| -> Result<u32> {
            conn.query_row(sql, [], |row| row.get::<_, u32>(0))
                .context("count items")
        };
        Ok(LibraryCounts {
            all: count("SELECT count(*) FROM item WHERE deleted_at IS NULL")?,
            uncategorized: count(
                "SELECT count(*) FROM item WHERE deleted_at IS NULL
                 AND NOT EXISTS (SELECT 1 FROM item_folder WHERE item_id = item.id)",
            )?,
            untagged: count(
                "SELECT count(*) FROM item WHERE deleted_at IS NULL
                 AND NOT EXISTS (SELECT 1 FROM item_tag WHERE item_id = item.id)",
            )?,
            trash: count("SELECT count(*) FROM item WHERE deleted_at IS NOT NULL")?,
        })
    }
}
