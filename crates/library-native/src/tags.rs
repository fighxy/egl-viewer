//! Tags of the native library: the vocabulary, rename, merge and delete everywhere, and tag
//! groups (docs/spec.md §3 Tags, §6). Item tag edits go through `apply_batch_edit`.
//!
//! A tag is one row of `tag`, named by its exact string; items, folders and groups refer to the
//! row. A tag that nothing refers to any more is removed with the write that left it alone.

use std::collections::HashMap;

use library_core::{
    SourceError, Tag, TagGroup, TagGroupChange, compare_tags, order_key_between, validate_tag_names,
};
use rusqlite::{OptionalExtension, Transaction, named_params, params};
use ulid::Ulid;

use crate::error::{Context, Result};
use crate::folders::{ensure_tag, log};
use crate::library::Library;
use crate::locks::VISIBLE;
use crate::time::now_ms;

impl Library {
    /// The vocabulary: every tag on an item the user may see, the trash included, and every
    /// tag kept in a tag group, by name. The count leaves out the trash. A tag that only hidden
    /// items carry stays out, so a locked folder does not show through its tags.
    pub fn tags(&self) -> Result<Vec<Tag>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let mut statement = conn
            .prepare(&format!(
                "SELECT tag.name, \
                   (SELECT count(*) FROM item_tag JOIN item ON item.id = item_tag.item_id \
                    WHERE item_tag.tag_id = tag.id AND item.deleted_at IS NULL AND {VISIBLE}) \
                 FROM tag \
                 WHERE EXISTS (SELECT 1 FROM item_tag JOIN item ON item.id = item_tag.item_id \
                               WHERE item_tag.tag_id = tag.id AND {VISIBLE}) \
                    OR EXISTS (SELECT 1 FROM tag_group_tag WHERE tag_group_tag.tag_id = tag.id)"
            ))
            .context("read tags")?;
        let mut tags: Vec<Tag> = statement
            .query_map(
                named_params! {
                    ":hidden": visibility.hidden,
                    ":unlocked": visibility.unlocked,
                },
                |row| {
                    Ok(Tag {
                        name: row.get(0)?,
                        image_count: Some(row.get(1)?),
                    })
                },
            )
            .and_then(Iterator::collect)
            .context("read tags")?;
        tags.sort_by(|a, b| compare_tags(&a.name, &b.name));
        Ok(tags)
    }

    /// Renames a tag on every item, folder and group. A name that is already a tag merges the
    /// two (D-046); a name that differs only in case is another tag.
    pub fn rename_tag(&self, from: &str, to: &str) -> Result<()> {
        self.retag(from, to, false)
    }

    /// Merges `source` into `target`, which must exist: every item, folder and group with
    /// `source` gets `target`, and `source` is gone.
    pub fn merge_tags(&self, source: &str, target: &str) -> Result<()> {
        self.retag(source, target, true)
    }

    fn retag(&self, from: &str, to: &str, target_must_exist: bool) -> Result<()> {
        self.check_writable()?;
        validate_tag_names(&[to.to_owned()])?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("rename the tag")?;
        let source = tag_id(&tx, from)?.ok_or(SourceError::NotFound)?;
        let target = tag_id(&tx, to)?;
        if target == Some(source) {
            return Ok(());
        }
        if target.is_none() && target_must_exist {
            return Err(SourceError::NotFound);
        }
        touch_items(&tx, source, now)?;
        match target {
            None => {
                tx.execute(
                    "UPDATE tag SET name = ?2 WHERE id = ?1",
                    params![source, to],
                )
                .context("rename the tag")?;
                log(&tx, "tag", &source.to_string(), "update", now)?;
            }
            Some(target) => {
                for (table, owner) in [
                    ("item_tag", "item_id"),
                    ("folder_tag", "folder_id"),
                    ("tag_group_tag", "group_id"),
                ] {
                    tx.execute(
                        &format!(
                            "INSERT OR IGNORE INTO {table} ({owner}, tag_id) \
                             SELECT {owner}, ?2 FROM {table} WHERE tag_id = ?1"
                        ),
                        params![source, target],
                    )
                    .context("merge the tags")?;
                }
                // The foreign keys remove the source's links with it.
                tx.execute("DELETE FROM tag WHERE id = ?1", params![source])
                    .context("merge the tags")?;
                log(&tx, "tag", &source.to_string(), "delete", now)?;
                log(&tx, "tag", &target.to_string(), "update", now)?;
            }
        }
        tx.commit().context("rename the tag")
    }

    /// Removes a tag from every item, folder and group.
    pub fn delete_tag(&self, name: &str) -> Result<()> {
        self.check_writable()?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("delete the tag")?;
        let id = tag_id(&tx, name)?.ok_or(SourceError::NotFound)?;
        touch_items(&tx, id, now)?;
        tx.execute("DELETE FROM tag WHERE id = ?1", params![id])
            .context("delete the tag")?;
        log(&tx, "tag", &id.to_string(), "delete", now)?;
        tx.commit().context("delete the tag")
    }

    /// Every tag group in its order, each with its tags by name.
    pub fn tag_groups(&self) -> Result<Vec<TagGroup>> {
        let conn = self.conn();
        let mut statement = conn
            .prepare(
                "SELECT tag_group_tag.group_id, tag.name FROM tag_group_tag
                 JOIN tag ON tag.id = tag_group_tag.tag_id",
            )
            .context("read tag groups")?;
        let mut tags: HashMap<String, Vec<String>> = HashMap::new();
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get(1)?)))
            .context("read tag groups")?;
        for row in rows {
            let (group, tag) = row.context("read tag groups")?;
            tags.entry(group).or_default().push(tag);
        }
        let mut statement = conn
            .prepare("SELECT id, name, color FROM tag_group ORDER BY sort_key, id")
            .context("read tag groups")?;
        let mut groups: Vec<TagGroup> = statement
            .query_map([], |row| {
                Ok(TagGroup {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    tags: Vec::new(),
                })
            })
            .and_then(Iterator::collect)
            .context("read tag groups")?;
        for group in &mut groups {
            group.tags = tags.remove(&group.id).unwrap_or_default();
            group.tags.sort_by(|a, b| compare_tags(a, b));
        }
        Ok(groups)
    }

    /// Creates a group after the last one, or replaces a group's name, colour and tags.
    /// Returns the group's id. Item tags do not change.
    pub fn save_tag_group(&self, change: &TagGroupChange) -> Result<String> {
        self.check_writable()?;
        change.validate()?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("save the tag group")?;
        let (id, tags, old) = match change {
            TagGroupChange::Create { name, color, tags } => {
                let id = Ulid::generate().to_string();
                let last: Option<String> = tx
                    .query_row(
                        "SELECT sort_key FROM tag_group ORDER BY sort_key DESC LIMIT 1",
                        [],
                        |row| row.get(0),
                    )
                    .optional()
                    .context("read the tag group order")?;
                tx.execute(
                    "INSERT INTO tag_group (id, name, color, sort_key) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        id,
                        name.trim(),
                        color,
                        order_key_between(last.as_deref(), None)
                    ],
                )
                .context("create the tag group")?;
                (id, tags, Vec::new())
            }
            TagGroupChange::Update {
                id,
                name,
                color,
                tags,
            } => {
                let changed = tx
                    .execute(
                        "UPDATE tag_group SET name = ?2, color = ?3 WHERE id = ?1",
                        params![id, name.trim(), color],
                    )
                    .context("edit the tag group")?;
                if changed == 0 {
                    return Err(SourceError::NotFound);
                }
                let old = group_tag_ids(&tx, id)?;
                tx.execute("DELETE FROM tag_group_tag WHERE group_id = ?1", params![id])
                    .context("edit the tag group")?;
                (id.clone(), tags, old)
            }
        };
        for tag in tags {
            let tag = ensure_tag(&tx, tag)?;
            tx.execute(
                "INSERT OR IGNORE INTO tag_group_tag (group_id, tag_id) VALUES (?1, ?2)",
                params![id, tag],
            )
            .context("save the tag group")?;
        }
        drop_unused(&tx, &old)?;
        log(&tx, "tag_group", &id, "update", now)?;
        tx.commit().context("save the tag group")?;
        Ok(id)
    }

    /// Deletes a group; its tags stay on their items.
    pub fn delete_tag_group(&self, id: &str) -> Result<()> {
        self.check_writable()?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("delete the tag group")?;
        let old = group_tag_ids(&tx, id)?;
        let deleted = tx
            .execute("DELETE FROM tag_group WHERE id = ?1", params![id])
            .context("delete the tag group")?;
        if deleted == 0 {
            return Err(SourceError::NotFound);
        }
        drop_unused(&tx, &old)?;
        log(&tx, "tag_group", id, "delete", now)?;
        tx.commit().context("delete the tag group")
    }
}

