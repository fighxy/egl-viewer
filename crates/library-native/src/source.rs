//! The native library as a [`LibrarySource`] (docs/architecture.md §15.2): what the views, the
//! detail panel and the `thumb` scheme read through.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use library_core::{
    Capabilities, Folder, Item, ItemId, ItemLite, LibraryInfo, LibrarySource, Progress, Scope,
    ScopeHints, SmartFolder, SourceError, Tag, TagGroup, Thumbnail,
};

use crate::library::Library;

/// The reads never wait: SQLite and the files are local. The caller runs them on a blocking
/// thread.
pub struct NativeSource {
    library: Arc<Library>,
}

impl NativeSource {
    pub fn new(library: Arc<Library>) -> Self {
        Self { library }
    }
}

#[async_trait]
impl LibrarySource for NativeSource {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            writable: self.library.read_only_reason().is_none(),
            ..Capabilities::READ_ONLY
        }
    }

    async fn info(&self) -> Result<LibraryInfo, SourceError> {
        Ok(self.library.info())
    }

    /// Folders arrive in phase 4; nothing writes the table before.
    async fn folder_tree(&self) -> Result<Vec<Folder>, SourceError> {
        Ok(Vec::new())
    }

    async fn load_scope(
        &self,
        scope: &Scope,
        _hints: &ScopeHints,
        progress: Progress<'_>,
    ) -> Result<Vec<ItemLite>, SourceError> {
        let items = self.library.load_scope(scope)?;
        progress(items.len(), items.len());
        Ok(items)
    }

    /// Search arrives in phase 6 (FTS5).
    async fn search_ids(&self, _text: &str) -> Result<HashSet<ItemId>, SourceError> {
        Err(SourceError::NotSupported("search".into()))
    }

    async fn item(&self, id: &ItemId) -> Result<Option<Item>, SourceError> {
        self.library.item(&id.0)
    }

    async fn tags(&self) -> Result<Vec<Tag>, SourceError> {
        self.library.tags()
    }

    /// Tag groups arrive in phase 5.
    async fn tag_groups(&self) -> Result<Vec<TagGroup>, SourceError> {
        Ok(Vec::new())
    }

    /// Smart folders arrive in phase 6.
    async fn smart_folders(&self) -> Result<Vec<SmartFolder>, SourceError> {
        Ok(Vec::new())
    }

    async fn thumbnail(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        self.library.thumbnail(&id.0)
    }

    async fn original(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        self.library.original(&id.0)
    }
}
