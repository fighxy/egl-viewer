//! Exporting originals to a folder tree or a ZIP archive (docs/spec.md §7.4 item 12): what a Backup or
//! an Export writes, where, and with what protection. Reads only, so a read-only library can
//! export; items of a locked folder export decrypted once the folder is unlocked.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use age::Encryptor;
use library_core::{
    Compression, ExportFormat, ExportPreview, ExportProgress, ExportRequest, ExportSelection,
    ExportState, ExportSummary, SourceError,
};
use rusqlite::{Connection, OptionalExtension, params, types::Value};
use secrecy::SecretString;
use serde::Serialize;
use zip::write::SimpleFileOptions;

use crate::add::file_name_for;
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

/// One item placed in the export.
struct ExportItem {
    id: String,
    name: String,
    ext: String,
    size: u64,
    star: i64,
    annotation: String,
    url: String,
    file_path: String,
    encrypted: bool,
    tags: Vec<String>,
    /// The direct folders' names, sorted; for the manifest.
    folder_names: Vec<String>,
    /// The direct folders' ids; for the layout.
    folder_ids: Vec<String>,
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
struct SelectionSql {
    prefix: String,
    condition: String,
    /// `:fN` folder ids and `:tN` tag names; `:hidden`/`:unlocked` come from the visibility.
    binds: Vec<(String, Value)>,
}

fn selection_sql(
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
fn bind<'a>(sql: &'a SelectionSql, visibility: Option<&'a Visibility>) -> Vec<(&'a str, Value)> {
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
    let conn = library.conn();
    let sql = selection_sql(&conn, &request.selection, request.include_subfolders)?;
    let items = load_export_items(library, &sql)?;
    if items.is_empty() {
        return Err(SourceError::Invalid("The selection has no items.".into()));
    }
    let folder_rows = load_folder_rows(&conn)?;
    let roots = layout_roots(&conn, &request.selection)?;
    let (planned, manifest_items) = plan_layout(items, &folder_rows, &roots);
    let bytes_total = planned.iter().map(|p| p.item.size).sum();
    let destination = PathBuf::from(&request.destination);
    fs::create_dir_all(&destination).context("create the destination folder")?;
    let base = export_base(library, &conn, &request.selection);
    let mut reporter = Reporter {
        emit: on_progress,
        id,
        total: planned.len() as u64,
        bytes_total,
        done: 0,
        bytes_done: 0,
        last: Instant::now() - PROGRESS_EVERY,
    };
    let outcome = match request.format {
        ExportFormat::Mirror => write_mirror(
            library,
            &destination,
            &base,
            &planned,
            &manifest_items,
            library.name().clone(),
            cancel,
            &mut reporter,
        ),
        ExportFormat::Zip { compression } => write_zip(
            library,
            &destination,
            &base,
            &planned,
            &manifest_items,
            library.name().clone(),
            compression,
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
    if request.encrypt.is_some() && !matches!(request.format, ExportFormat::Zip { .. }) {
        return Err(SourceError::Invalid(
            "Encryption needs the ZIP format.".into(),
        ));
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
    Ok(())
}

/// The visible items of the selection with their tags and folder links, names opened for
/// encrypted items of an unlocked folder.
fn load_export_items(library: &Library, sql: &SelectionSql) -> Result<Vec<ExportItem>> {
    let conn = library.conn();
    let visibility = library.visibility(&conn)?;
    let query = format!(
        "{}SELECT id, name, ext, size, star, annotation, url, file_path, encrypted, sealed \
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
) -> (Vec<Placement>, Vec<ManifestItem>) {
    let root_set: HashSet<&str> = roots.iter().map(String::as_str).collect();
    let mut used: HashSet<String> = HashSet::new();
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
            return Some(
                chain
                    .iter()
                    .rev()
                    .map(|id| {
                        let name = rows
                            .get(*id)
                            .map(|row| row.name.as_str())
                            .unwrap_or("folder");
                        sanitize_segment(name)
                    })
                    .collect::<Option<Vec<_>>>()?,
            );
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
    let (stem, ext) = rel.rsplit_once('.').unwrap_or((rel, ""));
    for n in 2.. {
        let candidate = if ext.is_empty() {
            format!("{stem} ({n})")
        } else {
            format!("{stem} ({n}).{ext}")
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
    let name = file_name_for(raw.as_deref().unwrap_or("").trim(), "");
    if name.is_empty() {
        library.name().clone()
    } else {
        name
    }
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
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut missing = Vec::new();
    for placement in planned {
        if cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_dir_all(&root);
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
    let manifest_path = root.join(MANIFEST);
    match write_manifest(&manifest_path, &library_name, &manifest_items) {
        Ok(()) => {
            files += 1;
            Ok(Outcome::Done(ExportSummary {
                items: manifest_items.len() as u64,
                files,
                bytes,
                missing,
                path: root.to_string_lossy().into_owned(),
            }))
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&root);
            Err(error)
        }
    }
}

/// Writes the placements and the manifest into `<destination>/<base>.zip`, wrapped in an age
/// passphrase as `<base>.zip.age` when `encrypt` is set.
#[allow(clippy::too_many_arguments)]
fn write_zip(
    library: &Library,
    destination: &Path,
    base: &str,
    planned: &[Placement],
    manifest_items: &[ManifestItem],
    library_name: String,
    compression: Compression,
    encrypt: Option<&str>,
    cancel: &AtomicBool,
    reporter: &mut Reporter,
) -> Result<Outcome> {
    let archive = unique_child(destination, base, "zip");
    let part = archive.with_extension("zip.part");
    let outcome = (|| -> Result<Outcome> {
        let file = File::create(&part).context("create the archive")?;
        let mut zip = zip::ZipWriter::new(file).set_auto_large_file();
        let mut files = 0u64;
        let mut bytes = 0u64;
        let mut missing = Vec::new();
        for placement in planned {
            if cancel.load(Ordering::Relaxed) {
                drop(zip);
                let _ = fs::remove_file(&part);
                return Ok(Outcome::Cancelled);
            }
            let entry = zip_entry(&placement.rel, base);
            reporter.copying(&entry);
            let Some(mut source) = export_source(library, &placement.item)? else {
                missing.push(placement.item.name.clone());
                reporter.item_done(0, &entry);
                continue;
            };
            zip.start_file(entry.clone(), file_options(compression))
                .map_err(zip_error)?;
            let written = io::copy(&mut source, &mut zip).context("write the archive")?;
            if written != placement.item.size {
                return Err(SourceError::Storage(
                    "an original changed while it was exported".into(),
                ));
            }
            files += 1;
            bytes += written;
            reporter.item_done(written, &entry);
        }
        let manifest_items = manifest_with_base(manifest_items, base);
        zip.start_file(format!("{base}/{MANIFEST}"), file_options(compression))
            .map_err(zip_error)?;
        let body = manifest_json(&library_name, &manifest_items)?;
        zip.write_all(&body).context("write the archive")?;
        files += 1;
        zip.finish().map_err(zip_error)?;
        drop(zip);
        Ok(Outcome::Done(ExportSummary {
            items: manifest_items.len() as u64,
            files,
            bytes,
            missing,
            path: archive.to_string_lossy().into_owned(),
        }))
    })();
    match outcome {
        Ok(Outcome::Cancelled) => Ok(Outcome::Cancelled),
        Ok(done @ Outcome::Done(_)) => {
            let Outcome::Done(mut summary) = done else {
                unreachable!()
            };
            if let Some(password) = encrypt {
                let encrypted = archive.with_file_name(format!(
                    "{}.age",
                    archive
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("export.zip")
                ));
                let part_len = fs::metadata(&part)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                encrypt_file(&part, &encrypted, password, reporter, part_len)?;
                fs::remove_file(&part).context("remove the temporary archive")?;
                summary.path = encrypted.to_string_lossy().into_owned();
            } else {
                rename_with_retry(&part, &archive).context("finish the archive")?;
            }
            Ok(Outcome::Done(summary))
        }
        Err(error) => {
            let _ = fs::remove_file(&part);
            Err(error)
        }
    }
}

fn file_options(compression: Compression) -> SimpleFileOptions {
    let options = SimpleFileOptions::default();
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
fn export_source(library: &Library, item: &ExportItem) -> Result<Option<Box<dyn Read>>> {
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

/// Encrypts the finished archive into `.zip.age`, with byte progress.
fn encrypt_file(
    source: &Path,
    target: &Path,
    password: &str,
    reporter: &mut Reporter,
    total: u64,
) -> Result<()> {
    reporter.report(ExportState::Encrypting);
    let input = BufReader::with_capacity(BUFFER, File::open(source).context("read the archive")?);
    let output = File::create(target).context("create the encrypted archive")?;
    let encryptor = Encryptor::with_user_passphrase(SecretString::from(password.to_owned()));
    let mut writer = encryptor
        .wrap_output(output)
        .context("start encrypting the archive")?;
    let mut input = input.take(total);
    let mut buffer = vec![0u8; BUFFER];
    let mut done = 0u64;
    let mut last = Instant::now() - PROGRESS_EVERY;
    loop {
        let read = input.read(&mut buffer).context("read the archive")?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .context("write the encrypted archive")?;
        done += read as u64;
        if last.elapsed() >= PROGRESS_EVERY {
            last = Instant::now();
            (reporter.emit)(ExportProgress {
                id: reporter.id,
                state: ExportState::Encrypting,
                total: reporter.total,
                done: reporter.done,
                bytes_total: total,
                bytes_done: done,
                current: None,
                summary: None,
                error: None,
            });
        }
    }
    writer
        .finish()
        .and_then(|mut file| file.sync_all())
        .context("finish the encrypted archive")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufReader, Read};
    use std::sync::atomic::AtomicBool;

    use image::ImageFormat;
    use library_core::{ExportFormat, ExportRequest, ExportSelection, ItemEdit};
    use media::fixtures;
    use secrecy::SecretString;

    use super::{export_items, export_preview};
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
        let format = ExportFormat::Zip {
            compression: library_core::Compression::Balanced,
        };
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
            ExportFormat::Zip {
                compression: library_core::Compression::Fast,
            },
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
}
