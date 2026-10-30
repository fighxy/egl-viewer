//! Folders of the native library: the tree with counts, create, edit, move, delete, item
//! membership and manual order (docs/spec.md §3, docs/architecture.md §7.1, §7.3).

use std::collections::{HashMap, HashSet};

use library_core::{
    Folder, FolderChange, Item, ItemEdit, ItemId, SortKey, SourceError, order_key_between,
};
use rusqlite::{OptionalExtension, Transaction, named_params, params};
use ulid::Ulid;

use crate::edits::is_field_edit;
use crate::error::{Context, Result};
use crate::library::Library;
use crate::locks::{LockMap, VISIBLE};
use crate::tags::{drop_unused, tag_id};
use crate::time::now_ms;

/// One folder row, before the tree is built.
struct Row {
    id: String,
    parent: Option<String>,
    name: String,
    description: String,
    icon_color: Option<String>,
    order_by: Option<String>,
    sort_ascending: Option<bool>,
    cover_item_id: Option<String>,
    locked: bool,
    lock_label: Option<String>,
    lock_color: Option<String>,
}

impl Library {
    /// The folder tree in sibling order. `image_count` counts the items directly in a folder;
    /// `descendant_image_count` the items in it or under it, each item once. The trash does not
    /// count, nor do hidden items. A locked folder that is not unlocked shows neither its
    /// subfolders nor counts.
    pub fn folder_tree(&self) -> Result<Vec<Folder>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let mut statement = conn
            .prepare(
                "SELECT id, parent_id, name, description, icon_color, order_by, sort_ascending,
                        cover_item_id, locked, label, label_color
                 FROM folder ORDER BY sort_key, id",
            )
            .context("read folders")?;
        let rows: Vec<Row> = statement
            .query_map([], |row| {
                Ok(Row {
                    id: row.get(0)?,
                    parent: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    icon_color: row.get(4)?,
                    order_by: row.get(5)?,
                    sort_ascending: row.get(6)?,
                    cover_item_id: row.get(7)?,
                    locked: row.get(8)?,
                    lock_label: row.get(9)?,
                    lock_color: row.get(10)?,
                })
            })
            .and_then(Iterator::collect)
            .context("read folders")?;
        let parents: HashMap<&str, &str> = rows
            .iter()
            .filter_map(|row| Some((row.id.as_str(), row.parent.as_deref()?)))
            .collect();

        let mut tags: HashMap<String, Vec<String>> = HashMap::new();
        let mut statement = conn
            .prepare(
                "SELECT folder_tag.folder_id, tag.name FROM folder_tag
                 JOIN tag ON tag.id = folder_tag.tag_id ORDER BY tag.name",
            )
            .context("read folder tags")?;
        let mut found = statement.query([]).context("read folder tags")?;
        while let Some(row) = found.next().context("read folder tags")? {
            tags.entry(row.get(0).context("read a folder tag")?)
                .or_default()
                .push(row.get(1).context("read a folder tag")?);
        }

        // Counts: each item adds one to its folders and, once, to every ancestor of them.
        let mut direct: HashMap<String, u32> = HashMap::new();
        let mut subtree: HashMap<String, u32> = HashMap::new();
        let mut statement = conn
            .prepare(&format!(
                "SELECT item_folder.item_id, item_folder.folder_id FROM item_folder
                 JOIN item ON item.id = item_folder.item_id
                 WHERE item.deleted_at IS NULL AND {VISIBLE} ORDER BY item_folder.item_id"
            ))
            .context("count folder items")?;
        let mut links = statement
            .query(named_params! {
                ":hidden": visibility.hidden,
                ":unlocked": visibility.unlocked,
            })
            .context("count folder items")?;
        let mut current: Option<String> = None;
        let mut reached: HashSet<String> = HashSet::new();
        let mut flush = |reached: &mut HashSet<String>| {
            for folder in reached.drain() {
                *subtree.entry(folder).or_default() += 1;
            }
        };
        while let Some(row) = links.next().context("count folder items")? {
            let item: String = row.get(0).context("read a folder link")?;
            let folder: String = row.get(1).context("read a folder link")?;
            if current.as_deref() != Some(item.as_str()) {
                flush(&mut reached);
                current = Some(item);
            }
            *direct.entry(folder.clone()).or_default() += 1;
            let mut at = Some(folder.as_str());
            while let Some(id) = at {
                if !reached.insert(id.to_owned()) {
                    break;
                }
                at = parents.get(id).copied();
            }
        }
        flush(&mut reached);

