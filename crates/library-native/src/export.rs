//! Exporting originals to a folder tree, a ZIP or a 7z archive (docs/spec.md §7.4 item 12): what a
//! Backup or an Export writes, where, and with what protection. Reads only, so a read-only library can
//! export; items of a locked folder export decrypted once the folder is unlocked.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use age::Encryptor;
use library_core::{
    Compression, DecryptedArchive, ExportFormat, ExportPreview, ExportProgress, ExportRequest,
    ExportSelection, ExportState, ExportSummary, MIN_PASSPHRASE_CHARS, SourceError, ZipEncryption,
};
use rusqlite::{Connection, OptionalExtension, params, types::Value};
use secrecy::SecretString;
use serde::Serialize;
use sevenz_rust2::encoder_options::{AesEncoderOptions, Lzma2Options};
use sevenz_rust2::{ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, Password};
use zip::write::{FileOptions, SimpleFileOptions};

use crate::add::file_name_for;
use crate::create::check_not_inside_a_library;
use crate::error::{Context, Result};
use crate::fsutil::rename_with_retry;
use crate::library::Library;
use crate::locks::{VISIBLE, Visibility};
use crate::recover::library_file;

/// The manifest written at the export root: the metadata a bare file copy loses.
const MANIFEST: &str = "egl-export.json";
/// Progress reports at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);
/// The copy and encryption buffer.
const BUFFER: usize = 1024 * 1024;
/// 7-Zip's own key derivation strength: 2^19 SHA-256 rounds (D-113). The library's default
/// (2^8) would make a 7z passphrase as cheap to guess as a ZIP's.
const SEVEN_ZIP_KEY_ROUNDS_POWER: u8 = 19;

/// One item placed in the export.
#[derive(Debug, Clone)]
pub(crate) struct ExportItem {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) ext: String,
    pub(crate) size: u64,
    pub(crate) star: i64,
    pub(crate) annotation: String,
    pub(crate) url: String,
    pub(crate) file_path: String,
    pub(crate) encrypted: bool,
    pub(crate) tags: Vec<String>,
    /// The direct folders' names, sorted; for the manifest.
    pub(crate) folder_names: Vec<String>,
    /// The direct folders' ids; for the layout.
    pub(crate) folder_ids: Vec<String>,
    /// Date added, ms: Eagle `modificationTime`.
    pub(crate) added_at: i64,
}

/// A folder row, for walking from an item's folder up to a selected root.
struct FolderRow {
    parent: Option<String>,
    name: String,
}

/// One file the export writes.
struct Placement {
    item: ExportItem,
    /// The path inside the export, folders separated by `/`.
    rel: String,
}

/// One item of [`Manifest`].
#[derive(Clone, Serialize)]
struct ManifestItem {
    id: String,
    name: String,
    ext: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    folders: Vec<String>,
    #[serde(skip_serializing_if = "is_zero")]
    star: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    annotation: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    url: String,
    /// Where the export wrote the file, relative to the export root.
    file: String,
}

/// What `egl-export.json` at the export root carries.
#[derive(Serialize)]
struct Manifest {
    app: &'static str,
    library: String,
    /// ms since the epoch.
    exported_at: i64,
    items: Vec<ManifestItem>,
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

/// The SQL of a selection: a WITH clause for the folder subtrees, the WHERE condition and its
/// binds. Shared by the preview and the export so the numbers they show cannot disagree.
pub(crate) struct SelectionSql {
    prefix: String,
    condition: String,
    /// `:fN` folder ids and `:tN` tag names; `:hidden`/`:unlocked` come from the visibility.
    binds: Vec<(String, Value)>,
}

pub(crate) fn selection_sql(
    conn: &Connection,
    selection: &ExportSelection,
    include_subfolders: bool,
) -> Result<SelectionSql> {
    if selection.is_empty() {
        return Err(SourceError::Invalid("Choose what to export.".into()));
    }
    let mut prefix = String::new();
    let mut conditions = Vec::new();
    let mut binds = Vec::new();
    if !selection.folders.is_empty() {
        // A stale folder id means the caller's tree is out of date; refuse instead of silently
        // exporting less than asked.
        let mut check = conn
            .prepare("SELECT 1 FROM folder WHERE id = ?1")
            .context("find a folder")?;
        for id in &selection.folders {
            let known: Option<i64> = check
                .query_row(params![id], |row| row.get(0))
                .optional()
                .context("find a folder")?;
            if known.is_none() {
                return Err(SourceError::NotFound);
            }
        }
        let names: Vec<String> = (0..selection.folders.len())
            .map(|n| format!(":f{n}"))
            .collect();
        binds.extend(
            selection
                .folders
                .iter()
                .enumerate()
                .map(|(n, id)| (format!(":f{n}"), Value::from(id.clone()))),
        );
        if include_subfolders {
            // The first SELECT and the UNION both sit inside the AS (…); a misplaced `)` after
            // the IN list would leave the recursive arm outside the CTE.
            prefix = format!(
                "WITH RECURSIVE sub(root, id) AS (SELECT id, id FROM folder WHERE id IN ({}) \
                 UNION SELECT sub.root, folder.id FROM folder JOIN sub ON folder.parent_id = sub.id) ",
                names.join(", ")
            );
            conditions.push(
                "EXISTS (SELECT 1 FROM item_folder JOIN sub ON item_folder.folder_id = sub.id \
                 WHERE item_folder.item_id = item.id)"
                    .into(),
            );
        } else {
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM item_folder WHERE item_folder.item_id = item.id \
                 AND item_folder.folder_id IN ({}))",
                names.join(", ")
            ));
        }
    }
    if !selection.tags.is_empty() {
        let names: Vec<String> = (0..selection.tags.len())
            .map(|n| format!(":t{n}"))
            .collect();
        binds.extend(
            selection
                .tags
                .iter()
                .enumerate()
                .map(|(n, name)| (format!(":t{n}"), Value::from(name.clone()))),
        );
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM item_tag JOIN tag ON tag.id = item_tag.tag_id \
             WHERE item_tag.item_id = item.id AND tag.name IN ({}))",
            names.join(", ")
        ));
    }
    let condition = if selection.whole || conditions.is_empty() {
        "1 = 1".to_owned()
    } else {
        conditions.join(" OR ")
    };
    Ok(SelectionSql {
        prefix,
        condition,
        binds,
    })
}

/// The binds of a selection query; `visibility` is `None` for a count over everything the
/// selection covers, locks ignored.
pub(crate) fn bind<'a>(
    sql: &'a SelectionSql,
    visibility: Option<&'a Visibility>,
) -> Vec<(&'a str, Value)> {
    let mut binds: Vec<(&str, Value)> = sql
        .binds
        .iter()
        .map(|(name, value)| (name.as_str(), value.clone()))
        .collect();
    if let Some(visibility) = visibility {
        binds.push((":hidden", visibility.hidden.clone().into()));
        binds.push((":unlocked", visibility.unlocked.clone().into()));
    }
    binds
}

