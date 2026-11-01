//! Applying an `ItemEdit` to the freshest copy of an item and building the complete
//! `item/update` body (docs/architecture.md §7.1): Eagle replaces `tags` and `folders`
//! wholesale, so the body always carries the full arrays.

use library_core::ItemEdit;
use serde_json::{Map, Value};

use crate::dto::ItemDto;
use crate::error::EagleError;

/// The `item/update` body for `edit` applied to `item` (which is re-read right before).
pub(crate) fn update_body(
    item: &ItemDto,
    edit: &ItemEdit,
) -> Result<Map<String, Value>, EagleError> {
    edit.validate()
        .map_err(|error| EagleError::Invalid(error.to_string()))?;
    let mut body = Map::from_iter([("id".to_owned(), Value::from(item.id.clone()))]);
    match edit {
        ItemEdit::AddTags(tags) => {
            let own = item.tags.clone();
            let merged: Vec<String> = own
                .iter()
                .cloned()
                .chain(tags.iter().filter(|tag| !own.contains(tag)).cloned())
                .collect();
            body.insert("tags".into(), Value::from(merged));
        }
        ItemEdit::RemoveTags(tags) => {
            body.insert(
                "tags".into(),
                Value::from(
                    item.tags
                        .iter()
                        .filter(|tag| !tags.contains(tag))
                        .cloned()
                        .collect::<Vec<_>>(),
                ),
            );
        }
        ItemEdit::AddToFolders(folders) => {
            let own = item.folders.clone().unwrap_or_default();
            let merged: Vec<String> = own
                .iter()
                .cloned()
                .chain(folders.iter().filter(|id| !own.contains(id)).cloned())
                .collect();
            body.insert("folders".into(), Value::from(merged));
        }
        ItemEdit::RemoveFromFolders(folders) => {
            body.insert(
                "folders".into(),
                Value::from(
                    item.folders
                        .clone()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|id| !folders.contains(id))
                        .collect::<Vec<_>>(),
                ),
            );
        }
        ItemEdit::SetFolders(folders) => {
            body.insert("folders".into(), Value::from(folders.clone()));
        }
        ItemEdit::SetStar(star) => {
            body.insert("star".into(), Value::from(*star));
        }
        ItemEdit::SetAnnotation(annotation) => {
            body.insert("annotation".into(), Value::from(annotation.clone()));
        }
        ItemEdit::SetUrl(url) => {
            body.insert("url".into(), Value::from(url.clone()));
        }
        ItemEdit::Rename(name) => {
            body.insert("name".into(), Value::from(name.clone()));
        }
        ItemEdit::Trash => {
            body.insert("isDeleted".into(), Value::from(true));
        }
        // Eagle's API cannot bring an item back from the trash (docs/spec.md §8.3 "Writes").
        ItemEdit::Restore => {
            return Err(EagleError::Invalid(
                "Eagle's API cannot restore from the trash".into(),
            ));
        }
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item() -> ItemDto {
        serde_json::from_value(json!({
            "id": "ABC123",
            "name": "one",
            "ext": "jpg",
            "tags": ["a", "b"],
            "folders": ["F1"],
            "star": 3
        }))
        .unwrap()
    }

    #[test]
    fn add_tags_keeps_the_existing_ones() {
        let body = update_body(&item(), &ItemEdit::AddTags(vec!["b".into(), "c".into()])).unwrap();
        assert_eq!(body["id"], json!("ABC123"));
        assert_eq!(body["tags"], json!(["a", "b", "c"]));
        assert!(body.get("folders").is_none());
    }

    #[test]
    fn remove_tags_leaves_the_rest() {
        let body = update_body(&item(), &ItemEdit::RemoveTags(vec!["a".into()])).unwrap();
        assert_eq!(body["tags"], json!(["b"]));
    }

    #[test]
    fn folders_merge_and_replace() {
        let body = update_body(&item(), &ItemEdit::AddToFolders(vec!["F2".into()])).unwrap();
        assert_eq!(body["folders"], json!(["F1", "F2"]));
        let body = update_body(&item(), &ItemEdit::SetFolders(vec!["F9".into()])).unwrap();
        assert_eq!(body["folders"], json!(["F9"]));
    }

    #[test]
    fn folders_missing_and_empty_differ() {
        let mut no_folders = item();
        no_folders.folders = None;
        let body = update_body(&no_folders, &ItemEdit::AddToFolders(vec!["F1".into()])).unwrap();
        assert_eq!(body["folders"], json!(["F1"]));
        let body =
            update_body(&no_folders, &ItemEdit::RemoveFromFolders(vec!["F1".into()])).unwrap();
        assert_eq!(body["folders"], json!([]));
    }

    #[test]
    fn field_edits_map_to_eagle_keys() {
        let body = update_body(&item(), &ItemEdit::SetStar(5)).unwrap();
        assert_eq!(body["star"], json!(5));
        let body = update_body(&item(), &ItemEdit::SetAnnotation("note".into())).unwrap();
        assert_eq!(body["annotation"], json!("note"));
        let body = update_body(&item(), &ItemEdit::SetUrl("https://x.example".into())).unwrap();
        assert_eq!(body["url"], json!("https://x.example"));
        let body = update_body(&item(), &ItemEdit::Rename("two".into())).unwrap();
        assert_eq!(body["name"], json!("two"));
        let body = update_body(&item(), &ItemEdit::Trash).unwrap();
        assert_eq!(body["isDeleted"], json!(true));
    }

    #[test]
    fn restore_is_not_possible() {
        assert!(update_body(&item(), &ItemEdit::Restore).is_err());
    }

    #[test]
    fn invalid_edits_are_rejected_before_any_request() {
        assert!(update_body(&item(), &ItemEdit::SetStar(6)).is_err());
        assert!(update_body(&item(), &ItemEdit::Rename("  ".into())).is_err());
    }
}