/// The row id of a tag, by its exact name.
pub(crate) fn tag_id(tx: &Transaction<'_>, name: &str) -> Result<Option<i64>> {
    tx.prepare_cached("SELECT id FROM tag WHERE name = ?1")
        .and_then(|mut statement| {
            statement
                .query_row(params![name], |row| row.get(0))
                .optional()
        })
        .context("find the tag")
}

/// Removes the tags that no item, folder or group refers to any more.
pub(crate) fn drop_unused(tx: &Transaction<'_>, tags: &[i64]) -> Result<()> {
    for tag in tags {
        tx.prepare_cached(
            "DELETE FROM tag WHERE id = ?1
               AND NOT EXISTS (SELECT 1 FROM item_tag WHERE tag_id = ?1)
               AND NOT EXISTS (SELECT 1 FROM folder_tag WHERE tag_id = ?1)
               AND NOT EXISTS (SELECT 1 FROM tag_group_tag WHERE tag_id = ?1)",
        )
        .and_then(|mut statement| statement.execute(params![tag]))
        .context("remove an unused tag")?;
    }
    Ok(())
}

fn group_tag_ids(tx: &Transaction<'_>, group: &str) -> Result<Vec<i64>> {
    let mut statement = tx
        .prepare("SELECT tag_id FROM tag_group_tag WHERE group_id = ?1")
        .context("read the tag group")?;
    statement
        .query_map(params![group], |row| row.get(0))
        .and_then(Iterator::collect)
        .context("read the tag group")
}

/// Items whose tags change get a new modification time.
fn touch_items(tx: &Transaction<'_>, tag: i64, now: i64) -> Result<()> {
    tx.execute(
        "UPDATE item SET updated_at = ?2
         WHERE id IN (SELECT item_id FROM item_tag WHERE tag_id = ?1)",
        params![tag, now],
    )
    .context("edit items")?;
    Ok(())
}