        let mut children: HashMap<Option<&str>, Vec<&Row>> = HashMap::new();
        for row in &rows {
            children.entry(row.parent.as_deref()).or_default().push(row);
        }
        struct Tree<'a> {
            children: HashMap<Option<&'a str>, Vec<&'a Row>>,
            tags: HashMap<String, Vec<String>>,
            direct: HashMap<String, u32>,
            subtree: HashMap<String, u32>,
            hidden: HashSet<String>,
        }
        fn build(parent: Option<&str>, inherited: &[String], tree: &Tree<'_>) -> Vec<Folder> {
            let Tree {
                children,
                tags,
                direct,
                subtree,
                hidden,
            } = tree;
            children
                .get(&parent)
                .into_iter()
                .flatten()
                .map(|row| {
                    let own = tags.get(&row.id).cloned().unwrap_or_default();
                    let mut extend = inherited.to_vec();
                    extend.extend(own.iter().filter(|tag| !inherited.contains(tag)).cloned());
                    // A locked folder keeps its subfolders and counts to itself.
                    let closed = row.locked && hidden.contains(&row.id);
                    let count = |counts: &HashMap<String, u32>| {
                        (!closed).then(|| counts.get(&row.id).copied().unwrap_or(0))
                    };
                    Folder {
                        id: row.id.clone(),
                        name: row.name.clone(),
                        description: row.description.clone(),
                        icon_color: row.icon_color.clone(),
                        children: if closed {
                            Vec::new()
                        } else {
                            build(Some(&row.id), &extend, tree)
                        },
                        tags: own,
                        extend_tags: extend,
                        image_count: count(direct),
                        descendant_image_count: count(subtree),
                        is_locked: row.locked,
                        is_unlocked: row.locked && !hidden.contains(&row.id),
                        lock_label: row.lock_label.clone(),
                        lock_color: row.lock_color.clone(),
                        cover_item_id: row.cover_item_id.clone(),
                        order_by: row.order_by.as_deref().and_then(sort_key),
                        sort_ascending: row.sort_ascending,
                    }
                })
                .collect()
        }
        let tree = Tree {
            children,
            tags,
            direct,
            subtree,
            hidden: visibility.hidden_folders,
        };
        Ok(build(None, &[], &tree))
    }

    /// Applies a folder edit; returns the id of the folder it created or changed. A move that
    /// takes items under a lock or out from under one encrypts or decrypts them, and needs the
    /// keys of those locks.
    pub fn save_folder(&self, change: &FolderChange) -> Result<String> {
        self.check_writable()?;
        change.validate()?;
        let mut reseal = None;
        if let FolderChange::Move { id, parent, .. } = change {
            let map = LockMap::load(&self.conn())?;
            if map.has_locks() {
                let items = self.items_in(&map.subtree(id))?;
                self.check_reseal(&items, &map.moved(id, parent.as_deref()), &|_, l| l, None)?;
                reseal = Some(items);
            }
        }
        let saved = self.write_folder(change)?;
        if let Some(items) = reseal {
            self.reseal(&items, &|_, _| {})?;
        }
        Ok(saved)
    }

    fn write_folder(&self, change: &FolderChange) -> Result<String> {
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("save the folder")?;
        let id = match change {
            FolderChange::Create {
                name,
                parent,
                description,
                icon_color,
            } => {
                if let Some(parent) = parent {
                    require_folder(&tx, parent)?;
                }
                let id = Ulid::generate().to_string();
                let last = last_sibling_key(&tx, parent.as_deref(), &id)?;
                let key = order_key_between(last.as_deref(), None);
                tx.execute(
                    "INSERT INTO folder (id, parent_id, name, description, icon_color, sort_key,
                                         created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                    params![id, parent, name.trim(), description, icon_color, key, now],
                )
                .context("create the folder")?;
                log(&tx, "folder", &id, "insert", now)?;
                id
            }
            FolderChange::Update {
                id,
                name,
                description,
                tags,
                icon_color,
                order_by,
                sort_ascending,
            } => {
                require_folder(&tx, id)?;
                tx.execute(
                    "UPDATE folder SET name = ?2, description = ?3, icon_color = ?4,
                                       order_by = ?5, sort_ascending = ?6, updated_at = ?7
                     WHERE id = ?1",
                    params![
                        id,
                        name.trim(),
                        description,
                        icon_color,
                        order_by.map(sort_key_name),
                        sort_ascending,
                        now
                    ],
                )
                .context("update the folder")?;
                tx.execute("DELETE FROM folder_tag WHERE folder_id = ?1", params![id])
                    .context("update the folder tags")?;
                for tag in tags {
                    let tag_id = ensure_tag(&tx, tag)?;
                    tx.execute(
                        "INSERT OR IGNORE INTO folder_tag (folder_id, tag_id) VALUES (?1, ?2)",
                        params![id, tag_id],
                    )
                    .context("update the folder tags")?;
                }
                log(&tx, "folder", id, "update", now)?;
                id.clone()
            }
            FolderChange::Move { id, parent, index } => {
                require_folder(&tx, id)?;
                if let Some(parent) = parent {
                    require_folder(&tx, parent)?;
                    if is_within(&tx, parent, id)? {
                        return Err(SourceError::Invalid(
                            "A folder cannot move into itself or into one of its subfolders."
                                .into(),
                        ));
                    }
                }
                let key = key_at(&tx, parent.as_deref(), id, *index as usize)?;
                tx.execute(
                    "UPDATE folder SET parent_id = ?2, sort_key = ?3, updated_at = ?4
                     WHERE id = ?1",
                    params![id, parent, key, now],
                )
                .context("move the folder")?;
                log(&tx, "folder", id, "update", now)?;
                id.clone()
            }
        };
        tx.commit().context("save the folder")?;
        Ok(id)
    }

    /// Deletes a folder and its subfolders. Items stay, without those folders; with
    /// `trash_items`, the items that were only in these folders go to the trash. Returns how
    /// many items went to the trash.
    pub fn delete_folder(&self, id: &str, trash_items: bool) -> Result<u32> {
        self.check_writable()?;
        // Items under a lock that goes, or under these folders' locks, are decrypted or
        // rewrapped first: their wraps must not outlive the folders.
        let map = LockMap::load(&self.conn())?;
        if map.has_locks() {
            let doomed = map.subtree(id);
            let items = self.items_in(&doomed)?;
            let after = map.without(&doomed);
            let links = |_: &str, links: Vec<String>| -> Vec<String> {
                links
                    .into_iter()
                    .filter(|folder| !doomed.contains(folder))
                    .collect()
            };
            self.check_reseal(&items, &after, &links, None)?;
            self.reseal_with(&items, &after, &links, &|_, _| {})?;
        }
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("delete the folder")?;
        require_folder(&tx, id)?;
        let subtree = subtree_ids(&tx, id)?;
        tx.execute_batch("CREATE TEMP TABLE IF NOT EXISTS doomed (id TEXT PRIMARY KEY)")
            .context("delete the folder")?;
        tx.execute("DELETE FROM temp.doomed", [])
            .context("delete the folder")?;
        for folder in &subtree {
            tx.execute("INSERT INTO temp.doomed (id) VALUES (?1)", params![folder])
                .context("delete the folder")?;
        }
        let mut trashed = Vec::new();
        if trash_items {
            let mut statement = tx
                .prepare(
                    "SELECT DISTINCT item.id FROM item
                     JOIN item_folder ON item_folder.item_id = item.id
                     WHERE item.deleted_at IS NULL
                       AND item_folder.folder_id IN (SELECT id FROM temp.doomed)
                       AND NOT EXISTS (SELECT 1 FROM item_folder other
                                       WHERE other.item_id = item.id
                                         AND other.folder_id NOT IN (SELECT id FROM temp.doomed))",
                )
                .context("find the folder's items")?;
            trashed = statement
                .query_map([], |row| row.get::<_, String>(0))
                .and_then(Iterator::collect)
                .context("find the folder's items")?;
        }
        for item in &trashed {
            tx.execute(
                "UPDATE item SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![item, now],
            )
            .context("move items to the trash")?;
            log(&tx, "item", item, "update", now)?;
        }
        // Subfolders, item links and folder tags go with the folder (ON DELETE CASCADE).
        tx.execute("DELETE FROM folder WHERE id = ?1", params![id])
            .context("delete the folder")?;
        for folder in &subtree {
            log(&tx, "folder", folder, "delete", now)?;
        }
        tx.execute("DELETE FROM temp.doomed", [])
            .context("delete the folder")?;
        tx.commit().context("delete the folder")?;
        Ok(u32::try_from(trashed.len()).unwrap_or(u32::MAX))
    }

    /// Applies an edit to one item and returns the stored result.
    pub fn apply_item_edit(&self, id: &str, edit: &ItemEdit) -> Result<Item> {
        self.apply_batch_edit(&[ItemId(id.to_owned())], edit)?;
        self.item(id)?.ok_or(SourceError::NotFound)
    }

    /// Applies an edit to many items in one transaction; returns how many items exist, are
    /// visible and were edited. Items that join or leave a locked folder are encrypted or
    /// decrypted after the write, which needs that folder unlocked. Tags are not sealed, so tag
    /// edits need no lock work; name, note, link, rating and the trash are in `edits.rs`.
    pub fn apply_batch_edit(&self, ids: &[ItemId], edit: &ItemEdit) -> Result<u32> {
        self.check_writable()?;
        edit.validate()?;
        if is_field_edit(edit) {
            return self.write_field_edit(ids, edit);
        }
        let moves = matches!(
            edit,
            ItemEdit::AddToFolders(_) | ItemEdit::RemoveFromFolders(_) | ItemEdit::SetFolders(_)
        );
        let map = LockMap::load(&self.conn())?;
        let items: Vec<String> = ids.iter().map(|id| id.0.clone()).collect();
        let reseal = moves && map.has_locks();
        if reseal {
            self.check_reseal(&items, &map, &|_, links| edited_links(edit, links), None)?;
        }
        let edited = self.write_batch_edit(ids, edit)?;
        if reseal {
            self.reseal(&items, &|_, _| {})?;
        }
        Ok(edited)
    }

    fn write_batch_edit(&self, ids: &[ItemId], edit: &ItemEdit) -> Result<u32> {
        let now = now_ms();
        let mut conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let tx = conn.transaction().context("edit items")?;
        let mut tags: Vec<i64> = Vec::new();
        match edit {
            ItemEdit::AddToFolders(folders)
            | ItemEdit::RemoveFromFolders(folders)
            | ItemEdit::SetFolders(folders) => {
                for folder in folders {
                    require_folder(&tx, folder)?;
                }
            }
            ItemEdit::AddTags(names) => {
                for name in names {
                    let tag = ensure_tag(&tx, name)?;
                    if !tags.contains(&tag) {
                        tags.push(tag);
                    }
                }
            }
            ItemEdit::RemoveTags(names) => {
                for name in names {
                    if let Some(tag) = tag_id(&tx, name)?
                        && !tags.contains(&tag)
                    {
                        tags.push(tag);
                    }
                }
            }
            _ => unreachable!("field edits are written in edits.rs"),
        }
        let exists_sql = format!("SELECT seq FROM item WHERE id = :id AND {VISIBLE}");
        let mut edited = 0u32;
        for id in ids {
            // A hidden item is left alone, like a missing one: a selection can outlive a lock.
            let exists: Option<i64> = tx
                .prepare_cached(&exists_sql)
                .and_then(|mut statement| {
                    statement
                        .query_row(
                            named_params! {
                                ":id": id.0,
                                ":hidden": visibility.hidden,
                                ":unlocked": visibility.unlocked,
                            },
                            |row| row.get(0),
                        )
                        .optional()
                })
                .context("edit items")?;
            if exists.is_none() {
                continue;
            }
            // Adding a tag an item already has changes nothing: no new time, no change log.
            // The other tags stay. A batch where every item already has them writes nothing.
            let changed = match edit {
                ItemEdit::AddToFolders(folders) => {
                    for folder in folders {
                        add_link(&tx, &id.0, folder)?;
                    }
                    true
                }
                ItemEdit::RemoveFromFolders(folders) => {
                    for folder in folders {
                        tx.prepare_cached(
                            "DELETE FROM item_folder WHERE item_id = ?1 AND folder_id = ?2",
                        )
                        .and_then(|mut statement| statement.execute(params![id.0, folder]))
                        .context("edit items")?;
                    }
                    true
                }
                ItemEdit::SetFolders(folders) => {
                    let mut statement = tx
                        .prepare("SELECT folder_id FROM item_folder WHERE item_id = ?1")
                        .context("edit items")?;
                    let current: Vec<String> = statement
                        .query_map(params![id.0], |row| row.get(0))
                        .and_then(Iterator::collect)
                        .context("edit items")?;
                    for old in current.iter().filter(|old| !folders.contains(old)) {
                        tx.execute(
                            "DELETE FROM item_folder WHERE item_id = ?1 AND folder_id = ?2",
                            params![id.0, old],
                        )
                        .context("edit items")?;
                    }
                    for folder in folders.iter().filter(|new| !current.contains(new)) {
                        add_link(&tx, &id.0, folder)?;
                    }
                    true
                }
                ItemEdit::AddTags(_) => {
                    let mut added = false;
                    for tag in &tags {
                        let inserted = tx
                            .prepare_cached(
                                "INSERT OR IGNORE INTO item_tag (item_id, tag_id) VALUES (?1, ?2)",
                            )
                            .and_then(|mut statement| statement.execute(params![id.0, tag]))
                            .context("tag items")?;
                        if inserted > 0 {
                            added = true;
                        }
                    }
                    added
                }
                ItemEdit::RemoveTags(_) => {
                    for tag in &tags {
                        tx.prepare_cached(
                            "DELETE FROM item_tag WHERE item_id = ?1 AND tag_id = ?2",
                        )
                        .and_then(|mut statement| statement.execute(params![id.0, tag]))
                        .context("untag items")?;
                    }
                    true
                }
                _ => unreachable!("checked above"),
            };
            if !changed {
                continue;
            }
            tx.prepare_cached("UPDATE item SET updated_at = ?2 WHERE id = ?1")
                .and_then(|mut statement| statement.execute(params![id.0, now]))
                .context("edit items")?;
            log(&tx, "item", &id.0, "update", now)?;
            edited += 1;
        }
        if matches!(edit, ItemEdit::RemoveTags(_)) {
            drop_unused(&tx, &tags)?;
        }
        tx.commit().context("edit items")?;
        Ok(edited)
    }

    /// Manual order: puts `ids` before `before`, or at the end, keeping their order among
    /// themselves, and numbers the folder's items again from 1.
    pub fn reorder_items(
        &self,
        folder: &str,
        ids: &[ItemId],
        before: Option<&ItemId>,
    ) -> Result<()> {
        self.check_writable()?;
        let now = now_ms();
        let mut conn = self.conn();
        let tx = conn.transaction().context("reorder items")?;
        require_folder(&tx, folder)?;
        let mut statement = tx
            .prepare(
                "SELECT item_folder.item_id FROM item_folder
                 JOIN item ON item.id = item_folder.item_id
                 WHERE item_folder.folder_id = ?1
                 ORDER BY item_folder.position IS NULL, item_folder.position, item.added_at,
                          item.id",
            )
            .context("reorder items")?;
        let order: Vec<String> = statement
            .query_map(params![folder], |row| row.get(0))
            .and_then(Iterator::collect)
            .context("reorder items")?;
        drop(statement);
        // The moved items keep their order among themselves.
        let wanted: HashSet<&str> = ids.iter().map(|id| id.0.as_str()).collect();
        let moving: Vec<&str> = order
            .iter()
            .map(String::as_str)
            .filter(|id| wanted.contains(id))
            .collect();
        let mut rest: Vec<&str> = order
            .iter()
            .map(String::as_str)
            .filter(|id| !wanted.contains(id))
            .collect();
        // `before` may be one of the moved items: then the first item after it that stays.
        let at = before
            .and_then(|before| order.iter().position(|id| *id == before.0))
            .and_then(|from| {
                order[from..]
                    .iter()
                    .find(|id| !wanted.contains(id.as_str()))
            })
            .and_then(|next| rest.iter().position(|id| *id == next.as_str()))
            .unwrap_or(rest.len());
        rest.splice(at..at, moving.iter().copied());
        for (position, id) in rest.iter().enumerate() {
            tx.prepare_cached(
                "UPDATE item_folder SET position = ?3 WHERE item_id = ?1 AND folder_id = ?2",
            )
            .and_then(|mut statement| statement.execute(params![id, folder, (position + 1) as f64]))
            .context("reorder items")?;
        }
        log(&tx, "folder", folder, "update", now)?;
        tx.commit().context("reorder items")
    }

    pub fn has_folder(&self, id: &str) -> Result<bool> {
        let found: Option<i64> = self
            .conn()
            .query_row("SELECT 1 FROM folder WHERE id = ?1", params![id], |row| {
                row.get(0)
            })
            .optional()
            .context("find the folder")?;
        Ok(found.is_some())
    }

    /// The tags a file added into `folder` gets: the folder's and its ancestors' (spec §3).
    pub(crate) fn inherited_folder_tags(&self, folder: &str) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut statement = conn
            .prepare(
                "WITH RECURSIVE up(id) AS (
                     SELECT ?1 UNION SELECT folder.parent_id FROM folder JOIN up ON folder.id = up.id
                     WHERE folder.parent_id IS NOT NULL)
                 SELECT DISTINCT tag.name FROM folder_tag JOIN tag ON tag.id = folder_tag.tag_id
                 WHERE folder_tag.folder_id IN (SELECT id FROM up) ORDER BY tag.name",
            )
            .context("read folder tags")?;
        statement
            .query_map(params![folder], |row| row.get(0))
            .and_then(Iterator::collect)
            .context("read folder tags")
    }

    /// The folder `parent / names[0] / names[1] …`, made where missing: a dropped folder keeps
    /// its structure. An existing child with the same name is reused.
    pub(crate) fn folder_path(
        &self,
        parent: Option<&str>,
        names: &[String],
    ) -> Result<Option<String>> {
        let mut at = parent.map(str::to_owned);
        for name in names {
            let existing: Option<String> = self
                .conn()
                .query_row(
                    "SELECT id FROM folder WHERE parent_id IS ?1 AND name = ?2
                     ORDER BY sort_key LIMIT 1",
                    params![at, name],
                    |row| row.get(0),
                )
                .optional()
                .context("find a folder")?;
            at = Some(match existing {
                Some(id) => id,
                None => self.save_folder(&FolderChange::Create {
                    name: name.clone(),
                    parent: at.clone(),
                    description: String::new(),
                    icon_color: None,
                })?,
            });
        }
        Ok(at)
    }
}

