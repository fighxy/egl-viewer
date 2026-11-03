//! Reading a library for an export into a running Eagle and writing the Eagle ids back
//! (docs/spec.md §8.4, docs/architecture.md §15.7). The ids an export created live in
//! `eagle_export` (format version 6, D-087); the `eagle_id` columns name where an imported
//! object came from and are never written here.

use std::collections::{HashMap, HashSet};

use library_core::{ExportSelection, SourceError};
use rusqlite::{Connection, params};

use crate::Library;
use crate::error::Result;
use crate::export::{self, ExportItem, export_source, load_export_items};

/// A folder as the Eagle export needs it, top-down (a parent always precedes its children).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EagleExportFolder {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    pub description: String,
    pub icon_color: Option<String>,
    pub order_by: Option<String>,
    pub sort_ascending: Option<bool>,
    pub eagle_id: Option<String>,
    /// A locked folder is exported without its lock, after the warning (docs/spec.md §8.4).
    pub locked: bool,
}

/// An item as the Eagle export needs it (the internal `ExportItem` stays crate-private).
#[derive(Debug, Clone)]
pub struct EagleExportItem {
    pub id: String,
    pub name: String,
    pub ext: String,
    pub size: u64,
    pub star: i64,
    pub annotation: String,
    pub url: String,
    /// Path of the stored original, relative to the library root.
    pub file_path: String,
    /// In a locked folder: read through the session keys (exported unlocked, with the warning).
    pub encrypted: bool,
    pub tags: Vec<String>,
    pub folder_ids: Vec<String>,
    /// Eagle id from a previous export into Eagle (docs/spec.md §8.4), from `eagle_export`.
    pub eagle_id: Option<String>,
    /// Date added, ms: Eagle `modificationTime`.
    pub added_at: i64,
}

