//! What one library screen shows (docs/spec.md §5, docs/architecture.md §6).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod filter;
pub mod sort;
pub mod view;

#[cfg(test)]
mod tests;

/// The part of a query that decides what to load. Everything else is computed locally.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Scope {
    #[default]
    All,
    Folder {
        id: String,
        include_descendants: bool,
    },
    Unfiled,
    Untagged,
    Smart {
        id: String,
    },
    /// Native library only: Eagle's API lists no trashed items.
    Trash,
}

/// Match modes for tag and folder chips (docs/spec.md §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TagMode {
    /// At least one selected value (the default, as in Eagle).
    #[default]
    Any,
    /// Every selected value.
    All,
    /// Every selected value and nothing else.
    Exact,
    /// None of the selected values.
    None,
}

/// Sort keys; the wire names follow docs/spec.md §5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SortKey {
    #[default]
    #[serde(rename = "added")]
    DateAdded,
    #[serde(rename = "btime")]
    DateCreated,
    #[serde(rename = "mtime")]
    DateModified,
    #[serde(rename = "name")]
    Title,
    #[serde(rename = "ext")]
    Extension,
    #[serde(rename = "size")]
    FileSize,
    #[serde(rename = "resolution")]
    Resolution,
    #[serde(rename = "star")]
    Rating,
    #[serde(rename = "duration")]
    Duration,
    #[serde(rename = "tag")]
    FirstTag,
    #[serde(rename = "random")]
    Random,
    /// The position in the open folder, then date added (Eagle: `order[folderId]`, correction 39).
    #[serde(rename = "manual")]
    Manual,
}

/// One library screen: scope, chips, search and sort.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct LibraryQuery {
    pub scope: Scope,
    pub tag_mode: TagMode,
    pub tags: Vec<String>,
    /// Folder chips beyond the open folder.
    pub extra_folder_ids: Vec<String>,
    pub folder_mode: TagMode,
    pub ext: Option<String>,
    /// `None` means no rating filter; `Some(0)` means unrated.
    pub rating: Option<u8>,
    pub shape: Option<String>,
    /// The top search field.
    pub text: Option<String>,
    pub sort: SortKey,
    pub ascending: bool,
}

impl Default for LibraryQuery {
    /// All items, newest first.
    fn default() -> Self {
        Self {
            scope: Scope::All,
            tag_mode: TagMode::Any,
            tags: Vec::new(),
            extra_folder_ids: Vec::new(),
            folder_mode: TagMode::Any,
            ext: None,
            rating: None,
            shape: None,
            text: None,
            sort: SortKey::DateAdded,
            ascending: false,
        }
    }
}

impl LibraryQuery {
    /// The chips a source may use to prefilter on its side (docs/architecture.md §15.2).
    pub fn hints(&self) -> ScopeHints {
        ScopeHints {
            tags: self.tags.clone(),
            tag_mode: self.tag_mode,
            ext: self.ext.clone(),
            rating: self.rating,
            shape: self.shape.clone(),
        }
    }
}

/// Chips passed to `LibrarySource::load_scope`. The Eagle source may turn them into safe server
/// prefilters on large libraries; other sources ignore them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScopeHints {
    pub tags: Vec<String>,
    pub tag_mode: TagMode,
    pub ext: Option<String>,
    pub rating: Option<u8>,
    pub shape: Option<String>,
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scope_is_tagged_by_kind_with_camel_case_fields() {
        let folder = Scope::Folder {
            id: "F1".into(),
            include_descendants: true,
        };
        assert_eq!(
            serde_json::to_value(&folder).unwrap(),
            json!({ "kind": "folder", "id": "F1", "includeDescendants": true })
        );
        assert_eq!(
            serde_json::to_value(Scope::All).unwrap(),
            json!({ "kind": "all" })
        );
        assert_eq!(
            serde_json::from_value::<Scope>(json!({ "kind": "smart", "id": "S1" })).unwrap(),
            Scope::Smart { id: "S1".into() }
        );
        assert_eq!(
            serde_json::to_value(Scope::Trash).unwrap(),
            json!({ "kind": "trash" })
        );
    }

    #[test]
    fn sort_keys_use_the_spec_names() {
        let names: Vec<_> = [
            SortKey::DateAdded,
            SortKey::DateCreated,
            SortKey::DateModified,
            SortKey::Title,
            SortKey::Extension,
            SortKey::FileSize,
            SortKey::Resolution,
            SortKey::Rating,
            SortKey::Duration,
            SortKey::FirstTag,
            SortKey::Random,
            SortKey::Manual,
        ]
        .iter()
        .map(|key| serde_json::to_value(key).unwrap())
        .collect();
        assert_eq!(
            names,
            [
                "added",
                "btime",
                "mtime",
                "name",
                "ext",
                "size",
                "resolution",
                "star",
                "duration",
                "tag",
                "random",
                "manual"
            ]
            .map(|n| json!(n))
        );
    }

    #[test]
    fn default_query_is_everything_newest_first() {
        let query = LibraryQuery::default();
        assert_eq!(query.scope, Scope::All);
        assert_eq!(query.sort, SortKey::DateAdded);
        assert!(!query.ascending);
        assert_eq!(query.tag_mode, TagMode::Any);
    }

    #[test]
    fn query_round_trips_and_fills_missing_fields() {
        let query: LibraryQuery = serde_json::from_value(json!({
            "scope": { "kind": "unfiled" },
            "tags": ["Poster"],
            "tagMode": "exact"
        }))
        .unwrap();
        assert_eq!(query.scope, Scope::Unfiled);
        assert_eq!(query.tag_mode, TagMode::Exact);
        assert_eq!(query.sort, SortKey::DateAdded);
        let back: LibraryQuery =
            serde_json::from_value(serde_json::to_value(&query).unwrap()).unwrap();
        assert_eq!(back, query);
    }

    #[test]
    fn hints_copy_the_chips() {
        let query = LibraryQuery {
            tags: vec!["a".into(), "b".into()],
            tag_mode: TagMode::All,
            ext: Some("jpg".into()),
            rating: Some(0),
            ..LibraryQuery::default()
        };
        let hints = query.hints();
        assert_eq!(hints.tags, ["a", "b"]);
        assert_eq!(hints.tag_mode, TagMode::All);
        assert_eq!(hints.ext.as_deref(), Some("jpg"));
        assert_eq!(hints.rating, Some(0));
    }
}
