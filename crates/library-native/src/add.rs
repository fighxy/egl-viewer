//! Adding one file: a temp copy with BLAKE3, the duplicate check, the `pending_file` journal,
//! the rename and the insert (docs/library-format.md §6, D-023).

use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

use std::sync::Arc;

use library_core::SourceError;
use rusqlite::{OptionalExtension, named_params, params};
use ulid::Ulid;

use crate::error::{Context, Result};
use crate::folders::{add_link, ensure_tag};
use crate::fsutil::rename_with_retry;
use crate::library::Library;
use crate::locks::{LockMap, VISIBLE};
use crate::name::{FORBIDDEN, RESERVED};
use crate::recover::{library_file, remove_with_empty_parent};
use crate::seal::{copy_hash_encrypt, sealed_stored_path};
use crate::time::{now_ms, system_ms};
use crate::vault::{ItemKey, SealedFields};

/// The longest stored file name before the extension, in UTF-16 units.
const MAX_STORED_STEM: usize = 100;
const BUFFER: usize = 1024 * 1024;

/// What to do when an item with the same content already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duplicates {
    /// Add nothing and report the existing item.
    Report,
    /// Add a second item with its own copy.
    AddAnyway,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddOutcome {
    /// `file` is the stored copy inside the library.
    Added {
        id: String,
        file: std::path::PathBuf,
    },
    Duplicate {
        existing: String,
        in_trash: bool,
    },
}

/// Where a simulated crash stops `add_file`, for the recovery tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Copied,
    Journaled,
    Renamed,
}

#[derive(Default)]
pub(crate) struct Hooks<'a> {
    /// Stop without any cleanup, as if the process died after this step.
    pub crash_after: Option<Step>,
    /// Runs between the copy and the size check: lets a test change the source.
    pub after_copy: Option<&'a dyn Fn()>,
}

/// Where a new item goes: a folder (or none: Uncategorized) and the tags it starts with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placement {
    pub folder: Option<String>,
    pub tags: Vec<String>,
}

impl Library {
    /// Copies `source` into the library as a new item. The source is only read.
    pub fn add_file(&self, source: &Path, duplicates: Duplicates) -> Result<AddOutcome> {
        self.add_file_to(source, duplicates, &Placement::default())
    }

    /// Like [`Library::add_file`], into a folder and with tags, in the same transaction as the
    /// item row.
    pub fn add_file_to(
        &self,
        source: &Path,
        duplicates: Duplicates,
        placement: &Placement,
    ) -> Result<AddOutcome> {
        self.add_file_placed(source, duplicates, placement, &Hooks::default())?
            .ok_or_else(|| SourceError::Storage("adding stopped unexpectedly".into()))
    }

    /// `Ok(None)` only when a hook simulated a crash.
    #[cfg(test)]
    pub(crate) fn add_file_with(
        &self,
        source: &Path,
        duplicates: Duplicates,
        hooks: &Hooks<'_>,
    ) -> Result<Option<AddOutcome>> {
        self.add_file_placed(source, duplicates, &Placement::default(), hooks)
    }

    fn add_file_placed(
        &self,
        source: &Path,
        duplicates: Duplicates,
        placement: &Placement,
        hooks: &Hooks<'_>,
    ) -> Result<Option<AddOutcome>> {
        self.check_writable()?;
        let metadata = fs::metadata(source).context("read the source file")?;
        if !metadata.is_file() {
            return Err(SourceError::Invalid(format!(
                "{} is not a file",
                source.display()
            )));
        }
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| SourceError::Invalid("the file name is not valid text".into()))?;

        // A folder under a lock: the copy is encrypted as it is made, so the file never lies in
        // the library in plaintext (docs/age-locked-folders.md §2).
        let seal = self.seal_for(placement.folder.as_deref())?;