/// Everything one export into Eagle sends: folders top-down, then items, tag groups, smart
/// folders and comments.
#[derive(Default)]
pub struct EagleExportPlan {
    pub folders: Vec<EagleExportFolder>,
    pub items: Vec<EagleExportItem>,
    pub groups: Vec<EagleGroup>,
    pub smart: Vec<EagleSmart>,
    pub comments: Vec<EagleComment>,
    /// Names of locked folders included in the export; the UI warns about them.
    pub unlocked: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EagleGroup {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub eagle_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EagleSmart {
    pub id: String,
    pub parent: Option<String>,
    pub name: String,
    /// Eagle's rule JSON, kept unchanged (docs/architecture.md §16).
    pub conditions: String,
    pub eagle_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EagleComment {
    pub item_id: String,
    pub annotation: String,
    /// `[x, y, w, h]` in image pixels; `None` for a video time.
    pub rect: Option<[f64; 4]>,
    pub at_seconds: Option<f64>,
}

/// Loads what an export into Eagle covers. `whole` takes every folder; a folder selection
/// takes it with its subfolders. Items hidden by a locked folder stay home, like every
/// export; the folders themselves are listed (exported unlocked, after the warning).
pub fn load_eagle_export(
    library: &Library,
    selection: &ExportSelection,
    include_subfolders: bool,
) -> Result<EagleExportPlan> {
    // `load_export_items` takes its own connection; build the SQL first, then drop the guard.
    let sql = {
        let conn = library.conn();
        export::selection_sql(&conn, selection, include_subfolders)?
    };
    let exported = {
        let conn = library.conn();
        exported_ids(&conn, "item")?
    };
    let items: Vec<EagleExportItem> = load_export_items(library, &sql)?
        .into_iter()
        .map(|item| EagleExportItem {
            eagle_id: exported.get(&item.id).cloned(),
            id: item.id,
            name: item.name,
            ext: item.ext,
            size: item.size,
            star: item.star,
            annotation: item.annotation,
            url: item.url,
            file_path: item.file_path,
            encrypted: item.encrypted,
            tags: item.tags,
            folder_ids: item.folder_ids,
            added_at: item.added_at,
        })
        .collect();
    let conn = library.conn();
    let hidden = library.visibility(&conn)?.hidden_folders;
    let folders = export_folders(&conn, selection, &hidden)?;
    let unlocked = folders
        .iter()
        .filter(|folder| folder.locked)
        .map(|folder| folder.name.clone())
        .collect();
    let by_id: HashSet<&str> = items.iter().map(|item| item.id.as_str()).collect();
    Ok(EagleExportPlan {
        folders,
        comments: export_comments(&conn, &by_id)?,
        groups: export_groups(&conn)?,
        smart: export_smart(&conn)?,
        items,
        unlocked,
    })
}

/// The stored original as a disk path, when it lives in `files/` and exists (the same-PC
/// `item/add` form takes a path on the Eagle PC).
pub fn item_path(library: &Library, item: &EagleExportItem) -> Option<std::path::PathBuf> {
    let path = crate::recover::library_file(library.root(), &item.file_path)?;
    path.is_file().then_some(path)
}

/// Reads the original of an export item (decrypting when needed) for the data-URL form.
pub fn item_bytes(library: &Library, item: &EagleExportItem) -> Result<Option<Vec<u8>>> {
    let internal = ExportItem {
        id: item.id.clone(),
        name: String::new(),
        ext: String::new(),
        size: item.size,
        star: 0,
        annotation: String::new(),
        url: String::new(),
        file_path: item.file_path.clone(),
        encrypted: item.encrypted,
        tags: Vec::new(),
        folder_names: Vec::new(),
        folder_ids: Vec::new(),
        added_at: 0,
    };
    let Some(mut source) = export_source(library, &internal)? else {
        return Ok(None);
    };
    use std::io::Read;
    let mut bytes = Vec::with_capacity(item.size as usize);
    source
        .read_to_end(&mut bytes)
        .map_err(|error| SourceError::Storage(format!("read an original: {error}")))?;
    Ok(Some(bytes))
}

/// Stores the Eagle ids of everything the export created, so a second export updates
/// instead of duplicating (docs/spec.md §8.4 step 6). One transaction.
pub fn store_eagle_ids(
    library: &Library,
    folders: &[(String, String)],
    items: &[(String, String)],
    groups: &[(String, String)],
    smart: &[(String, String)],
) -> Result<()> {
    let mut conn = library.conn();
    let tx = conn
        .transaction()
        .map_err(|error| SourceError::Storage(format!("store the Eagle ids: {error}")))?;
    {
        let mut statement = tx
            .prepare(
                "INSERT INTO eagle_export (kind, native_id, eagle_id) VALUES (?1, ?2, ?3)
                 ON CONFLICT (kind, native_id) DO UPDATE SET eagle_id = excluded.eagle_id",
            )
            .map_err(|error| SourceError::Storage(format!("store the Eagle ids: {error}")))?;
        for (kind, pairs) in [
            ("folder", folders),
            ("item", items),
            ("tag_group", groups),
            ("smart_folder", smart),
        ] {
            for (native, eagle) in pairs {
                statement
                    .execute(params![kind, native, eagle])
                    .map_err(|error| {
                        SourceError::Storage(format!("store the Eagle ids: {error}"))
                    })?;
            }
        }
    }
    tx.commit()
        .map_err(|error| SourceError::Storage(format!("store the Eagle ids: {error}")))?;
    Ok(())
}

/// Native id → the Eagle id a previous export created, for one kind of object.
fn exported_ids(conn: &Connection, kind: &str) -> Result<HashMap<String, String>> {
    let mut statement = conn
        .prepare("SELECT native_id, eagle_id FROM eagle_export WHERE kind = ?1")
        .map_err(|error| SourceError::Storage(format!("read the Eagle ids: {error}")))?;
    statement
        .query_map(params![kind], |row| Ok((row.get(0)?, row.get(1)?)))
        .and_then(Iterator::collect)
        .map_err(|error| SourceError::Storage(format!("read the Eagle ids: {error}")))
}

struct FolderRow {
    id: String,
    parent: Option<String>,
    name: String,
    description: String,
    icon_color: Option<String>,
    order_by: Option<String>,
    sort_ascending: Option<bool>,
    eagle_id: Option<String>,
    locked: bool,
}

/// The folders of the selection, parents before children and siblings in their order. `whole`
/// takes every folder; otherwise the selected folders and everything under them.
fn export_folders(
    conn: &Connection,
    selection: &ExportSelection,
    hidden: &HashSet<String>,
) -> Result<Vec<EagleExportFolder>> {
    let mut statement = conn
        .prepare(
            "SELECT folder.id, parent_id, name, description, icon_color, order_by, \
             sort_ascending, eagle_export.eagle_id, locked FROM folder \
             LEFT JOIN eagle_export ON eagle_export.kind = 'folder' \
             AND eagle_export.native_id = folder.id \
             ORDER BY folder.sort_key",
        )
        .map_err(|error| SourceError::Storage(format!("read the folders: {error}")))?;
    let mut rows = statement
        .query([])
        .map_err(|error| SourceError::Storage(format!("read the folders: {error}")))?;
    let mut by_id: HashMap<String, FolderRow> = HashMap::new();
    let mut children: HashMap<Option<String>, Vec<String>> = HashMap::new();
    let mut roots: Vec<String> = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| SourceError::Storage(format!("read the folders: {error}")))?
    {
        let row = FolderRow {
            id: row.get(0).map_err(read_folder)?,
            parent: row.get(1).map_err(read_folder)?,
            name: row.get(2).map_err(read_folder)?,
            description: row.get(3).map_err(read_folder)?,
            icon_color: row.get(4).map_err(read_folder)?,
            order_by: row.get(5).map_err(read_folder)?,
            sort_ascending: row.get(6).map_err(read_folder)?,
            eagle_id: row.get(7).map_err(read_folder)?,
            locked: row.get(8).map_err(read_folder)?,
        };
        let parent = row.parent.clone();
        let id = row.id.clone();
        if parent.is_none() {
            roots.push(id.clone());
        }
        children.entry(parent).or_default().push(id.clone());
        by_id.insert(id, row);
    }
    let wanted: HashSet<String> = if selection.whole {
        by_id.keys().cloned().collect()
    } else {
        let mut wanted = HashSet::new();
        let mut stack: Vec<String> = selection.folders.clone();
        while let Some(id) = stack.pop() {
            if wanted.insert(id.clone())
                && let Some(kids) = children.get(&Some(id))
            {
                stack.extend(kids.iter().cloned());
            }
        }
        wanted
    };
    // One walk over the whole tree in sibling order: parents always precede their children,
    // also for a selected folder under a parent that is not exported. Subfolders of a locked
    // folder that is not unlocked stay home, as the tree hides them; the locked folder itself
    // is listed (exported unlocked, after the warning).
    let mut ordered = Vec::with_capacity(wanted.len());
    let mut stack: Vec<String> = roots.into_iter().rev().collect();
    while let Some(id) = stack.pop() {
        let Some(row) = by_id.remove(&id) else {
            continue;
        };
        let under_a_lock = hidden.contains(&row.id)
            && row
                .parent
                .as_ref()
                .is_some_and(|parent| hidden.contains(parent));
        if under_a_lock {
            continue;
        }
        if let Some(kids) = children.get(&Some(id.clone())) {
            stack.extend(kids.iter().rev().cloned());
        }
        if wanted.contains(&row.id) {
            ordered.push(EagleExportFolder {
                id: row.id,
                parent: row.parent,
                name: row.name,
                description: row.description,
                icon_color: row.icon_color,
                order_by: row.order_by,
                sort_ascending: row.sort_ascending,
                eagle_id: row.eagle_id,
                locked: row.locked,
            });
        }
    }
    Ok(ordered)
}

fn read_folder(error: rusqlite::Error) -> SourceError {
    SourceError::Storage(format!("read the folders: {error}"))
}

/// The comments of the exported items, for `item/addComment` (docs/spec.md §8.4 step 4).
fn export_comments(conn: &Connection, items: &HashSet<&str>) -> Result<Vec<EagleComment>> {
    let mut statement = conn
        .prepare("SELECT item_id, annotation, x, y, w, h, at_seconds FROM comment")
        .map_err(|error| SourceError::Storage(format!("read the comments: {error}")))?;
    let mut rows = statement
        .query([])
        .map_err(|error| SourceError::Storage(format!("read the comments: {error}")))?;
    let mut comments = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| SourceError::Storage(format!("read the comments: {error}")))?
    {
        let item_id: String = row
            .get(0)
            .map_err(|error| SourceError::Storage(format!("read a comment: {error}")))?;
        if !items.contains(item_id.as_str()) {
            continue;
        }
        let rect = match (
            row.get::<_, Option<f64>>(2),
            row.get::<_, Option<f64>>(3),
            row.get::<_, Option<f64>>(4),
            row.get::<_, Option<f64>>(5),
        ) {
            (Ok(Some(x)), Ok(Some(y)), Ok(Some(w)), Ok(Some(h))) => Some([x, y, w, h]),
            _ => None,
        };
        comments.push(EagleComment {
            item_id,
            annotation: row
                .get(1)
                .map_err(|error| SourceError::Storage(format!("read a comment: {error}")))?,
            rect,
            at_seconds: row
                .get(6)
                .map_err(|error| SourceError::Storage(format!("read a comment: {error}")))?,
        });
    }
    Ok(comments)
}

fn export_groups(conn: &Connection) -> Result<Vec<EagleGroup>> {
    let mut statement = conn
        .prepare(
            "SELECT tag_group.id, name, color, eagle_export.eagle_id FROM tag_group \
             LEFT JOIN eagle_export ON eagle_export.kind = 'tag_group' \
             AND eagle_export.native_id = tag_group.id ORDER BY sort_key",
        )
        .map_err(|error| SourceError::Storage(format!("read the tag groups: {error}")))?;
    let mut rows = statement
        .query([])
        .map_err(|error| SourceError::Storage(format!("read the tag groups: {error}")))?;
    let mut groups = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| SourceError::Storage(format!("read the tag groups: {error}")))?
    {
        groups.push(EagleGroup {
            id: row.get(0).map_err(read_group)?,
            name: row.get(1).map_err(read_group)?,
            color: row.get(2).map_err(read_group)?,
            eagle_id: row.get(3).map_err(read_group)?,
            tags: Vec::new(),
        });
    }
    drop(rows);
    drop(statement);
    let mut statement = conn
        .prepare(
            "SELECT tag_group_tag.group_id, tag.name FROM tag_group_tag \
             JOIN tag ON tag.id = tag_group_tag.tag_id ORDER BY tag.name",
        )
        .map_err(|error| SourceError::Storage(format!("read the tag groups: {error}")))?;
    let mut rows = statement
        .query([])
        .map_err(|error| SourceError::Storage(format!("read the tag groups: {error}")))?;
    let at: HashMap<String, usize> = groups
        .iter()
        .enumerate()
        .map(|(at, group)| (group.id.clone(), at))
        .collect();
    while let Some(row) = rows
        .next()
        .map_err(|error| SourceError::Storage(format!("read the tag groups: {error}")))?
    {
        let id: String = row.get(0).map_err(read_group)?;
        if let Some(&at) = at.get(&id) {
            groups[at].tags.push(row.get(1).map_err(read_group)?);
        }
    }
    Ok(groups)
}

