//! A running Eagle as a `LibrarySource` (docs/architecture.md §4, §6.2–§6.4).
//!
//! The source owns a small Tokio runtime: the app's view layer reads sources on blocking
//! threads, so each trait method blocks on that runtime and the outer future is immediately
//! ready. Snapshots of `FetchPlan` results live in an LRU with a 60 s TTL; a stale snapshot is
//! reloaded synchronously — stale-while-revalidate needs a push channel to the views that
//! arrives with the write path (phase 10).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use library_core::{
    Capabilities, Folder, Item, ItemId, ItemLite, LibraryCounts, LibraryInfo, LibrarySource,
    Progress, Scope, ScopeHints, SearchMatch, SmartFolder, SourceError, Tag, TagGroup,
};
use secrecy::SecretString;
use url::Url;

use crate::api::{EagleApi, GRID_FIELDS};
use crate::dto::ItemDto;
use crate::error::EagleError;
use crate::page::page_all;
use crate::plan::{FetchPlan, ServerFilter, plan};

const SNAPSHOT_TTL: Duration = Duration::from_secs(60);
const SNAPSHOT_KEEP: usize = 8;
const TREE_TTL: Duration = Duration::from_secs(60);

/// One open connection to a running Eagle's library.
pub struct EagleSource {
    /// `None` only while `Drop` moves the runtime onto a plain thread: a runtime cannot be
    /// dropped from inside an async context (the tests).
    runtime: Option<tokio::runtime::Runtime>,
    api: EagleApi,
    info: LibraryInfo,
    eagle_version: String,
    state: Mutex<State>,
}

struct State {
    /// From `item/countAll`, trash included: enough to choose a fetch strategy.
    library_count: usize,
    tree: Option<(Vec<Folder>, Instant)>,
    snapshots: SnapshotCache,
    counts: Option<(LibraryCounts, Instant)>,
}

#[derive(Default)]
struct SnapshotCache {
    order: VecDeque<FetchPlan>,
    map: HashMap<FetchPlan, (Arc<Vec<ItemLite>>, Instant)>,
}

impl SnapshotCache {
    fn get(&mut self, key: &FetchPlan) -> Option<Arc<Vec<ItemLite>>> {
        let (items, at) = self.map.get(key)?;
        if at.elapsed() > SNAPSHOT_TTL {
            return None;
        }
        if let Some(position) = self.order.iter().position(|plan| plan == key) {
            let plan = self.order.remove(position).expect("position is valid");
            self.order.push_back(plan);
        }
        Some(Arc::clone(items))
    }

    fn insert(&mut self, key: FetchPlan, items: Arc<Vec<ItemLite>>) {
        if self
            .map
            .insert(key.clone(), (items, Instant::now()))
            .is_none()
        {
            self.order.push_back(key);
        }
        while self.order.len() > SNAPSHOT_KEEP {
            if let Some(oldest) = self.order.pop_front() {
                self.map.remove(&oldest);
            }
        }
    }
}