/// The wire name of a sort key, as stored in `folder.order_by`.
fn sort_key_name(key: SortKey) -> String {
    serde_json::to_value(key)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn sort_key(name: &str) -> Option<SortKey> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
}

pub(crate) fn log(tx: &Transaction<'_>, entity: &str, id: &str, op: &str, at: i64) -> Result<()> {
    tx.prepare_cached("INSERT INTO change_log (entity, entity_id, op, at) VALUES (?1, ?2, ?3, ?4)")
        .and_then(|mut statement| statement.execute(params![entity, id, op, at]))
        .context("log the change")?;
    Ok(())
}

fn require_folder(tx: &Transaction<'_>, id: &str) -> Result<()> {
    let found: Option<i64> = tx
        .query_row("SELECT 1 FROM folder WHERE id = ?1", params![id], |row| {
            row.get(0)
        })
        .optional()
        .context("find the folder")?;
    found
        .map(|_| ())
        .ok_or_else(|| SourceError::Invalid(format!("unknown folder {id}")))
}

/// `folder` itself and every folder under it.
fn subtree_ids(tx: &Transaction<'_>, folder: &str) -> Result<Vec<String>> {
    let mut statement = tx
        .prepare(
            "WITH RECURSIVE sub(id) AS (
                 SELECT ?1 UNION SELECT folder.id FROM folder JOIN sub ON folder.parent_id = sub.id)
             SELECT id FROM sub",
        )
        .context("read subfolders")?;
    statement
        .query_map(params![folder], |row| row.get(0))
        .and_then(Iterator::collect)
        .context("read subfolders")
}

