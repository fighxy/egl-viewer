//! The filtering steps of the local pipeline (docs/architecture.md §6.4, docs/spec.md §5).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::model::{Folder, ItemId, ItemLite};
use crate::query::{LibraryQuery, Scope, TagMode};

/// The folder tree as parent → children, for subfolder unions and the lock filter.
#[derive(Debug, Clone, Default)]
pub struct FolderIndex {
    children: HashMap<String, Vec<String>>,
    /// Locked folders that are not unlocked in this session.
    locked: Vec<String>,
}

impl FolderIndex {
    pub fn new(tree: &[Folder]) -> Self {
        let mut index = Self::default();
        let mut stack: Vec<&Folder> = tree.iter().collect();
        while let Some(folder) = stack.pop() {
            index.children.insert(
                folder.id.clone(),
                folder
                    .children
                    .iter()
                    .map(|child| child.id.clone())
                    .collect(),
            );
            if folder.is_locked && !folder.is_unlocked {
                index.locked.push(folder.id.clone());
            }
            stack.extend(&folder.children);
        }
        index
    }

    /// `id` and every folder under it.
    pub fn descendant_ids(&self, id: &str) -> HashSet<String> {
        let mut found = HashSet::new();
        let mut stack = vec![id.to_owned()];
        while let Some(next) = stack.pop() {
            if let Some(children) = self.children.get(&next) {
                stack.extend(children.iter().cloned());
            }
            found.insert(next);
        }
        found
    }

    /// Locked folders that are not unlocked, with everything under them: their items are hidden
    /// in every scope. Unlocking one folder unlocks no other.
    pub fn hidden_ids(&self) -> HashSet<String> {
        self.locked
            .iter()
            .flat_map(|id| self.descendant_ids(id))
            .collect()
    }
}

/// What the pipeline needs besides the snapshot and the query.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    pub folders: &'a FolderIndex,
    /// Ids that match the search text; `None` when the query searches nothing.
    pub text_ids: Option<&'a HashSet<ItemId>>,
}

/// The positions in `items` that the query keeps, in snapshot order: lock filter, scope union,
/// tags, extra folders, type, rating, shape, search text.
pub fn filter(items: &[ItemLite], query: &LibraryQuery, context: Context<'_>) -> Vec<u32> {
    let hidden = context.folders.hidden_ids();
    let scope_folders = match &query.scope {
        Scope::Folder {
            id,
            include_descendants: true,
        } => Some(context.folders.descendant_ids(id)),
        Scope::Folder { id, .. } => Some(HashSet::from([id.clone()])),
        _ => None,
    };
    let in_any = |item: &ItemLite, set: &HashSet<String>| {
        folders_of(item).iter().any(|folder| set.contains(folder))
    };
    let keep = |item: &ItemLite| {
        if !hidden.is_empty() && in_any(item, &hidden) {
            return false;
        }
        let in_scope = match &query.scope {
            Scope::Folder { .. } => scope_folders.as_ref().is_some_and(|set| in_any(item, set)),
            Scope::Unfiled => folders_of(item).is_empty(),
            Scope::Untagged => item.tags.is_empty(),
            Scope::All | Scope::Smart { .. } | Scope::Trash => true,
        };
        in_scope
            && (query.tags.is_empty() || matches_tags(&item.tags, &query.tags, query.tag_mode))
            && (query.extra_folder_ids.is_empty()
                || matches_values(folders_of(item), &query.extra_folder_ids, query.folder_mode))
            && query
                .ext
                .as_deref()
                .is_none_or(|ext| type_matches(&item.ext, ext))
            && query.rating.is_none_or(|star| item.star == star)
            && query
                .shape
                .as_deref()
                .is_none_or(|shape| shape_of(item.width, item.height) == Some(shape))
            && context.text_ids.is_none_or(|ids| ids.contains(&item.id))
    };
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| keep(item))
        .map(|(index, _)| u32::try_from(index).unwrap_or(u32::MAX))
        .collect()
}

fn folders_of(item: &ItemLite) -> &[String] {
    item.folders.as_deref().unwrap_or_default()
}

/// Tag chips (docs/spec.md §5). Strings compare exactly: `"Poster"` is not `"poster"`. An empty
/// selection is no filter at all, whatever the mode.
pub fn matches_tags(item_tags: &[Arc<str>], selected: &[String], mode: TagMode) -> bool {
    let has = |tag: &String| item_tags.iter().any(|own| **own == **tag);
    match mode {
        TagMode::Any => selected.is_empty() || selected.iter().any(has),
        TagMode::All => selected.iter().all(has),
        TagMode::Exact => item_tags.len() == selected.len() && selected.iter().all(has),
        TagMode::None => !selected.iter().any(has),
    }
}

/// Folder chips: the same four modes over folder ids.
fn matches_values(own: &[String], selected: &[String], mode: TagMode) -> bool {
    let has = |value: &String| own.contains(value);
    match mode {
        TagMode::Any => selected.is_empty() || selected.iter().any(has),
        TagMode::All => selected.iter().all(has),
        TagMode::Exact => own.len() == selected.len() && selected.iter().all(has),
        TagMode::None => !selected.iter().any(has),
    }
}

/// Type buckets (docs/spec.md §5, docs/architecture.md §16.6); anything else is an extension.
const BUCKETS: &[(&str, &[&str])] = &[
    (
        "image",
        &[
            "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "tif", "tiff", "bmp", "psd",
        ],
    ),
    (
        "video",
        &[
            "ts", "3gp", "360", "afx", "vap", "eva", "mp4", "mov", "m4v", "webm", "mkv", "avi",
            "wmv", "mpg", "mts", "flv", "m2ts", "f4v",
        ],
    ),
    ("audio", &["mp3", "wav", "flac", "ogg", "aac", "m4a"]),
    ("font", &["ttf", "otf", "ttc", "woff", "woff2"]),
];

/// `filter` is a bucket (`image`, `video`, …) or an extension; extensions ignore ASCII case.
pub fn type_matches(ext: &str, filter: &str) -> bool {
    let bucket = if filter == "videos" { "video" } else { filter };
    match BUCKETS.iter().find(|(name, _)| *name == bucket) {
        Some((_, extensions)) => extensions
            .iter()
            .any(|known| known.eq_ignore_ascii_case(ext)),
        None => ext.eq_ignore_ascii_case(filter),
    }
}

/// The shape of an image as Eagle names it (docs/architecture.md §16.6); none without dimensions.
pub fn shape_of(width: Option<u32>, height: Option<u32>) -> Option<&'static str> {
    let (width, height) = (u64::from(width?), u64::from(height?));
    if width == 0 || height == 0 {
        return None;
    }
    Some(if width == height {
        "square"
    } else if width * 2 >= height * 5 {
        "panoramic-landscape"
    } else if height * 2 >= width * 5 {
        "panoramic-portrait"
    } else if width > height {
        "landscape"
    } else {
        "portrait"
    })
}
