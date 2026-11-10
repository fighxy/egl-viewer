//! What a library service tells its app, and the shapes the app's UI sees: opened views, scope
//! progress, recomputed views, a selection as an edit target, and task progress
//! (docs/architecture.md §6.5, §11).

use std::sync::Arc;

use library_core::{ExportProgress, ImportProgress, ItemId};
use serde::{Deserialize, Serialize};

/// `open_view` answers with the view's id and how many items it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewOpened {
    pub view_id: u64,
    pub total: u32,
}

/// Sent through the `Channel` of `open_view` while a scope loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeProgress {
    pub loaded: u32,
    pub total: u32,
}

/// The payload of `views-invalidated`: views recomputed under the same ids, with their totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewsInvalidated {
    pub views: Vec<ViewOpened>,
}

/// The items an edit applies to: ids, or every item of a view except some (Select All).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemsTarget {
    /// The view the selection belongs to; needed with `all`.
    pub view_id: Option<u64>,
    /// `true`: every item of the view except `ids`.
    pub all: bool,
    pub ids: Vec<ItemId>,
}

/// The event for views recomputed under their ids, unless there were none.
pub fn invalidated(totals: Vec<(u64, usize)>) -> Option<Event> {
    (!totals.is_empty()).then(|| {
        Event::ViewsInvalidated(ViewsInvalidated {
            views: totals
                .into_iter()
                .map(|(view_id, total)| ViewOpened {
                    view_id,
                    total: count(total),
                })
                .collect(),
        })
    })
}

/// What the service tells its app while it works: task progress and recomputed views.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ImportProgress(ImportProgress),
    ExportProgress(ExportProgress),
    ViewsInvalidated(ViewsInvalidated),
}

/// Hands an event to the app: Tauri emits it to the WebView, the C ABI to its callback.
pub type Emit = Arc<dyn Fn(Event) + Send + Sync>;

/// A length as the `u32` the UI counts in, saturating.
pub fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

impl ItemsTarget {
    /// Listed items, no view needed.
    pub fn ids(ids: Vec<ItemId>) -> Self {
        Self {
            view_id: None,
            all: false,
            ids,
        }
    }
}
