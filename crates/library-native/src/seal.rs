//! Brings items in line with the locks that cover them (docs/age-locked-folders.md §2, §6): an
//! item under a lock is encrypted with its own key, wrapped once per covering lock; an item
//! under none is plaintext. Every file swap goes through the `pending_file` journal, so a crash
//! leaves neither a row without its file nor a stray copy (docs/library-format.md §6).

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use library_core::SourceError;
use rusqlite::{Connection, OptionalExtension, params};
use ulid::Ulid;

use crate::add::file_name_for;
use crate::error::{Context, Result};
use crate::fsutil::rename_with_retry;
use crate::library::Library;
use crate::locks::{LockMap, sealing_locks, vault_error};
use crate::recover::{library_file, remove_with_empty_parent};
use crate::time::now_ms;
use crate::vault::{FolderKey, ItemKey, SealedFields};

const BUFFER: usize = 1024 * 1024;

/// What one item needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Plan {
    pub item: String,
    pub encrypted: bool,
    /// Locks the item's key is wrapped for now.
    pub wraps: BTreeSet<String>,
    /// Locks that cover the item.
    pub required: BTreeSet<String>,
}

impl Plan {
    fn is_done(&self) -> bool {
        self.encrypted == !self.required.is_empty() && self.wraps == self.required
    }
}

/// Maps an item's folder links to the links a planned write would leave.
pub(crate) type Links<'a> = &'a dyn Fn(&str, Vec<String>) -> Vec<String>;

impl Library {
    /// What `items` need under `map`, with their links changed by `links`; items already in
    /// line are left out.
    pub(crate) fn plans(
        &self,
        items: &[String],
        map: &LockMap,
        links: Links<'_>,
    ) -> Result<Vec<Plan>> {
        let conn = self.conn();
        let mut plans = Vec::new();
        for item in items {
            let Some(encrypted) = conn
                .prepare_cached("SELECT encrypted FROM item WHERE id = ?1")
                .and_then(|mut statement| {
                    statement
                        .query_row(params![item], |row| row.get::<_, bool>(0))
                        .optional()
                })
                .context("plan encryption")?
            else {
                continue;
            };
            let folders = column(
                &conn,
                "SELECT folder_id FROM item_folder WHERE item_id = ?1",
                item,
            )?;
            let wraps = column(
                &conn,
                "SELECT folder_id FROM item_key_wrap WHERE item_id = ?1",
                item,
            )?;
            let plan = Plan {
                item: item.clone(),
                encrypted,
                wraps: wraps.into_iter().collect(),
                required: sealing_locks(map, &links(item, folders)),
            };
            if !plan.is_done() {
                plans.push(plan);
            }
        }
        Ok(plans)
    }

    /// Refuses before anything is written when a planned change needs a key that is not
    /// unlocked: the item key of an encrypted item, or a lock to wrap for. `extra` is a lock
    /// being made, not stored yet.
    pub(crate) fn check_reseal(
        &self,
        items: &[String],
        map: &LockMap,
        links: Links<'_>,
        extra: Option<(&str, &FolderKey)>,
    ) -> Result<()> {
        let plans = self.plans(items, map, links)?;
        let keys = self.keys();
        let usable = |lock: &str| keys.is_unlocked(lock) || extra.is_some_and(|(id, _)| id == lock);
        for plan in &plans {
            if plan.encrypted
                && keys.cached_item(&plan.item).is_none()
                && !plan.wraps.iter().any(|lock| usable(lock))
            {
                let lock = plan.wraps.first().cloned().unwrap_or_default();
                return Err(SourceError::Locked {
                    folder: map.name(&lock),
                });
            }
            let missing = plan
                .required
                .iter()
                .filter(|lock| !plan.encrypted || !plan.wraps.contains(*lock))
                .find(|lock| !usable(lock));
            if let Some(lock) = missing {
                return Err(SourceError::Locked {
                    folder: map.name(lock),
                });
            }
        }
        Ok(())
    }

    /// Brings `items` in line with the locks stored now.
    pub(crate) fn reseal(&self, items: &[String], progress: &dyn Fn(u32, u32)) -> Result<()> {
        let map = LockMap::load(&self.conn())?;
        self.reseal_with(items, &map, &|_, folders| folders, progress)
    }