/// What an export would write, from the database alone (docs/spec.md §7.4 item 12).
pub fn export_preview(
    library: &Library,
    selection: &ExportSelection,
    include_subfolders: bool,
) -> Result<ExportPreview> {
    let conn = library.conn();
    let sql = selection_sql(&conn, selection, include_subfolders)?;
    let visibility = library.visibility(&conn)?;
    let query = format!(
        "{}SELECT COUNT(*), COALESCE(SUM(size), 0) FROM item WHERE deleted_at IS NULL AND {} AND {VISIBLE}",
        sql.prefix, sql.condition
    );
    let mut statement = conn.prepare(&query).context("preview an export")?;
    let (items, bytes) = statement
        .query_row(&bind(&sql, Some(&visibility))[..], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .context("preview an export")?;
    // What the selection covers without the lock rules: the difference stays home, hidden.
    let query = format!(
        "{}SELECT COUNT(*) FROM item WHERE deleted_at IS NULL AND {}",
        sql.prefix, sql.condition
    );
    let mut statement = conn.prepare(&query).context("preview an export")?;
    let covered: i64 = statement
        .query_row(&bind(&sql, None)[..], |row| row.get(0))
        .context("preview an export")?;
    let visible = u64::try_from(items).unwrap_or(u64::MAX);
    Ok(ExportPreview {
        items: visible,
        bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
        hidden: u64::try_from(covered)
            .unwrap_or(u64::MAX)
            .saturating_sub(visible),
    })
}

/// Runs one export: lists the items, plans the layout, writes the files and the manifest, and
/// reports progress. `Ok(None)` means the user cancelled; the partial output is removed either
/// way.
pub fn export_items(
    library: &Library,
    id: u64,
    request: &ExportRequest,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(ExportProgress),
) -> Result<Option<ExportSummary>> {
    validate(request, library)?;
    on_progress(ExportProgress {
        id,
        state: ExportState::Preparing,
        total: 0,
        done: 0,
        bytes_total: 0,
        bytes_done: 0,
        current: None,
        summary: None,
        error: None,
    });
    // The connection mutex is not reentrant: `load_export_items` and the writers lock it
    // themselves, so every guard here is dropped before they run.
    let sql = {
        let conn = library.conn();
        selection_sql(&conn, &request.selection, request.include_subfolders)?
    };
    let items = load_export_items(library, &sql)?;
    if items.is_empty() {
        return Err(SourceError::Invalid("The selection has no items.".into()));
    }
    let (folder_rows, roots, base) = {
        let conn = library.conn();
        (
            load_folder_rows(&conn)?,
            layout_roots(&conn, &request.selection)?,
            export_base(library, &conn, &request.selection),
        )
    };
    // The manifest sits at the top of the export: no item may take its name.
    let reserved = [MANIFEST.to_owned(), format!("{base}/{MANIFEST}")];
    let (planned, manifest_items) = plan_layout(items, &folder_rows, &roots, &reserved);
    let bytes_total = planned.iter().map(|p| p.item.size).sum();
    let destination = PathBuf::from(&request.destination);
    fs::create_dir_all(&destination).context("create the destination folder")?;
    let mut reporter = Reporter {
        emit: on_progress,
        id,
        total: planned.len() as u64,
        bytes_total,
        done: 0,
        bytes_done: 0,
        last: Instant::now() - PROGRESS_EVERY,
    };
    // Cloned up front so no name guard is held while files are written.
    let library_name = library.name().clone();
    let outcome = match request.format {
        ExportFormat::Mirror => write_mirror(
            library,
            &destination,
            &base,
            &planned,
            &manifest_items,
            library_name,
            cancel,
            &mut reporter,
        ),
        ExportFormat::Zip {
            compression,
            encryption,
        } => write_zip(
            library,
            &destination,
            &base,
            &planned,
            &manifest_items,
            library_name,
            compression,
            request
                .encrypt
                .as_deref()
                .map(|password| (password, encryption)),
            cancel,
            &mut reporter,
        ),
        ExportFormat::SevenZip { compression } => write_7z(
            &Entries {
                library,
                base: &base,
                planned: &planned,
                manifest_items: &manifest_items,
                library_name: &library_name,
                compression,
            },
            &destination,
            request.encrypt.as_deref(),
            cancel,
            &mut reporter,
        ),
    }?;
    match outcome {
        Outcome::Cancelled => {
            reporter.report(ExportState::Cancelled);
            Ok(None)
        }
        Outcome::Done(summary) => {
            (reporter.emit)(ExportProgress {
                id: reporter.id,
                state: ExportState::Done,
                total: reporter.total,
                done: reporter.done,
                bytes_total: reporter.bytes_total,
                bytes_done: reporter.bytes_done,
                current: None,
                summary: Some(summary.clone()),
                error: None,
            });
            Ok(Some(summary))
        }
    }
}

fn validate(request: &ExportRequest, library: &Library) -> Result<()> {
    if request.selection.is_empty() {
        return Err(SourceError::Invalid("Choose what to export.".into()));
    }
    if request.encrypt.is_some() && !request.format.is_archive() {
        return Err(SourceError::Invalid(
            "Encryption needs an archive: ZIP or 7z.".into(),
        ));
    }
    if let Some(passphrase) = &request.encrypt
        && passphrase.chars().count() < MIN_PASSPHRASE_CHARS
    {
        return Err(SourceError::Invalid(format!(
            "The passphrase needs at least {MIN_PASSPHRASE_CHARS} characters."
        )));
    }
    if request.destination.trim().is_empty() {
        return Err(SourceError::Invalid("Choose where to export.".into()));
    }
    // Exporting into the library folder would leave stray files next to the managed ones.
    let mut probe = PathBuf::from(&request.destination);
    loop {
        if probe.exists() {
            break;
        }
        if !probe.pop() {
            break;
        }
    }
    let destination = fs::canonicalize(&probe)
        .map_err(|_| SourceError::Storage("resolve the destination".into()))?;
    let root = fs::canonicalize(library.root())
        .map_err(|_| SourceError::Storage("the library folder is gone".into()))?;
    if destination.starts_with(&root) {
        return Err(SourceError::Invalid(
            "Choose a folder outside the library.".into(),
        ));
    }
    // Nor into another library: Eagle is the only writer of its `.library` folder, and a
    // native library's files are only written through its journal.
    check_not_inside_a_library(&destination)
}

/// The visible items of the selection with their tags and folder links, names opened for
/// encrypted items of an unlocked folder.
pub(crate) fn load_export_items(library: &Library, sql: &SelectionSql) -> Result<Vec<ExportItem>> {
    let conn = library.conn();
    let visibility = library.visibility(&conn)?;
    let query = format!(
        "{}SELECT id, name, ext, size, star, annotation, url, file_path, encrypted, sealed, \
         added_at \
         FROM item WHERE deleted_at IS NULL AND {} AND {VISIBLE} ORDER BY seq",
        sql.prefix, sql.condition
    );
    let mut statement = conn.prepare(&query).context("list the export")?;
    let mut rows = statement
        .query(&bind(sql, Some(&visibility))[..])
        .context("list the export")?;
    let mut items = Vec::new();
    let mut sealed: Vec<(usize, Vec<u8>)> = Vec::new();
    while let Some(row) = rows.next().context("list the export")? {
        let encrypted = row.get::<_, bool>(8).context("read an item")?;
        let mut item = ExportItem {
            id: row.get(0).context("read an item")?,
            name: row.get(1).context("read an item")?,
            ext: row.get(2).context("read an item")?,
            size: u64::try_from(row.get::<_, i64>(3).context("read an item")?).unwrap_or(u64::MAX),
            star: row.get(4).context("read an item")?,
            annotation: row.get(5).context("read an item")?,
            url: row.get(6).context("read an item")?,
            file_path: row.get(7).context("read an item")?,
            encrypted,
            tags: Vec::new(),
            folder_names: Vec::new(),
            folder_ids: Vec::new(),
            added_at: row.get(10).context("read an item")?,
        };
        if encrypted {
            sealed.push((
                items.len(),
                row.get::<_, Option<Vec<u8>>>(9)
                    .context("read an item")?
                    .unwrap_or_default(),
            ));
            item.name.clear();
        }
        items.push(item);
    }
    drop(rows);
    drop(statement);
    // An encrypted item's real name sits in its sealed fields; the key of an unlocked folder
    // is in the session keyring. An item whose key is gone stays hidden, like the rest of the
    // folder (mirrors `load_scope`).
    let mut unreadable = Vec::new();
    for (at, blob) in &sealed {
        let id = items[*at].id.clone();
        match library.open_fields(&conn, &id, blob) {
            Ok(fields) => items[*at].name = fields.name,
            Err(error) => {
                tracing::warn!("cannot open an encrypted item: {error}");
                unreadable.push(*at);
            }
        }
    }
    for at in unreadable.into_iter().rev() {
        items.remove(at);
    }
    let by_id: HashMap<String, usize> = items
        .iter()
        .enumerate()
        .map(|(at, item)| (item.id.clone(), at))
        .collect();
    let mut statement = conn
        .prepare(
            "SELECT item_tag.item_id, tag.name FROM item_tag \
             JOIN tag ON tag.id = item_tag.tag_id ORDER BY tag.name",
        )
        .context("read tags")?;
    let mut rows = statement.query([]).context("read tags")?;
    while let Some(row) = rows.next().context("read tags")? {
        let id: String = row.get(0).context("read a tag")?;
        if let Some(&at) = by_id.get(&id) {
            items[at].tags.push(row.get(1).context("read a tag")?);
        }
    }
    drop(rows);
    drop(statement);
    let mut statement = conn
        .prepare(
            "SELECT item_folder.item_id, item_folder.folder_id, folder.name FROM item_folder \
             JOIN folder ON folder.id = item_folder.folder_id ORDER BY folder.name",
        )
        .context("read folder links")?;
    let mut rows = statement.query([]).context("read folder links")?;
    while let Some(row) = rows.next().context("read a folder link")? {
        let id: String = row.get(0).context("read a folder link")?;
        if let Some(&at) = by_id.get(&id) {
            items[at]
                .folder_ids
                .push(row.get(1).context("read a folder link")?);
            items[at]
                .folder_names
                .push(row.get(2).context("read a folder link")?);
        }
    }
    Ok(items)
}

fn load_folder_rows(conn: &Connection) -> Result<HashMap<String, FolderRow>> {
    let mut statement = conn
        .prepare("SELECT id, parent_id, name FROM folder")
        .context("read folders")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                FolderRow {
                    parent: row.get(1)?,
                    name: row.get(2)?,
                },
            ))
        })
        .context("read folders")?;
    let mut map = HashMap::new();
    for row in rows {
        let (id, folder) = row.context("read folders")?;
        map.insert(id, folder);
    }
    Ok(map)
}

/// The folders the layout mirrors: the selection's, or the top-level ones for the whole
/// library.
fn layout_roots(conn: &Connection, selection: &ExportSelection) -> Result<Vec<String>> {
    if !selection.whole {
        return Ok(selection.folders.clone());
    }
    let mut statement = conn
        .prepare("SELECT id FROM folder WHERE parent_id IS NULL")
        .context("read folders")?;
    let rows = statement
        .query_map([], |row| row.get(0))
        .context("read folders")?;
    let mut roots = Vec::new();
    for row in rows {
        roots.push(row.context("read folders")?);
    }
    Ok(roots)
}

