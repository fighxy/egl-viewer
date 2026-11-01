//! Importing an Eagle `.library` folder into a native library (docs/spec.md §8.2,
//! docs/architecture.md §15.6). Read-only on the Eagle side: this module never writes into
//! the Eagle folder. Resumable: every imported item is recorded in `eagle_import`, so an
//! interrupted import continues where it stopped, and commits happen in transactions of
//! [`ITEM_BATCH`] items.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use eagle_folder::{EagleLibrary, ManifestEntry};
use library_core::{
    EagleImportReport, ImportFailure, ImportKind, ImportProgress, ImportState, SourceError,
    order_key_between,
};
use rusqlite::{OptionalExtension, Transaction, params};
use ulid::Ulid;

use crate::add::stored_file_name;
use crate::error::{Context, Result};
use crate::folders::{add_link, ensure_tag, log};
use crate::fsutil::rename_with_retry;
use crate::import::{ImportObserver, Tracker, note_failure};
use crate::library::Library;
use crate::recover::{library_file, remove_with_empty_parent};
use crate::time::now_ms;

/// Rows are committed every this many items (docs/architecture.md §15.6 step 5).
const ITEM_BATCH: usize = 500;

/// Skipped items listed in the report, like the file import (docs/spec.md §8.2 step 5).
const LISTED: usize = 200;

const BUFFER: usize = 1024 * 1024;

/// What to import and where it goes (docs/spec.md §8.2 steps 2–3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EagleImportOptions {
    /// Eagle's trash items become native trash items; off by default.
    pub import_trash: bool,
    /// `None`: the Eagle folders go at the library root (a new library made for the import).
    /// `Some(name)`: a root folder of that name is made (or reused) in the open library and
    /// the Eagle folders go under it.
    pub root_folder: Option<String>,
}

/// The Eagle → native id mapping built with the folder tree.
struct Structure {
    /// Eagle folder id → native folder id.
    folders: HashMap<String, String>,
    /// Locked Eagle folders: native id and name, for the report.
    locked: Vec<String>,
    /// Cover per native folder: the Eagle item id to point the cover at once items exist.
    covers: Vec<(String, String)>,
}

/// One item ready to be written: the file is already in place (journaled), the row follows
/// in the next batch transaction.
struct Prepared {
    eagle_id: String,
    last_modified: i64,
    id: String,
    name: String,
    ext: String,
    stored: String,
    size: u64,
    width: Option<u32>,
    height: Option<u32>,
    star: u8,
    annotation: String,
    url: String,
    added_at: i64,
    file_created_at: Option<i64>,
    file_modified_at: Option<i64>,
    duration: Option<f64>,
    bpm: Option<f64>,
    exif: Option<String>,
    palette: Option<String>,
    deleted_at: Option<i64>,
    /// The Eagle thumbnail converted to WebP; `None` asks for a generated preview later.
    thumb: Option<Vec<u8>>,
    folders: Vec<(String, Option<f64>)>,
    tags: Vec<String>,
    comments: Vec<eagle_folder::CommentDto>,
    content_hash: [u8; 32],
}

/// Imports `eagle` into `library` and returns the final progress, the report included.
pub fn import_eagle(
    library: &Library,
    eagle: &EagleLibrary,
    options: &EagleImportOptions,
    id: u64,
    observer: &dyn ImportObserver,
    cancel: &AtomicBool,
) -> ImportProgress {
    let tracker = Tracker::new(id, ImportKind::Eagle, observer);
    let mut report = EagleImportReport::default();
    tracker.update(true, |progress| progress.state = ImportState::Scanning);

    let outcome = import(library, eagle, options, &tracker, cancel, &mut report);
    if let Err(error) = outcome {
        tracing::warn!("Eagle import failed: {error}");
        tracker.update(true, |progress| {
            note_failure(
                progress,
                ImportFailure {
                    path: eagle.root().to_string_lossy().into_owned(),
                    reason: error.to_string(),
                },
            );
        });
    }
    tracker.finish_with(cancel, move |progress| progress.report = Some(report))
}