        // 1. Copy into files/.tmp while hashing; 2. flush.
        let tmp = self
            .root()
            .join("files")
            .join(".tmp")
            .join(Ulid::generate().to_string());
        let copied = match &seal {
            Some((key, _)) => copy_hash_encrypt(source, &tmp, key).and_then(|(hash, copied)| {
                let after = fs::metadata(source).context("read the source file")?.len();
                if copied == metadata.len() && after == metadata.len() {
                    Ok(hash)
                } else {
                    Err(SourceError::Storage(
                        "the source file changed while it was copied".into(),
                    ))
                }
            }),
            None => copy_and_hash(source, &tmp, metadata.len(), hooks),
        };
        let hash = match copied {
            Ok(hash) => hash,
            Err(error) => {
                let _ = fs::remove_file(&tmp);
                return Err(error);
            }
        };
        if hooks.crash_after == Some(Step::Copied) {
            return Ok(None);
        }

        // 3. Duplicates, the trash included.
        if duplicates == Duplicates::Report {
            let existing = self.find_by_hash(&hash);
            match existing {
                Ok(Some((existing, in_trash))) => {
                    let _ = fs::remove_file(&tmp);
                    return Ok(Some(AddOutcome::Duplicate { existing, in_trash }));
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = fs::remove_file(&tmp);
                    return Err(error);
                }
            }
        }

        let id = Ulid::generate().to_string();
        let shard = &id[id.len() - 2..];
        let stored = match &seal {
            Some(_) => sealed_stored_path(&id),
            None => format!("files/{shard}/{id}/{}", stored_file_name(file_name)),
        };
        let target = library_file(self.root(), &stored)
            .ok_or_else(|| SourceError::Storage("bad stored path".into()))?;

        // 4. Journal the target before the file appears there.
        let now = now_ms();
        if let Err(error) = self.conn().execute(
            "INSERT INTO pending_file (path, created_at) VALUES (?1, ?2)",
            params![stored, now],
        ) {
            let _ = fs::remove_file(&tmp);
            return Err(error).context("write the add journal");
        }
        if hooks.crash_after == Some(Step::Journaled) {
            return Ok(None);
        }