/// Decides the relative path of every file: an item goes under each selected folder subtree
/// one of its folders is in (nearest root first), and flat at the export root when no subtree
/// covers it. Returns the placements and the manifest items (one per item, with the first
/// written path).
fn plan_layout(
    items: Vec<ExportItem>,
    folder_rows: &HashMap<String, FolderRow>,
    roots: &[String],
    reserved: &[String],
) -> (Vec<Placement>, Vec<ManifestItem>) {
    let root_set: HashSet<&str> = roots.iter().map(String::as_str).collect();
    let mut used: HashSet<String> = reserved.iter().map(|path| path.to_lowercase()).collect();
    let mut placements = Vec::new();
    let mut manifest = Vec::new();
    for item in &items {
        let mut dirs: Vec<Vec<String>> = Vec::new();
        for folder_id in &item.folder_ids {
            if let Some(path) = subtree_path(folder_id, &root_set, folder_rows)
                && !dirs.contains(&path)
            {
                dirs.push(path);
            }
        }
        if dirs.is_empty() {
            dirs.push(Vec::new());
        }
        let file_name = file_name_for(&item.name, &item.ext);
        for (n, dir) in dirs.iter().enumerate() {
            let mut segments: Vec<&str> = dir.iter().map(String::as_str).collect();
            segments.push(&file_name);
            let rel = unique_rel(&segments.join("/"), &mut used);
            if n == 0 {
                manifest.push(ManifestItem {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    ext: item.ext.clone(),
                    tags: item.tags.clone(),
                    folders: item.folder_names.clone(),
                    star: item.star,
                    annotation: item.annotation.clone(),
                    url: item.url.clone(),
                    file: rel.clone(),
                });
            }
            placements.push(Placement {
                item: ExportItem {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    ext: item.ext.clone(),
                    size: item.size,
                    star: item.star,
                    annotation: String::new(),
                    url: String::new(),
                    file_path: item.file_path.clone(),
                    encrypted: item.encrypted,
                    tags: Vec::new(),
                    folder_names: Vec::new(),
                    folder_ids: Vec::new(),
                    added_at: 0,
                },
                rel,
            });
        }
    }
    placements.sort_by(|a, b| a.rel.cmp(&b.rel));
    (placements, manifest)
}

/// The path from the nearest selected root down to `folder` (root's name first), or `None`
/// when the folder is inside no selected subtree.
fn subtree_path(
    folder: &str,
    roots: &HashSet<&str>,
    rows: &HashMap<String, FolderRow>,
) -> Option<Vec<String>> {
    let mut chain: Vec<&str> = Vec::new();
    let mut current = folder;
    loop {
        chain.push(current);
        if roots.contains(current) {
            return chain
                .iter()
                .rev()
                .map(|id| {
                    let name = rows
                        .get(*id)
                        .map(|row| row.name.as_str())
                        .unwrap_or("folder");
                    sanitize_segment(name)
                })
                .collect::<Option<Vec<_>>>();
        }
        current = rows.get(current)?.parent.as_deref()?;
    }
}

/// A folder name as one export path segment: Windows-safe, never empty.
fn sanitize_segment(name: &str) -> Option<String> {
    let segment = file_name_for(name, "");
    (!segment.is_empty()).then_some(segment)
}

/// `rel`, or `name (2).ext`, `name (3).ext`, … when the export already writes that path
/// (Windows compares names case-insensitively).
fn unique_rel(rel: &str, used: &mut HashSet<String>) -> String {
    if used.insert(rel.to_lowercase()) {
        return rel.to_owned();
    }
    // The counter goes into the file name, never into a folder of the path: `a.b/README`
    // becomes `a.b/README (2)`, and `.gitignore` keeps its leading dot as part of the stem.
    let (dir, name) = match rel.rsplit_once('/') {
        Some((dir, name)) => (format!("{dir}/"), name),
        None => (String::new(), rel),
    };
    let (stem, ext) = name
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .unwrap_or((name, ""));
    for n in 2.. {
        let candidate = if ext.is_empty() {
            format!("{dir}{stem} ({n})")
        } else {
            format!("{dir}{stem} ({n}).{ext}")
        };
        if used.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }
    unreachable!("more names than fit");
}

/// The export root's name: the single selected folder or tag, else the library name.
fn export_base(library: &Library, conn: &Connection, selection: &ExportSelection) -> String {
    let raw: Option<String> =
        if !selection.whole && selection.folders.len() + selection.tags.len() == 1 {
            if let [folder] = selection.folders.as_slice() {
                conn.query_row(
                    "SELECT name FROM folder WHERE id = ?1",
                    params![folder],
                    |row| row.get(0),
                )
                .optional()
                .ok()
                .flatten()
            } else {
                selection.tags.first().cloned()
            }
        } else {
            None
        };
    // `file_name_for` never returns an empty name (it falls back to "file"), so pick the
    // library name before sanitizing rather than after.
    let raw = raw
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| library.name().clone());
    file_name_for(&raw, "")
}

/// `folder/name.ext` as a path on disk.
fn rel_to_path(rel: &str) -> PathBuf {
    rel.split('/').collect()
}

/// Path under the named export folder: when the layout already starts with `base/`, strip it so
/// `out/Trip/` + `Trip/one.png` does not become `out/Trip/Trip/one.png`.
fn rel_under_base<'a>(rel: &'a str, base: &str) -> &'a str {
    rel.strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(rel)
}

/// Zip entry path: keep a layout that already starts with `base/`, otherwise nest under `base/`.
fn zip_entry(rel: &str, base: &str) -> String {
    if rel == base || rel.starts_with(&format!("{base}/")) {
        rel.to_owned()
    } else {
        format!("{base}/{rel}")
    }
}

/// Manifest `file` paths relative to the destination: always under the named export root.
fn manifest_with_base(items: &[ManifestItem], base: &str) -> Vec<ManifestItem> {
    items
        .iter()
        .map(|item| ManifestItem {
            file: zip_entry(&item.file, base),
            ..item.clone()
        })
        .collect()
}

/// A child of `dir` that does not exist yet: `base.ext`, `base (2).ext`, …
fn unique_child(dir: &Path, base: &str, ext: &str) -> PathBuf {
    for n in 1.. {
        let stem = if n > 1 {
            format!("{base} ({n})")
        } else {
            base.to_owned()
        };
        let name = if ext.is_empty() {
            stem
        } else {
            format!("{stem}.{ext}")
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("more names than fit");
}

/// Throttled progress reports over one export.
struct Reporter<'a> {
    emit: &'a mut dyn FnMut(ExportProgress),
    id: u64,
    total: u64,
    bytes_total: u64,
    done: u64,
    bytes_done: u64,
    last: Instant,
}

impl Reporter<'_> {
    fn report(&mut self, state: ExportState) {
        (self.emit)(ExportProgress {
            id: self.id,
            state,
            total: self.total,
            done: self.done,
            bytes_total: self.bytes_total,
            bytes_done: self.bytes_done,
            current: None,
            summary: None,
            error: None,
        });
    }

    fn copying(&mut self, current: &str) {
        if self.last.elapsed() >= PROGRESS_EVERY {
            self.last = Instant::now();
            (self.emit)(ExportProgress {
                id: self.id,
                state: ExportState::Copying,
                total: self.total,
                done: self.done,
                bytes_total: self.bytes_total,
                bytes_done: self.bytes_done,
                current: Some(current.to_owned()),
                summary: None,
                error: None,
            });
        }
    }

    fn item_done(&mut self, bytes: u64, current: &str) {
        self.done += 1;
        self.bytes_done += bytes;
        self.copying(current);
    }
}

/// What the writing loops return: `Cancelled` when the flag went up between files.
enum Outcome {
    Cancelled,
    Done(ExportSummary),
}

/// Removes a partial export (a folder tree, a `.zip.part`, a `.zip.age.part`) when it is
/// dropped, unless the export finished and called [`Leftovers::keep`]. A cancelled, failed or
/// panicking export leaves nothing behind, decrypted copies of locked items included.
struct Leftovers {
    paths: Vec<PathBuf>,
}

impl Leftovers {
    fn new(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }

    fn keep(mut self) {
        self.paths.clear();
    }
}

impl Drop for Leftovers {
    fn drop(&mut self) {
        for path in &self.paths {
            let removed = if path.is_dir() {
                fs::remove_dir_all(path)
            } else {
                fs::remove_file(path)
            };
            if let Err(error) = removed
                && error.kind() != ErrorKind::NotFound
            {
                tracing::warn!("cannot remove a partial export: {error}");
            }
        }
    }
}