fn import(
    library: &Library,
    eagle: &EagleLibrary,
    options: &EagleImportOptions,
    tracker: &Tracker<'_>,
    cancel: &AtomicBool,
    report: &mut EagleImportReport,
) -> Result<()> {
    library.check_writable()?;
    let structure = import_structure(library, eagle, options)?;
    report.locked_folders = structure.locked.clone();

    let manifest = eagle.manifest().map_err(eagle_error)?;
    tracker.update(true, |progress| {
        progress.total = manifest.total;
        progress.state = ImportState::Adding;
    });
    let known = imported_ids(library)?;
    let mut seen_hashes: HashSet<[u8; 32]> = HashSet::new();
    let mut prepared: Vec<Prepared> = Vec::new();
    let mut added = 0_u32;

    for entry in &manifest.items {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        tracker.update(false, |progress| progress.current = Some(entry.id.clone()));
        if known.contains(&entry.id) {
            tracker.update(false, |progress| progress.processed += 1);
            continue;
        }
        match prepare_item(
            library,
            eagle,
            &structure,
            options,
            entry,
            &mut seen_hashes,
            report,
        ) {
            Ok(Some(item)) => {
                let needs_preview = item.thumb.is_none();
                added += 1;
                prepared.push(item);
                tracker.update(false, |progress| {
                    progress.added += 1;
                    progress.processed += 1;
                    if needs_preview {
                        progress.previews_total += 1;
                    }
                });
            }
            Ok(None) => tracker.update(false, |progress| progress.processed += 1),
            Err(reason) => {
                let line = failure(eagle, &entry.id, reason.to_string());
                if report.skipped.len() < LISTED {
                    report.skipped.push(line.clone());
                }
                tracker.update(false, |progress| {
                    progress.processed += 1;
                    note_failure(progress, line);
                });
            }
        }
        if prepared.len() >= ITEM_BATCH {
            flush(library, &mut prepared, &structure.covers)?;
        }
    }
    flush(library, &mut prepared, &structure.covers)?;
    report.imported = added;
    tracker.update(true, |progress| progress.current = None);

    if !cancel.load(Ordering::Relaxed) {
        make_previews(library, tracker, cancel);
    }
    Ok(())
}

/// Folders, tag groups and smart folders, idempotent by `eagle_id` so a resumed import
/// reuses what the interrupted one wrote.
fn import_structure(
    library: &Library,
    eagle: &EagleLibrary,
    options: &EagleImportOptions,
) -> Result<Structure> {
    let metadata = eagle.metadata().map_err(eagle_error)?;
    let mut conn = library.conn();
    let tx = conn.transaction().context("import the Eagle structure")?;

    let root = match &options.root_folder {
        Some(name) => Some(find_or_create_folder(
            &tx, None, name, "", None, None, None, None, None, false,
        )?),
        None => None,
    };

    let mut structure = Structure {
        folders: HashMap::new(),
        locked: Vec::new(),
        covers: Vec::new(),
    };
    import_folders(&tx, &metadata.folders, root.as_deref(), &mut structure)?;
    import_tag_groups(&tx, &metadata.tag_groups)?;
    import_smart_folders(&tx, &metadata.smart_folders, None)?;
    tx.commit().context("import the Eagle structure")?;
    Ok(structure)
}

fn import_folders(
    tx: &Transaction<'_>,
    folders: &[eagle_folder::FolderDto],
    parent: Option<&str>,
    structure: &mut Structure,
) -> Result<()> {
    let mut previous: Option<String> = None;
    for folder in folders {
        let (id, sort_key) = match folder_by_eagle(tx, &folder.id)? {
            Some(existing) => existing,
            None => {
                let sort_key = order_key_between(previous.as_deref(), None);
                let id = find_or_create_folder(
                    tx,
                    parent,
                    &folder.name,
                    &folder.description,
                    Some(&folder.id),
                    Some(sort_key.clone()),
                    folder.icon_color.as_deref(),
                    folder.order_by.as_deref(),
                    folder.sort_increase,
                    folder.is_locked,
                )?;
                (id, sort_key)
            }
        };
        if folder.is_locked {
            structure.locked.push(folder.name.clone());
        }
        for tag in &folder.tags {
            let tag_id = ensure_tag(tx, tag)?;
            tx.execute(
                "INSERT OR IGNORE INTO folder_tag (folder_id, tag_id) VALUES (?1, ?2)",
                params![id, tag_id],
            )
            .context("import a folder tag")?;
        }
        if let Some(cover) = &folder.cover_id {
            structure.covers.push((id.clone(), cover.clone()));
        }
        structure.folders.insert(folder.id.clone(), id.clone());
        import_folders(tx, &folder.children, Some(&id), structure)?;
        previous = Some(sort_key);
    }
    Ok(())
}

