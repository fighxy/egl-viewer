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

    /// `item/update`: complete desired state. Returns the full updated item; `data: false`
    /// means the id is gone (docs/spec.md §8.3).
    pub async fn item_update(
        &self,
        body: Map<String, Value>,
    ) -> Result<Option<ItemDto>, EagleError> {
        let data = self.http.post(&v2("item/update"), body).await?;
        if data.as_bool() == Some(false) || data.is_null() {
            return Ok(None);
        }
        Ok(Some(decode(data)?))
    }

    /// `folder/create` returns Eagle's internal folder object; the client takes only the id
    /// and re-reads the tree (correction 33).
    pub async fn folder_create(&self, body: Map<String, Value>) -> Result<String, EagleError> {
        let data = self.http.post(&v2("folder/create"), body).await?;
        let id = data
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| EagleError::Invalid("folder/create returned no id".into()))?;
        Ok(id.to_owned())
    }

    /// `folder/update` with the full desired state. An unknown id returns success without
    /// `data`; that is "not found".
    pub async fn folder_update(&self, body: Map<String, Value>) -> Result<(), EagleError> {
        let data = self.http.post(&v2("folder/update"), body).await?;
        if data.is_null() {
            return Err(EagleError::NotFound);
        }
        Ok(())
    }

    /// **v1.** `POST /api/folder/unlock`: the only way to reveal a locked folder's children.
    /// The error text arrives in `data`, which `parse_jsend` already reads (correction 20).
    pub async fn folder_unlock(&self, id: &str, password: &str) -> Result<(), EagleError> {
        let body = Map::from_iter([
            ("folderId".to_owned(), Value::from(id)),
            ("password".to_owned(), Value::from(password)),
        ]);
        self.http.post("api/folder/unlock", body).await?;
        Ok(())
    }

    /// `folder/get {id}`: one folder. The wire shape differs from the paged tree call, so
    /// both are tried.
    pub async fn folder_by_id(&self, id: &str) -> Result<Option<FolderDto>, EagleError> {
        let body = Map::from_iter([("id".to_owned(), Value::from(id))]);
        let data = self.http.post(&v2("folder/get"), body).await?;
        if let Ok(page) = serde_json::from_value::<Page<FolderDto>>(data.clone()) {
            return Ok(page.data.into_iter().next());
        }
        Ok(serde_json::from_value::<FolderDto>(data).ok())
    }

    /// `tag/update`: rename everywhere. `originalName` is the identity.
    pub async fn tag_update(&self, from: &str, to: &str) -> Result<(), EagleError> {
        let body = Map::from_iter([
            ("originalName".to_owned(), Value::from(from)),
            ("name".to_owned(), Value::from(to)),
        ]);
        self.http.post(&v2("tag/update"), body).await?;
        Ok(())
    }

    /// `tag/merge`: `source` is removed, `target` stays on the items.
    pub async fn tag_merge(&self, source: &str, target: &str) -> Result<(), EagleError> {
        let body = Map::from_iter([
            ("source".to_owned(), Value::from(source)),
            ("target".to_owned(), Value::from(target)),
        ]);
        self.http.post(&v2("tag/merge"), body).await?;
        Ok(())
    }

    /// Batch `item/add` with `path` entries: the form for a same-PC client, even for one
    /// file (it keeps `star` and `modificationTime`, correction 35). The ids come back
    /// before the items exist (correction 2): poll `item/get {id}`.
    pub async fn item_add_batch(
        &self,
        items: Vec<Map<String, Value>>,
    ) -> Result<Vec<String>, EagleError> {
        let body = Map::from_iter([("items".to_owned(), Value::from(items))]);
        let data = self.http.post(&v2("item/add"), body).await?;
        let ids = data
            .get("ids")
            .and_then(Value::as_array)
            .ok_or_else(|| EagleError::Invalid("item/add returned no ids".into()))?;
        Ok(ids
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect())
    }

    /// Single `item/add` with a `base64` data URL: the form for a client on another
    /// computer. Returns the new id.
    pub async fn item_add_single(&self, body: Map<String, Value>) -> Result<String, EagleError> {
        let data = self.http.post(&v2("item/add"), body).await?;
        let id = data
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| EagleError::Invalid("item/add returned no id".into()))?;
        Ok(id.to_owned())
    }

    /// **v1.** `GET /api/item/thumbnail?id=` answers with a Windows path *as a string*, not
    /// image bytes (docs/spec.md §8.3 "Thumbnails").
    pub async fn thumbnail_path(&self, id: &str) -> Result<Option<String>, EagleError> {
        let query = [("id", id)];
        let data = self.http.get("api/item/thumbnail", &query).await?;
        Ok(data.as_str().map(str::to_owned))
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