/// Writes the placements and the manifest as a folder tree under `<destination>/<base>`.
#[allow(clippy::too_many_arguments)]
fn write_mirror(
    library: &Library,
    destination: &Path,
    base: &str,
    planned: &[Placement],
    manifest_items: &[ManifestItem],
    library_name: String,
    cancel: &AtomicBool,
    reporter: &mut Reporter,
) -> Result<Outcome> {
    let root = unique_child(destination, base, "");
    fs::create_dir_all(&root).context("create the export folder")?;
    let leftovers = Leftovers::new(vec![root.clone()]);
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut missing = Vec::new();
    for placement in planned {
        if cancel.load(Ordering::Relaxed) {
            return Ok(Outcome::Cancelled);
        }
        let under = rel_under_base(&placement.rel, base);
        reporter.copying(under);
        let target = root.join(rel_to_path(under));
        match copy_item(library, &placement.item, &target)? {
            Some(written) => {
                files += 1;
                bytes += written;
            }
            None => missing.push(placement.item.name.clone()),
        }
        reporter.item_done(placement.item.size, under);
    }
    let manifest_items = manifest_with_base(manifest_items, base);
    write_manifest(&root.join(MANIFEST), &library_name, &manifest_items)?;
    files += 1;
    leftovers.keep();
    Ok(Outcome::Done(ExportSummary {
        items: manifest_items.len() as u64,
        files,
        bytes,
        missing,
        path: root.to_string_lossy().into_owned(),
    }))
}

/// Writes the placements and the manifest into `<destination>/<base>.zip`; with an age
/// passphrase into `<destination>/<base>.zip.age`, with an AES one into a `.zip` whose entries
/// are AES-256 encrypted (D-112). The age archive is encrypted while it is written (D-110):
/// the ZIP streams into the age writer, so no plaintext archive touches the disk, not even for
/// a moment in a synced destination. Either way the output is written as `.part` and renamed
/// when complete.
#[allow(clippy::too_many_arguments)]
fn write_zip(
    library: &Library,
    destination: &Path,
    base: &str,
    planned: &[Placement],
    manifest_items: &[ManifestItem],
    library_name: String,
    compression: Compression,
    encrypt: Option<(&str, ZipEncryption)>,
    cancel: &AtomicBool,
    reporter: &mut Reporter,
) -> Result<Outcome> {
    let archive = unique_child(
        destination,
        base,
        match encrypt {
            Some((_, ZipEncryption::Age)) => "zip.age",
            _ => "zip",
        },
    );
    let part = part_path(&archive);
    let leftovers = Leftovers::new(vec![part.clone()]);
    let entries = Entries {
        library,
        base,
        planned,
        manifest_items,
        library_name: &library_name,
        compression,
    };
    let outcome = match encrypt {
        None | Some((_, ZipEncryption::Aes)) => {
            let file = File::create(&part).context("create the archive")?;
            let mut sink = ZipSink {
                zip: zip::ZipWriter::new(file).set_auto_large_file(),
                compression,
                password: encrypt.map(|(password, _)| password),
            };
            let outcome = entries.write(&mut sink, cancel, reporter)?;
            if let Outcome::Done(_) = outcome {
                sink.zip
                    .finish()
                    .map_err(zip_error)?
                    .sync_all()
                    .context("finish the archive")?;
            }
            outcome
        }
        Some((password, ZipEncryption::Age)) => {
            let file = File::create(&part).context("create the encrypted archive")?;
            let encryptor =
                Encryptor::with_user_passphrase(SecretString::from(password.to_owned()));
            let writer = encryptor
                .wrap_output(BufWriter::with_capacity(BUFFER, file))
                .context("start encrypting the archive")?;
            let mut sink = ZipSink {
                zip: zip::ZipWriter::new_stream(writer),
                compression,
                password: None,
            };
            let outcome = entries.write(&mut sink, cancel, reporter)?;
            if let Outcome::Done(_) = outcome {
                sink.zip
                    .finish()
                    .map_err(zip_error)?
                    .into_inner()
                    .finish()
                    .context("finish the encrypted archive")?
                    .into_inner()
                    .map_err(|error| error.into_error())
                    .context("finish the encrypted archive")?
                    .sync_all()
                    .context("finish the encrypted archive")?;
            }
            outcome
        }
    };
    let Outcome::Done(mut summary) = outcome else {
        return Ok(Outcome::Cancelled);
    };
    rename_with_retry(&part, &archive).context("finish the archive")?;
    summary.path = archive.to_string_lossy().into_owned();
    leftovers.keep();
    Ok(Outcome::Done(summary))
}

/// Writes the placements and the manifest into `<destination>/<base>.7z` (D-113). With a
/// passphrase the entries and the header with their names are AES-256 encrypted, so the
/// archive shows nothing without it. Each original is its own LZMA2 stream (not solid), so a
/// damaged spot costs one file and a cancel stops between or inside files.
fn write_7z(
    entries: &Entries,
    destination: &Path,
    encrypt: Option<&str>,
    cancel: &AtomicBool,
    reporter: &mut Reporter,
) -> Result<Outcome> {
    let archive = unique_child(destination, entries.base, "7z");
    let part = part_path(&archive);
    let leftovers = Leftovers::new(vec![part.clone()]);
    let file = File::create(&part).context("create the archive")?;
    let mut writer = ArchiveWriter::new(BufWriter::with_capacity(BUFFER, file))
        .map_err(|error| SourceError::Storage(format!("create the archive: {error}")))?;
    let mut methods = Vec::new();
    if let Some(password) = encrypt {
        let mut aes = AesEncoderOptions::new(Password::new(password));
        aes.num_cycles_power = SEVEN_ZIP_KEY_ROUNDS_POWER;
        methods.push(EncoderConfiguration::from(aes));
    }
    methods.push(match seven_zip_level(entries.compression) {
        None => EncoderConfiguration::new(EncoderMethod::COPY),
        Some(level) => Lzma2Options::from_level(level).into(),
    });
    writer.set_content_methods(methods);
    writer.set_encrypt_header(encrypt.is_some());
    let mut sink = SevenZipSink { writer };
    let Outcome::Done(mut summary) = entries.write(&mut sink, cancel, reporter)? else {
        return Ok(Outcome::Cancelled);
    };
    sink.writer
        .finish()
        .context("finish the archive")?
        .into_inner()
        .map_err(|error| error.into_error())
        .context("finish the archive")?
        .sync_all()
        .context("finish the archive")?;
    rename_with_retry(&part, &archive).context("finish the archive")?;
    summary.path = archive.to_string_lossy().into_owned();
    leftovers.keep();
    Ok(Outcome::Done(summary))
}

/// The LZMA2 preset for a 7z export; `None` stores the entries. "Smallest" is preset 7, not 9:
/// 9 takes about 670 MiB to compress, too much for a phone, and media barely shrink anyway.
fn seven_zip_level(compression: Compression) -> Option<u32> {
    match compression {
        Compression::None => None,
        Compression::Fast => Some(1),
        Compression::Balanced => Some(5),
        Compression::Smallest => Some(7),
    }
}

/// Opens an age archive (an encrypted export, `name.zip.age`) with its passphrase into
/// `destination`, as `name.zip` or `name (2).zip` beside one that exists (D-110). The
/// plaintext is written as `.part` and renamed only once the whole stream has authenticated,
/// so a wrong passphrase, a damaged file or a cancel leaves nothing behind. `Ok(None)` when
/// cancelled. Never writes into a native or an Eagle library.
pub fn decrypt_archive(
    source: &Path,
    destination: &Path,
    passphrase: &str,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<Option<DecryptedArchive>> {
    if !destination.is_dir() {
        return Err(SourceError::Invalid(
            "Choose a folder to save it in.".into(),
        ));
    }
    let destination = fs::canonicalize(destination)
        .map_err(|_| SourceError::Storage("resolve the destination".into()))?;
    check_not_inside_a_library(&destination)?;
    let input = match File::open(source) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(SourceError::Invalid("The archive is not there.".into()));
        }
        Err(error) => return Err(error).context("read the archive"),
    };
    // Progress is the share of the archive read: the plaintext size is not known up front.
    let total = input.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let read = Arc::new(AtomicU64::new(0));
    let counted = Counted {
        inner: input,
        read: Arc::clone(&read),
    };
    let decryptor = age::Decryptor::new_buffered(BufReader::with_capacity(BUFFER, counted))
        .map_err(age_error)?;
    if !decryptor.is_scrypt() {
        return Err(SourceError::Invalid(
            "This archive is encrypted for a key, not with a passphrase.".into(),
        ));
    }
    let identity = age::scrypt::Identity::new(SecretString::from(passphrase.to_owned()));
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(age_error)?;

    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let plain = match name.len().checked_sub(4) {
        Some(cut) if cut > 0 && name[cut..].eq_ignore_ascii_case(".age") => name[..cut].to_owned(),
        _ => format!("{name}.decrypted"),
    };
    let (stem, ext) = match plain.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_owned(), ext.to_owned()),
        _ => (plain.clone(), String::new()),
    };
    let target = unique_child(&destination, &stem, &ext);
    let part = part_path(&target);
    let leftovers = Leftovers::new(vec![part.clone()]);
    let mut output = BufWriter::with_capacity(
        BUFFER,
        File::create(&part).context("create the decrypted file")?,
    );
    // age authenticates every chunk: a bad or missing one reads as invalid data.
    let read_archive = |error: io::Error| match error.kind() {
        ErrorKind::InvalidData | ErrorKind::UnexpectedEof => {
            SourceError::Invalid("The archive is damaged or incomplete.".into())
        }
        _ => SourceError::Storage(format!("read the archive: {error}")),
    };
    let mut last = Instant::now() - PROGRESS_EVERY;
    let mut report = |_: u64| {
        if last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            on_progress(read.load(Ordering::Relaxed).min(total), total);
        }
    };
    let copied = copy_cancellable(&mut reader, &mut output, cancel, &read_archive, &mut report)?;
    let Some(bytes) = copied else {
        return Ok(None);
    };
    on_progress(total, total);
    output
        .into_inner()
        .map_err(|error| error.into_error())
        .context("write the decrypted file")?
        .sync_all()
        .context("write the decrypted file")?;
    rename_with_retry(&part, &target).context("finish the decrypted file")?;
    leftovers.keep();
    Ok(Some(DecryptedArchive {
        path: target.to_string_lossy().into_owned(),
        bytes,
    }))
}