/// The native folder this Eagle folder was imported into, and its order key, when it was.
fn folder_by_eagle(tx: &Transaction<'_>, eagle_id: &str) -> Result<Option<(String, String)>> {
    tx.query_row(
        "SELECT id, sort_key FROM folder WHERE eagle_id = ?1",
        params![eagle_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .context("look for an imported folder")
}

/// Creates a folder row, or returns the existing row for the root folder (matched by name,
/// which has no Eagle id).
#[allow(clippy::too_many_arguments)] // the folder columns arrive together
fn find_or_create_folder(
    tx: &Transaction<'_>,
    parent: Option<&str>,
    name: &str,
    description: &str,
    eagle_id: Option<&str>,
    sort_key: Option<String>,
    icon_color: Option<&str>,
    order_by: Option<&str>,
    sort_ascending: Option<bool>,
    locked: bool,
) -> Result<String> {
    if eagle_id.is_none()
        && let Some(existing) = tx
            .query_row(
                "SELECT id FROM folder WHERE parent_id IS ?1 AND name = ?2",
                params![parent, name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("look for the import root folder")?
    {
        return Ok(existing);
    }
    let id = Ulid::generate().to_string();
    let now = now_ms();
    tx.execute(
        "INSERT INTO folder (id, parent_id, name, description, sort_key, icon_color, order_by,
                             sort_ascending, created_at, updated_at, eagle_id, locked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11)",
        params![
            id,
            parent,
            name,
            description,
            sort_key.unwrap_or_else(|| order_key_between(None, None)),
            icon_color,
            order_by,
            sort_ascending,
            now,
            eagle_id,
            locked
        ],
    )
    .context("import a folder")?;
    log(tx, "folder", &id, "insert", now)?;
    Ok(id)
}

fn import_tag_groups(tx: &Transaction<'_>, groups: &[eagle_folder::TagGroupDto]) -> Result<()> {
    let mut previous: Option<String> = None;
    for group in groups {
        if group_id_by_eagle(tx, &group.id)?.is_some() {
            continue;
        }
        let id = Ulid::generate().to_string();
        let sort_key = order_key_between(previous.as_deref(), None);
        tx.execute(
            "INSERT INTO tag_group (id, name, color, description, sort_key, eagle_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                group.name,
                group.color,
                group.description,
                sort_key,
                group.id
            ],
        )
        .context("import a tag group")?;
        for tag in &group.tags {
            let tag_id = ensure_tag(tx, tag)?;
            tx.execute(
                "INSERT OR IGNORE INTO tag_group_tag (group_id, tag_id) VALUES (?1, ?2)",
                params![id, tag_id],
            )
            .context("import a tag group member")?;
        }
        previous = Some(sort_key);
    }
    Ok(())
}

fn group_id_by_eagle(tx: &Transaction<'_>, eagle_id: &str) -> Result<Option<String>> {
    tx.query_row(
        "SELECT id FROM tag_group WHERE eagle_id = ?1",
        params![eagle_id],
        |row| row.get(0),
    )
    .optional()
    .context("look for an imported tag group")
}

fn import_smart_folders(
    tx: &Transaction<'_>,
    folders: &[eagle_folder::SmartFolderDto],
    parent: Option<&str>,
) -> Result<()> {
    let mut previous: Option<String> = None;
    for folder in folders {
        let (id, sort_key) = match smart_by_eagle(tx, &folder.id)? {
            Some(existing) => existing,
            None => {
                let id = Ulid::generate().to_string();
                let sort_key = order_key_between(previous.as_deref(), None);
                let conditions =
                    serde_json::to_string(&folder.conditions).context("encode the rules")?;
                tx.execute(
                    "INSERT INTO smart_folder (id, parent_id, name, conditions, order_by,
                                               sort_ascending, sort_key, eagle_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        id,
                        parent,
                        folder.name,
                        conditions,
                        folder.order_by,
                        folder.sort_increase,
                        sort_key,
                        folder.id
                    ],
                )
                .context("import a smart folder")?;
                (id, sort_key)
            }
        };
        import_smart_folders(tx, &folder.children, Some(&id))?;
        previous = Some(sort_key);
    }
    Ok(())
}

fn smart_by_eagle(tx: &Transaction<'_>, eagle_id: &str) -> Result<Option<(String, String)>> {
    tx.query_row(
        "SELECT id, sort_key FROM smart_folder WHERE eagle_id = ?1",
        params![eagle_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .context("look for an imported smart folder")
}

/// Eagle ids the library already holds: a resumed import skips them.
fn imported_ids(library: &Library) -> Result<HashSet<String>> {
    let conn = library.conn();
    let mut statement = conn
        .prepare("SELECT eagle_id FROM eagle_import")
        .context("read the Eagle import state")?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .context("read the Eagle import state")?;
    let mut ids = HashSet::new();
    for row in rows {
        ids.insert(row.context("read the Eagle import state")?);
    }
    Ok(ids)
}

/// Reads, hashes and copies one item; `Ok(None)` when the options or the file leave nothing
/// to import. Anything wrong becomes a report line, not a failed import.
fn prepare_item(
    library: &Library,
    eagle: &EagleLibrary,
    structure: &Structure,
    options: &EagleImportOptions,
    entry: &ManifestEntry,
    seen_hashes: &mut HashSet<[u8; 32]>,
    report: &mut EagleImportReport,
) -> Result<Option<Prepared>> {
    let item = match eagle.item(&entry.id) {
        Ok(item) => item,
        Err(error) => return Err(SourceError::Invalid(error.to_string())),
    };
    if item.is_deleted.unwrap_or(false) && !options.import_trash {
        report.trash_skipped += 1;
        return Ok(None);
    }
    let Some(original) = eagle.original(&item) else {
        return Err(SourceError::Invalid("the original file is missing".into()));
    };
    let metadata = fs::metadata(&original).context("read the original")?;
    let file_name = original
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file")
        .to_owned();

    // Copy into files/.tmp while hashing, flush, journal, rename into place: the same
    // contract as adding a file (docs/library-format.md §6).
    let tmp = library
        .root()
        .join("files")
        .join(".tmp")
        .join(Ulid::generate().to_string());
    let hash = match copy_hash(&original, &tmp, metadata.len()) {
        Ok(hash) => hash,
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            return Err(SourceError::Invalid(error.to_string()));
        }
    };

    let id = Ulid::generate().to_string();
    let shard = &id[id.len() - 2..];
    let stored = format!("files/{shard}/{id}/{}", stored_file_name(&file_name));
    let target = library_file(library.root(), &stored)
        .ok_or_else(|| SourceError::Storage("bad stored path".into()))?;
    if let Err(error) = library.conn().execute(
        "INSERT INTO pending_file (path, created_at) VALUES (?1, ?2)",
        params![stored, now_ms()],
    ) {
        let _ = fs::remove_file(&tmp);
        return Err(SourceError::Invalid(format!(
            "write the add journal: {error}"
        )));
    }
    if let Err(error) = target
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| rename_with_retry(&tmp, &target))
    {
        let _ = fs::remove_file(&tmp);
        let _ = remove_with_empty_parent(&target);
        forget_journal(library, &stored);
        return Err(SourceError::Invalid(format!(
            "move the file into the library: {error}"
        )));
    }

    if !seen_hashes.insert(*hash.as_bytes()) {
        report.duplicates += 1;
    }

    // Eagle's 320 px thumbnail becomes the first thumbnail; a missing or unreadable one
    // leaves thumb_version 0 and the background pass makes one from the original.
    let thumb = eagle
        .thumbnail(&entry.id)
        .and_then(|path| media::render(&path, "png").ok())
        .map(|rendered| rendered.thumbnail.webp);

    let folders = item
        .folders
        .iter()
        .filter_map(|eagle_id| {
            structure.folders.get(eagle_id).map(|native| {
                let position = item
                    .order
                    .as_ref()
                    .and_then(|order| order.get(eagle_id))
                    .and_then(|value| value.as_f64());
                (native.clone(), position)
            })
        })
        .collect();

    Ok(Some(Prepared {
        eagle_id: entry.id.clone(),
        last_modified: entry.last_modified,
        id,
        name: if item.name.is_empty() {
            file_name.clone()
        } else {
            item.name.clone()
        },
        ext: item.ext.to_lowercase(),
        stored,
        size: metadata.len(),
        width: item.width,
        height: item.height,
        star: item.star.unwrap_or(0).min(5),
        annotation: item.annotation.clone().unwrap_or_default(),
        url: item.url.clone().unwrap_or_default(),
        added_at: item.modification_time.unwrap_or_else(now_ms),
        file_created_at: item.btime,
        file_modified_at: item.mtime,
        duration: item.duration,
        bpm: item.bpm,
        exif: item.raw_metas.as_ref().map(|value| value.to_string()),
        palette: (!item.palettes.is_empty())
            .then(|| serde_json::to_string(&item.palettes).unwrap_or_default()),
        deleted_at: item.is_deleted.unwrap_or(false).then(now_ms),
        thumb,
        folders,
        tags: item.tags.clone(),
        comments: item.comments.clone(),
        content_hash: *hash.as_bytes(),
    }))
}

/// Writes the prepared rows in one transaction: the items, their tags, folders, comments,
/// the search rows, the import state and the journal cleanup; then the folder covers,
/// which point at items.
fn flush(
    library: &Library,
    prepared: &mut Vec<Prepared>,
    covers: &[(String, String)],
) -> Result<()> {
    if prepared.is_empty() && covers.is_empty() {
        return Ok(());
    }
    let now = now_ms();
    let mut conn = library.conn();
    let tx = conn.transaction().context("import Eagle items")?;
    for item in prepared.drain(..) {
        let seq = insert_item(&tx, &item, now)?;
        tx.execute(
            "INSERT INTO item_fts (rowid, name, annotation, url) VALUES (?1, ?2, ?3, ?4)",
            params![seq, item.name, item.annotation, item.url],
        )
        .context("index the imported item")?;
        for (folder, position) in &item.folders {
            add_link(&tx, &item.id, folder)?;
            if let Some(position) = position {
                tx.execute(
                    "UPDATE item_folder SET position = ?3
                     WHERE item_id = ?1 AND folder_id = ?2",
                    params![item.id, folder, position],
                )
                .context("set the manual order")?;
            }
        }
        for tag in &item.tags {
            let tag_id = ensure_tag(&tx, tag)?;
            tx.execute(
                "INSERT OR IGNORE INTO item_tag (item_id, tag_id) VALUES (?1, ?2)",
                params![item.id, tag_id],
            )
            .context("tag the imported item")?;
        }
        insert_comments(&tx, &item, now)?;
        tx.execute(
            "INSERT INTO eagle_import (eagle_id, last_modified, imported_at)
             VALUES (?1, ?2, ?3)",
            params![item.eagle_id, item.last_modified, now],
        )
        .context("record the imported item")?;
        tx.execute(
            "DELETE FROM pending_file WHERE path = ?1",
            params![item.stored],
        )
        .context("clear the add journal")?;
    }
    for (folder, cover) in covers {
        tx.execute(
            "UPDATE folder SET cover_item_id =
                 (SELECT id FROM item WHERE eagle_id = ?2), updated_at = ?3
             WHERE id = ?1",
            params![folder, cover, now],
        )
        .context("set the folder cover")?;
    }
    tx.commit().context("import Eagle items")
}

fn insert_item(tx: &Transaction<'_>, item: &Prepared, now: i64) -> Result<i64> {
    tx.execute(
        "INSERT INTO item (id, name, ext, file_path, size, width, height, duration, bpm,
                           exif, star, annotation, url, added_at, file_created_at,
                           file_modified_at, content_hash, palette, thumb_version,
                           deleted_at, updated_at, eagle_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                 ?17, ?18, ?19, ?20, ?21, ?22)",
        params![
            item.id,
            item.name,
            item.ext,
            item.stored,
            i64::try_from(item.size).unwrap_or(i64::MAX),
            item.width,
            item.height,
            item.duration,
            item.bpm,
            item.exif,
            item.star,
            item.annotation,
            item.url,
            item.added_at,
            item.file_created_at,
            item.file_modified_at,
            item.content_hash.as_slice(),
            item.palette,
            if item.thumb.is_some() { 1 } else { 0 },
            item.deleted_at,
            now,
            item.eagle_id,
        ],
    )
    .context("import an item")?;
    let seq = tx.last_insert_rowid();
    log(tx, "item", &item.id, "insert", now)?;
    Ok(seq)
}

