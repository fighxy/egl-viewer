//! Eagle's JSON parsed into DTOs and converted into the models of `library-core`
//! (docs/architecture.md §5). Passwords are dropped while parsing and never stored.

use std::sync::Arc;

use library_core::{
    Folder, Item, ItemId, ItemLite, LibraryInfo, LibraryKind, Palette, SmartFolder, SortKey, Tag,
    TagGroup,
};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub platform: String,
}

/// `GET /api/v2/library/info`: reads `metadata.json` from disk. `{}` means no library is open.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfoDto {
    /// Absent only when no library is open.
    pub folders: Option<Vec<FolderDto>>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    pub modification_time: Option<i64>,
}

impl LibraryInfoDto {
    /// Library identity for the session; `None` when Eagle has no library open.
    pub fn to_info(&self) -> Option<LibraryInfo> {
        self.folders.as_ref()?;
        Some(LibraryInfo {
            kind: LibraryKind::EagleApi,
            id: None,
            name: self.name.clone(),
            path: (!self.path.is_empty()).then(|| self.path.clone()),
            modification_time: self.modification_time,
            // Phase 9 is browse-only; phase 10 turns writes on (docs/spec.md §9, §10).
            read_only: true,
            read_only_reason: None,
            holder: None,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDto {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ext: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size: Option<u64>,
    /// Absent until a rating is set (spec §8.3).
    pub star: Option<u8>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Missing and `[]` are different cases (correction 15).
    pub folders: Option<Vec<String>>,
    #[serde(default)]
    pub palettes: Vec<PaletteDto>,
    pub modification_time: Option<i64>,
    pub mtime: Option<i64>,
    pub btime: Option<i64>,
    pub duration: Option<f64>,
    pub no_thumbnail: Option<bool>,
    pub is_deleted: Option<bool>,
    pub annotation: Option<String>,
    pub url: Option<String>,
    pub bpm: Option<f64>,
    pub raw_metas: Option<Value>,
}

impl ItemDto {
    pub fn to_lite(&self) -> ItemLite {
        ItemLite {
            id: ItemId(self.id.clone()),
            name: self.name.clone(),
            ext: self.ext.clone(),
            width: self.width,
            height: self.height,
            size: self.size,
            star: self.star.unwrap_or(0),
            tags: self
                .tags
                .iter()
                .map(|tag| Arc::from(tag.as_str()))
                .collect(),
            folders: self.folders.clone(),
            palette0: self.palettes.first().and_then(|palette| palette.color()),
            added_at: self.modification_time,
            file_created_at: self.btime.or(self.modification_time),
            file_modified_at: self.mtime.or(self.modification_time),
            deleted_at: None,
            duration: self.duration,
            no_thumbnail: self.no_thumbnail.unwrap_or(false),
            thumb_version: self.modification_time.unwrap_or(0),
            position: None,
        }
    }

    pub fn to_item(&self) -> Item {
        Item {
            id: ItemId(self.id.clone()),
            name: self.name.clone(),
            ext: self.ext.clone(),
            width: self.width,
            height: self.height,
            size: self.size,
            star: self.star.unwrap_or(0),
            tags: self.tags.clone(),
            folders: self.folders.clone().unwrap_or_default(),
            annotation: self.annotation.clone().unwrap_or_default(),
            url: self.url.clone().unwrap_or_default(),
            palettes: self
                .palettes
                .iter()
                .filter_map(|palette| {
                    palette.color().map(|color| Palette {
                        color,
                        ratio: palette.ratio,
                    })
                })
                .collect(),
            added_at: self.modification_time,
            file_created_at: self.btime.or(self.modification_time),
            file_modified_at: self.mtime.or(self.modification_time),
            duration: self.duration,
            bpm: self.bpm,
            exif: self.raw_metas.clone(),
            no_thumbnail: self.no_thumbnail.unwrap_or(false),
            thumb_version: self.modification_time.unwrap_or(0),
            is_deleted: self.is_deleted.unwrap_or(false),
            deleted_at: None,
        }
    }
}

/// `[{ "color": [r, g, b], "ratio": 47 }, …]` sorted by ratio descending; ratio can be
/// fractional. A colour with fewer than three components is ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct PaletteDto {
    #[serde(default)]
    pub color: Vec<u8>,
    #[serde(default)]
    pub ratio: f64,
}

impl PaletteDto {
    fn color(&self) -> Option<[u8; 3]> {
        match self.color.as_slice() {
            [r, g, b, ..] => Some([*r, *g, *b]),
            _ => None,
        }
    }
}

/// `password` becomes `is_locked` here and is never stored (docs/architecture.md §5).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub children: Vec<FolderDto>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub extend_tags: Vec<String>,
    pub icon_color: Option<String>,
    pub cover_id: Option<String>,
    pub order_by: Option<String>,
    pub sort_increase: Option<bool>,
    /// Absent right after `folder/create`: show "—", not 0 (correction 32).
    pub image_count: Option<u32>,
    pub descendant_image_count: Option<u32>,
    #[serde(rename = "isUnLock", default)]
    pub is_unlock: bool,
    #[serde(rename = "password", default, deserialize_with = "non_empty")]
    pub is_locked: bool,
    // passwordTips and the desktop's service keys (correction 33) are ignored.
}

impl FolderDto {
    pub fn to_folder(&self) -> Folder {
        Folder {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            icon_color: self.icon_color.clone(),
            tags: self.tags.clone(),
            extend_tags: self.extend_tags.clone(),
            children: self.children.iter().map(FolderDto::to_folder).collect(),
            image_count: self.image_count,
            descendant_image_count: self.descendant_image_count,
            is_locked: self.is_locked,
            is_unlocked: self.is_unlock,
            lock_label: None,
            lock_color: None,
            cover_item_id: self.cover_id.clone(),
            order_by: self.order_by(),
            sort_ascending: self.sort_increase,
        }
    }

    fn order_by(&self) -> Option<SortKey> {
        let value = Value::String(self.order_by.clone()?);
        serde_json::from_value(value).ok()
    }
}

/// A non-empty string becomes `true`; absent or empty stays `false`.
fn non_empty<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.is_some_and(|value| !value.is_empty()))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    #[serde(default)]
    pub name: String,
    pub image_count: Option<u32>,
}