    /// Brings `items` in line with `map` and `links`, which may differ from what is stored (a
    /// lock being removed, folders being deleted).
    pub(crate) fn reseal_with(
        &self,
        items: &[String],
        map: &LockMap,
        links: Links<'_>,
        progress: &dyn Fn(u32, u32),
    ) -> Result<()> {
        let plans = self.plans(items, map, links)?;
        let total = u32::try_from(plans.len()).unwrap_or(u32::MAX);
        progress(0, total);
        for (done, plan) in plans.iter().enumerate() {
            self.apply(plan, map)?;
            progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
        }
        Ok(())
    }

    fn apply(&self, plan: &Plan, map: &LockMap) -> Result<()> {
        let recipients = |locks: &mut dyn Iterator<Item = &String>| -> Result<Vec<(String, age::x25519::Recipient)>> {
            let keys = self.keys();
            locks
                .map(|lock| match keys.folder(lock) {
                    Some(key) => Ok((lock.clone(), key.recipient())),
                    None => Err(SourceError::Locked {
                        folder: map.name(lock),
                    }),
                })
                .collect()
        };
        match (plan.required.is_empty(), plan.encrypted) {
            (true, false) => Ok(()),
            (true, true) => {
                let key = self.item_key(&plan.item)?;
                self.decrypt_item(&plan.item, &key)
            }
            (false, false) => {
                let wraps = recipients(&mut plan.required.iter())?;
                self.encrypt_item(&plan.item, ItemKey::generate(), &wraps)
            }
            (false, true) => {
                let key = self.item_key(&plan.item)?;
                let added = recipients(&mut plan.required.difference(&plan.wraps))?;
                let removed: Vec<&String> = plan.wraps.difference(&plan.required).collect();
                let now = now_ms();
                let mut conn = self.conn();
                let tx = conn.transaction().context("wrap an item key")?;
                for (lock, recipient) in &added {
                    tx.execute(
                        "INSERT OR REPLACE INTO item_key_wrap (item_id, folder_id, wrapped_key)
                         VALUES (?1, ?2, ?3)",
                        params![plan.item, lock, key.wrap_for(recipient)],
                    )
                    .context("wrap an item key")?;
                }
                for lock in removed {
                    tx.execute(
                        "DELETE FROM item_key_wrap WHERE item_id = ?1 AND folder_id = ?2",
                        params![plan.item, lock],
                    )
                    .context("unwrap an item key")?;
                }
                log_item(&tx, &plan.item, now)?;
                tx.commit().context("wrap an item key")
            }
        }
    }

    /// The key of an encrypted item, from the session or through a wrap of an unlocked folder.
    pub(crate) fn item_key(&self, item: &str) -> Result<Arc<ItemKey>> {
        let conn = self.conn();
        self.item_key_with(&conn, item)
    }

    /// Like [`Library::item_key`] on a connection the caller holds.
    pub(crate) fn item_key_with(&self, conn: &Connection, item: &str) -> Result<Arc<ItemKey>> {
        if let Some(key) = self.keys().cached_item(item) {
            return Ok(key);
        }
        let mut statement = conn
            .prepare_cached("SELECT folder_id, wrapped_key FROM item_key_wrap WHERE item_id = ?1")
            .context("read an item key")?;
        let wraps: Vec<(String, Vec<u8>)> = statement
            .query_map(params![item], |row| Ok((row.get(0)?, row.get(1)?)))
            .and_then(Iterator::collect)
            .context("read an item key")?;
        let mut keys = self.keys();
        let mut first_lock = None;
        for (lock, blob) in &wraps {
            first_lock.get_or_insert(lock.as_str());
            if let Some(folder) = keys.folder(lock) {
                let key = Arc::new(ItemKey::unwrap(blob, folder).map_err(vault_error)?);
                keys.remember_item(item, Arc::clone(&key));
                return Ok(key);
            }
        }
        Err(SourceError::Locked {
            folder: first_lock
                .map(|lock| folder_name(conn, lock))
                .unwrap_or_default(),
        })
    }

    /// The sealed fields of an encrypted item, from the session or opened now.
    pub(crate) fn open_fields(
        &self,
        conn: &Connection,
        item: &str,
        sealed: &[u8],
    ) -> Result<SealedFields> {
        if let Some(fields) = self.keys().cached_fields(item) {
            return Ok(fields.clone());
        }
        let key = self.item_key_with(conn, item)?;
        let fields = SealedFields::open(sealed, &key).map_err(vault_error)?;
        self.keys().remember_fields(item, fields.clone());
        Ok(fields)
    }