/// `candidate` is `folder` or lies under it: the cycle guard of a move.
fn is_within(tx: &Transaction<'_>, candidate: &str, folder: &str) -> Result<bool> {
    Ok(subtree_ids(tx, folder)?.iter().any(|id| id == candidate))
}

fn last_sibling_key(
    tx: &Transaction<'_>,
    parent: Option<&str>,
    except: &str,
) -> Result<Option<String>> {
    tx.query_row(
        "SELECT sort_key FROM folder WHERE parent_id IS ?1 AND id <> ?2
         ORDER BY sort_key DESC LIMIT 1",
        params![parent, except],
        |row| row.get(0),
    )
    .optional()
    .context("read the folder order")
}

/// An item's folder links after a membership edit, for planning what locks it will need.
fn edited_links(edit: &ItemEdit, mut links: Vec<String>) -> Vec<String> {
    match edit {
        ItemEdit::AddToFolders(folders) => {
            links.extend(folders.iter().cloned());
            links
        }
        ItemEdit::RemoveFromFolders(folders) => {
            links.retain(|folder| !folders.contains(folder));
            links
        }
        ItemEdit::SetFolders(folders) => folders.clone(),
        _ => links,
    }
}

/// The order key for position `index` among the children of `parent`, `folder` left out.
/// Siblings with equal keys (an import) are numbered again first.
fn key_at(
    tx: &Transaction<'_>,
    parent: Option<&str>,
    folder: &str,
    index: usize,
) -> Result<String> {
    let siblings = sibling_keys(tx, parent, folder)?;
    let unique = siblings.windows(2).all(|pair| pair[0].1 < pair[1].1);
    let siblings = if unique {
        siblings
    } else {
        let mut key: Option<String> = None;
        let mut renumbered = Vec::with_capacity(siblings.len());
        for (id, _) in siblings {
            let next = order_key_between(key.as_deref(), None);
            tx.execute(
                "UPDATE folder SET sort_key = ?2 WHERE id = ?1",
                params![id, next],
            )
            .context("number the folders")?;
            key = Some(next.clone());
            renumbered.push((id, next));
        }
        renumbered
    };
    let index = index.min(siblings.len());
    let before = index
        .checked_sub(1)
        .and_then(|at| siblings.get(at))
        .map(|(_, key)| key.as_str());
    let after = siblings.get(index).map(|(_, key)| key.as_str());
    Ok(order_key_between(before, after))
}

