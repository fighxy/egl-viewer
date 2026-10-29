//! The item fields of phase 7 (name, note, link, rating), the trash, removing items for good,
//! and the state undo needs (docs/spec.md §6, docs/library-format.md §6).

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use library_core::{ItemEdit, ItemId, ItemState, SourceError};
use rusqlite::{Connection, OptionalExtension, Transaction, named_params, params};
use ulid::Ulid;

use crate::add::file_name_for;
use crate::error::{Context, Result};
use crate::folders::log;
use crate::fsutil::rename_with_retry;
use crate::library::Library;
use crate::locks::{VISIBLE, Visibility};
use crate::recover::{journal_file, library_file, remove_with_empty_parent};
use crate::tags::drop_unused;
use crate::time::now_ms;
use crate::vault::SealedFields;

/// The edits written here; the folder and tag edits are in `folders.rs`.
pub(crate) fn is_field_edit(edit: &ItemEdit) -> bool {
    matches!(
        edit,
        ItemEdit::SetStar(_)
            | ItemEdit::SetAnnotation(_)
            | ItemEdit::SetUrl(_)
            | ItemEdit::Rename(_)
            | ItemEdit::Trash
            | ItemEdit::Restore
    )
}

/// A visible item, as a field edit needs it.
struct Target {
    id: String,
    seq: i64,
    file_path: String,
    ext: String,
    sealed: Option<Vec<u8>>,
}

/// A stored original that a rename gives a new name.
struct Move {
    /// Index into the targets.
    item: usize,
    old: String,
    new: String,
    old_path: PathBuf,
    new_path: PathBuf,
    /// Linked or copied under the new name, which the journal holds until the row commits.
    /// Otherwise the file system ignores case and the file was renamed in place.
    journaled: bool,
}

/// Which items [`Library::remove_items`] may take.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Removable {
    /// Only items in the trash: Delete Permanently, Empty Trash.
    Trashed,
    /// Any visible item: repairing items whose original is gone.
    Any,
}

impl Library {
    /// Name, note, link, rating and the trash for many items in one transaction. A rename
    /// renames the stored original too: the new name is linked (or copied) beside the old one
    /// under the journal, the row commits, then the old name goes (docs/library-format.md §6).
    /// The original of an encrypted item keeps its sealed name; its fields are sealed again.
    pub(crate) fn write_field_edit(&self, ids: &[ItemId], edit: &ItemEdit) -> Result<u32> {
        let targets = self.edit_targets(ids)?;
        let moves = match edit {
            ItemEdit::Rename(name) => self.prepare_moves(&targets, name)?,
            _ => Vec::new(),
        };
        if let Err(error) = self.commit_field_edit(&targets, edit, &moves) {
            self.abandon_moves(&moves);
            return Err(error);
        }
        self.finish_moves(&moves);
        Ok(u32::try_from(targets.len()).unwrap_or(u32::MAX))
    }

