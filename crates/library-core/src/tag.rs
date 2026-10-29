//! Tag groups and the tags of a selection (docs/spec.md §3 Tags, §7.3).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::edit::MAX_TEXT_UTF16;
use crate::error::SourceError;
use crate::folder::FOLDER_COLORS;
use crate::model::Tag;
use crate::query::view::ItemCell;

/// A tag group edit: a named, coloured set of tag names. Editing a group never changes the
/// tags of items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum TagGroupChange {
    /// A new group after the last one.
    Create {
        name: String,
        color: Option<String>,
        tags: Vec<String>,
    },
    /// The group's full desired state: the tags replace the old set.
    Update {
        id: String,
        name: String,
        color: Option<String>,
        tags: Vec<String>,
    },
}

impl TagGroupChange {
    /// Checks the edit before anything is written.
    pub fn validate(&self) -> Result<(), SourceError> {
        let (Self::Create { name, color, tags }
        | Self::Update {
            name, color, tags, ..
        }) = self;
        if name.trim().is_empty() {
            return Err(SourceError::Invalid("A tag group needs a name.".into()));
        }
        if name.encode_utf16().count() > MAX_TEXT_UTF16 {
            return Err(SourceError::Invalid("The name is too long.".into()));
        }
        if let Some(color) = color
            && !FOLDER_COLORS.contains(&color.as_str())
        {
            return Err(SourceError::Invalid(format!("Unknown colour: {color}.")));
        }
        validate_tag_names(tags)
    }
}

/// Tags are exact strings: none may be empty or longer than the text limit.
pub fn validate_tag_names(tags: &[String]) -> Result<(), SourceError> {
    if tags.iter().any(String::is_empty) {
        return Err(SourceError::Invalid("A tag cannot be empty.".into()));
    }
    if tags
        .iter()
        .any(|tag| tag.encode_utf16().count() > MAX_TEXT_UTF16)
    {
        return Err(SourceError::Invalid("A tag is too long.".into()));
    }
    Ok(())
}

/// The tags of a selection: how many items it has, and per tag on how many of them it is
/// (the right bar dims a tag that only some items carry).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SelectionTags {
    pub items: u32,
    /// By name; `image_count` is the number of selected items with the tag.
    pub tags: Vec<Tag>,
    /// Bytes of the selected items, for the right bar.
    pub size: u64,
    /// The first selected items in view order, for the right bar's stack of thumbnails.
    pub stack: Vec<ItemCell>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_changes_use_camel_case_tags() {
        let change = TagGroupChange::Update {
            id: "G1".into(),
            name: "Type".into(),
            color: Some("blue".into()),
            tags: vec!["Serif".into(), "sans".into()],
        };
        assert_eq!(
            serde_json::to_value(&change).unwrap(),
            serde_json::json!({
                "type": "update", "id": "G1", "name": "Type", "color": "blue",
                "tags": ["Serif", "sans"]
            })
        );
    }

    #[test]
    fn groups_need_a_name_a_known_colour_and_real_tags() {
        let ok = TagGroupChange::Create {
            name: "Type".into(),
            color: None,
            tags: vec!["Serif".into()],
        };
        assert!(ok.validate().is_ok());
        for bad in [
            TagGroupChange::Create {
                name: "  ".into(),
                color: None,
                tags: vec![],
            },
            TagGroupChange::Create {
                name: "Type".into(),
                color: Some("teal".into()),
                tags: vec![],
            },
            TagGroupChange::Create {
                name: "Type".into(),
                color: None,
                tags: vec![String::new()],
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
    }
}