/// A reader that counts the bytes it hands out, for progress.
struct Counted<R> {
    inner: R,
    read: Arc<AtomicU64>,
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.read.fetch_add(read as u64, Ordering::Relaxed);
        Ok(read)
    }
}

/// What a person can act on, for an archive age refuses.
fn age_error(error: age::DecryptError) -> SourceError {
    use age::DecryptError;
    match error {
        DecryptError::DecryptionFailed
        | DecryptError::KeyDecryptionFailed
        | DecryptError::NoMatchingKeys => SourceError::Invalid("Wrong passphrase.".into()),
        DecryptError::ExcessiveWork { .. } => SourceError::Invalid(
            "This archive asks for more work to open than this device allows.".into(),
        ),
        DecryptError::InvalidHeader | DecryptError::UnknownFormat => {
            SourceError::Invalid("This is not an age archive.".into())
        }
        DecryptError::InvalidMac => {
            SourceError::Invalid("The archive is damaged or incomplete.".into())
        }
        DecryptError::Io(error) => SourceError::Storage(format!("read the archive: {error}")),
        #[allow(unreachable_patterns)]
        other => SourceError::Storage(format!("open the archive: {other}")),
    }
}

/// `<name>.part` next to `path`.
fn part_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export".to_owned());
    path.with_file_name(format!("{name}.part"))
}

/// What goes into an archive: the placed originals, then the manifest.
struct Entries<'a> {
    library: &'a Library,
    base: &'a str,
    planned: &'a [Placement],
    manifest_items: &'a [ManifestItem],
    library_name: &'a str,
    compression: Compression,
}

/// An archive being written: ZIP or 7z.
trait ArchiveSink {
    /// Adds one file of `size` bytes from `source`; `None` when `cancel` went up first.
    fn add(
        &mut self,
        name: &str,
        size: u64,
        source: &mut dyn Read,
        cancel: &AtomicBool,
    ) -> Result<Option<u64>>;
}

/// A ZIP being written; `password` encrypts every entry with AES-256 (D-112).
struct ZipSink<'p, W: Write + io::Seek> {
    zip: zip::ZipWriter<W>,
    compression: Compression,
    password: Option<&'p str>,
}

impl<W: Write + io::Seek> ArchiveSink for ZipSink<'_, W> {
    fn add(
        &mut self,
        name: &str,
        size: u64,
        source: &mut dyn Read,
        cancel: &AtomicBool,
    ) -> Result<Option<u64>> {
        let options = file_options(self.compression, size);
        let options: FileOptions<'_, '_, ()> = match self.password {
            Some(password) => options.with_aes_encryption(zip::AesMode::Aes256, password),
            None => options,
        };
        self.zip
            .start_file(name.to_owned(), options)
            .map_err(zip_error)?;
        copy_cancellable(source, &mut self.zip, cancel, &read_original, &mut |_| {})
    }
}

/// A 7z being written (D-113).
struct SevenZipSink<W: Write + io::Seek> {
    writer: ArchiveWriter<W>,
}

impl<W: Write + io::Seek> ArchiveSink for SevenZipSink<W> {
    fn add(
        &mut self,
        name: &str,
        _size: u64,
        source: &mut dyn Read,
        cancel: &AtomicBool,
    ) -> Result<Option<u64>> {
        let mut guarded = Guarded {
            source,
            cancel,
            failure: None,
        };
        match self
            .writer
            .push_archive_entry(ArchiveEntry::new_file(name), Some(&mut guarded))
        {
            Ok(entry) => Ok(Some(entry.size)),
            Err(_) if cancel.load(Ordering::Relaxed) => Ok(None),
            Err(error) => Err(match guarded.failure.take() {
                Some(failure) => read_original(failure),
                None => SourceError::Storage(format!("write the archive: {error}")),
            }),
        }
    }
}

/// A source that stops with an error once `cancel` goes up, and keeps its own read error so
/// the caller can tell it from a failed write.
struct Guarded<'a> {
    source: &'a mut dyn Read,
    cancel: &'a AtomicBool,
    failure: Option<io::Error>,
}

impl Read for Guarded<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(ErrorKind::Interrupted, "cancelled"));
        }
        loop {
            match self.source.read(buffer) {
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => {
                    let message = error.to_string();
                    self.failure = Some(error);
                    return Err(io::Error::other(message));
                }
                Ok(read) => return Ok(read),
            }
        }
    }
}

fn read_original(error: io::Error) -> SourceError {
    SourceError::Storage(format!("read an original: {error}"))
}

impl Entries<'_> {
    /// Writes every entry into `sink`; `Cancelled` as soon as the flag goes up, between files
    /// or in the middle of a large one. The caller finishes the archive.
    fn write(
        &self,
        sink: &mut dyn ArchiveSink,
        cancel: &AtomicBool,
        reporter: &mut Reporter,
    ) -> Result<Outcome> {
        let mut files = 0u64;
        let mut bytes = 0u64;
        let mut missing = Vec::new();
        for placement in self.planned {
            if cancel.load(Ordering::Relaxed) {
                return Ok(Outcome::Cancelled);
            }
            let entry = zip_entry(&placement.rel, self.base);
            reporter.copying(&entry);
            let Some(mut source) = export_source(self.library, &placement.item)? else {
                missing.push(placement.item.name.clone());
                reporter.item_done(0, &entry);
                continue;
            };
            let Some(written) = sink.add(&entry, placement.item.size, &mut source, cancel)? else {
                return Ok(Outcome::Cancelled);
            };
            if written != placement.item.size {
                return Err(SourceError::Storage(
                    "an original changed while it was exported".into(),
                ));
            }
            files += 1;
            bytes += written;
            reporter.item_done(written, &entry);
        }
        let manifest_items = manifest_with_base(self.manifest_items, self.base);
        let body = manifest_json(self.library_name, &manifest_items)?;
        let manifest = format!("{}/{MANIFEST}", self.base);
        if sink
            .add(&manifest, body.len() as u64, &mut body.as_slice(), cancel)?
            .is_none()
        {
            return Ok(Outcome::Cancelled);
        }
        files += 1;
        Ok(Outcome::Done(ExportSummary {
            items: manifest_items.len() as u64,
            files,
            bytes,
            missing,
            path: String::new(),
        }))
    }
}

/// Copies `source` into `target` in [`BUFFER`] chunks; `None` when `cancel` went up first.
/// `read_error` names a failed read; `on_chunk` hears the bytes copied so far.
fn copy_cancellable(
    source: &mut dyn Read,
    target: &mut dyn Write,
    cancel: &AtomicBool,
    read_error: &dyn Fn(io::Error) -> SourceError,
    on_chunk: &mut dyn FnMut(u64),
) -> Result<Option<u64>> {
    let mut buffer = vec![0u8; BUFFER];
    let mut done = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let read = match source.read(&mut buffer) {
            Ok(0) => return Ok(Some(done)),
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(read_error(error)),
        };
        target
            .write_all(&buffer[..read])
            .context("write the output")?;
        done += read as u64;
        on_chunk(done);
    }
}

/// Entry options; ZIP64 for an entry of 4 GiB or more, which a streamed archive must know
/// before it writes the entry.
fn file_options(compression: Compression, size: u64) -> SimpleFileOptions {
    let options = SimpleFileOptions::default().large_file(size >= u64::from(u32::MAX));
    match compression.level() {
        None => options.compression_method(zip::CompressionMethod::Stored),
        Some(level) => options
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(i64::from(level))),
    }
}

fn zip_error(error: zip::result::ZipError) -> SourceError {
    SourceError::Storage(format!("write the archive: {error}"))
}

/// Copies one item's original to `target`; `None` when the original is missing.
fn copy_item(library: &Library, item: &ExportItem, target: &Path) -> Result<Option<u64>> {
    let Some(mut source) = export_source(library, item)? else {
        return Ok(None);
    };
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).context("create the export folders")?;
    }
    let mut output = File::create(target).context("create an exported file")?;
    let written = io::copy(&mut source, &mut output).context("write an exported file")?;
    if written != item.size {
        let _ = fs::remove_file(target);
        return Err(SourceError::Storage(
            "an original changed while it was exported".into(),
        ));
    }
    Ok(Some(written))
}