fn sibling_keys(
    tx: &Transaction<'_>,
    parent: Option<&str>,
    folder: &str,
) -> Result<Vec<(String, String)>> {
    let mut statement = tx
        .prepare(
            "SELECT id, sort_key FROM folder WHERE parent_id IS ?1 AND id <> ?2
             ORDER BY sort_key, id",
        )
        .context("read the folder order")?;
    statement
        .query_map(params![parent, folder], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .and_then(Iterator::collect)
        .context("read the folder order")
}

/// The tag's row id, made when the name is new. Names are exact strings (case-sensitive).
pub(crate) fn ensure_tag(tx: &Transaction<'_>, name: &str) -> Result<i64> {
    tx.execute(
        "INSERT OR IGNORE INTO tag (name) VALUES (?1)",
        params![name],
    )
    .context("add a tag")?;
    tx.query_row("SELECT id FROM tag WHERE name = ?1", params![name], |row| {
        row.get(0)
    })
    .context("add a tag")
}

/// Puts an item into a folder, at the end of its manual order.
pub(crate) fn add_link(tx: &Transaction<'_>, item: &str, folder: &str) -> Result<()> {
    // No manual position: the item comes after the ordered ones, by date added, until the
    // folder is reordered (Eagle falls back the same way, correction 39). Looking up the last
    // position here made large adds quadratic.
    tx.prepare_cached("INSERT OR IGNORE INTO item_folder (item_id, folder_id) VALUES (?1, ?2)")
        .and_then(|mut statement| statement.execute(params![item, folder]))
        .context("add the item to the folder")?;
    Ok(())
}