impl TagDto {
    pub fn to_tag(&self) -> Tag {
        Tag {
            name: self.name.clone(),
            image_count: self.image_count,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagGroupDto {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub color: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl TagGroupDto {
    pub fn to_tag_group(&self) -> TagGroup {
        TagGroup {
            id: self.id.clone(),
            name: self.name.clone(),
            color: self.color.clone(),
            tags: self.tags.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartFolderDto {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub children: Vec<SmartFolderDto>,
    /// Eagle's rule language, kept unchanged (docs/architecture.md §16).
    #[serde(default)]
    pub conditions: Value,
    pub order_by: Option<String>,
    pub sort_increase: Option<bool>,
}

impl SmartFolderDto {
    pub fn to_smart_folder(&self) -> SmartFolder {
        SmartFolder {
            id: self.id.clone(),
            name: self.name.clone(),
            children: self
                .children
                .iter()
                .map(SmartFolderDto::to_smart_folder)
                .collect(),
            conditions: self.conditions.clone(),
            order_by: self.order_by.clone(),
            sort_ascending: self.sort_increase,
            unsupported: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn item_maps_onto_lite_and_detail() {
        let item: ItemDto = serde_json::from_value(json!({
            "id": "MVQTEQPE8FAQM",
            "name": "eagle-api-test-1",
            "ext": "jpg",
            "width": 640,
            "height": 400,
            "size": 16228,
            "tags": ["eagle-api-test", "тест-апи"],
            "folders": [],
            "palettes": [{ "color": [142, 83, 55], "ratio": 47 }],
            "modificationTime": 1793113379762u64,
            "mtime": 1793113379764u64,
            "btime": 1793113379764u64,
            "noThumbnail": true
        }))
        .unwrap();
        let lite = item.to_lite();
        assert_eq!(lite.id, ItemId::from("MVQTEQPE8FAQM"));
        assert_eq!(lite.star, 0); // no key until a rating is set
        assert_eq!(lite.palette0, Some([142, 83, 55]));
        assert_eq!(lite.thumb_version, 1793113379762);
        assert_eq!(lite.folders, Some(Vec::new())); // missing and [] differ
        assert!(lite.no_thumbnail);
        let detail = item.to_item();
        assert_eq!(detail.palettes.len(), 1);
        assert!(!detail.is_deleted);
    }

    #[test]
    fn folder_password_becomes_a_flag_and_is_dropped() {
        let locked: FolderDto = serde_json::from_value(json!({
            "id": "F1",
            "name": "Secret",
            "password": "c2VjcmV0",
            "passwordTips": "hint",
            "children": []
        }))
        .unwrap();
        assert!(locked.is_locked);
        let folder = locked.to_folder();
        assert!(folder.is_locked);
        assert!(!folder.is_unlocked);
        assert_eq!(folder.image_count, None);

        let open: FolderDto = serde_json::from_value(json!({
            "id": "F2",
            "name": "Open",
            "password": "",
            "children": [{ "id": "F3", "name": "Child", "iconColor": "red" }]
        }))
        .unwrap();
        assert!(!open.is_locked);
        let folder = open.to_folder();
        assert_eq!(folder.children.len(), 1);
        assert_eq!(folder.children[0].icon_color.as_deref(), Some("red"));
    }

    #[test]
    fn folder_without_password_key_is_not_locked() {
        let folder: FolderDto =
            serde_json::from_value(json!({ "id": "F1", "name": "Tree" })).unwrap();
        assert!(!folder.is_locked);
    }

    #[test]
    fn library_info_without_folders_means_none_open() {
        let empty: LibraryInfoDto = serde_json::from_value(json!({})).unwrap();
        assert!(empty.to_info().is_none());
        let open: LibraryInfoDto = serde_json::from_value(json!({
            "folders": [],
            "name": "ggwp",
            "path": "C:\\Libraries\\ggwp.library",
            "modificationTime": 1793113651878u64
        }))
        .unwrap();
        let info = open.to_info().unwrap();
        assert_eq!(info.kind, LibraryKind::EagleApi);
        assert_eq!(info.name, "ggwp");
        assert_eq!(info.modification_time, Some(1793113651878));
        // Phase 9 is browse-only: the UI disables every write for a read-only library.
        assert!(info.read_only);
    }

    #[test]
    fn tag_dto_keeps_the_count_optional() {
        let tag: TagDto = serde_json::from_value(
            json!({ "name": "тест-апи", "imageCount": 1, "pinyin": "тест-апи" }),
        )
        .unwrap();
        assert_eq!(tag.to_tag().image_count, Some(1));
        let tag: TagDto = serde_json::from_value(json!({ "name": "fresh" })).unwrap();
        assert_eq!(tag.to_tag().image_count, None);
    }
}