        // 5. Rename into place.
        let renamed = target
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| rename_with_retry(&tmp, &target));
        if let Err(error) = renamed {
            let _ = fs::remove_file(&tmp);
            let _ = remove_with_empty_parent(&target);
            self.forget_journal(&stored);
            return Err(error).context("move the file into the library");
        }
        if hooks.crash_after == Some(Step::Renamed) {
            return Ok(None);
        }

        // 6. The row, and the journal entry gone, in one transaction.
        let (name, ext) = name_and_ext(file_name);
        let sealed = seal.as_ref().map(|(key, recipients)| {
            let fields = SealedFields {
                name: name.clone(),
                ..SealedFields::default()
            };
            let wraps: Vec<(String, Vec<u8>)> = recipients
                .iter()
                .map(|(lock, recipient)| (lock.clone(), key.wrap_for(recipient)))
                .collect();
            (fields.seal(key), wraps, fields)
        });
        let row = NewItem {
            id: &id,
            name: &name,
            ext: &ext,
            file_path: &stored,
            size: metadata.len(),
            added_at: now,
            file_created_at: metadata.created().ok().and_then(system_ms),
            file_modified_at: metadata.modified().ok().and_then(system_ms),
            content_hash: hash.as_bytes(),
            placement,
            sealed: sealed
                .as_ref()
                .map(|(blob, wraps, _)| (blob.as_slice(), wraps.as_slice())),
        };
        if let Err(error) = self.insert_item(&row) {
            if remove_with_empty_parent(&target).is_ok() {
                self.forget_journal(&stored);
            }
            return Err(error);
        }
        if let (Some((key, _)), Some((_, _, fields))) = (seal, sealed) {
            let mut keys = self.keys();
            keys.remember_item(&id, Arc::new(key));
            keys.remember_fields(&id, fields);
        }
        Ok(Some(AddOutcome::Added { id, file: target }))
    }

    /// A new item key and the locks to wrap it for, when `folder` is under a lock. Every such
    /// lock must be unlocked: adding there needs its key.
    fn seal_for(&self, folder: Option<&str>) -> Result<Option<Seal>> {
        let Some(folder) = folder else {
            return Ok(None);
        };
        let map = LockMap::load(&self.conn())?;
        let locks: Vec<String> = map.sealing(folder).map(str::to_owned).collect();
        if locks.is_empty() {
            return Ok(None);
        }
        let keys = self.keys();
        let recipients = locks
            .iter()
            .map(|lock| match keys.folder(lock) {
                Some(key) => Ok((lock.clone(), key.recipient())),
                None => Err(SourceError::Locked {
                    folder: map.name(lock),
                }),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Some((ItemKey::generate(), recipients)))
    }

    /// A visible item with the same content. Items of locked folders do not count: the
    /// question would tell that a locked folder holds this file.
    fn find_by_hash(&self, hash: &blake3::Hash) -> Result<Option<(String, bool)>> {
        let conn = self.conn();
        let visibility = self.visibility(&conn)?;
        conn.query_row(
            &format!(
                "SELECT id, deleted_at IS NOT NULL FROM item WHERE content_hash = :hash
                 AND {VISIBLE} ORDER BY deleted_at IS NOT NULL, added_at LIMIT 1"
            ),
            named_params! {
                ":hash": hash.as_bytes().as_slice(),
                ":hidden": visibility.hidden,
                ":unlocked": visibility.unlocked,
            },
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .context("look for duplicates")
    }

    fn insert_item(&self, item: &NewItem<'_>) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().context("add the item")?;
        tx.execute(
            "INSERT INTO item (id, name, ext, file_path, size, added_at, file_created_at,
                               file_modified_at, content_hash, updated_at, encrypted, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?6, ?10, ?11)",
            params![
                item.id,
                if item.sealed.is_some() { "" } else { item.name },
                item.ext,
                item.file_path,
                i64::try_from(item.size).unwrap_or(i64::MAX),
                item.added_at,
                item.file_created_at,
                item.file_modified_at,
                item.content_hash.as_slice(),
                item.sealed.is_some(),
                item.sealed.map(|(blob, _)| blob),
            ],
        )
        .context("add the item")?;
        let seq = tx.last_insert_rowid();
        match item.sealed {
            // An encrypted item has no search row: its name is sealed.
            Some((_, wraps)) => {
                for (lock, wrapped) in wraps {
                    tx.execute(
                        "INSERT INTO item_key_wrap (item_id, folder_id, wrapped_key)
                         VALUES (?1, ?2, ?3)",
                        params![item.id, lock, wrapped],
                    )
                    .context("wrap the item key")?;
                }
            }
            None => {
                tx.execute(
                    "INSERT INTO item_fts (rowid, name, annotation, url) VALUES (?1, ?2, '', '')",
                    params![seq, item.name],
                )
                .context("index the item")?;
            }
        }
        tx.execute(
            "INSERT INTO change_log (entity, entity_id, op, at) VALUES ('item', ?1, 'insert', ?2)",
            params![item.id, item.added_at],
        )
        .context("log the change")?;
        if let Some(folder) = &item.placement.folder {
            add_link(&tx, item.id, folder)?;
        }
        for tag in &item.placement.tags {
            let tag_id = ensure_tag(&tx, tag)?;
            tx.execute(
                "INSERT OR IGNORE INTO item_tag (item_id, tag_id) VALUES (?1, ?2)",
                params![item.id, tag_id],
            )
            .context("tag the item")?;
        }
        tx.execute("DELETE FROM pending_file WHERE path = ?1", [item.file_path])
            .context("clear the add journal")?;
        tx.commit().context("add the item")
    }
}

struct NewItem<'a> {
    id: &'a str,
    name: &'a str,
    ext: &'a str,
    file_path: &'a str,
    size: u64,
    added_at: i64,
    file_created_at: Option<i64>,
    file_modified_at: Option<i64>,
    content_hash: &'a [u8; 32],
    placement: &'a Placement,
    /// For a locked folder: the sealed fields and the wrapped item key per lock.
    sealed: Option<SealedRow<'a>>,
}

/// The sealed fields of a new encrypted item, and its key wrapped once per lock.
type SealedRow<'a> = (&'a [u8], &'a [(String, Vec<u8>)]);

/// The key of a new encrypted item and the locks it is wrapped for.
type Seal = (ItemKey, Vec<(String, age::x25519::Recipient)>);

