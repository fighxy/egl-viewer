//! Reading a library for an export into a running Eagle and writing the Eagle ids back
//! (docs/spec.md §8.4, docs/architecture.md §15.7). The `eagle_id` columns exist since
//! format version 1, so storing the mapping needs no format change.

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
    /// Eagle id from a previous export into Eagle (docs/spec.md §8.4).
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
    let items: Vec<EagleExportItem> = load_export_items(library, &sql)?
        .into_iter()
        .map(|item| EagleExportItem {
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
            eagle_id: item.eagle_id,
            added_at: item.added_at,
        })
        .collect();
    let conn = library.conn();
    let folders = export_folders(&conn, selection)?;
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
        eagle_id: None,
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
    for (table, pairs) in [
        ("folder", folders),
        ("item", items),
        ("tag_group", groups),
        ("smart_folder", smart),
    ] {
        let mut statement = tx
            .prepare(format!("UPDATE {table} SET eagle_id = ?2 WHERE id = ?1").as_str())
            .map_err(|error| SourceError::Storage(format!("store the Eagle ids: {error}")))?;
        for (native, eagle) in pairs {
            statement
                .execute(params![native, eagle])
                .map_err(|error| SourceError::Storage(format!("store the Eagle ids: {error}")))?;
        }
    }
    tx.commit()
        .map_err(|error| SourceError::Storage(format!("store the Eagle ids: {error}")))?;
    Ok(())
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

/// The folders of the selection, parents before children. `whole` takes every folder;
/// otherwise the selected folders and everything under them.
fn export_folders(
    conn: &Connection,
    selection: &ExportSelection,
) -> Result<Vec<EagleExportFolder>> {
    let mut statement = conn
        .prepare(
            "SELECT id, parent_id, name, description, icon_color, order_by, sort_ascending, \
             eagle_id, locked FROM folder",
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
    let mut ordered = Vec::with_capacity(wanted.len());
    let mut stack: Vec<String> = roots.into_iter().rev().collect();
    while let Some(id) = stack.pop() {
        if !wanted.contains(&id) {
            continue;
        }
        if let Some(kids) = children.get(&Some(id.clone())) {
            stack.extend(kids.iter().rev().cloned());
        }
        if let Some(row) = by_id.remove(&id) {
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
    // Folders whose parents are not exported keep their relative subtree order; anything
    // missed above (a selected child under an unselected parent) appends in id order.
    let mut rest: Vec<EagleExportFolder> = by_id
        .into_values()
        .filter(|row| wanted.contains(&row.id))
        .map(|row| EagleExportFolder {
            id: row.id,
            parent: row.parent,
            name: row.name,
            description: row.description,
            icon_color: row.icon_color,
            order_by: row.order_by,
            sort_ascending: row.sort_ascending,
            eagle_id: row.eagle_id,
            locked: row.locked,
        })
        .collect();
    rest.sort_by(|a, b| a.id.cmp(&b.id));
    ordered.extend(rest);
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
        .prepare("SELECT id, name, color, eagle_id FROM tag_group ORDER BY sort_key")
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
            "SELECT id, parent_id, name, conditions, eagle_id FROM smart_folder ORDER BY sort_key",
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
