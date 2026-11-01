//! A running Eagle as a `LibrarySource` (docs/architecture.md §4, §6.2–§6.4, §7).
//!
//! The source owns a small Tokio runtime: the app's view layer reads sources on blocking
//! threads, so each trait method blocks on that runtime and the outer future is immediately
//! ready. Writes are strictly sequential (an async `item/update` recomputes bindings on the
//! Eagle desktop, which takes noticeable time in a large library, arch §7.1).
//!
//! Snapshots of `FetchPlan` results live in an LRU with a 60 s TTL. After `item/update` the
//! returned item is patched into every snapshot whose server filter still matches it
//! locally; adds and folder/tag writes drop everything (docs/architecture.md §6.3).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use library_core::{
    AddTarget, Capabilities, Folder, FolderChange, FolderIndex, Item, ItemCell, ItemEdit, ItemId,
    ItemLite, LibraryCounts, LibraryInfo, LibraryKind, LibrarySource, NewFile, Progress, Scope,
    ScopeHints, SearchMatch, SmartFolder, SourceError, Tag, TagGroup, Thumbnail,
};
use secrecy::SecretString;
use serde_json::Value;
use url::Url;

use crate::add::{self, PendingAdd};
use crate::api::{EagleApi, GRID_FIELDS};
use crate::dto::{FolderDto, ItemDto};
use crate::error::EagleError;
use crate::page::page_all;
use crate::plan::{FetchPlan, ServerFilter, plan};
use crate::write::update_body;

const SNAPSHOT_TTL: Duration = Duration::from_secs(60);
const SNAPSHOT_KEEP: usize = 8;
const TREE_TTL: Duration = Duration::from_secs(60);
/// Thumbnails kept in memory; the WebView caches by `?v=` anyway, this absorbs grid
/// refetches (scroll back and forth) without touching Eagle again.
const THUMBS_KEPT: usize = 512;

/// One open connection to a running Eagle's library.
pub struct EagleSource {
    /// `None` only while `Drop` moves the runtime onto a plain thread: a runtime cannot be
    /// dropped from inside an async context (the tests).
    runtime: Option<tokio::runtime::Runtime>,
    api: EagleApi,
    info: LibraryInfo,
    eagle_version: String,
    /// Eagle runs on this PC: files are added by `path` and thumbnails read from disk.
    same_pc: bool,
    /// The library folder on the Eagle PC (`ggwp.library`); thumbnails and originals are
    /// read only from inside it.
    library_path: Option<PathBuf>,
    /// Writes are strictly sequential (docs/architecture.md §7.1).
    write: Mutex<()>,
    state: Mutex<State>,
    thumbs: Mutex<ThumbCache>,
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

    fn clear(&mut self) {
        self.order.clear();
        self.map.clear();
    }

    /// After `item/update`: patches the item into every snapshot whose filter still matches
    /// it locally, removes it where it no longer matches, and drops snapshots that carry
    /// `rating`/`shape` filters (docs/architecture.md §6.3).
    fn apply_update(&mut self, item: &ItemLite) {
        let keys: Vec<FetchPlan> = self.order.iter().cloned().collect();
        for key in keys {
            self.patch_or_drop(&key, item);
        }
    }

    fn patch_or_drop(&mut self, key: &FetchPlan, item: &ItemLite) {
        let Some((_, at)) = self.map.get(key) else {
            return;
        };
        let stale = at.elapsed() > SNAPSHOT_TTL;
        let patchable = match key {
            FetchPlan::SmartFolder(_) => false,
            FetchPlan::ItemGet(filters) => {
                !stale && !filters.iter().any(ServerFilter::has_unpatchable_filter)
            }
        };
        if !patchable {
            self.map.remove(key);
            self.order.retain(|plan| plan != key);
            return;
        }
        let matches = match key {
            FetchPlan::ItemGet(filters) => filters.iter().any(|f| f.matches_locally(item)),
            FetchPlan::SmartFolder(_) => unreachable!("checked above"),
        };
        let (items, _) = self.map.get_mut(key).expect("checked above");
        let items = Arc::make_mut(items);
        if matches {
            match items.iter_mut().find(|old| old.id == item.id) {
                Some(slot) => *slot = item.clone(),
                None => items.push(item.clone()),
            }
        } else {
            items.retain(|old| old.id != item.id);
        }
    }
}