impl EagleSource {
    /// Connects: `GET /app/info`, then `GET /library/info`, then the rough size. Every failure
    /// is one of the distinct messages of the Connect screen (docs/spec.md §8.3).
    pub fn connect(
        host: &str,
        port: u16,
        token: SecretString,
        over_tailscale: bool,
    ) -> Result<Self, EagleError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("eagle-api")
            .enable_all()
            .build()
            .map_err(|error| EagleError::Invalid(format!("runtime: {error}")))?;
        let api = EagleApi::new(host, port, token, over_tailscale)?;
        let (app, library, count) = runtime.block_on(async {
            let app = api.app_info().await?;
            let library = api.library_info().await?;
            let count = api.item_count().await?;
            Ok::<_, EagleError>((app, library, count))
        })?;
        let info = library.to_info().ok_or(EagleError::NoLibraryOpen)?;
        Ok(Self {
            runtime: Some(runtime),
            api,
            info,
            eagle_version: app.version,
            state: Mutex::new(State {
                library_count: count,
                tree: None,
                snapshots: SnapshotCache::default(),
                counts: None,
            }),
        })
    }

    /// For tests: a source over an arbitrary base URL, skipping the connection probe.
    pub fn at(base: Url, token: SecretString) -> Result<Self, EagleError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| EagleError::Invalid(format!("runtime: {error}")))?;
        let api = EagleApi::at(base, token)?;
        Ok(Self {
            runtime: Some(runtime),
            api,
            info: LibraryInfo {
                kind: library_core::LibraryKind::EagleApi,
                id: None,
                name: String::new(),
                path: None,
                modification_time: None,
                read_only: false,
                read_only_reason: None,
                holder: None,
            },
            eagle_version: String::new(),
            state: Mutex::new(State {
                library_count: 0,
                tree: None,
                snapshots: SnapshotCache::default(),
                counts: None,
            }),
        })
    }

    pub fn info(&self) -> LibraryInfo {
        self.info.clone()
    }

    pub fn eagle_version(&self) -> &str {
        &self.eagle_version
    }

    /// The left-bar counts. Eagle's API lists no trashed items, so the trash stays 0 and the
    /// UI hides that row for this source.
    pub fn counts(&self) -> Result<LibraryCounts, EagleError> {
        let mut state = self.lock()?;
        if let Some((counts, at)) = &state.counts
            && at.elapsed() <= TREE_TTL
        {
            return Ok(*counts);
        }
        let count = state.library_count;
        let (unfiled, untagged) = self.runtime().block_on(async {
            let unfiled = self
                .api
                .item_total(&ServerFilter {
                    is_unfiled: true,
                    ..ServerFilter::default()
                })
                .await?;
            let untagged = self
                .api
                .item_total(&ServerFilter {
                    is_untagged: true,
                    ..ServerFilter::default()
                })
                .await?;
            Ok::<_, EagleError>((unfiled, untagged))
        })?;
        let counts = LibraryCounts {
            all: u32::try_from(count).unwrap_or(u32::MAX),
            uncategorized: u32::try_from(unfiled).unwrap_or(u32::MAX),
            untagged: u32::try_from(untagged).unwrap_or(u32::MAX),
            trash: 0,
        };
        state.counts = Some((counts, Instant::now()));
        Ok(counts)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>, EagleError> {
        self.state
            .lock()
            .map_err(|_| EagleError::Invalid("cache lock poisoned".into()))
    }

    fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime.as_ref().expect("runtime set outside Drop")
    }
}

impl Drop for EagleSource {
    fn drop(&mut self) {
        // A runtime cannot be dropped from inside an async context, which is where the
        // wiremock tests (and any async caller) live; a plain thread is always allowed.
        if let Some(runtime) = self.runtime.take() {
            std::thread::spawn(move || drop(runtime)).join().ok();
        }
    }
}

#[async_trait]
impl LibrarySource for EagleSource {
    fn capabilities(&self) -> Capabilities {
        // Writes, thumbnails and the trash arrive in phase 10; locked folders are listed and
        // hidden by the pipeline's lock filter already.
        Capabilities {
            writable: false,
            change_feed: false,
            delete_items: false,
            delete_folders: false,
            write_comments: false,
            locked_folders: true,
        }
    }

    async fn info(&self) -> Result<LibraryInfo, SourceError> {
        Ok(self.info())
    }

    async fn folder_tree(&self) -> Result<Vec<Folder>, SourceError> {
        let mut state = self.lock()?;
        if let Some((tree, at)) = &state.tree
            && at.elapsed() <= TREE_TTL
        {
            return Ok(tree.clone());
        }
        let api = self.api.clone();
        let folders = self.runtime().block_on(async {
            page_all(
                |offset| {
                    let api = api.clone();
                    async move { api.folder_page(offset).await }
                },
                |folder: &crate::dto::FolderDto| folder.id.clone(),
                |_, _| {},
            )
            .await
        })?;
        let tree: Vec<Folder> = folders.iter().map(|folder| folder.to_folder()).collect();
        state.tree = Some((tree.clone(), Instant::now()));
        Ok(tree)
    }

    async fn load_scope(
        &self,
        scope: &Scope,
        hints: &ScopeHints,
        progress: Progress<'_>,
    ) -> Result<Vec<ItemLite>, SourceError> {
        if matches!(scope, Scope::Trash) {
            return Err(SourceError::NotSupported(
                "Eagle's API lists no trashed items".into(),
            ));
        }
        let key = plan(scope, hints, self.lock()?.library_count);
        if let Some(cached) = self.lock()?.snapshots.get(&key) {
            return Ok((*cached).clone());
        }
        let items = self.fetch(&key, progress)?;
        self.lock()?.snapshots.insert(key, Arc::new(items.clone()));
        Ok(items)
    }

    async fn search_ids(&self, text: &str) -> Result<SearchMatch, SourceError> {
        let api = self.api.clone();
        let text = text.to_owned();
        let ids = self.runtime().block_on(async {
            page_all(
                |offset| {
                    let api = api.clone();
                    let text = text.clone();
                    async move { api.item_query_page(&text, offset).await }
                },
                |item: &ItemDto| item.id.clone(),
                |_, _| {},
            )
            .await
        })?;
        Ok(SearchMatch::Only(
            ids.into_iter().map(|item| ItemId(item.id)).collect(),
        ))
    }