fn read_group(error: rusqlite::Error) -> SourceError {
    SourceError::Storage(format!("read the tag groups: {error}"))
}

fn export_smart(conn: &Connection) -> Result<Vec<EagleSmart>> {
    let mut statement = conn
        .prepare(
            "SELECT smart_folder.id, parent_id, name, conditions, eagle_export.eagle_id \
             FROM smart_folder LEFT JOIN eagle_export ON eagle_export.kind = 'smart_folder' \
             AND eagle_export.native_id = smart_folder.id ORDER BY sort_key",
        )
        .map_err(|error| SourceError::Storage(format!("read the smart folders: {error}")))?;
    let mut rows = statement
        .query([])
        .map_err(|error| SourceError::Storage(format!("read the smart folders: {error}")))?;
    let mut smart = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| SourceError::Storage(format!("read the smart folders: {error}")))?
    {
        smart.push(EagleSmart {
            id: row.get(0).map_err(read_smart)?,
            parent: row.get(1).map_err(read_smart)?,
            name: row.get(2).map_err(read_smart)?,
            conditions: row.get(3).map_err(read_smart)?,
            eagle_id: row.get(4).map_err(read_smart)?,
        });
    }
    Ok(smart)
}

fn read_smart(error: rusqlite::Error) -> SourceError {
    SourceError::Storage(format!("read the smart folders: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use library_core::ExportSelection;

    fn fixture() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let root = crate::test_support::new_library(dir.path(), "Refs");
        let library = crate::test_support::open(&root);
        let parent = dir.path().join("incoming");
        std::fs::create_dir_all(&parent).unwrap();
        crate::test_support::add_png(&library, &parent, "one.png", 1);
        crate::test_support::add_png(&library, &parent, "two.png", 2);
        crate::test_support::create(&library, "Posters", None);
        (dir, library)
    }

    #[test]
    fn whole_library_loads_folders_top_down_with_items_and_groups() {
        let (_dir, library) = fixture();
        let plan = load_eagle_export(
            &library,
            &ExportSelection {
                whole: true,
                ..ExportSelection::default()
            },
            true,
        )
        .unwrap();
        assert!(!plan.items.is_empty());
        // Parents precede children.
        let at: HashMap<&str, usize> = plan
            .folders
            .iter()
            .enumerate()
            .map(|(at, folder)| (folder.id.as_str(), at))
            .collect();
        for folder in &plan.folders {
            if let Some(parent) = &folder.parent {
                assert!(at[parent.as_str()] < at[folder.id.as_str()]);
            }
        }
    }

    #[test]
    fn storing_eagle_ids_makes_a_second_export_skip() {
        let (_dir, library) = fixture();
        let selection = ExportSelection {
            whole: true,
            ..ExportSelection::default()
        };
        let plan = load_eagle_export(&library, &selection, true).unwrap();
        let folder_pairs: Vec<(String, String)> = plan
            .folders
            .iter()
            .map(|folder| (folder.id.clone(), format!("EAGLE-{}", folder.id)))
            .collect();
        let item_pairs: Vec<(String, String)> = plan
            .items
            .iter()
            .map(|item| (item.id.clone(), format!("EAGLE-{}", item.id)))
            .collect();
        store_eagle_ids(&library, &folder_pairs, &item_pairs, &[], &[]).unwrap();
        let again = load_eagle_export(&library, &selection, true).unwrap();
        for folder in &again.folders {
            assert_eq!(
                folder.eagle_id.as_deref(),
                Some(format!("EAGLE-{}", folder.id).as_str())
            );
        }
        for item in &again.items {
            assert_eq!(
                item.eagle_id.as_deref(),
                Some(format!("EAGLE-{}", item.id).as_str())
            );
        }
    }

    #[test]
    fn a_selected_folder_under_an_unselected_parent_still_comes_before_its_children() {
        let (_dir, library) = fixture();
        let root = crate::test_support::create(&library, "Root", None);
        // The child is older than its parent: it was made first and moved in later, so id
        // order would put it first.
        let child = crate::test_support::create(&library, "Child", None);
        let parent = crate::test_support::create(&library, "Parent", Some(&root));
        let second = crate::test_support::create(&library, "Second", Some(&parent));
        library
            .save_folder(&library_core::FolderChange::Move {
                id: child.clone(),
                parent: Some(parent.clone()),
                index: 0,
            })
            .unwrap();
        let plan = load_eagle_export(
            &library,
            &ExportSelection {
                whole: false,
                folders: vec![parent.clone()],
                tags: Vec::new(),
            },
            true,
        )
        .unwrap();
        let ids: Vec<&str> = plan
            .folders
            .iter()
            .map(|folder| folder.id.as_str())
            .collect();
        assert_eq!(ids, [parent.as_str(), child.as_str(), second.as_str()]);
    }

    #[test]
    fn the_export_ids_never_touch_where_an_import_came_from() {
        let (_dir, library) = fixture();
        let item: String = library
            .conn()
            .query_row("SELECT id FROM item LIMIT 1", [], |row| row.get(0))
            .unwrap();
        library
            .conn()
            .execute(
                "UPDATE item SET eagle_id = 'FROM-THE-EAGLE-LIBRARY' WHERE id = ?1",
                params![item],
            )
            .unwrap();
        let selection = ExportSelection {
            whole: true,
            ..ExportSelection::default()
        };
        let plan = load_eagle_export(&library, &selection, true).unwrap();
        let planned = plan
            .items
            .iter()
            .find(|planned| planned.id == item)
            .unwrap();
        assert_eq!(planned.eagle_id, None, "an import's origin is no export id");
        store_eagle_ids(
            &library,
            &[],
            &[(item.clone(), "IN-THE-RUNNING-EAGLE".into())],
            &[],
            &[],
        )
        .unwrap();
        let origin: String = library
            .conn()
            .query_row(
                "SELECT eagle_id FROM item WHERE id = ?1",
                params![item],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(origin, "FROM-THE-EAGLE-LIBRARY");
        let again = load_eagle_export(&library, &selection, true).unwrap();
        let planned = again
            .items
            .iter()
            .find(|planned| planned.id == item)
            .unwrap();
        assert_eq!(planned.eagle_id.as_deref(), Some("IN-THE-RUNNING-EAGLE"));
    }

    #[test]
    fn subfolders_of_a_locked_folder_stay_home() {
        let (_dir, library) = fixture();
        let private = crate::test_support::create(&library, "Private", None);
        let inner = crate::test_support::create(&library, "Inner", Some(&private));
        library
            .lock_folder(&private, "correct horse".into(), None, None, &|_, _| {})
            .unwrap();
        let plan = load_eagle_export(
            &library,
            &ExportSelection {
                whole: true,
                ..ExportSelection::default()
            },
            true,
        )
        .unwrap();
        assert!(plan.folders.iter().any(|folder| folder.id == private));
        assert!(plan.folders.iter().all(|folder| folder.id != inner));
        library
            .unlock_folder(&private, "correct horse".into())
            .unwrap();
        let open = load_eagle_export(
            &library,
            &ExportSelection {
                whole: true,
                ..ExportSelection::default()
            },
            true,
        )
        .unwrap();
        assert!(open.folders.iter().any(|folder| folder.id == inner));
    }

    #[test]
    fn a_folder_selection_takes_the_subtree_only() {
        let (_dir, library) = fixture();
        let all = load_eagle_export(
            &library,
            &ExportSelection {
                whole: true,
                ..ExportSelection::default()
            },
            true,
        )
        .unwrap();
        let Some(first) = all.folders.first().cloned() else {
            return;
        };
        let plan = load_eagle_export(
            &library,
            &ExportSelection {
                whole: false,
                folders: vec![first.id.clone()],
                tags: Vec::new(),
            },
            true,
        )
        .unwrap();
        assert!(plan.folders.iter().any(|folder| folder.id == first.id));
        assert!(plan.folders.len() <= all.folders.len());
    }
}
