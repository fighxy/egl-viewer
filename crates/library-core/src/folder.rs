//! Folder edits and the order of siblings (docs/spec.md §3, docs/architecture.md §7.2, §7.3).

use serde::{Deserialize, Serialize};

use crate::edit::MAX_TEXT_UTF16;
use crate::error::SourceError;
use crate::query::SortKey;

/// The folder colours of Eagle; `None` is no colour.
pub const FOLDER_COLORS: [&str; 8] = [
    "red", "orange", "yellow", "green", "aqua", "blue", "purple", "pink",
];

/// The RGB the apps draw for each of [`FOLDER_COLORS`], in the same order.
const FOLDER_COLOR_RGB: [u32; 8] = [
    0xef4444, 0xf97316, 0xeab308, 0x22c55e, 0x06b6d4, 0x3b82f6, 0xa855f7, 0xec4899,
];

/// A colour a folder or a tag group may carry: one of [`FOLDER_COLORS`], or any colour as
/// `#rrggbb` in lowercase hex (D-159).
pub fn is_valid_color(color: &str) -> bool {
    FOLDER_COLORS.contains(&color) || parse_hex(color).is_some()
}

/// The colour as `0xrrggbb`: a named colour's value, or the hex itself.
pub fn color_rgb(color: &str) -> Option<u32> {
    match FOLDER_COLORS.iter().position(|name| *name == color) {
        Some(index) => Some(FOLDER_COLOR_RGB[index]),
        None => parse_hex(color),
    }
}

/// `0xrrggbb` as the stored form `#rrggbb`.
pub fn hex_color(rgb: u32) -> String {
    format!("#{:06x}", rgb & 0xff_ffff)
}

/// What Eagle gets for a colour, which knows only [`FOLDER_COLORS`]: a named colour as it
/// is, a hex the nearest named colour (D-159).
pub fn eagle_color(color: &str) -> Option<&'static str> {
    if let Some(name) = FOLDER_COLORS.iter().find(|name| **name == color) {
        return Some(name);
    }
    let rgb = parse_hex(color)?;
    FOLDER_COLORS
        .iter()
        .zip(FOLDER_COLOR_RGB)
        .min_by_key(|(_, named)| color_distance(rgb, *named))
        .map(|(name, _)| *name)
}

fn parse_hex(color: &str) -> Option<u32> {
    let digits = color.strip_prefix('#')?;
    if digits.len() != 6
        || !digits
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

/// The "redmean" weighted RGB distance (squared): closer to what the eye sees than plain RGB.
pub(crate) fn color_distance(a: u32, b: u32) -> i64 {
    let channel = |rgb: u32, shift: u32| i64::from((rgb >> shift) & 0xff);
    let (r1, g1, b1) = (channel(a, 16), channel(a, 8), channel(a, 0));
    let (r2, g2, b2) = (channel(b, 16), channel(b, 8), channel(b, 0));
    let mean = (r1 + r2) / 2;
    let (dr, dg, db) = (r1 - r2, g1 - g2, b1 - b2);
    (((512 + mean) * dr * dr) >> 8) + 4 * dg * dg + (((767 - mean) * db * db) >> 8)
}

/// A folder edit. `Update` carries the folder's full desired properties: Eagle moves a nested
/// folder to the root and clears its colour when fields are missing (corrections 3, 31).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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
        Some(color) if !is_valid_color(color) => invalid("unknown folder colour"),
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
        assert_eq!(create("Posters", Some("#1a2b3c")).validate(), Ok(()));
        for bad in ["#1A2B3C", "#1a2b3", "#1a2b3c4d", "1a2b3c", "#gggggg"] {
            assert!(create("Posters", Some(bad)).validate().is_err(), "{bad}");
        }
        assert!(create(&"x".repeat(10_001), None).validate().is_err());
        let into_itself = FolderChange::Move {
            id: "F1".into(),
            parent: Some("F1".into()),
            index: 0,
        };
        assert!(into_itself.validate().is_err());
    }

    #[test]
    fn colours_are_named_or_hex_and_eagle_gets_the_nearest_name() {
        assert_eq!(color_rgb("aqua"), Some(0x06b6d4));
        assert_eq!(color_rgb("#00ff80"), Some(0x00ff80));
        assert_eq!(color_rgb("teal"), None);
        assert_eq!(hex_color(0x00ff80), "#00ff80");
        assert_eq!(color_rgb(&hex_color(0x0a0b0c)), Some(0x0a0b0c));
        // Every named colour is its own nearest, as a name and as its hex.
        for name in FOLDER_COLORS {
            assert_eq!(eagle_color(name), Some(name));
            assert_eq!(
                eagle_color(&hex_color(color_rgb(name).unwrap())),
                Some(name)
            );
        }
        assert_eq!(eagle_color("#b91c1c"), Some("red"));
        assert_eq!(eagle_color("#1e3a8a"), Some("blue"));
        assert_eq!(eagle_color("#16a34a"), Some("green"));
        assert_eq!(eagle_color("#7c3aed"), Some("purple"));
        assert_eq!(eagle_color("teal"), None);
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