    async fn item(&self, id: &ItemId) -> Result<Option<Item>, SourceError> {
        let api = self.api.clone();
        let id = id.0.clone();
        let item = self
            .runtime()
            .block_on(async move { api.item_by_id(&id).await })?;
        Ok(item.as_ref().map(ItemDto::to_item))
    }

    async fn tags(&self) -> Result<Vec<Tag>, SourceError> {
        let api = self.api.clone();
        let tags = self.runtime().block_on(async {
            page_all(
                |offset| {
                    let api = api.clone();
                    async move { api.tag_page(offset).await }
                },
                |tag: &crate::dto::TagDto| tag.name.clone(),
                |_, _| {},
            )
            .await
        })?;
        Ok(tags.iter().map(|tag| tag.to_tag()).collect())
    }

    async fn tag_groups(&self) -> Result<Vec<TagGroup>, SourceError> {
        let api = self.api.clone();
        let groups = self.runtime().block_on(async {
            page_all(
                |offset| {
                    let api = api.clone();
                    async move { api.tag_group_page(offset).await }
                },
                |group: &crate::dto::TagGroupDto| group.id.clone(),
                |_, _| {},
            )
            .await
        })?;
        Ok(groups.iter().map(|group| group.to_tag_group()).collect())
    }

    async fn smart_folders(&self) -> Result<Vec<SmartFolder>, SourceError> {
        let api = self.api.clone();
        let folders = self.runtime().block_on(async {
            page_all(
                |offset| {
                    let api = api.clone();
                    async move { api.smart_folder_page(offset).await }
                },
                |folder: &crate::dto::SmartFolderDto| folder.id.clone(),
                |_, _| {},
            )
            .await
        })?;
        Ok(folders
            .iter()
            .map(|folder| folder.to_smart_folder())
            .collect())
    }
}

impl EagleSource {
    /// Executes one plan: every `ServerFilter` pages through `item/get`, the results union by
    /// id; a smart folder pages through `smartFolder/getItems`.
    fn fetch(&self, key: &FetchPlan, progress: Progress<'_>) -> Result<Vec<ItemLite>, EagleError> {
        let api = self.api.clone();
        match key {
            FetchPlan::SmartFolder(id) => {
                let id = id.clone();
                let items = self.runtime().block_on(async {
                    page_all(
                        |offset| {
                            let api = api.clone();
                            let id = id.clone();
                            async move { api.smart_folder_items_page(&id, offset).await }
                        },
                        |item: &ItemDto| item.id.clone(),
                        progress,
                    )
                    .await
                })?;
                Ok(items.iter().map(|item| item.to_lite()).collect())
            }
            FetchPlan::ItemGet(filters) => {
                // First-seen order, deduplicated by id: several filters union, and a shifted
                // page can repeat an item (docs/architecture.md §4.5).
                let mut order: Vec<ItemLite> = Vec::new();
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                let mut loaded = 0usize;
                let mut total = 0usize;
                for filter in filters {
                    let filter = filter.clone();
                    let items = self.runtime().block_on(async {
                        page_all(
                            |offset| {
                                let api = api.clone();
                                let filter = filter.clone();
                                async move { api.item_page(&filter, GRID_FIELDS, offset).await }
                            },
                            |item: &ItemDto| item.id.clone(),
                            |done, page_total| {
                                loaded = done;
                                total = page_total;
                                progress(loaded, total);
                            },
                        )
                        .await
                    })?;
                    for item in items {
                        if seen.insert(item.id.clone()) {
                            order.push(item.to_lite());
                        }
                    }
                }
                Ok(order)
            }
        }
    }
}

impl std::fmt::Debug for EagleSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EagleSource")
            .field("info", &self.info)
            .field("eagle_version", &self.eagle_version)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_cache_is_lru_with_ttl() {
        let mut cache = SnapshotCache::default();
        let key = || FetchPlan::SmartFolder("S".into());
        cache.insert(
            key(),
            Arc::new(vec![ItemLite {
                id: ItemId::from("A"),
                ..ItemLite::default()
            }]),
        );
        assert_eq!(cache.get(&key()).map(|items| items.len()), Some(1));
        // Fill past the cap: the oldest goes.
        for n in 0..SNAPSHOT_KEEP {
            cache.insert(
                FetchPlan::SmartFolder(format!("S{n}")),
                Arc::new(Vec::new()),
            );
        }
        assert_eq!(cache.order.len(), SNAPSHOT_KEEP);
        assert!(!cache.map.contains_key(&key()));
    }
}