    /// The bytes of an encrypted file of an item, decrypted in memory.
    pub(crate) fn read_sealed(&self, item: &str, path: &Path) -> Result<Option<Vec<u8>>> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("read an encrypted file"),
        };
        let key = self.item_key(item)?;
        let mut plain = Vec::new();
        key.decrypt_from(io::BufReader::new(file))
            .map_err(vault_error)?
            .read_to_end(&mut plain)
            .context("decrypt a file")?;
        Ok(Some(plain))
    }

    /// `thumbs/<shard>/<id>.age`: the preview of an encrypted item.
    pub(crate) fn sealed_thumbnail_path(&self, id: &str) -> Result<PathBuf> {
        Ok(self.thumbnail_path(id)?.with_extension("age"))
    }

    fn encrypt_item(
        &self,
        item: &str,
        key: ItemKey,
        wraps: &[(String, age::x25519::Recipient)],
    ) -> Result<()> {
        let row = self.seal_row(item)?;
        if row.encrypted {
            return Ok(());
        }
        let stored = sealed_stored_path(item);
        let target = library_file(self.root(), &stored)
            .ok_or_else(|| SourceError::Storage("bad stored path".into()))?;
        let source = library_file(self.root(), &row.file_path);
        let wrote = match &source {
            Some(source) if source.is_file() => {
                self.swap_in(&stored, &target, |output| {
                    let mut input = File::open(source)?;
                    let mut writer = key.encrypt_to(output)?;
                    io::copy(&mut input, &mut writer)?;
                    writer.finish()?.sync_all()
                })?;
                true
            }
            _ => false,
        };
        let plain_thumb = self.thumbnail_path(item)?;
        let sealed_thumb = self.sealed_thumbnail_path(item)?;
        let had_thumb = plain_thumb.is_file();
        if had_thumb {
            let bytes = fs::read(&plain_thumb).context("read a thumbnail")?;
            write_atomically(&sealed_thumb, &key.seal(&bytes))?;
        }
        let fields = SealedFields {
            name: row.name,
            annotation: row.annotation,
            url: row.url,
            exif: row.exif,
            palette: row.palette,
        };
        let sealed = fields.seal(&key);
        let old_thumb = journal_name(self.root(), &plain_thumb);
        let now = now_ms();
        {
            let mut conn = self.conn();
            let tx = conn.transaction().context("encrypt an item")?;
            tx.execute(
                "UPDATE item SET encrypted = 1, sealed = ?2, name = '', annotation = '', url = '',
                     exif = NULL, palette = NULL, file_path = ?3, updated_at = ?4
                 WHERE id = ?1",
                params![item, sealed, stored, now],
            )
            .context("encrypt an item")?;
            tx.execute("DELETE FROM item_fts WHERE rowid = ?1", params![row.seq])
                .context("encrypt an item")?;
            for (lock, recipient) in wraps {
                tx.execute(
                    "INSERT OR REPLACE INTO item_key_wrap (item_id, folder_id, wrapped_key)
                     VALUES (?1, ?2, ?3)",
                    params![item, lock, key.wrap_for(recipient)],
                )
                .context("wrap an item key")?;
            }
            tx.execute("DELETE FROM pending_file WHERE path = ?1", params![stored])
                .context("encrypt an item")?;
            // The plaintext copies go once the row no longer names them.
            if wrote {
                journal(&tx, &row.file_path, now)?;
            }
            if had_thumb {
                journal(&tx, &old_thumb, now)?;
            }
            log_item(&tx, item, now)?;
            tx.commit().context("encrypt an item")?;
        }
        if wrote && let Some(source) = source {
            remove_with_empty_parent(&source)?;
            self.forget_journal(&row.file_path);
        }
        if had_thumb {
            remove_with_empty_parent(&plain_thumb)?;
            self.forget_journal(&old_thumb);
        }
        let mut keys = self.keys();
        keys.remember_item(item, Arc::new(key));
        keys.remember_fields(item, fields);
        Ok(())
    }

    fn decrypt_item(&self, item: &str, key: &ItemKey) -> Result<()> {
        let row = self.seal_row(item)?;
        if !row.encrypted {
            return Ok(());
        }
        let sealed = row
            .sealed
            .as_deref()
            .ok_or_else(|| SourceError::Storage("an encrypted item has no sealed fields".into()))?;
        let fields = SealedFields::open(sealed, key).map_err(vault_error)?;
        let shard = &item[item.len().saturating_sub(2)..];
        let stored = format!(
            "files/{shard}/{item}/{}",
            file_name_for(&fields.name, &row.ext)
        );
        let target = library_file(self.root(), &stored)
            .ok_or_else(|| SourceError::Storage("bad stored path".into()))?;
        let source = library_file(self.root(), &row.file_path);
        let wrote = match &source {
            Some(source) if source.is_file() => {
                self.swap_in(&stored, &target, |mut output| {
                    let input = File::open(source)?;
                    let mut reader = key
                        .decrypt_from(io::BufReader::new(input))
                        .map_err(io::Error::other)?;
                    io::copy(&mut reader, &mut output)?;
                    output.sync_all()
                })?;
                true
            }
            _ => false,
        };
        let plain_thumb = self.thumbnail_path(item)?;
        let sealed_thumb = self.sealed_thumbnail_path(item)?;
        let had_thumb = sealed_thumb.is_file();
        if had_thumb {
            let bytes = fs::read(&sealed_thumb).context("read a thumbnail")?;
            write_atomically(&plain_thumb, &key.open(&bytes).map_err(vault_error)?)?;
        }
        let old_thumb = journal_name(self.root(), &sealed_thumb);
        let now = now_ms();
        {
            let mut conn = self.conn();
            let tx = conn.transaction().context("decrypt an item")?;
            tx.execute(
                "UPDATE item SET encrypted = 0, sealed = NULL, name = ?2, annotation = ?3,
                     url = ?4, exif = ?5, palette = ?6, file_path = ?7, updated_at = ?8
                 WHERE id = ?1",
                params![
                    item,
                    fields.name,
                    fields.annotation,
                    fields.url,
                    fields.exif,
                    fields.palette,
                    if wrote { &stored } else { &row.file_path },
                    now
                ],
            )
            .context("decrypt an item")?;
            tx.execute("DELETE FROM item_fts WHERE rowid = ?1", params![row.seq])
                .context("index the item")?;
            tx.execute(
                "INSERT INTO item_fts (rowid, name, annotation, url) VALUES (?1, ?2, ?3, ?4)",
                params![row.seq, fields.name, fields.annotation, fields.url],
            )
            .context("index the item")?;
            tx.execute(
                "DELETE FROM item_key_wrap WHERE item_id = ?1",
                params![item],
            )
            .context("decrypt an item")?;
            tx.execute("DELETE FROM pending_file WHERE path = ?1", params![stored])
                .context("decrypt an item")?;
            if wrote {
                journal(&tx, &row.file_path, now)?;
            }
            if had_thumb {
                journal(&tx, &old_thumb, now)?;
            }
            log_item(&tx, item, now)?;
            tx.commit().context("decrypt an item")?;
        }
        if wrote && let Some(source) = source {
            remove_with_empty_parent(&source)?;
            self.forget_journal(&row.file_path);
        }
        if had_thumb {
            remove_with_empty_parent(&sealed_thumb)?;
            self.forget_journal(&old_thumb);
        }
        self.keys().forget_item(item);
        Ok(())
    }

    /// Writes a new copy of a file through `files/.tmp/`, journals its target, and renames it
    /// into place. The row is updated by the caller; until then the journal names the copy.
    fn swap_in(
        &self,
        stored: &str,
        target: &Path,
        write: impl FnOnce(File) -> io::Result<()>,
    ) -> Result<()> {
        let tmp = self
            .root()
            .join("files")
            .join(".tmp")
            .join(Ulid::generate().to_string());
        let written = File::create_new(&tmp).and_then(write);
        if let Err(error) = written {
            let _ = fs::remove_file(&tmp);
            return Err(error).context("write an encrypted copy");
        }
        journal_now(&self.conn(), stored)?;
        let renamed = target
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| rename_with_retry(&tmp, target));
        if let Err(error) = renamed {
            let _ = fs::remove_file(&tmp);
            self.forget_journal(stored);
            return Err(error).context("move the copy into place");
        }
        Ok(())
    }

    fn seal_row(&self, item: &str) -> Result<SealRow> {
        self.conn()
            .query_row(
                "SELECT seq, file_path, name, annotation, url, exif, palette, encrypted, sealed, ext
                 FROM item WHERE id = ?1",
                params![item],
                |row| {
                    Ok(SealRow {
                        seq: row.get(0)?,
                        file_path: row.get(1)?,
                        name: row.get(2)?,
                        annotation: row.get(3)?,
                        url: row.get(4)?,
                        exif: row.get(5)?,
                        palette: row.get(6)?,
                        encrypted: row.get(7)?,
                        sealed: row.get(8)?,
                        ext: row.get(9)?,
                    })
                },
            )
            .optional()
            .context("read an item")?
            .ok_or(SourceError::NotFound)
    }

    /// Best effort: if it fails, recovery on the next open clears the row.
    pub(crate) fn forget_journal(&self, stored: &str) {
        let _ = self
            .conn()
            .execute("DELETE FROM pending_file WHERE path = ?1", [stored]);
    }
}