    /// The visible items among `ids`; hidden and unknown ones are left alone.
    fn edit_targets(&self, ids: &[ItemId]) -> Result<Vec<Target>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let sql = format!(
            "SELECT seq, file_path, ext, encrypted, sealed FROM item WHERE id = :id AND {VISIBLE}"
        );
        let mut statement = conn.prepare(&sql).context("edit items")?;
        let mut targets = Vec::new();
        for id in ids {
            let found = statement
                .query_row(
                    named_params! {
                        ":id": id.0,
                        ":hidden": visibility.hidden,
                        ":unlocked": visibility.unlocked,
                    },
                    |row| {
                        let sealed = if row.get::<_, bool>(3)? {
                            Some(row.get::<_, Option<Vec<u8>>>(4)?.unwrap_or_default())
                        } else {
                            None
                        };
                        Ok(Target {
                            id: id.0.clone(),
                            seq: row.get(0)?,
                            file_path: row.get(1)?,
                            ext: row.get(2)?,
                            sealed,
                        })
                    },
                )
                .optional()
                .context("edit items")?;
            if let Some(target) = found
                && !targets.iter().any(|known: &Target| known.id == target.id)
            {
                targets.push(target);
            }
        }
        Ok(targets)
    }

    /// Puts each plaintext original under its new name next to the old one. On an error, what
    /// was done so far is undone.
    fn prepare_moves(&self, targets: &[Target], name: &str) -> Result<Vec<Move>> {
        let mut moves = Vec::new();
        for (at, target) in targets.iter().enumerate() {
            if target.sealed.is_some() {
                continue;
            }
            let Some((folder, _)) = target.file_path.rsplit_once('/') else {
                continue;
            };
            let new = format!("{folder}/{}", file_name_for(name, &target.ext));
            if new == target.file_path {
                continue;
            }
            let (Some(old_path), Some(new_path)) = (
                library_file(self.root(), &target.file_path),
                library_file(self.root(), &new),
            ) else {
                self.abandon_moves(&moves);
                return Err(SourceError::Storage("bad stored path".into()));
            };
            // A missing original keeps its path; Check Library reports it.
            if !old_path.is_file() {
                continue;
            }
            let mut step = Move {
                item: at,
                old: target.file_path.clone(),
                new,
                old_path,
                new_path,
                journaled: false,
            };
            let placed = if same_file(&step.old_path, &step.new_path) {
                // Only the case changes and the file system ignores case: the old path keeps
                // finding the file, so it is renamed in place, without the journal (which would
                // delete it on recovery).
                rename_with_retry(&step.old_path, &step.new_path).context("rename the original")
            } else {
                step.journaled = true;
                self.journal(&step.new)
                    .and_then(|()| self.link_or_copy(&step.old_path, &step.new_path))
            };
            if let Err(error) = placed {
                if step.journaled {
                    self.forget_journal(&step.new);
                }
                self.abandon_moves(&moves);
                return Err(error);
            }
            moves.push(step);
        }
        Ok(moves)
    }

    fn journal(&self, stored: &str) -> Result<()> {
        self.conn()
            .execute(
                "INSERT OR IGNORE INTO pending_file (path, created_at) VALUES (?1, ?2)",
                params![stored, now_ms()],
            )
            .context("write the journal")?;
        Ok(())
    }

    /// A hard link costs nothing; a drive without them (FAT32, exFAT) gets a flushed copy
    /// through `files/.tmp`.
    fn link_or_copy(&self, from: &Path, to: &Path) -> Result<()> {
        match fs::hard_link(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(SourceError::Storage(format!(
                    "a file named {} is already in the item's folder",
                    to.file_name().unwrap_or_default().to_string_lossy()
                )));
            }
            Err(error) => tracing::info!("no hard link for a rename, copying: {error}"),
        }
        let tmp = self
            .root()
            .join("files")
            .join(".tmp")
            .join(Ulid::generate().to_string());
        let copied = fs::copy(from, &tmp)
            .and_then(|_| fs::File::open(&tmp)?.sync_all())
            .and_then(|()| rename_with_retry(&tmp, to));
        if let Err(error) = copied {
            let _ = fs::remove_file(&tmp);
            return Err(error).context("copy the original under its new name");
        }
        Ok(())
    }

    fn commit_field_edit(&self, targets: &[Target], edit: &ItemEdit, moves: &[Move]) -> Result<()> {
        let now = now_ms();
        let new_paths: HashMap<usize, &str> = moves
            .iter()
            .map(|step| (step.item, step.new.as_str()))
            .collect();
        let mut conn = self.conn();
        let tx = conn.transaction().context("edit items")?;
        let mut resealed: Vec<(String, SealedFields)> = Vec::new();
        for (at, target) in targets.iter().enumerate() {
            let id = target.id.as_str();
            match (edit, &target.sealed) {
                (ItemEdit::SetStar(star), _) => execute(
                    &tx,
                    "UPDATE item SET star = ?2 WHERE id = ?1",
                    params![id, star],
                )?,
                (ItemEdit::Trash, _) => execute(
                    &tx,
                    "UPDATE item SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
                    params![id, now],
                )?,
                (ItemEdit::Restore, _) => execute(
                    &tx,
                    "UPDATE item SET deleted_at = NULL WHERE id = ?1",
                    params![id],
                )?,
                (
                    ItemEdit::SetAnnotation(_) | ItemEdit::SetUrl(_) | ItemEdit::Rename(_),
                    Some(blob),
                ) => {
                    let mut fields = self.open_fields(&tx, id, blob)?;
                    match edit {
                        ItemEdit::SetAnnotation(text) => fields.annotation.clone_from(text),
                        ItemEdit::SetUrl(url) => fields.url.clone_from(url),
                        ItemEdit::Rename(name) => fields.name.clone_from(name),
                        _ => {}
                    }
                    let key = self.item_key_with(&tx, id)?;
                    execute(
                        &tx,
                        "UPDATE item SET sealed = ?2 WHERE id = ?1",
                        params![id, fields.seal(&key)],
                    )?;
                    resealed.push((target.id.clone(), fields));
                }
                (ItemEdit::SetAnnotation(text), None) => {
                    execute(
                        &tx,
                        "UPDATE item SET annotation = ?2 WHERE id = ?1",
                        params![id, text],
                    )?;
                    execute(
                        &tx,
                        "UPDATE item_fts SET annotation = ?2 WHERE rowid = ?1",
                        params![target.seq, text],
                    )?;
                }
                (ItemEdit::SetUrl(url), None) => {
                    execute(
                        &tx,
                        "UPDATE item SET url = ?2 WHERE id = ?1",
                        params![id, url],
                    )?;
                    execute(
                        &tx,
                        "UPDATE item_fts SET url = ?2 WHERE rowid = ?1",
                        params![target.seq, url],
                    )?;
                }
                (ItemEdit::Rename(name), None) => {
                    execute(
                        &tx,
                        "UPDATE item SET name = ?2, file_path = COALESCE(?3, file_path) WHERE id = ?1",
                        params![id, name, new_paths.get(&at)],
                    )?;
                    execute(
                        &tx,
                        "UPDATE item_fts SET name = ?2 WHERE rowid = ?1",
                        params![target.seq, name],
                    )?;
                }
                _ => unreachable!("folder and tag edits are written in folders.rs"),
            }
            execute(
                &tx,
                "UPDATE item SET updated_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
            log(&tx, "item", id, "update", now)?;
        }
        // The new names are referenced now; the old ones wait in the journal until removed.
        for step in moves.iter().filter(|step| step.journaled) {
            execute(
                &tx,
                "DELETE FROM pending_file WHERE path = ?1",
                params![step.new],
            )?;
            execute(
                &tx,
                "INSERT OR IGNORE INTO pending_file (path, created_at) VALUES (?1, ?2)",
                params![step.old, now],
            )?;
        }
        tx.commit().context("edit items")?;
        let mut keys = self.keys();
        for (id, fields) in resealed {
            keys.remember_fields(&id, fields);
        }
        Ok(())
    }

    /// The row did not change: the new names go, and a case-only rename goes back.
    fn abandon_moves(&self, moves: &[Move]) {
        for step in moves {
            if step.journaled {
                if fs::remove_file(&step.new_path).is_ok() {
                    self.forget_journal(&step.new);
                }
            } else {
                let _ = rename_with_retry(&step.new_path, &step.old_path);
            }
        }
    }

    /// The row points at the new names: the old ones go. One that cannot be removed now (open
    /// in another app) stays in the journal, and the next open removes it.
    fn finish_moves(&self, moves: &[Move]) {
        for step in moves.iter().filter(|step| step.journaled) {
            match fs::remove_file(&step.old_path) {
                Ok(()) => self.forget_journal(&step.old),
                Err(error) if error.kind() == ErrorKind::NotFound => self.forget_journal(&step.old),
                Err(error) => {
                    tracing::warn!("cannot remove the old name of a renamed file: {error}")
                }
            }
        }
    }

    /// Delete Permanently: items of the trash among `ids`, with their files. No undo.
    pub fn delete_items(&self, ids: &[ItemId]) -> Result<u32> {
        self.check_writable()?;
        let ids: Vec<String> = ids.iter().map(|id| id.0.clone()).collect();
        self.remove_items(&ids, Removable::Trashed)
    }

    /// Empty Trash: every visible item of the trash, with its files. Items of locked folders
    /// stay until their folder is unlocked.
    pub fn empty_trash(&self) -> Result<u32> {
        self.check_writable()?;
        let ids: Vec<String> = {
            let conn = self.conn();
            let visibility = self.visibility(&conn)?;
            let mut statement = conn
                .prepare(&format!(
                    "SELECT id FROM item WHERE deleted_at IS NOT NULL AND {VISIBLE}"
                ))
                .context("read the trash")?;
            statement
                .query_map(
                    named_params! {
                        ":hidden": visibility.hidden,
                        ":unlocked": visibility.unlocked,
                    },
                    |row| row.get(0),
                )
                .and_then(Iterator::collect)
                .context("read the trash")?
        };
        self.remove_items(&ids, Removable::Trashed)
    }

    /// Removes visible items for good. Their rows go in one transaction that also journals the
    /// original and both preview names; the files are deleted after it commits, and a file that
    /// cannot be deleted now is deleted on the next open (docs/library-format.md §6).
    pub(crate) fn remove_items(&self, ids: &[String], which: Removable) -> Result<u32> {
        let now = now_ms();
        let mut files: Vec<String> = Vec::new();
        let mut removed: Vec<String> = Vec::new();
        {
            let mut conn = self.conn();
            let visibility = self.visibility(&conn)?;
            let tx = conn.transaction().context("remove items")?;
            let condition = match which {
                Removable::Trashed => "deleted_at IS NOT NULL",
                Removable::Any => "1",
            };
            let sql = format!(
                "SELECT seq, file_path FROM item WHERE id = :id AND {condition} AND {VISIBLE}"
            );
            let mut tags: Vec<i64> = Vec::new();
            for id in ids {
                let found: Option<(i64, String)> = tx
                    .prepare_cached(&sql)
                    .and_then(|mut statement| {
                        statement
                            .query_row(
                                named_params! {
                                    ":id": id,
                                    ":hidden": visibility.hidden,
                                    ":unlocked": visibility.unlocked,
                                },
                                |row| Ok((row.get(0)?, row.get(1)?)),
                            )
                            .optional()
                    })
                    .context("remove items")?;
                let Some((seq, file_path)) = found else {
                    continue;
                };
                let mut statement = tx
                    .prepare_cached("SELECT tag_id FROM item_tag WHERE item_id = ?1")
                    .context("remove items")?;
                for tag in statement
                    .query_map(params![id], |row| row.get::<_, i64>(0))
                    .context("remove items")?
                {
                    let tag = tag.context("remove items")?;
                    if !tags.contains(&tag) {
                        tags.push(tag);
                    }
                }
                drop(statement);
                let shard = &id[id.len().saturating_sub(2)..];
                for stored in [
                    file_path,
                    format!("thumbs/{shard}/{id}.webp"),
                    format!("thumbs/{shard}/{id}.age"),
                ] {
                    execute(
                        &tx,
                        "INSERT OR IGNORE INTO pending_file (path, created_at) VALUES (?1, ?2)",
                        params![stored, now],
                    )?;
                    files.push(stored);
                }
                execute(&tx, "DELETE FROM item_fts WHERE rowid = ?1", params![seq])?;
                // Links, tags, comments and key wraps go with the row (ON DELETE CASCADE).
                execute(&tx, "DELETE FROM item WHERE id = ?1", params![id])?;
                log(&tx, "item", id, "delete", now)?;
                removed.push(id.clone());
            }
            drop_unused(&tx, &tags)?;
            tx.commit().context("remove items")?;
        }
        {
            let mut keys = self.keys();
            for id in &removed {
                keys.forget_item(id);
            }
        }
        for stored in &files {
            let deleted = match journal_file(self.root(), stored) {
                Some(path) if stored.starts_with("files/") => {
                    remove_with_empty_parent(&path).is_ok()
                }
                Some(path) => match fs::remove_file(&path) {
                    Ok(()) => true,
                    Err(error) => error.kind() == ErrorKind::NotFound,
                },
                None => true,
            };
            if deleted {
                self.forget_journal(stored);
            }
        }
        Ok(u32::try_from(removed.len()).unwrap_or(u32::MAX))
    }

    /// What an edit of `ids` may change, read just before it, for undo. Hidden and unknown
    /// items are left out, like the edit leaves them alone.
    pub fn item_states(&self, ids: &[ItemId]) -> Result<Vec<ItemState>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        let mut states = Vec::new();
        for id in ids {
            if let Some(state) = self.item_state(&conn, &visibility, &id.0)? {
                states.push(state);
            }
        }
        Ok(states)
    }

    fn item_state(
        &self,
        conn: &Connection,
        visibility: &Visibility,
        id: &str,
    ) -> Result<Option<ItemState>> {
        let sql = format!(
            "SELECT name, star, annotation, url, deleted_at IS NOT NULL, encrypted, sealed \
             FROM item WHERE id = :id AND {VISIBLE}"
        );
        let found = conn
            .prepare_cached(&sql)
            .and_then(|mut statement| {
                statement
                    .query_row(
                        named_params! {
                            ":id": id,
                            ":hidden": visibility.hidden,
                            ":unlocked": visibility.unlocked,
                        },
                        |row| {
                            let sealed = if row.get::<_, bool>(5)? {
                                Some(row.get::<_, Option<Vec<u8>>>(6)?.unwrap_or_default())
                            } else {
                                None
                            };
                            Ok((
                                ItemState {
                                    id: ItemId(id.to_owned()),
                                    name: row.get(0)?,
                                    star: row.get(1)?,
                                    annotation: row.get(2)?,
                                    url: row.get(3)?,
                                    in_trash: row.get(4)?,
                                    ..ItemState::default()
                                },
                                sealed,
                            ))
                        },
                    )
                    .optional()
            })
            .context("read an item")?;
        let Some((mut state, sealed)) = found else {
            return Ok(None);
        };
        if let Some(blob) = sealed {
            let fields = self.open_fields(conn, id, &blob)?;
            state.name = fields.name;
            state.annotation = fields.annotation;
            state.url = fields.url;
        }
        state.tags = conn
            .prepare_cached(
                "SELECT tag.name FROM item_tag JOIN tag ON tag.id = item_tag.tag_id \
                 WHERE item_tag.item_id = ?1",
            )
            .and_then(|mut statement| {
                statement
                    .query_map(params![id], |row| row.get(0))?
                    .collect()
            })
            .context("read the item's tags")?;
        state.folders = conn
            .prepare_cached("SELECT folder_id FROM item_folder WHERE item_id = ?1")
            .and_then(|mut statement| {
                statement
                    .query_map(params![id], |row| row.get(0))?
                    .collect()
            })
            .context("read the item's folders")?;
        Ok(Some(state))
    }
}

fn execute(tx: &Transaction<'_>, sql: &str, params: impl rusqlite::Params) -> Result<()> {
    tx.prepare_cached(sql)
        .and_then(|mut statement| statement.execute(params))
        .context("edit items")?;
    Ok(())
}

/// Both paths name one existing file: a file system that ignores case, and a new name that
/// differs only in case.
fn same_file(old: &Path, new: &Path) -> bool {
    match (fs::canonicalize(old), fs::canonicalize(new)) {
        (Ok(old), Ok(new)) => old == new,
        _ => false,
    }
}
