//! Folder edits and the order of siblings (docs/spec.md §3, docs/architecture.md §7.2, §7.3).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::edit::MAX_TEXT_UTF16;
use crate::error::SourceError;
use crate::query::SortKey;

/// The folder colours of Eagle; `None` is no colour.
pub const FOLDER_COLORS: [&str; 8] = [
    "red", "orange", "yellow", "green", "aqua", "blue", "purple", "pink",
];

/// A folder edit. `Update` carries the folder's full desired properties: Eagle moves a nested
/// folder to the root and clears its colour when fields are missing (corrections 3, 31).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum FolderChange {
    /// A new folder after the last child of `parent` (`None`: at the root).
    Create {
        name: String,
        parent: Option<String>,
        description: String,
        icon_color: Option<String>,
    },
    /// The folder's own properties. Folder tags are labels on the folder, not item tags.
    Update {
        id: String,
        name: String,
        description: String,
        tags: Vec<String>,
        icon_color: Option<String>,
        /// The sort the folder opens with; `None` follows the global sort.
        order_by: Option<SortKey>,
        sort_ascending: Option<bool>,
    },
    /// Under `parent` (`None`: the root), at `index` among its other children. Refused before
    /// any write when `parent` is the folder itself or one of its descendants.
    Move {
        id: String,
        parent: Option<String>,
        index: u32,
    },
}

impl FolderChange {
    /// Checks the edit before anything is written.
    pub fn validate(&self) -> Result<(), SourceError> {
        match self {
            Self::Create {
                name,
                description,
                icon_color,
                ..
            } => {
                check_name(name)?;
                check_text(description)?;
                check_color(icon_color.as_deref())
            }
            Self::Update {
                name,
                description,
                tags,
                icon_color,
                ..
            } => {
                check_name(name)?;
                check_text(description)?;
                check_color(icon_color.as_deref())?;
                if tags.iter().any(|tag| tag.is_empty()) {
                    return invalid("tags must not be empty strings");
                }
                Ok(())
            }
            Self::Move { id, parent, .. } => {
                if parent.as_deref() == Some(id.as_str()) {
                    return invalid("a folder cannot be moved into itself");
                }
                Ok(())
            }
        }
    }
}

fn invalid(message: &str) -> Result<(), SourceError> {
    Err(SourceError::Invalid(message.to_owned()))
}

fn check_name(name: &str) -> Result<(), SourceError> {
    if name.trim().is_empty() {
        return invalid("the folder needs a name");
    }
    check_text(name)
}

fn check_text(text: &str) -> Result<(), SourceError> {
    if text.encode_utf16().count() > MAX_TEXT_UTF16 {
        return invalid("text is longer than 10000 characters");
    }
    Ok(())
}

fn check_color(color: Option<&str>) -> Result<(), SourceError> {
    match color {
        Some(color) if !FOLDER_COLORS.contains(&color) => invalid("unknown folder colour"),
        _ => Ok(()),
    }
}

/// Digits of the order keys, in ASCII (and SQLite `BINARY`) order.
const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// An order key between `before` and `after` (fractional indexing): moving a folder among its
/// siblings changes only its own key. `None` stands for the start or the end. Keys are base-62
/// fractions without trailing zeros; the result is always strictly between its neighbours.
pub fn order_key_between(before: Option<&str>, after: Option<&str>) -> String {
    let lower = before.unwrap_or("").as_bytes();
    let upper = after.map(str::as_bytes);
    if let Some(upper) = upper {
        debug_assert!(lower < upper, "order keys out of order");
    }
    String::from_utf8(midpoint(lower, upper)).expect("order keys are ASCII")
}

fn digit(byte: u8) -> usize {
    DIGITS.iter().position(|&d| d == byte).unwrap_or(0)
}

/// The midpoint of two fractions `0.lower` and `0.upper` (`None`: 1).
fn midpoint(lower: &[u8], upper: Option<&[u8]>) -> Vec<u8> {
    if let Some(upper) = upper {
        // Keep the common prefix, with `lower` padded by zeros.
        let common = upper
            .iter()
            .enumerate()
            .take_while(|(i, byte)| lower.get(*i).copied().unwrap_or(DIGITS[0]) == **byte)
            .count();
        if common > 0 {
            let mut key = upper[..common].to_vec();
            key.extend(midpoint(
                lower.get(common..).unwrap_or_default(),
                Some(&upper[common..]),
            ));
            return key;
        }
    }
    let low = lower.first().map_or(0, |&byte| digit(byte));
    let high = upper
        .and_then(|upper| upper.first())
        .map_or(DIGITS.len(), |&byte| digit(byte));
    if high - low > 1 {
        return vec![DIGITS[(low + high).div_ceil(2)]];
    }
    // Adjacent digits: take `upper`'s first digit when it has more, else go one level deeper.
    match upper {
        Some(upper) if upper.len() > 1 => vec![upper[0]],
        _ => {
            let mut key = vec![DIGITS[low]];
            key.extend(midpoint(lower.get(1..).unwrap_or_default(), None));
            key
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn folder_change_is_tagged_and_camel_case() {
        let change = FolderChange::Update {
            id: "F1".into(),
            name: "Posters".into(),
            description: String::new(),
            tags: vec![],
            icon_color: Some("blue".into()),
            order_by: Some(SortKey::Title),
            sort_ascending: Some(true),
        };
        assert_eq!(
            serde_json::to_value(&change).unwrap(),
            json!({
                "type": "update",
                "id": "F1",
                "name": "Posters",
                "description": "",
                "tags": [],
                "iconColor": "blue",
                "orderBy": "name",
                "sortAscending": true
            })
        );
        let moved: FolderChange = serde_json::from_value(
            json!({ "type": "move", "id": "F1", "parent": null, "index": 2 }),
        )
        .unwrap();
        assert_eq!(
            moved,
            FolderChange::Move {
                id: "F1".into(),
                parent: None,
                index: 2
            }
        );
    }

    #[test]
    fn edits_are_checked_before_writing() {
        let create = |name: &str, color: Option<&str>| FolderChange::Create {
            name: name.into(),
            parent: None,
            description: String::new(),
            icon_color: color.map(Into::into),
        };
        assert_eq!(create("Posters", Some("aqua")).validate(), Ok(()));
        assert!(create("  ", None).validate().is_err());
        assert!(create("Posters", Some("teal")).validate().is_err());
        assert!(create(&"x".repeat(10_001), None).validate().is_err());
        let into_itself = FolderChange::Move {
            id: "F1".into(),
            parent: Some("F1".into()),
            index: 0,
        };
        assert!(into_itself.validate().is_err());
    }

    #[test]
    fn order_keys_fall_strictly_between_their_neighbours() {
        let first = order_key_between(None, None);
        let after = order_key_between(Some(&first), None);
        let before = order_key_between(None, Some(&first));
        assert!(before < first && first < after, "{before} {first} {after}");

        // Inserting again and again into the same gap keeps working.
        let (mut low, high) = (first.clone(), after.clone());
        for _ in 0..200 {
            let mid = order_key_between(Some(&low), Some(&high));
            assert!(low < mid && mid < high, "{low} < {mid} < {high}");
            assert!(!mid.ends_with('0'));
            low = mid;
        }
        // And at the very start.
        let mut upper = before;
        for _ in 0..200 {
            let key = order_key_between(None, Some(&upper));
            assert!(key < upper && !key.is_empty());
            upper = key;
        }
    }
}