fn insert_comments(tx: &Transaction<'_>, item: &Prepared, now: i64) -> Result<()> {
    for comment in &item.comments {
        tx.execute(
            "INSERT INTO comment (id, item_id, annotation, x, y, w, h, at_seconds, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Ulid::generate().to_string(),
                item.id,
                comment.annotation,
                comment.x,
                comment.y,
                comment.w,
                comment.h,
                comment.at_seconds,
                comment.updated_at.unwrap_or(now),
            ],
        )
        .context("import a comment")?;
    }
    Ok(())
}

/// Thumbnails the Eagle library did not have are made from the originals in the
/// background (docs/spec.md §8.2): the same pending list as after a restart. Items of
/// locked folders wait until their folder is unlocked.
fn make_previews(library: &Library, tracker: &Tracker<'_>, cancel: &AtomicBool) {
    tracker.update(true, |progress| progress.state = ImportState::Previews);
    let pool = match rayon::ThreadPoolBuilder::new()
        .num_threads(crate::import::preview_threads())
        .build()
    {
        Ok(pool) => pool,
        Err(error) => {
            tracing::warn!("no thread pool for previews: {error}");
            return;
        }
    };
    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(batch) = library.pending_previews(64) else {
            break;
        };
        if batch.is_empty() {
            break;
        }
        pool.in_place_scope(|scope| {
            for pending in &batch {
                let tracker = &*tracker;
                scope.spawn(move |_| {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let rendered = media::render(&pending.file, &pending.ext);
                    let failure = match rendered {
                        Ok(rendered) => library
                            .store_imported_preview(&pending.id, &rendered.thumbnail.webp)
                            .err()
                            .map(|error| error.to_string()),
                        Err(media::MediaError::Unsupported) => None,
                        Err(error) => {
                            let _ = library.mark_no_preview(&pending.id);
                            Some(format!("No preview: {error}"))
                        }
                    };
                    tracker.update(false, |progress| {
                        progress.previews_done += 1;
                        if let Some(reason) = failure {
                            note_failure(
                                progress,
                                ImportFailure {
                                    path: pending.file.to_string_lossy().into_owned(),
                                    reason,
                                },
                            );
                        }
                    });
                });
            }
        });
    }
}

