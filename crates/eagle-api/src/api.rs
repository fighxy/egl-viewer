//! The typed layer over the transport (docs/architecture.md §4.1), phase 9 subset: everything
//! browsing needs. Writes and thumbnails arrive in phase 10.

use secrecy::SecretString;
use serde_json::{Map, Value};
use url::Url;

use crate::dto::{
    AppInfoDto, FolderDto, ItemDto, LibraryInfoDto, SmartFolderDto, TagDto, TagGroupDto,
};
use crate::error::EagleError;
use crate::http::{EagleHttp, Endpoint, v2};
use crate::page::{Page, decode_page};
use crate::plan::ServerFilter;

/// The list fields the grid and local sorting need (docs/spec.md §8.3).
pub const GRID_FIELDS: &[&str] = &[
    "id",
    "name",
    "ext",
    "width",
    "height",
    "size",
    "star",
    "tags",
    "folders",
    "palettes",
    "modificationTime",
    "mtime",
    "btime",
    "duration",
    "noThumbnail",
];

const PAGE_LIMIT: usize = 1000;

/// A connection to one running Eagle.
#[derive(Debug, Clone)]
pub struct EagleApi {
    http: EagleHttp,
}

impl EagleApi {
    pub fn new(
        host: &str,
        port: u16,
        token: SecretString,
        over_tailscale: bool,
    ) -> Result<Self, EagleError> {
        let base = Url::parse(&format!("http://{host}:{port}/"))
            .map_err(|error| EagleError::Invalid(format!("bad host {host}: {error}")))?;
        Ok(Self {
            http: EagleHttp::new(Endpoint { base, token }, over_tailscale)?,
        })
    }

    /// For tests: an API over an arbitrary base URL.
    pub fn at(base: Url, token: SecretString) -> Result<Self, EagleError> {
        Ok(Self {
            http: EagleHttp::new(Endpoint { base, token }, false)?,
        })
    }

    /// The connection test (docs/spec.md §8.3 "Session").
    pub async fn app_info(&self) -> Result<AppInfoDto, EagleError> {
        decode(self.http.get(&v2("app/info"), &[]).await?)
    }

    /// Reads `metadata.json` from disk; `{}` when no library is open. Never logged raw: the
    /// folder nodes carry `password` fields (docs/spec.md §8.3).
    pub async fn library_info(&self) -> Result<LibraryInfoDto, EagleError> {
        decode(self.http.get(&v2("library/info"), &[]).await?)
    }

    /// The rough library size, trash included; good enough to choose a fetch strategy.
    pub async fn item_count(&self) -> Result<usize, EagleError> {
        let data = self.http.get(&v2("item/countAll"), &[]).await?;
        serde_json::from_value(data)
            .map_err(|error| EagleError::Invalid(format!("unexpected item/countAll: {error}")))
    }

    /// One `item/get` page. Arrays go in the POST body: GET query strings cannot carry them
    /// reliably (docs/spec.md §8.3).
    pub async fn item_page(
        &self,
        filter: &ServerFilter,
        fields: &[&str],
        offset: usize,
    ) -> Result<Page<ItemDto>, EagleError> {
        let mut body = serde_json::to_value(filter)
            .map_err(|error| EagleError::Invalid(format!("bad filter: {error}")))?;
        let map = body
            .as_object_mut()
            .expect("ServerFilter serializes into an object");
        map.insert(
            "fields".into(),
            fields.iter().map(|f| Value::from(*f)).collect(),
        );
        map.insert("offset".into(), Value::from(offset));
        map.insert("limit".into(), Value::from(PAGE_LIMIT));
        decode_page(self.http.post(&v2("item/get"), take(map)).await?)
    }

    /// Full-text search through `item/query`; the client takes only the ids (spec §8.3).
    pub async fn item_query_page(
        &self,
        text: &str,
        offset: usize,
    ) -> Result<Page<ItemDto>, EagleError> {
        let body = Map::from_iter([
            ("query".to_owned(), Value::from(text)),
            ("offset".to_owned(), Value::from(offset)),
            ("limit".to_owned(), Value::from(PAGE_LIMIT)),
        ]);
        decode_page(self.http.post(&v2("item/query"), body).await?)
    }

    /// One item, full fields; `item/get {id}` ignores `fields` (correction 14).
    pub async fn item_by_id(&self, id: &str) -> Result<Option<ItemDto>, EagleError> {
        let body = Map::from_iter([("id".to_owned(), Value::from(id))]);
        let page: Page<ItemDto> = decode_page(self.http.post(&v2("item/get"), body).await?)?;
        Ok(page.data.into_iter().next())
    }

    /// One page of the live in-memory folder tree, root folders only, `limit=1000`.
    pub async fn folder_page(&self, offset: usize) -> Result<Page<FolderDto>, EagleError> {
        let query = [
            ("limit", PAGE_LIMIT.to_string()),
            ("offset", offset.to_string()),
        ];
        let query: Vec<(&str, &str)> = query
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        decode_page(self.http.get(&v2("folder/get"), &query).await?)
    }

    pub async fn tag_page(&self, offset: usize) -> Result<Page<TagDto>, EagleError> {
        let query = [
            ("limit", PAGE_LIMIT.to_string()),
            ("offset", offset.to_string()),
        ];
        let query: Vec<(&str, &str)> = query
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        decode_page(self.http.get(&v2("tag/get"), &query).await?)
    }

    pub async fn tag_group_page(&self, offset: usize) -> Result<Page<TagGroupDto>, EagleError> {
        let query = [
            ("limit", PAGE_LIMIT.to_string()),
            ("offset", offset.to_string()),
        ];
        let query: Vec<(&str, &str)> = query
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        decode_page(self.http.get(&v2("tagGroup/get"), &query).await?)
    }

    pub async fn smart_folder_page(
        &self,
        offset: usize,
    ) -> Result<Page<SmartFolderDto>, EagleError> {
        let query = [
            ("limit", PAGE_LIMIT.to_string()),
            ("offset", offset.to_string()),
        ];
        let query: Vec<(&str, &str)> = query
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        decode_page(self.http.get(&v2("smartFolder/get"), &query).await?)
    }

    /// `smartFolder/getItems` ignores `fields` unless it is an array: use POST (correction 17).
    pub async fn smart_folder_items_page(
        &self,
        id: &str,
        offset: usize,
    ) -> Result<Page<ItemDto>, EagleError> {
        let body = Map::from_iter([
            ("smartFolderId".to_owned(), Value::from(id)),
            ("offset".to_owned(), Value::from(offset)),
            ("limit".to_owned(), Value::from(PAGE_LIMIT)),
        ]);
        decode_page(self.http.post(&v2("smartFolder/getItems"), body).await?)
    }

    /// How many items match the filter, from one small page.
    pub async fn item_total(&self, filter: &ServerFilter) -> Result<usize, EagleError> {
        let mut body = serde_json::to_value(filter)
            .map_err(|error| EagleError::Invalid(format!("bad filter: {error}")))?;
        let map = body
            .as_object_mut()
            .expect("ServerFilter serializes into an object");
        map.insert("offset".into(), Value::from(0));
        map.insert("limit".into(), Value::from(1));
        let page: Page<Value> = decode_page(self.http.post(&v2("item/get"), take(map)).await?)?;
        Ok(page.total)
    }
}

fn take(map: &mut Map<String, Value>) -> Map<String, Value> {
    std::mem::take(map)
}

fn decode<T: for<'de> serde::Deserialize<'de>>(data: Value) -> Result<T, EagleError> {
    serde_json::from_value(data)
        .map_err(|error| EagleError::Invalid(format!("unexpected response shape: {error}")))
}