struct SealRow {
    seq: i64,
    file_path: String,
    name: String,
    annotation: String,
    url: String,
    exif: Option<String>,
    palette: Option<String>,
    encrypted: bool,
    sealed: Option<Vec<u8>>,
    ext: String,
}

/// Where the original of an encrypted item lives: no file name, which is sealed with the rest.
pub(crate) fn sealed_stored_path(item: &str) -> String {
    let shard = &item[item.len().saturating_sub(2)..];
    format!("files/{shard}/{item}/{item}.age")
}

/// A path inside the library as the journal stores it: `files/…` or `thumbs/…`.
fn journal_name(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn journal(tx: &rusqlite::Transaction<'_>, stored: &str, now: i64) -> Result<()> {
    tx.execute(
        "INSERT OR IGNORE INTO pending_file (path, created_at) VALUES (?1, ?2)",
        params![stored, now],
    )
    .context("write the journal")?;
    Ok(())
}

fn journal_now(conn: &Connection, stored: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO pending_file (path, created_at) VALUES (?1, ?2)",
        params![stored, now_ms()],
    )
    .context("write the journal")?;
    Ok(())
}

/// A small file (a preview) through a temp file beside it, then renamed into place.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
    fs::create_dir_all(&dir).context("create a folder")?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp"));
    let written = File::create(&tmp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(error) = written {
        let _ = fs::remove_file(&tmp);
        return Err(error).context("write a file");
    }
    if let Err(error) = rename_with_retry(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error).context("move a file into place");
    }
    Ok(())
}

