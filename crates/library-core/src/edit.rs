//! Item edits as intents (docs/architecture.md §7.1).
//!
//! A source applies an intent to the freshest copy of the item, so `tags` and `folders` are
//! always written as complete arrays (Eagle replaces them wholesale).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::SourceError;

/// Eagle rejects longer names and notes with HTTP 500. JavaScript counts UTF-16 code units,
/// so the limit is checked in the same unit.
pub const MAX_TEXT_UTF16: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
#[ts(export)]
pub enum ItemEdit {
    AddTags(Vec<String>),
    RemoveTags(Vec<String>),
    AddToFolders(Vec<String>),
    RemoveFromFolders(Vec<String>),
    /// The folder checklist: the complete new set.
    SetFolders(Vec<String>),
    /// 0..=5; Eagle silently ignores other values, so they are rejected here.
    SetStar(u8),
    SetAnnotation(String),
    /// The source link: `http`, `https` or empty.
    SetUrl(String),
    /// Renames the stored file too.
    Rename(String),
    /// Moves the item to the trash.
    Trash,
    /// Brings the item back from the trash (native library only).
    Restore,
}

impl ItemEdit {
    /// Checks the intent before any request is sent.
    pub fn validate(&self) -> Result<(), SourceError> {
        match self {
            Self::SetStar(star) if *star > 5 => invalid("star must be between 0 and 5"),
            Self::Rename(name) if name.trim().is_empty() => invalid("name must not be empty"),
            Self::Rename(text) | Self::SetAnnotation(text) | Self::SetUrl(text)
                if text.encode_utf16().count() > MAX_TEXT_UTF16 =>
            {
                invalid("text is longer than 10000 characters")
            }
            Self::SetUrl(url) if !is_web_link_or_empty(url) => {
                invalid("the link must start with http:// or https://")
            }
            Self::AddTags(tags) | Self::RemoveTags(tags) if tags.iter().any(|t| t.is_empty()) => {
                invalid("tags must not be empty strings")
            }
            Self::AddToFolders(ids) | Self::RemoveFromFolders(ids) | Self::SetFolders(ids)
                if ids.iter().any(|id| id.is_empty()) =>
            {
                invalid("folder ids must not be empty")
            }
            _ => Ok(()),
        }
    }
}

fn invalid(message: &str) -> Result<(), SourceError> {
    Err(SourceError::Invalid(message.to_owned()))
}

/// Links are opened in the system browser, so only web links are stored.
fn is_web_link_or_empty(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.is_empty() || lower.starts_with("http://") || lower.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_ordinary_edits() {
        for edit in [
            ItemEdit::SetStar(0),
            ItemEdit::SetStar(5),
            ItemEdit::AddTags(vec!["Poster".into(), "тест".into()]),
            ItemEdit::SetFolders(vec![]),
            ItemEdit::Rename("IMG_1042".into()),
            ItemEdit::SetAnnotation(String::new()),
            ItemEdit::SetUrl(String::new()),
            ItemEdit::SetUrl("https://example.com/a.jpg".into()),
            ItemEdit::SetUrl("HTTP://example.com".into()),
            ItemEdit::Trash,
            ItemEdit::Restore,
        ] {
            assert_eq!(edit.validate(), Ok(()), "{edit:?}");
        }
    }

    #[test]
    fn rejects_links_that_are_not_web_links() {
        for url in ["file:///C:/x.jpg", "javascript:alert(1)", "example.com"] {
            assert!(
                matches!(
                    ItemEdit::SetUrl(url.into()).validate(),
                    Err(SourceError::Invalid(_))
                ),
                "{url}"
            );
        }
    }

    #[test]
    fn rejects_a_star_above_five() {
        assert!(matches!(
            ItemEdit::SetStar(6).validate(),
            Err(SourceError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_an_empty_name() {
        assert!(matches!(
            ItemEdit::Rename("  ".into()).validate(),
            Err(SourceError::Invalid(_))
        ));
    }

    #[test]
    fn counts_text_length_in_utf16_units_like_eagle() {
        let at_limit = "a".repeat(MAX_TEXT_UTF16);
        assert_eq!(ItemEdit::SetAnnotation(at_limit).validate(), Ok(()));

        // 5001 emoji are 10002 UTF-16 units but only 5001 chars.
        let emoji = "😀".repeat(5_001);
        assert!(matches!(
            ItemEdit::SetAnnotation(emoji).validate(),
            Err(SourceError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_empty_tags_and_folder_ids() {
        assert!(ItemEdit::AddTags(vec![String::new()]).validate().is_err());
        assert!(
            ItemEdit::SetFolders(vec![String::new()])
                .validate()
                .is_err()
        );
    }

    #[test]
    fn is_tagged_on_the_wire() {
        assert_eq!(
            serde_json::to_value(ItemEdit::SetStar(3)).unwrap(),
            json!({ "type": "setStar", "value": 3 })
        );
        assert_eq!(
            serde_json::to_value(ItemEdit::Trash).unwrap(),
            json!({ "type": "trash" })
        );
    }
}