fn forget_journal(library: &Library, stored: &str) {
    if let Err(error) = library
        .conn()
        .execute("DELETE FROM pending_file WHERE path = ?1", params![stored])
    {
        tracing::warn!("cannot clear the add journal: {error}");
    }
}

fn failure(eagle: &EagleLibrary, id: &str, reason: String) -> ImportFailure {
    ImportFailure {
        path: eagle
            .root()
            .join("images")
            .join(format!("{id}.info"))
            .to_string_lossy()
            .into_owned(),
        reason,
    }
}

fn eagle_error(error: eagle_folder::EagleError) -> SourceError {
    SourceError::Invalid(format!("reading the Eagle library: {error}"))
}

/// One pass: read the original once, hash and write, flush; then check the size.
fn copy_hash(
    source: &Path,
    tmp: &Path,
    size_before: u64,
) -> std::result::Result<blake3::Hash, SourceError> {
    let mut input = File::open(source).context("open the original")?;
    let mut output = File::create_new(tmp).context("create a temp file")?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; BUFFER];
    let mut copied: u64 = 0;
    loop {
        let read = match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("read the original"),
        };
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .context("write the copy")?;
        copied += read as u64;
    }
    if copied != size_before
        || fs::metadata(source).map_or(0, |metadata| metadata.len()) != size_before
    {
        return Err(SourceError::Storage(
            "the original changed while it was copied".into(),
        ));
    }
    output.sync_all().context("flush the copy")?;
    Ok(hasher.finalize())
}