fn column(conn: &Connection, sql: &str, item: &str) -> Result<Vec<String>> {
    conn.prepare_cached(sql)
        .and_then(|mut statement| {
            statement
                .query_map(params![item], |row| row.get(0))?
                .collect()
        })
        .context("read an item's locks")
}

fn folder_name(conn: &Connection, id: &str) -> String {
    conn.query_row(
        "SELECT name FROM folder WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
    .unwrap_or_default()
}

fn log_item(tx: &rusqlite::Transaction<'_>, item: &str, now: i64) -> Result<()> {
    tx.execute(
        "INSERT INTO change_log (entity, entity_id, op, at) VALUES ('item', ?1, 'update', ?2)",
        params![item, now],
    )
    .context("log the change")?;
    Ok(())
}

/// Copies `source` into `tmp` encrypted to `key` while hashing the plaintext: a file added to a
/// locked folder never lies on disk in plaintext. Returns the BLAKE3 of the plaintext and its
/// length.
pub(crate) fn copy_hash_encrypt(
    source: &Path,
    tmp: &Path,
    key: &ItemKey,
) -> Result<(blake3::Hash, u64)> {
    let mut input = File::open(source).context("open the source file")?;
    let output = File::create_new(tmp).context("create a temp file")?;
    let mut writer = key.encrypt_to(output).context("start encrypting")?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; BUFFER];
    let mut copied: u64 = 0;
    loop {
        let read = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("read the source file"),
        };
        hasher.update(&buffer[..read]);
        writer
            .write_all(&buffer[..read])
            .context("write the encrypted copy")?;
        copied += read as u64;
    }
    writer
        .finish()
        .and_then(|file| file.sync_all())
        .context("flush the encrypted copy")?;
    Ok((hasher.finalize(), copied))
}