#[derive(Default)]
struct ThumbCache {
    order: VecDeque<(String, i64)>,
    map: HashMap<(String, i64), Thumbnail>,
}

impl ThumbCache {
    fn get(&self, key: &(String, i64)) -> Option<Thumbnail> {
        self.map.get(key).cloned()
    }

    fn insert(&mut self, key: (String, i64), thumbnail: Thumbnail) {
        if self.map.insert(key.clone(), thumbnail).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > THUMBS_KEPT {
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
        let library_path = info.path.as_deref().map(PathBuf::from);
        let same_pc = is_loopback(host);
        Ok(Self::assemble(
            runtime,
            api,
            info,
            app.version,
            count,
            same_pc,
            library_path,
        ))
    }

    /// For tests: a source over an arbitrary base URL, skipping the connection probe.
    pub fn at(base: Url, token: SecretString) -> Result<Self, EagleError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| EagleError::Invalid(format!("runtime: {error}")))?;
        let api = EagleApi::at(base, token)?;
        Ok(Self::assemble(
            runtime,
            api,
            LibraryInfo {
                kind: LibraryKind::EagleApi,
                id: None,
                name: String::new(),
                path: None,
                modification_time: None,
                read_only: false,
                read_only_reason: None,
                holder: None,
            },
            String::new(),
            0,
            false,
            None,
        ))
    }

    fn assemble(
        runtime: tokio::runtime::Runtime,
        api: EagleApi,
        info: LibraryInfo,
        eagle_version: String,
        library_count: usize,
        same_pc: bool,
        library_path: Option<PathBuf>,
    ) -> Self {
        Self {
            runtime: Some(runtime),
            api,
            info,
            eagle_version,
            same_pc,
            library_path,
            write: Mutex::new(()),
            state: Mutex::new(State {
                library_count,
                tree: None,
                snapshots: SnapshotCache::default(),
                counts: None,
            }),
            thumbs: Mutex::new(ThumbCache::default()),
        }
    }

    pub fn info(&self) -> LibraryInfo {
        self.info.clone()
    }

    pub fn eagle_version(&self) -> &str {
        &self.eagle_version
    }

    /// Eagle runs on this PC: `add_files` uses `path` and thumbnails come from disk.
    pub fn same_pc(&self) -> bool {
        self.same_pc
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

    /// Sends `files` to Eagle, reporting each accepted id through `on_pending` (the items
    /// do not exist yet, correction 2). Folder ids are checked against the tree first.
    pub fn start_add(
        &self,
        files: Vec<NewFile>,
        target: &AddTarget,
        on_pending: impl FnMut(PendingAdd),
    ) -> Result<Vec<PendingAdd>, EagleError> {
        let mut target = target.clone();
        let tree = self.fresh_tree()?;
        let known: HashSet<&str> = folder_ids(&tree).into_iter().collect();
        target.folders.retain(|id| known.contains(id.as_str()));
        let api = self.api.clone();
        let same_pc = self.same_pc;
        self.runtime().block_on(async move {
            add::start_add(&api, same_pc, &files, &target, on_pending).await
        })
    }

    /// Polls `item/get {id}` until the item appears (about 20 s at most).
    pub fn wait_for_item(
        &self,
        id: &str,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Option<ItemDto>, EagleError> {
        let api = self.api.clone();
        let id = id.to_owned();
        self.runtime()
            .block_on(async move { add::wait_for_item(&api, &id, cancel).await })
    }

    /// After a data-URL add, a chosen rating: the single `item/add` ignores `star`
    /// (correction 11), so it is set with a separate `item/update`.
    pub fn finish_add_star(&self, id: &str, star: u8) -> Result<(), EagleError> {
        let _guard = self.write_lock()?;
        let api = self.api.clone();
        let id = id.to_owned();
        self.runtime().block_on(async move {
            let body = serde_json::Map::from_iter([
                ("id".to_owned(), Value::from(id)),
                ("star".to_owned(), Value::from(star)),
            ]);
            api.item_update(body).await.map(|_| ())
        })
    }

    /// Opens a locked folder through the v1 API. This also unlocks it in the Eagle desktop
    /// until Eagle restarts (docs/spec.md §8.3 "Writes").
    pub fn unlock_folder(&self, id: &str, password: &str) -> Result<(), SourceError> {
        let _guard = self.write_lock()?;
        let api = self.api.clone();
        let id = id.to_owned();
        let password = password.to_owned();
        self.runtime()
            .block_on(async move { api.folder_unlock(&id, &password).await })?;
        // The tree nodes carry `isUnLock`; re-read it.
        self.lock()?.tree = None;
        Ok(())
    }

    /// Drops every cached snapshot and the folder tree: the next read reloads. Called after
    /// writes whose effect on cached lists cannot be computed locally.
    pub fn invalidate_all(&self) -> Result<(), EagleError> {
        let mut state = self.lock()?;
        state.snapshots.clear();
        state.tree = None;
        state.counts = None;
        Ok(())
    }

    /// The grid's "Importing…" cell for one accepted file.
    pub fn pending_cell(pending: &PendingAdd) -> ItemCell {
        ItemCell {
            id: ItemId(pending.id.clone()),
            name: pending.name.clone(),
            ext: pending.ext.clone(),
            has_thumbnail: false,
            pending: true,
            ..ItemCell::default()
        }
    }

    /// The thumbnail for the `thumb` scheme, same-PC only. `version` is the item's
    /// `thumbVersion` from the cell URL; `i64::MIN` means "unknown" (the bare trait call).
    pub fn thumbnail_versioned(
        &self,
        id: &ItemId,
        version: i64,
    ) -> Result<Option<Thumbnail>, SourceError> {
        if !self.same_pc {
            return Ok(None);
        }
        let key = (id.0.clone(), version);
        if let Some(hit) = self.thumbs.lock().ok().and_then(|c| c.get(&key)) {
            return Ok(Some(hit));
        }
        let api = self.api.clone();
        let raw = self
            .runtime()
            .block_on(async move { api.thumbnail_path(&id.0).await })?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let candidate = PathBuf::from(&raw);
        match self.read_inside_library(&candidate) {
            Ok(bytes) => {
                let thumbnail = Thumbnail {
                    content_type: mime_of_path(&candidate).unwrap_or_else(|| "image/webp".into()),
                    bytes,
                    private: false,
                };
                if let Ok(mut cache) = self.thumbs.lock() {
                    cache.insert(key, thumbnail.clone());
                }
                Ok(Some(thumbnail))
            }
            Err(_) => self.no_thumbnail_original(id, &key),
        }
    }

    /// `noThumbnail: true`: the desktop shows the original, and so does this client — from
    /// `images/{id}.info/`, downscaled (docs/spec.md §8.3 "Thumbnails").
    fn no_thumbnail_original(
        &self,
        id: &ItemId,
        key: &(String, i64),
    ) -> Result<Option<Thumbnail>, SourceError> {
        let Some(item) = self.item_blocking(id)? else {
            return Ok(None);
        };
        if !item.no_thumbnail.unwrap_or(false) {
            return Ok(None);
        }
        let Some(bytes) = self.read_original(&id.0, &item.ext) else {
            return Ok(None);
        };
        let rendered = media::render_bytes(&bytes, &item.ext)
            .map_err(|error| SourceError::Storage(format!("downscale {}: {error}", id.0)))?;
        let thumbnail = Thumbnail {
            bytes: rendered.thumbnail.webp,
            content_type: "image/webp".into(),
            private: false,
        };
        if let Ok(mut cache) = self.thumbs.lock() {
            cache.insert(key.clone(), thumbnail.clone());
        }
        Ok(Some(thumbnail))
    }

    /// The original file for the full-size preview, same-PC only.
    pub fn original_file(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        if !self.same_pc {
            return Ok(None);
        }
        let Some(item) = self.item_blocking(id)? else {
            return Ok(None);
        };
        Ok(self.read_original(&id.0, &item.ext).map(|bytes| Thumbnail {
            content_type: mime_of_ext(&item.ext)
                .unwrap_or_else(|| "application/octet-stream".into()),
            bytes,
            private: false,
        }))
    }

    fn item_blocking(&self, id: &ItemId) -> Result<Option<ItemDto>, SourceError> {
        let api = self.api.clone();
        let id = id.0.clone();
        Ok(self
            .runtime()
            .block_on(async move { api.item_by_id(&id).await })?)
    }

    /// Reads `path` only when it exists and lies inside the library folder (the v1 endpoint
    /// answers with a path string; it must not be able to point outside, arch §11).
    fn read_inside_library(&self, path: &Path) -> Result<Vec<u8>, SourceError> {
        let Some(root) = &self.library_path else {
            return Err(SourceError::NotFound);
        };
        let (Ok(root), Ok(path)) = (root.canonicalize(), path.canonicalize()) else {
            return Err(SourceError::NotFound);
        };
        if !path.starts_with(&root) {
            return Err(SourceError::NotFound);
        }
        std::fs::read(&path).map_err(|_| SourceError::NotFound)
    }

    /// The item's original: `images/{id}.info/` holds one file, matched by extension.
    fn read_original(&self, id: &str, ext: &str) -> Option<Vec<u8>> {
        let root = self.library_path.as_ref()?;
        let dir = root.join("images").join(format!("{id}.info"));
        let wanted = ext.to_ascii_lowercase();
        let found = std::fs::read_dir(dir).ok()?.find_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_string_lossy().into_owned();
            let dot = name.rfind('.')?;
            (name[dot + 1..].eq_ignore_ascii_case(&wanted)).then_some(path)
        })?;
        self.read_inside_library(&found).ok()
    }

    fn fresh_tree(&self) -> Result<Vec<Folder>, EagleError> {
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
                |folder: &FolderDto| folder.id.clone(),
                |_, _| {},
            )
            .await
        })?;
        let tree: Vec<Folder> = folders.iter().map(|folder| folder.to_folder()).collect();
        state.tree = Some((tree.clone(), Instant::now()));
        Ok(tree)
    }

    fn write_lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, EagleError> {
        self.write
            .lock()
            .map_err(|_| EagleError::Invalid("write lock poisoned".into()))
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

#[async_trait]
impl LibrarySource for EagleSource {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            writable: true,
            change_feed: false,
            // Eagle's API cannot delete permanently or empty the trash.
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
        Ok(self.fresh_tree()?)
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
        Ok(self.item_blocking(id)?.as_ref().map(ItemDto::to_item))
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

    async fn thumbnail(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        self.thumbnail_versioned(id, i64::MIN)
    }

    async fn original(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        self.original_file(id)
    }

    async fn apply_item_edit(&self, id: &ItemId, edit: ItemEdit) -> Result<Item, SourceError> {
        let _guard = self.write_lock()?;
        let api = self.api.clone();
        let id = id.0.clone();
        let updated = self
            .runtime()
            .block_on(async move {
                let fresh = api.item_by_id(&id).await?.ok_or(EagleError::NotFound)?;
                let body = update_body(&fresh, &edit)?;
                api.item_update(body).await?.ok_or(EagleError::NotFound)
            })
            .map_err(SourceError::from)?;
        let lite = updated.to_lite();
        if let Ok(mut state) = self.lock() {
            state.snapshots.apply_update(&lite);
        }
        Ok(updated.to_item())
    }

    /// One `item/update` per item; unknown ids are skipped, like every source.
    async fn apply_batch_edit(&self, ids: &[ItemId], edit: ItemEdit) -> Result<u32, SourceError> {
        if matches!(edit, ItemEdit::Restore) {
            return Err(SourceError::NotSupported(
                "Eagle's API cannot restore from the trash".into(),
            ));
        }
        edit.validate()?;
        let _guard = self.write_lock()?;
        let mut edited = 0u32;
        for id in ids {
            match self.apply_item_edit_locked(id, &edit) {
                Ok(item) => {
                    edited += 1;
                    if let Ok(mut state) = self.lock() {
                        state.snapshots.apply_update(&item.to_lite());
                    }
                }
                Err(SourceError::NotFound) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(edited)
    }

    async fn save_folder(&self, change: FolderChange) -> Result<Vec<Folder>, SourceError> {
        change.validate()?;
        let _guard = self.write_lock()?;
        match &change {
            FolderChange::Create { .. } => {
                let body = folder_create_body(&change)?;
                let api = self.api.clone();
                self.runtime()
                    .block_on(async move { api.folder_create(body).await })?;
            }
            FolderChange::Update { id, .. } => {
                let (fresh, parent) = self.fresh_folder_with_parent(id)?;
                let body = folder_update_body(&change, &fresh, parent.as_deref())?;
                let api = self.api.clone();
                self.runtime()
                    .block_on(async move { api.folder_update(body).await })?;
            }
            FolderChange::Move { id, parent, .. } => {
                // The cycle guard (docs/architecture.md §7.3): a silent server-side ignore
                // becomes an error below, when the re-read tree disagrees.
                let tree = self.fresh_tree()?;
                let index = FolderIndex::new(&tree);
                if !index.can_move(id, parent.as_deref()) {
                    return Err(SourceError::Invalid(
                        "a folder cannot move into itself or its own subfolder".into(),
                    ));
                }
                let (fresh, _) = self.fresh_folder_with_parent(id)?;
                let body = folder_update_body(
                    &FolderChange::Update {
                        id: id.clone(),
                        name: fresh.name.clone(),
                        description: fresh.description.clone(),
                        tags: fresh.tags.clone(),
                        icon_color: fresh.icon_color.clone(),
                        order_by: None,
                        sort_ascending: None,
                    },
                    &fresh,
                    parent.as_deref(),
                )?;
                let api = self.api.clone();
                self.runtime()
                    .block_on(async move { api.folder_update(body).await })?;
                let tree = self.fresh_tree_forced()?;
                let (_, new_parent) = find_folder(&tree, id).ok_or(SourceError::NotFound)?;
                if new_parent.as_deref() != parent.as_deref() {
                    return Err(SourceError::Rejected {
                        http: 200,
                        message: "Eagle refused the move".into(),
                    });
                }
            }
        }
        self.invalidate_all()?;
        self.fresh_tree().map_err(SourceError::from)
    }

    async fn rename_tag(&self, from: &str, to: &str) -> Result<(), SourceError> {
        let _guard = self.write_lock()?;
        let api = self.api.clone();
        let (from, to) = (from.to_owned(), to.to_owned());
        self.runtime()
            .block_on(async move { api.tag_update(&from, &to).await })?;
        // Every snapshot can carry the old name in item tags.
        self.invalidate_all()?;
        Ok(())
    }

    async fn merge_tags(&self, source: &str, target: &str) -> Result<(), SourceError> {
        let _guard = self.write_lock()?;
        let api = self.api.clone();
        let (source, target) = (source.to_owned(), target.to_owned());
        self.runtime()
            .block_on(async move { api.tag_merge(&source, &target).await })?;
        self.invalidate_all()?;
        Ok(())
    }

    async fn delete_tag(&self, _name: &str) -> Result<(), SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API cannot delete a tag globally".into(),
        ))
    }

    async fn delete_folder(
        &self,
        _id: &str,
        _trash_items: bool,
    ) -> Result<Vec<Folder>, SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API cannot delete a folder".into(),
        ))
    }

    async fn delete_items(&self, _ids: &[ItemId]) -> Result<u32, SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API cannot delete permanently".into(),
        ))
    }

    async fn empty_trash(&self) -> Result<u32, SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API cannot empty the trash".into(),
        ))
    }

    async fn trash_info(&self) -> Result<library_core::TrashInfo, SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API lists no trashed items".into(),
        ))
    }

    async fn save_tag_group(
        &self,
        _change: library_core::TagGroupChange,
    ) -> Result<Vec<TagGroup>, SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API cannot edit tag groups".into(),
        ))
    }

    async fn delete_tag_group(&self, _id: &str) -> Result<Vec<TagGroup>, SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API cannot delete a tag group".into(),
        ))
    }

    async fn save_smart_folder(
        &self,
        _change: library_core::SmartFolderChange,
    ) -> Result<Vec<SmartFolder>, SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API cannot edit smart folders".into(),
        ))
    }

    async fn delete_smart_folder(&self, _id: &str) -> Result<Vec<SmartFolder>, SourceError> {
        Err(SourceError::NotSupported(
            "Eagle's API cannot delete a smart folder".into(),
        ))
    }
}

