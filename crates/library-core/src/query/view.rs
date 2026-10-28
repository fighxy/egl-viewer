//! Views: the sorted result of one query, served to the UI in windows (docs/architecture.md §6.3,
//! §6.5). Whole item lists never cross IPC.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use crate::error::SourceError;
use crate::model::{ItemId, ItemLite};
use crate::query::filter::{Context, FolderIndex, filter};
use crate::query::sort::sort;
use crate::query::{LibraryQuery, Scope, ScopeHints};
use crate::source::Progress;

/// The most cells one `view_range` call returns.
pub const MAX_WINDOW: usize = 200;
/// Views kept open; the least recently used goes first.
const VIEWS_KEPT: usize = 4;
/// Loaded scopes kept, so a new sort or chip on the same scope does not load it again.
const SNAPSHOTS_KEPT: usize = 6;

/// What one grid cell or list row needs.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemCell {
    pub id: ItemId,
    pub name: String,
    pub ext: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub size: Option<u64>,
    pub star: u8,
    pub palette0: Option<[u8; 3]>,
    /// The thumbnail URL answers with an image; otherwise the cell shows `palette0` and `ext`.
    pub has_thumbnail: bool,
    /// Part of the thumbnail URL, so a new thumbnail is never served from the cache.
    pub thumb_version: i64,
    pub added_at: Option<i64>,
    pub tags: Vec<String>,
}

impl From<&ItemLite> for ItemCell {
    fn from(item: &ItemLite) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            ext: item.ext.clone(),
            width: item.width,
            height: item.height,
            size: item.size,
            star: item.star,
            palette0: item.palette0,
            has_thumbnail: !item.no_thumbnail,
            thumb_version: item.thumb_version,
            added_at: item.added_at,
            tags: item.tags.iter().map(|tag| tag.to_string()).collect(),
        }
    }
}

/// What views need from a library source. The app adapts its `LibrarySource` to this.
pub trait ViewSource {
    fn load_scope(
        &self,
        scope: &Scope,
        hints: &ScopeHints,
        progress: Progress<'_>,
    ) -> Result<Vec<ItemLite>, SourceError>;
    fn folder_index(&self) -> Result<FolderIndex, SourceError>;
    fn search_ids(&self, text: &str) -> Result<HashSet<ItemId>, SourceError>;
}

/// The sorted result of one query: positions into a shared snapshot of its scope.
#[derive(Debug, Clone)]
pub struct View {
    id: u64,
    query: LibraryQuery,
    snapshot: Arc<[ItemLite]>,
    order: Vec<u32>,
}

impl View {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn query(&self) -> &LibraryQuery {
        &self.query
    }

    pub fn total(&self) -> usize {
        self.order.len()
    }

    fn items(&self, start: usize, len: usize) -> impl Iterator<Item = &ItemLite> {
        self.order
            .iter()
            .skip(start)
            .take(len)
            .map(|&index| &self.snapshot[index as usize])
    }

    /// Cells `start..start + len`, at most [`MAX_WINDOW`] of them.
    pub fn range(&self, start: usize, len: usize) -> Vec<ItemCell> {
        self.items(start, len.min(MAX_WINDOW))
            .map(ItemCell::from)
            .collect()
    }

    /// Ids `start..start + len`: what a selection needs, never whole items.
    pub fn ids(&self, start: usize, len: usize) -> Vec<ItemId> {
        self.items(start, len).map(|item| item.id.clone()).collect()
    }
}

/// Open views and loaded scopes of one library.
#[derive(Debug, Default)]
pub struct Views {
    next_id: u64,
    /// Least recently used first.
    views: Vec<View>,
    /// Least recently used first.
    snapshots: Vec<(Scope, Arc<[ItemLite]>)>,
}