/// Opens the item's original for export: a plaintext file or a decrypting reader. `None` when
/// the file is missing.
pub(crate) fn export_source(library: &Library, item: &ExportItem) -> Result<Option<Box<dyn Read>>> {
    let path = library_file(library.root(), &item.file_path)
        .ok_or_else(|| SourceError::Storage("bad stored path".into()))?;
    if item.encrypted {
        return library.open_sealed(&item.id, &path);
    }
    match File::open(&path) {
        Ok(file) => Ok(Some(Box::new(BufReader::new(file)))),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("read an original"),
    }
}

/// Serializes the manifest.
fn manifest_json(library_name: &str, items: &[ManifestItem]) -> Result<Vec<u8>> {
    let manifest = Manifest {
        app: "egl-viewer",
        library: library_name.to_owned(),
        exported_at: crate::time::now_ms(),
        items: items.to_vec(),
    };
    serde_json::to_vec_pretty(&manifest).context("serialize the manifest")
}

/// Writes the manifest next to the exported files.
fn write_manifest(path: &Path, library_name: &str, items: &[ManifestItem]) -> Result<()> {
    let body = manifest_json(library_name, items)?;
    let mut file = File::create(path).context("create the manifest")?;
    file.write_all(&body).context("write the manifest")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::io::{BufReader, Read, Write};
    use std::sync::atomic::AtomicBool;

    use image::ImageFormat;
    use library_core::{
        Compression, ExportFormat, ExportRequest, ExportSelection, ItemEdit, ZipEncryption,
    };
    use media::fixtures;
    use secrecy::SecretString;
    use sevenz_rust2::Password;

    use super::{MANIFEST, decrypt_archive, export_items, export_preview, unique_rel};
    use crate::Library;
    use crate::test_support::{add_png, create, files_under, new_library, open, put};

    fn request(
        selection: ExportSelection,
        include_subfolders: bool,
        format: ExportFormat,
        destination: &std::path::Path,
    ) -> ExportRequest {
        ExportRequest {
            selection,
            include_subfolders,
            format,
            destination: destination.to_string_lossy().into_owned(),
            encrypt: None,
            eagle: None,
        }
    }

    fn run(library: &Library, request: &ExportRequest) -> library_core::ExportSummary {
        let cancel = AtomicBool::new(false);
        export_items(library, 1, request, &cancel, &mut |_| {})
            .expect("export")
            .expect("not cancelled")
    }

    #[test]
    fn preview_counts_whole_folders_tags_without_double_counting() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Preview"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        let a = create(&library, "A", None);
        let b = create(&library, "B", None);
        let sub = create(&library, "Sub", Some(&a));
        let one = add_png(&library, &plain, "one.png", 1);
        let two = add_png(&library, &plain, "two.png", 2);
        let three = add_png(&library, &plain, "three.png", 3);
        put(&library, &one, &[&a]);
        put(&library, &two, &[&sub]);
        put(&library, &three, &[&b]);
        library
            .apply_item_edit(&one, &ItemEdit::AddTags(vec!["nature".into()]))
            .unwrap();
        library
            .apply_item_edit(&two, &ItemEdit::AddTags(vec!["nature".into()]))
            .unwrap();

        let whole = export_preview(
            &library,
            &ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
        )
        .unwrap();
        assert_eq!((whole.items, whole.hidden), (3, 0));

        // Without subfolders only the item directly in A counts; with them, both.
        let folders = ExportSelection {
            whole: false,
            folders: vec![a.clone()],
            tags: vec![],
        };
        let own = export_preview(&library, &folders, false).unwrap();
        assert_eq!(own.items, 1);
        let deep = export_preview(&library, &folders, true).unwrap();
        assert_eq!(deep.items, 2);

        // Two folders overlapping through a parent count an item once.
        let both = ExportSelection {
            whole: false,
            folders: vec![a.clone(), sub.clone()],
            tags: vec![],
        };
        assert_eq!(export_preview(&library, &both, true).unwrap().items, 2);

        let tag = ExportSelection {
            whole: false,
            folders: vec![],
            tags: vec!["nature".into()],
        };
        assert_eq!(export_preview(&library, &tag, false).unwrap().items, 2);

        // Unknown folder: refused, not silently less.
        let stale = ExportSelection {
            whole: false,
            folders: vec!["missing".into()],
            tags: vec![],
        };
        assert!(export_preview(&library, &stale, true).is_err());
    }

    #[test]
    fn mirror_writes_the_folder_tree_the_manifest_and_unique_second_runs() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Mirror"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        let trip = create(&library, "Trip", None);
        let sub = create(&library, "2024", Some(&trip));
        let one = add_png(&library, &plain, "one.png", 1);
        let two = add_png(&library, &plain, "two.png", 2);
        put(&library, &one, &[&trip]);
        put(&library, &two, &[&sub]);
        library
            .apply_item_edit(&one, &ItemEdit::AddTags(vec!["beach".into()]))
            .unwrap();
        let bytes = fixtures::raster(ImageFormat::Png, 8, 8, 1);

        let out = dir.path().join("out");
        let selection = ExportSelection {
            whole: false,
            folders: vec![trip.clone()],
            tags: vec![],
        };
        let summary = run(
            &library,
            &request(selection.clone(), true, ExportFormat::Mirror, &out),
        );
        assert_eq!(summary.items, 2);
        assert_eq!(summary.missing, Vec::<String>::new());
        let mut files = files_under(&out);
        files.retain(|name| name != "Trip/egl-export.json");
        assert_eq!(files, vec!["Trip/2024/two.png", "Trip/one.png"]);
        assert_eq!(fs::read(out.join("Trip/one.png")).unwrap(), bytes);

        let manifest = fs::read_to_string(out.join("Trip/egl-export.json")).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["items"][0]["tags"], serde_json::json!(["beach"]));
        assert_eq!(manifest["items"][0]["file"], "Trip/one.png");

        // The same export again does not merge into the first folder.
        run(
            &library,
            &request(selection, true, ExportFormat::Mirror, &out),
        );
        assert!(out.join("Trip (2)/one.png").is_file());
    }

    #[test]
    fn name_collisions_get_a_counter_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Collide"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        let folder = create(&library, "F", None);
        let one = add_png(&library, &plain, "same.png", 1);
        let two = add_png(&library, &plain, "same.png", 2);
        put(&library, &one, &[&folder]);
        put(&library, &two, &[&folder]);

        let out = dir.path().join("out");
        let selection = ExportSelection {
            whole: false,
            folders: vec![folder],
            tags: vec![],
        };
        run(
            &library,
            &request(selection, false, ExportFormat::Mirror, &out),
        );
        let files = files_under(&out);
        assert!(files.contains(&"F/same.png".to_string()));
        assert!(files.contains(&"F/same (2).png".to_string()));
    }

    #[test]
    fn zip_entries_mirror_the_tree_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Zip"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        let folder = create(&library, "Z", None);
        let one = add_png(&library, &plain, "one.png", 1);
        put(&library, &one, &[&folder]);

        let out = dir.path().join("out");
        let selection = ExportSelection {
            whole: false,
            folders: vec![folder],
            tags: vec![],
        };
        let format = ExportFormat::zip(library_core::Compression::Balanced);
        let summary = run(&library, &request(selection, false, format, &out));
        let archive = out.join("Z.zip");
        assert_eq!(summary.path, archive.to_string_lossy());

        let file = fs::File::open(&archive).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|at| zip.by_index(at).unwrap().name().unwrap().into_owned())
            .collect();
        assert_eq!(names, vec!["Z/one.png", "Z/egl-export.json"]);
        let mut body = Vec::new();
        zip.by_name("Z/one.png")
            .unwrap()
            .read_to_end(&mut body)
            .unwrap();
        assert_eq!(body, fixtures::raster(ImageFormat::Png, 8, 8, 1));
    }

    #[test]
    fn encrypted_archive_decrypts_with_the_passphrase() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Age"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);

        let out = dir.path().join("out");
        let mut request = request(
            ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
            ExportFormat::zip(library_core::Compression::Fast),
            &out,
        );
        request.encrypt = Some("correct horse".into());
        let summary = run(&library, &request);
        let encrypted = out.join("Age.zip.age");
        assert_eq!(summary.path, encrypted.to_string_lossy());
        assert!(
            !out.join("Age.zip").exists(),
            "the plain archive is removed"
        );

        let decryptor =
            age::Decryptor::new(BufReader::new(fs::File::open(&encrypted).unwrap())).unwrap();
        assert!(
            decryptor.is_scrypt(),
            "a passphrase export decrypts with a passphrase"
        );
        let identity = age::scrypt::Identity::new(SecretString::from("correct horse".to_owned()));
        let mut reader = decryptor
            .decrypt(std::iter::once(&identity as &dyn age::Identity))
            .unwrap();
        let mut body = Vec::new();
        reader.read_to_end(&mut body).unwrap();
        assert_eq!(&body[..2], b"PK", "decrypting gives the ZIP back");
    }

    #[test]
    fn missing_originals_are_reported_and_do_not_stop_the_export() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Missing"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        let gone = add_png(&library, &plain, "gone.png", 1);
        add_png(&library, &plain, "here.png", 2);
        // Remove one original: the row stays, the file is gone.
        let item_dir = library
            .root()
            .join("files")
            .join(&gone[gone.len() - 2..])
            .join(&gone);
        let file = fs::read_dir(&item_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::remove_file(&file).unwrap();

        let out = dir.path().join("out");
        let summary = run(
            &library,
            &request(
                ExportSelection {
                    whole: true,
                    ..Default::default()
                },
                true,
                ExportFormat::Mirror,
                &out,
            ),
        );
        assert_eq!(summary.items, 2);
        assert_eq!(summary.files, 2, "one item and the manifest");
        assert_eq!(summary.missing, vec!["gone".to_string()]);
        assert!(out.join("Missing/here.png").is_file());
        assert!(!out.join("Missing/gone.png").exists());
    }

    #[test]
    fn cancel_leaves_no_partial_output() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Cancel"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);

        let out = dir.path().join("out");
        let cancel = AtomicBool::new(true);
        let result = export_items(
            &library,
            1,
            &request(
                ExportSelection {
                    whole: true,
                    ..Default::default()
                },
                true,
                ExportFormat::Mirror,
                &out,
            ),
            &cancel,
            &mut |_| {},
        );
        assert!(result.unwrap().is_none());
        assert!(!out.join("Cancel").exists());
    }

    #[test]
    fn the_destination_must_stay_outside_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let root = new_library(dir.path(), "Inside");
        let library = open(&root);
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);

        let request = request(
            ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
            ExportFormat::Mirror,
            &root,
        );
        let cancel = AtomicBool::new(false);
        let error = export_items(&library, 1, &request, &cancel, &mut |_| {}).unwrap_err();
        assert!(matches!(error, library_core::SourceError::Invalid(_)));
    }

    #[test]
    fn a_failure_mid_export_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let root = new_library(dir.path(), "Broken");
        let library = open(&root);
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "a.png", 1);
        let b = add_png(&library, &plain, "b.png", 2);
        // The stored original grows behind the library's back: its copy fails.
        let stored: String = library
            .conn()
            .query_row("SELECT file_path FROM item WHERE id = ?1", [&b], |row| {
                row.get(0)
            })
            .unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(root.join(stored))
            .unwrap()
            .write_all(b"more")
            .unwrap();
        let whole = ExportSelection {
            whole: true,
            ..Default::default()
        };
        let cancel = AtomicBool::new(false);
        for (at, format) in [
            ExportFormat::Mirror,
            ExportFormat::zip(Compression::None),
            ExportFormat::Zip {
                compression: Compression::None,
                encryption: ZipEncryption::Aes,
            },
            ExportFormat::SevenZip {
                compression: Compression::Fast,
            },
        ]
        .into_iter()
        .enumerate()
        {
            let out = dir.path().join(format!("out-{at}"));
            let mut request = request(whole.clone(), true, format, &out);
            if format.is_archive() {
                request.encrypt = Some("correct horse".into());
            }
            assert!(export_items(&library, 1, &request, &cancel, &mut |_| {}).is_err());
            assert_eq!(files_under(&out), Vec::<String>::new(), "{format:?}");
        }
    }

    #[test]
    fn no_item_takes_the_manifest_name() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Names"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        let id = add_png(&library, &plain, "one.png", 1);
        library
            .apply_item_edit(&id, &ItemEdit::Rename("egl-export".into()))
            .unwrap();
        library
            .conn()
            .execute("UPDATE item SET ext = 'json' WHERE id = ?1", [&id])
            .unwrap();
        let out = dir.path().join("out");
        let summary = run(
            &library,
            &request(
                ExportSelection {
                    whole: true,
                    ..Default::default()
                },
                true,
                ExportFormat::Mirror,
                &out,
            ),
        );
        let root = std::path::Path::new(&summary.path);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(MANIFEST)).unwrap()).unwrap();
        assert_eq!(manifest["items"][0]["file"], "Names/egl-export (2).json");
        assert!(root.join("egl-export (2).json").is_file());
    }

    #[test]
    fn the_counter_goes_into_the_file_name_not_a_folder() {
        let mut used = HashSet::new();
        assert_eq!(unique_rel("a.b/README", &mut used), "a.b/README");
        assert_eq!(unique_rel("a.b/README", &mut used), "a.b/README (2)");
        assert_eq!(unique_rel("x/photo.jpg", &mut used), "x/photo.jpg");
        assert_eq!(unique_rel("x/Photo.JPG", &mut used), "x/Photo (2).JPG");
        assert_eq!(unique_rel(".gitignore", &mut used), ".gitignore");
        assert_eq!(unique_rel(".gitignore", &mut used), ".gitignore (2)");
    }

    #[test]
    fn the_destination_is_never_an_eagle_library() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Mine"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);
        let eagle = dir.path().join("Theirs.library");
        fs::create_dir_all(eagle.join("images")).unwrap();
        fs::write(eagle.join("metadata.json"), b"{}").unwrap();
        let request = request(
            ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
            ExportFormat::Mirror,
            &eagle.join("images"),
        );
        let cancel = AtomicBool::new(false);
        let error = export_items(&library, 1, &request, &cancel, &mut |_| {}).unwrap_err();
        assert!(
            matches!(
                error,
                library_core::SourceError::UnsupportedLocation(
                    library_core::LocationProblem::InsideEagleLibrary
                )
            ),
            "{error:?}"
        );
        assert_eq!(files_under(&eagle.join("images")), Vec::<String>::new());
    }

    #[test]
    fn whole_export_roots_unfiled_items_and_tag_exports_are_flat() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Roots"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        let folder = create(&library, "F", None);
        let filed = add_png(&library, &plain, "filed.png", 1);
        let loose = add_png(&library, &plain, "loose.png", 2);
        put(&library, &filed, &[&folder]);
        library
            .apply_item_edit(&loose, &ItemEdit::AddTags(vec!["solo".into()]))
            .unwrap();

        let out = dir.path().join("out");
        run(
            &library,
            &request(
                ExportSelection {
                    whole: true,
                    ..Default::default()
                },
                true,
                ExportFormat::Mirror,
                &out,
            ),
        );
        assert!(out.join("Roots/F/filed.png").is_file());
        assert!(out.join("Roots/loose.png").is_file());

        let out2 = dir.path().join("out2");
        run(
            &library,
            &request(
                ExportSelection {
                    whole: false,
                    folders: vec![],
                    tags: vec!["solo".into()],
                },
                false,
                ExportFormat::Mirror,
                &out2,
            ),
        );
        assert!(out2.join("solo/loose.png").is_file());

        // Encryption is only meaningful on an archive.
        let mut bad = request(
            ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
            ExportFormat::Mirror,
            &dir.path().join("out3"),
        );
        bad.encrypt = Some("password".into());
        let cancel = AtomicBool::new(false);
        assert!(export_items(&library, 1, &bad, &cancel, &mut |_| {}).is_err());
    }

    fn encrypted_request(out: &std::path::Path, passphrase: &str) -> ExportRequest {
        let mut request = request(
            ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
            ExportFormat::zip(library_core::Compression::None),
            out,
        );
        request.encrypt = Some(passphrase.into());
        request
    }

    fn names_in(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// D-110: the ZIP streams into the age writer, and the app opens it again.
    #[test]
    fn an_encrypted_export_is_streamed_and_decrypt_archive_opens_it() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Age"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);
        add_png(&library, &plain, "two.png", 2);
        let out = dir.path().join("out");
        let summary = run(&library, &encrypted_request(&out, "correct horse"));
        assert_eq!(
            names_in(&out),
            vec!["Age.zip.age".to_owned()],
            "no plaintext archive and no part file"
        );
        let encrypted = out.join("Age.zip.age");
        assert_eq!(summary.path, encrypted.to_string_lossy());
        let header = fs::read(&encrypted).unwrap();
        assert!(header.starts_with(b"age-encryption.org/v1"));

        let opened = dir.path().join("opened");
        fs::create_dir(&opened).unwrap();
        let cancel = AtomicBool::new(false);
        let mut heard = Vec::new();
        let decrypted = decrypt_archive(
            &encrypted,
            &opened,
            "correct horse",
            &cancel,
            &mut |done, total| {
                heard.push((done, total));
            },
        )
        .unwrap()
        .expect("not cancelled");
        let size = fs::metadata(&encrypted).unwrap().len();
        assert_eq!(
            heard.last(),
            Some(&(size, size)),
            "progress ends at the whole archive"
        );
        let zip_path = std::path::PathBuf::from(&decrypted.path);
        assert_eq!(zip_path.file_name().unwrap(), "Age.zip");
        assert_eq!(decrypted.bytes, fs::metadata(&zip_path).unwrap().len());
        let mut zip = zip::ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
        let mut entries: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
            .collect();
        entries.sort();
        assert_eq!(entries.len(), 3, "two images and the manifest: {entries:?}");
        assert!(entries.contains(&format!("Age/{MANIFEST}")));

        // A second copy goes beside the first.
        let again = decrypt_archive(
            &encrypted,
            &opened,
            "correct horse",
            &cancel,
            &mut |_, _| {},
        )
        .unwrap()
        .unwrap();
        assert!(again.path.ends_with("Age (2).zip"), "{}", again.path);
        assert_eq!(names_in(&opened), vec!["Age (2).zip", "Age.zip"]);
    }

    #[test]
    fn a_short_passphrase_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Short"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);
        let out = dir.path().join("out");
        let cancel = AtomicBool::new(false);
        let error = export_items(
            &library,
            1,
            &encrypted_request(&out, "1234567"),
            &cancel,
            &mut |_| {},
        )
        .unwrap_err();
        assert!(error.to_string().contains("8 characters"), "{error}");
        assert!(names_in(&out).is_empty());
        // Eight characters, counted as characters, not bytes.
        assert!(
            export_items(
                &library,
                1,
                &encrypted_request(&out, "пароль12"),
                &cancel,
                &mut |_| {}
            )
            .is_ok()
        );
    }

    /// A cancel while an encrypted archive is written stops it and leaves nothing (D-110).
    #[test]
    fn cancel_while_encrypting_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Stop"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);
        add_png(&library, &plain, "two.png", 2);
        let out = dir.path().join("out");
        let cancel = AtomicBool::new(false);
        let result = export_items(
            &library,
            1,
            &encrypted_request(&out, "correct horse"),
            &cancel,
            &mut |progress| {
                if progress.state == library_core::ExportState::Copying {
                    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            },
        );
        assert!(result.unwrap().is_none(), "cancelled");
        assert!(names_in(&out).is_empty(), "{:?}", names_in(&out));
    }

    /// D-112: a ZIP whose entries are AES-256 encrypted; the names stay readable, the
    /// contents open only with the password.
    #[test]
    fn a_zip_with_a_password_opens_only_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Aes"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);
        let out = dir.path().join("out");
        let mut request = request(
            ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
            ExportFormat::Zip {
                compression: Compression::Balanced,
                encryption: ZipEncryption::Aes,
            },
            &out,
        );
        request.encrypt = Some("пароль 🔑 длинный".into());
        let summary = run(&library, &request);
        let archive = out.join("Aes.zip");
        assert_eq!(summary.path, archive.to_string_lossy());
        assert_eq!(
            names_in(&out),
            vec!["Aes.zip".to_owned()],
            "no .age, no .part"
        );

        let mut zip = zip::ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|at| zip.name_for_index(at).unwrap().unwrap().into_owned())
            .collect();
        assert_eq!(names, vec!["Aes/one.png", "Aes/egl-export.json"]);
        assert!(
            zip.by_name("Aes/one.png").is_err(),
            "no password, no contents"
        );
        assert!(
            zip.by_name_decrypt("Aes/one.png", "wrong password".as_bytes())
                .is_err(),
            "a wrong password is refused"
        );
        let mut body = Vec::new();
        let mut entry = zip
            .by_name_decrypt("Aes/one.png", "пароль 🔑 длинный".as_bytes())
            .unwrap();
        assert!(entry.encrypted());
        entry.read_to_end(&mut body).unwrap();
        assert_eq!(body, fixtures::raster(ImageFormat::Png, 8, 8, 1));
    }

    /// D-113: a 7z archive; with a password nothing, not even a name, shows without it.
    #[test]
    fn a_seven_zip_export_reads_back_and_a_password_hides_the_names() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Seven"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);
        add_png(&library, &plain, "Фото 🎨.png", 2);
        let whole = ExportSelection {
            whole: true,
            ..Default::default()
        };
        for (at, compression) in [Compression::None, Compression::Smallest]
            .into_iter()
            .enumerate()
        {
            let out = dir.path().join(format!("open-{at}"));
            let format = ExportFormat::SevenZip { compression };
            let summary = run(&library, &request(whole.clone(), true, format, &out));
            let archive = out.join("Seven.7z");
            assert_eq!(summary.path, archive.to_string_lossy());
            let mut reader =
                sevenz_rust2::ArchiveReader::open(&archive, Password::empty()).unwrap();
            let mut names: Vec<String> = reader
                .archive()
                .files
                .iter()
                .map(|entry| entry.name().to_owned())
                .collect();
            names.sort();
            assert_eq!(
                names,
                vec![
                    "Seven/egl-export.json",
                    "Seven/one.png",
                    "Seven/Фото 🎨.png"
                ]
            );
            assert_eq!(
                reader.read_file("Seven/Фото 🎨.png").unwrap(),
                fixtures::raster(ImageFormat::Png, 8, 8, 2)
            );
        }

        let out = dir.path().join("locked");
        let mut request = request(
            whole,
            true,
            ExportFormat::SevenZip {
                compression: Compression::Fast,
            },
            &out,
        );
        request.encrypt = Some("correct horse".into());
        let summary = run(&library, &request);
        assert_eq!(names_in(&out), vec!["Seven.7z".to_owned()]);
        let raw = fs::read(&summary.path).unwrap();
        // 7z stores names as UTF-16LE.
        let utf16: Vec<u8> = "one.png"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert!(
            !raw.windows(b"one.png".len()).any(|at| at == b"one.png")
                && !raw.windows(utf16.len()).any(|at| at == utf16.as_slice()),
            "the names are encrypted too"
        );
        assert!(
            sevenz_rust2::ArchiveReader::open(&summary.path, Password::empty()).is_err(),
            "no password, no list of names"
        );
        assert!(
            sevenz_rust2::ArchiveReader::open(&summary.path, Password::new("wrong horse")).is_err(),
            "a wrong password is refused"
        );
        let mut reader =
            sevenz_rust2::ArchiveReader::open(&summary.path, Password::new("correct horse"))
                .unwrap();
        assert_eq!(
            reader.read_file("Seven/one.png").unwrap(),
            fixtures::raster(ImageFormat::Png, 8, 8, 1)
        );
    }

    #[test]
    fn cancel_inside_a_seven_zip_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Stop7"));
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);
        add_png(&library, &plain, "two.png", 2);
        let out = dir.path().join("out");
        let cancel = AtomicBool::new(false);
        let mut request = request(
            ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
            ExportFormat::SevenZip {
                compression: Compression::Balanced,
            },
            &out,
        );
        request.encrypt = Some("correct horse".into());
        let result = export_items(&library, 1, &request, &cancel, &mut |progress| {
            if progress.state == library_core::ExportState::Copying {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        });
        assert!(result.unwrap().is_none(), "cancelled");
        assert!(names_in(&out).is_empty(), "{:?}", names_in(&out));
    }

    #[test]
    fn a_folder_export_takes_no_passphrase() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&new_library(dir.path(), "Plain"));
        let out = dir.path().join("out");
        let mut request = request(
            ExportSelection {
                whole: true,
                ..Default::default()
            },
            true,
            ExportFormat::Mirror,
            &out,
        );
        request.encrypt = Some("correct horse".into());
        let error =
            export_items(&library, 1, &request, &AtomicBool::new(false), &mut |_| {}).unwrap_err();
        assert!(error.to_string().contains("ZIP or 7z"), "{error}");
    }

    #[test]
    fn decrypting_refuses_a_wrong_passphrase_a_damaged_file_and_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = new_library(dir.path(), "Refuse");
        let library = open(&root);
        let plain = dir.path().join("plain");
        fs::create_dir(&plain).unwrap();
        add_png(&library, &plain, "one.png", 1);
        let out = dir.path().join("out");
        run(&library, &encrypted_request(&out, "correct horse"));
        let encrypted = out.join("Refuse.zip.age");
        let opened = dir.path().join("opened");
        fs::create_dir(&opened).unwrap();
        let cancel = AtomicBool::new(false);

        let wrong = decrypt_archive(&encrypted, &opened, "wrong horse!", &cancel, &mut |_, _| {})
            .unwrap_err();
        assert!(wrong.to_string().contains("Wrong passphrase."), "{wrong}");

        let mut bytes = fs::read(&encrypted).unwrap();
        bytes.truncate(bytes.len() - 40);
        let damaged = out.join("Damaged.zip.age");
        fs::write(&damaged, &bytes).unwrap();
        let error = decrypt_archive(&damaged, &opened, "correct horse", &cancel, &mut |_, _| {})
            .unwrap_err();
        assert!(error.to_string().contains("damaged"), "{error}");

        let other = out.join("notes.txt.age");
        fs::write(&other, b"just text").unwrap();
        let error =
            decrypt_archive(&other, &opened, "correct horse", &cancel, &mut |_, _| {}).unwrap_err();
        assert!(error.to_string().contains("not an age archive"), "{error}");

        assert!(
            names_in(&opened).is_empty(),
            "nothing left: {:?}",
            names_in(&opened)
        );

        // Never into a library.
        let inside = root.join("files");
        assert!(
            decrypt_archive(
                &encrypted,
                &inside,
                "correct horse",
                &cancel,
                &mut |_, _| {}
            )
            .is_err()
        );
        let cancelled = AtomicBool::new(true);
        assert!(
            decrypt_archive(
                &encrypted,
                &opened,
                "correct horse",
                &cancelled,
                &mut |_, _| {}
            )
            .unwrap()
            .is_none()
        );
        assert!(names_in(&opened).is_empty());
    }
}