impl EagleSource {
    /// `apply_item_edit` with the write lock already held (batch edits).
    fn apply_item_edit_locked(&self, id: &ItemId, edit: &ItemEdit) -> Result<ItemDto, SourceError> {
        let api = self.api.clone();
        let id = id.0.clone();
        let edit = edit.clone();
        self.runtime()
            .block_on(async move {
                let fresh = api.item_by_id(&id).await?.ok_or(EagleError::NotFound)?;
                let body = update_body(&fresh, &edit)?;
                api.item_update(body).await?.ok_or(EagleError::NotFound)
            })
            .map_err(SourceError::from)
    }

    /// The tree, ignoring the cache: after a folder write the cached one is stale.
    fn fresh_tree_forced(&self) -> Result<Vec<Folder>, EagleError> {
        self.lock()?.tree = None;
        self.fresh_tree()
    }

    /// A fresh folder node and its parent id (`None` at the root).
    fn fresh_folder_with_parent(&self, id: &str) -> Result<(Folder, Option<String>), SourceError> {
        let tree = self.fresh_tree()?;
        find_folder(&tree, id).ok_or(SourceError::NotFound)
    }

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
                let mut seen: HashSet<String> = HashSet::new();
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

impl Drop for EagleSource {
    fn drop(&mut self) {
        // A runtime cannot be dropped from inside an async context, which is where the
        // wiremock tests (and any async caller) live; a plain thread is always allowed.
        if let Some(runtime) = self.runtime.take() {
            std::thread::spawn(move || drop(runtime)).join().ok();
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

fn is_loopback(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Every folder id in the tree, for validating add targets.
fn folder_ids(tree: &[Folder]) -> Vec<&str> {
    let mut ids = Vec::new();
    let mut stack: Vec<&Folder> = tree.iter().collect();
    while let Some(folder) = stack.pop() {
        ids.push(folder.id.as_str());
        stack.extend(&folder.children);
    }
    ids
}

/// A folder node and its parent id (`None` at the root or when unknown).
fn find_folder(tree: &[Folder], id: &str) -> Option<(Folder, Option<String>)> {
    fn visit(nodes: &[Folder], parent: Option<&str>, id: &str) -> Option<(Folder, Option<String>)> {
        for node in nodes {
            if node.id == id {
                return Some((node.clone(), parent.map(str::to_owned)));
            }
            if let Some(found) = visit(&node.children, Some(&node.id), id) {
                return Some(found);
            }
        }
        None
    }
    visit(tree, None, id)
}

/// `folder/create` body from the change.
fn folder_create_body(change: &FolderChange) -> Result<serde_json::Map<String, Value>, EagleError> {
    let FolderChange::Create {
        name,
        parent,
        description,
        icon_color,
    } = change
    else {
        return Err(EagleError::Invalid("not a create".into()));
    };
    let mut body = serde_json::Map::from_iter([("name".to_owned(), Value::from(name.clone()))]);
    if let Some(parent) = parent {
        body.insert("parent".into(), Value::from(parent.clone()));
    }
    if !description.is_empty() {
        body.insert("description".into(), Value::from(description.clone()));
    }
    if let Some(color) = icon_color {
        body.insert("iconColor".into(), Value::from(color.clone()));
    }
    Ok(body)
}

/// `folder/update` body: the full desired state (docs/architecture.md §7.2). `parent` is
/// mandatory for a nested folder — an omitted `parent` moves it to the root — and never
/// serialized as `null` (correction 31).
fn folder_update_body(
    change: &FolderChange,
    fresh: &Folder,
    parent: Option<&str>,
) -> Result<serde_json::Map<String, Value>, EagleError> {
    let (name, description, tags, icon_color, order_by, sort_ascending) = match change {
        FolderChange::Update {
            name,
            description,
            tags,
            icon_color,
            order_by,
            sort_ascending,
            ..
        } => (
            name,
            description,
            tags,
            icon_color,
            *order_by,
            *sort_ascending,
        ),
        _ => return Err(EagleError::Invalid("not an update".into())),
    };
    let mut body = serde_json::Map::from_iter([
        ("id".to_owned(), Value::from(fresh.id.clone())),
        ("name".to_owned(), Value::from(name.clone())),
        ("description".to_owned(), Value::from(description.clone())),
        ("tags".to_owned(), Value::from(tags.clone())),
    ]);
    // "No colour" and "at the root" are expressed by omitting the field (corrections 3, 31).
    if let Some(color) = icon_color {
        body.insert("iconColor".into(), Value::from(color.clone()));
    }
    if let Some(parent) = parent {
        body.insert("parent".into(), Value::from(parent.to_owned()));
    }
    if let Some(order_by) = order_by
        && let Ok(value) = serde_json::to_value(order_by)
    {
        body.insert("orderBy".into(), value);
    }
    if let Some(ascending) = sort_ascending {
        body.insert("sortIncrease".into(), Value::from(ascending));
    }
    Ok(body)
}

/// The MIME type of a file served through the `thumb` scheme, by extension.
fn mime_of_path(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    mime_of_ext(&ext)
}

fn mime_of_ext(ext: &str) -> Option<String> {
    Some(
        match ext {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            "svg" => "image/svg+xml",
            "tif" | "tiff" => "image/tiff",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "pdf" => "application/pdf",
            _ => return None,
        }
        .to_owned(),
    )
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
        for n in 0..SNAPSHOT_KEEP {
            cache.insert(
                FetchPlan::SmartFolder(format!("S{n}")),
                Arc::new(Vec::new()),
            );
        }
        assert_eq!(cache.order.len(), SNAPSHOT_KEEP);
        assert!(!cache.map.contains_key(&key()));
        cache.clear();
        assert!(cache.order.is_empty() && cache.map.is_empty());
    }

    #[test]
    fn apply_update_patches_matching_snapshots() {
        let mut cache = SnapshotCache::default();
        let filter = ServerFilter {
            folders: Some(vec!["F1".into()]),
            ..ServerFilter::default()
        };
        let key = FetchPlan::ItemGet(vec![filter]);
        cache.insert(
            key.clone(),
            Arc::new(vec![ItemLite {
                id: ItemId::from("A"),
                tags: ["old".into()].into(),
                ..ItemLite::default()
            }]),
        );
        // Still in F1: replaced in place.
        cache.apply_update(&ItemLite {
            id: ItemId::from("A"),
            folders: Some(vec!["F1".into()]),
            tags: ["new".into()].into(),
            ..ItemLite::default()
        });
        let items = cache.get(&key).unwrap();
        assert_eq!(items[0].tags, ["new".into()]);
        // Left F1: removed from the snapshot.
        cache.apply_update(&ItemLite {
            id: ItemId::from("A"),
            folders: Some(vec!["F9".into()]),
            ..ItemLite::default()
        });
        assert!(cache.get(&key).unwrap().is_empty());
    }

    #[test]
    fn apply_update_drops_unpatchable_and_smart_snapshots() {
        let mut cache = SnapshotCache::default();
        let smart = FetchPlan::SmartFolder("S".into());
        let rated = FetchPlan::ItemGet(vec![ServerFilter {
            rating: Some(3),
            ..ServerFilter::default()
        }]);
        cache.insert(smart.clone(), Arc::new(Vec::new()));
        cache.insert(rated.clone(), Arc::new(Vec::new()));
        let item = ItemLite {
            id: ItemId::from("A"),
            star: 3,
            ..ItemLite::default()
        };
        cache.apply_update(&item);
        assert!(!cache.map.contains_key(&smart));
        assert!(!cache.map.contains_key(&rated));
    }

    #[test]
    fn folder_tree_lookup_finds_parents() {
        let tree = vec![Folder {
            id: "F1".into(),
            children: vec![Folder {
                id: "F2".into(),
                ..Folder::default()
            }],
            ..Folder::default()
        }];
        let (found, parent) = find_folder(&tree, "F2").unwrap();
        assert_eq!(found.id, "F2");
        assert_eq!(parent.as_deref(), Some("F1"));
        let (_, parent) = find_folder(&tree, "F1").unwrap();
        assert_eq!(parent, None);
        assert!(find_folder(&tree, "F9").is_none());
    }

    #[test]
    fn update_body_omits_null_parent_and_colour() {
        let change = FolderChange::Update {
            id: "F1".into(),
            name: "Renamed".into(),
            description: String::new(),
            tags: vec![],
            icon_color: None,
            order_by: None,
            sort_ascending: None,
        };
        let fresh = Folder {
            id: "F1".into(),
            ..Folder::default()
        };
        // At the root: no parent key, no iconColor key (corrections 3, 31).
        let body = folder_update_body(&change, &fresh, None).unwrap();
        assert!(body.get("parent").is_none());
        assert!(body.get("iconColor").is_none());
        assert_eq!(body["name"], Value::from("Renamed"));
        // Nested: parent is mandatory.
        let body = folder_update_body(&change, &fresh, Some("P1")).unwrap();
        assert_eq!(body["parent"], Value::from("P1"));
        // A colour rides along; so does a per-folder sort.
        let change = FolderChange::Update {
            id: "F1".into(),
            name: "Renamed".into(),
            description: String::new(),
            tags: vec![],
            icon_color: Some("red".into()),
            order_by: Some(library_core::SortKey::Title),
            sort_ascending: Some(true),
        };
        let body = folder_update_body(&change, &fresh, Some("P1")).unwrap();
        assert_eq!(body["iconColor"], Value::from("red"));
        assert_eq!(body["orderBy"], Value::from("name"));
        assert_eq!(body["sortIncrease"], Value::from(true));
    }

    #[test]
    fn loopback_detection_matches_connection_settings() {
        for host in ["127.0.0.1", "localhost", "[::1]", "LOCALHOST"] {
            assert!(is_loopback(host), "{host}");
        }
        for host in ["192.168.1.20", "pc", "100.64.0.1"] {
            assert!(!is_loopback(host), "{host}");
        }
    }

    #[test]
    fn pending_cells_mark_themselves() {
        let cell = EagleSource::pending_cell(&PendingAdd {
            id: "NEW1".into(),
            name: "IMG 1".into(),
            ext: "jpg".into(),
        });
        assert!(cell.pending);
        assert!(!cell.has_thumbnail);
        assert_eq!(cell.id, ItemId::from("NEW1"));
    }

    #[test]
    fn same_pc_thumbnail_path_outside_the_library_is_refused() {
        // docs/spec.md §10: a thumbnail path outside the library folder is refused.
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let inside_file = root.path().join("thumb.png");
        let outside_file = outside.path().join("secret.png");
        std::fs::write(&inside_file, b"inside").unwrap();
        std::fs::write(&outside_file, b"outside").unwrap();

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let api = EagleApi::at(
            url::Url::parse("http://127.0.0.1:9/").unwrap(),
            SecretString::new("".into()),
        )
        .unwrap();
        let source = EagleSource::assemble(
            runtime,
            api,
            LibraryInfo {
                kind: LibraryKind::EagleApi,
                id: None,
                name: "test".into(),
                path: Some(root.path().display().to_string()),
                modification_time: None,
                read_only: false,
                read_only_reason: None,
                holder: None,
            },
            String::new(),
            0,
            true,
            Some(root.path().to_path_buf()),
        );
        assert_eq!(
            source.read_inside_library(&inside_file).unwrap(),
            b"inside"
        );
        assert!(matches!(
            source.read_inside_library(&outside_file),
            Err(SourceError::NotFound)
        ));
    }
}
