//! Views of the open library and the `thumb` scheme, without Tauri (docs/architecture.md §6.5,
//! §11): the IPC types, reading the source on a blocking thread, and recomputing the views after
//! imports wrote.

use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use library_core::{
    FolderIndex, ImportProgress, ItemId, ItemLite, LibrarySource, Progress, Scope, ScopeHints,
    SourceError, Thumbnail, ViewSource, Views,
};
use serde::Serialize;
use ts_rs::TS;

/// `open_view` answers with the view's id and how many items it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ViewOpened {
    pub view_id: u64,
    pub total: u32,
}

/// Sent through the `Channel` of `open_view` while a scope loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScopeProgress {
    pub loaded: u32,
    pub total: u32,
}

/// The payload of `views-invalidated`: views recomputed under the same ids, with their totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ViewsInvalidated {
    pub views: Vec<ViewOpened>,
}

/// What the session tells the UI (events in the app).
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ImportProgress(ImportProgress),
    ViewsInvalidated(ViewsInvalidated),
}

/// Sends an event to the UI.
pub type Emit = Arc<dyn Fn(Event) + Send + Sync>;

pub(crate) fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Runs a source call that never waits. The native source does its work in the first poll; a
/// source that waits (a running Eagle, phase 9) needs a runtime here instead.
pub(crate) fn ready<T>(
    future: impl Future<Output = Result<T, SourceError>>,
) -> Result<T, SourceError> {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => Err(SourceError::NotSupported(
            "a library source that waits".into(),
        )),
    }
}

/// A library source read on the calling (blocking) thread.
pub(crate) struct Blocking<'a>(pub &'a dyn LibrarySource);

impl ViewSource for Blocking<'_> {
    fn load_scope(
        &self,
        scope: &Scope,
        hints: &ScopeHints,
        progress: Progress<'_>,
    ) -> Result<Vec<ItemLite>, SourceError> {
        ready(self.0.load_scope(scope, hints, progress))
    }

    fn folder_index(&self) -> Result<FolderIndex, SourceError> {
        Ok(FolderIndex::new(&ready(self.0.folder_tree())?))
    }

    fn search_ids(&self, text: &str) -> Result<HashSet<ItemId>, SourceError> {
        ready(self.0.search_ids(text))
    }
}

/// How often the open views follow a running import.
const REFRESH_EVERY: Duration = Duration::from_millis(500);

/// Recomputes the open views while imports write, at most every [`REFRESH_EVERY`] and once at
/// the end, and tells the UI which views changed.
pub(crate) struct Refresher {
    views: Arc<Mutex<Views>>,
    emit: Emit,
    /// When the views were last recomputed, and the import's writes seen then.
    last: Mutex<(Instant, u32)>,
}

impl Refresher {
    pub(crate) fn new(views: Arc<Mutex<Views>>, emit: Emit) -> Self {
        Self {
            views,
            emit,
            last: Mutex::new((Instant::now(), 0)),
        }
    }

    /// Called with each progress update of an import that writes to `source`.
    pub(crate) fn after(&self, progress: &ImportProgress, source: &dyn LibrarySource) {
        let writes = progress.added + progress.previews_done;
        let finished = progress.finished_at.is_some();
        {
            let mut last = crate::session::lock(&self.last);
            if writes == last.1 || (!finished && last.0.elapsed() < REFRESH_EVERY) {
                return;
            }
            *last = (Instant::now(), writes);
        }
        let totals = crate::session::lock(&self.views).refresh(&Blocking(source));
        match totals {
            Ok(totals) if !totals.is_empty() => {
                (self.emit)(Event::ViewsInvalidated(ViewsInvalidated {
                    views: totals
                        .into_iter()
                        .map(|(view_id, total)| ViewOpened {
                            view_id,
                            total: count(total),
                        })
                        .collect(),
                }));
            }
            Ok(_) => {}
            Err(error) => tracing::warn!("cannot refresh the views: {error}"),
        }
    }
}

/// What a `thumb` URL asks for: `/<id>` is the thumbnail, `/<id>/original` the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageRequest {
    Thumbnail(ItemId),
    Original(ItemId),
}

impl ImageRequest {
    /// Ids are short ASCII letters and digits (ULIDs, Eagle ids); anything else is refused
    /// before a lookup.
    pub(crate) fn parse(path: &str) -> Option<Self> {
        let path = path.trim_start_matches('/');
        let (id, original) = match path.split_once('/') {
            Some((id, "original")) => (id, true),
            Some(_) => return None,
            None => (path, false),
        };
        let valid =
            !id.is_empty() && id.len() <= 64 && id.bytes().all(|byte| byte.is_ascii_alphanumeric());
        if !valid {
            return None;
        }
        let id = ItemId(id.to_owned());
        Some(if original {
            Self::Original(id)
        } else {
            Self::Thumbnail(id)
        })
    }

    pub(crate) fn read(
        &self,
        source: &dyn LibrarySource,
    ) -> Result<Option<Thumbnail>, SourceError> {
        match self {
            Self::Thumbnail(id) => ready(source.thumbnail(id)),
            Self::Original(id) => ready(source.original(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_paths_name_a_thumbnail_or_an_original() {
        assert_eq!(
            ImageRequest::parse("/01JB7Q"),
            Some(ImageRequest::Thumbnail(ItemId::from("01JB7Q")))
        );
        assert_eq!(
            ImageRequest::parse("/01JB7Q/original"),
            Some(ImageRequest::Original(ItemId::from("01JB7Q")))
        );
        let long = format!("/{}", "a".repeat(65));
        for path in ["", "/", "/a/b", "/../x", "/a.b", "/a%2F", long.as_str()] {
            assert_eq!(ImageRequest::parse(path), None, "{path}");
        }
    }

    #[test]
    fn a_source_that_waits_is_refused() {
        let pending = std::future::pending::<Result<(), SourceError>>();
        assert!(matches!(ready(pending), Err(SourceError::NotSupported(_))));
        assert_eq!(ready(async { Ok::<_, SourceError>(7) }), Ok(7));
    }
}