impl Views {
    /// Opens a view for `query`. Every call gets its own view, so closing one never closes
    /// another that shows the same query. A scope that is already loaded is not loaded again.
    pub fn open(
        &mut self,
        query: LibraryQuery,
        source: &dyn ViewSource,
        progress: Progress<'_>,
    ) -> Result<&View, SourceError> {
        let snapshot = self.snapshot(&query, source, progress)?;
        let order = compute(&snapshot, &query, source)?;
        self.next_id += 1;
        self.views.push(View {
            id: self.next_id,
            query,
            snapshot,
            order,
        });
        if self.views.len() > VIEWS_KEPT {
            self.views.remove(0);
        }
        Ok(self.views.last().expect("a view was just pushed"))
    }

    /// The view `id`, marked as recently used.
    pub fn get(&mut self, id: u64) -> Option<&View> {
        let position = self.views.iter().position(|view| view.id == id)?;
        let view = self.views.remove(position);
        self.views.push(view);
        self.views.last()
    }

    pub fn close(&mut self, id: u64) {
        self.views.retain(|view| view.id != id);
    }

    /// Drops every view and snapshot (the library closed).
    pub fn clear(&mut self) {
        self.views.clear();
        self.snapshots.clear();
    }

    /// After a write: loads the scopes of the open views again and recomputes the views under
    /// the same ids. Returns each view's id and new total.
    pub fn refresh(&mut self, source: &dyn ViewSource) -> Result<Vec<(u64, usize)>, SourceError> {
        self.snapshots.clear();
        let mut loaded: HashMap<Scope, Arc<[ItemLite]>> = HashMap::new();
        let mut totals = Vec::with_capacity(self.views.len());
        for view in &mut self.views {
            let snapshot = match loaded.get(&view.query.scope) {
                Some(snapshot) => Arc::clone(snapshot),
                None => {
                    // A folder that is gone (deleted, or in another library) shows nothing.
                    let items: Arc<[ItemLite]> =
                        match source.load_scope(&view.query.scope, &view.query.hints(), &|_, _| {})
                        {
                            Ok(items) => items.into(),
                            Err(SourceError::NotFound) => Vec::new().into(),
                            Err(error) => return Err(error),
                        };
                    loaded.insert(view.query.scope.clone(), Arc::clone(&items));
                    items
                }
            };
            view.order = compute(&snapshot, &view.query, source)?;
            view.snapshot = snapshot;
            totals.push((view.id, view.order.len()));
        }
        self.snapshots = loaded.into_iter().collect();
        Ok(totals)
    }

    fn snapshot(
        &mut self,
        query: &LibraryQuery,
        source: &dyn ViewSource,
        progress: Progress<'_>,
    ) -> Result<Arc<[ItemLite]>, SourceError> {
        if let Some(position) = self
            .snapshots
            .iter()
            .position(|(scope, _)| *scope == query.scope)
        {
            let entry = self.snapshots.remove(position);
            let snapshot = Arc::clone(&entry.1);
            self.snapshots.push(entry);
            return Ok(snapshot);
        }
        let items: Arc<[ItemLite]> = source
            .load_scope(&query.scope, &query.hints(), progress)?
            .into();
        self.snapshots
            .push((query.scope.clone(), Arc::clone(&items)));
        if self.snapshots.len() > SNAPSHOTS_KEPT {
            self.snapshots.remove(0);
        }
        Ok(items)
    }
}

/// Search applies from two characters on (docs/spec.md §5).
fn search_text(query: &LibraryQuery) -> Option<&str> {
    query
        .text
        .as_deref()
        .map(str::trim)
        .filter(|text| text.chars().count() >= 2)
}

/// The pipeline: filter, then sort.
fn compute(
    snapshot: &[ItemLite],
    query: &LibraryQuery,
    source: &dyn ViewSource,
) -> Result<Vec<u32>, SourceError> {
    let folders = source.folder_index()?;
    let text_ids = search_text(query)
        .map(|text| source.search_ids(text))
        .transpose()?;
    let mut order = filter(
        snapshot,
        query,
        Context {
            folders: &folders,
            text_ids: text_ids.as_ref(),
        },
    );
    sort(snapshot, &mut order, query);
    Ok(order)
}