/// One pass: read the source once, hash and write; then flush the copy to disk.
fn copy_and_hash(
    source: &Path,
    tmp: &Path,
    size_before: u64,
    hooks: &Hooks<'_>,
) -> Result<blake3::Hash> {
    let mut input = File::open(source).context("open the source file")?;
    let mut output = File::create_new(tmp).context("create a temp file")?;
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
        output
            .write_all(&buffer[..read])
            .context("write the copy")?;
        copied += read as u64;
    }
    if let Some(after_copy) = hooks.after_copy {
        after_copy();
    }
    let size_after = fs::metadata(source).context("read the source file")?.len();
    if copied != size_before || size_after != size_before {
        return Err(SourceError::Storage(
            "the source file changed while it was copied".into(),
        ));
    }
    output.sync_all().context("flush the copy")?;
    Ok(hasher.finalize())
}

/// The item name (the whole file name without the extension) and the lowercase extension.
fn name_and_ext(file_name: &str) -> (String, String) {
    let path = Path::new(file_name);
    let name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name)
        .to_owned();
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    (name, ext)
}

/// The stored file name for an item name, which the user may have typed: characters Windows
/// refuses become `_`, the stem is cut to 100 UTF-16 units without trailing dots or spaces, and
/// a device name gets a leading `_`. `ext` is the item's extension.
pub(crate) fn file_name_for(name: &str, ext: &str) -> String {
    let mut units = 0;
    let cut: String = name
        .chars()
        .map(|c| {
            if c.is_control() || FORBIDDEN.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .take_while(|c| {
            units += c.len_utf16();
            units <= MAX_STORED_STEM
        })
        .collect();
    let mut stem = cut.trim_end_matches(['.', ' ']).to_owned();
    if stem.trim().is_empty() {
        stem = "file".to_owned();
    }
    let device = stem.split('.').next().unwrap_or_default().trim_end();
    if RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(device))
    {
        stem.insert(0, '_');
    }
    if ext.is_empty() {
        stem
    } else {
        format!("{stem}.{ext}")
    }
}

/// The original name with the part before the extension cut to 100 UTF-16 units.
pub(crate) fn stored_file_name(file_name: &str) -> String {
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    let mut units = 0;
    let cut: String = stem
        .chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= MAX_STORED_STEM
        })
        .collect();
    let cut = if cut.trim().is_empty() {
        "file".to_owned()
    } else {
        cut
    };
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => format!("{cut}.{ext}"),
        None => cut,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_names_and_lowercases_extensions() {
        assert_eq!(
            name_and_ext("IMG_1042.JPG"),
            ("IMG_1042".into(), "jpg".into())
        );
        assert_eq!(
            name_and_ext("archive.tar.gz"),
            ("archive.tar".into(), "gz".into())
        );
        assert_eq!(name_and_ext("README"), ("README".into(), String::new()));
        assert_eq!(
            name_and_ext(".gitignore"),
            (".gitignore".into(), String::new())
        );
    }

    #[test]
    fn stored_names_keep_the_extension_and_cut_long_stems() {
        assert_eq!(stored_file_name("IMG_1042.JPG"), "IMG_1042.JPG");
        let long = format!("{}.png", "я".repeat(150));
        let stored = stored_file_name(&long);
        assert_eq!(stored, format!("{}.png", "я".repeat(100)));
        let emoji = format!("{}.png", "🖼".repeat(60));
        assert_eq!(stored_file_name(&emoji), format!("{}.png", "🖼".repeat(50)));
    }

    #[test]
    fn typed_names_become_file_names_windows_accepts() {
        assert_eq!(file_name_for("IMG_1042", "jpg"), "IMG_1042.jpg");
        assert_eq!(file_name_for("a/b:c*d?", "png"), "a_b_c_d_.png");
        assert_eq!(file_name_for("end. . ", "png"), "end.png");
        assert_eq!(file_name_for("...", "png"), "file.png");
        assert_eq!(file_name_for("con", "jpg"), "_con.jpg");
        assert_eq!(file_name_for("LPT1.draft", "jpg"), "_LPT1.draft.jpg");
        assert_eq!(file_name_for("console", "jpg"), "console.jpg");
        assert_eq!(file_name_for("tab\there", ""), "tab_here");
        assert_eq!(
            file_name_for(&"я".repeat(150), "png"),
            format!("{}.png", "я".repeat(100))
        );
    }
}
