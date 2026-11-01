//! The native library as a [`LibrarySource`] (docs/architecture.md §15.2): what the views, the
//! detail panel and the `thumb` scheme read through.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use library_core::{
    Capabilities, ExtensionCount, Folder, FolderChange, ImportPreview, Item, ItemEdit, ItemId,
    ItemLite, LibraryInfo, LibrarySource, Progress, Scope, ScopeHints, SearchMatch, SmartFolder,
    SmartFolderChange, SourceError, Tag, TagGroup, TagGroupChange, Thumbnail, TrashInfo,
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
            delete_items: self.library.read_only_reason().is_none(),
            delete_folders: self.library.read_only_reason().is_none(),
            ..Capabilities::READ_ONLY
        }
    }

    async fn info(&self) -> Result<LibraryInfo, SourceError> {
        Ok(self.library.info())
    }

    async fn folder_tree(&self) -> Result<Vec<Folder>, SourceError> {
        self.library.folder_tree()
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

    async fn search_ids(&self, text: &str) -> Result<SearchMatch, SourceError> {
        self.library.search(text)
    }

    async fn item(&self, id: &ItemId) -> Result<Option<Item>, SourceError> {
        self.library.item(&id.0)
    }

    async fn tags(&self) -> Result<Vec<Tag>, SourceError> {
        self.library.tags()
    }

    async fn tag_groups(&self) -> Result<Vec<TagGroup>, SourceError> {
        self.library.tag_groups()
    }

    async fn smart_folders(&self) -> Result<Vec<SmartFolder>, SourceError> {
        self.library.smart_folders()
    }

    async fn thumbnail(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        self.library.thumbnail(&id.0)
    }

    async fn original(&self, id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        self.library.original(&id.0)
    }

    async fn apply_item_edit(&self, id: &ItemId, edit: ItemEdit) -> Result<Item, SourceError> {
        self.library.apply_item_edit(&id.0, &edit)
    }

    async fn save_folder(&self, change: FolderChange) -> Result<Vec<Folder>, SourceError> {
        self.library.save_folder(&change)?;
        self.library.folder_tree()
    }

    async fn delete_folder(&self, id: &str, trash_items: bool) -> Result<Vec<Folder>, SourceError> {
        self.library.delete_folder(id, trash_items)?;
        self.library.folder_tree()
    }

    async fn apply_batch_edit(&self, ids: &[ItemId], edit: ItemEdit) -> Result<u32, SourceError> {
        self.library.apply_batch_edit(ids, &edit)
    }

    async fn delete_items(&self, ids: &[ItemId]) -> Result<u32, SourceError> {
        self.library.delete_items(ids)
    }

    async fn empty_trash(&self) -> Result<u32, SourceError> {
        self.library.empty_trash(
            &|_, _| {},
            &std::sync::atomic::AtomicBool::new(false),
            &|| {},
        )
    }

    async fn trash_info(&self) -> Result<TrashInfo, SourceError> {
        Ok(self.library.trash_info()?)
    }

    async fn trash_ids(&self) -> Result<Vec<ItemId>, SourceError> {
        Ok(self.library.trash_ids()?)
    }

    async fn preview_import(&self, source: &Path) -> Result<ImportPreview, SourceError> {
        self.library.preview_import(source)
    }

    async fn reorder_items(
        &self,
        folder: &str,
        ids: &[ItemId],
        before: Option<&ItemId>,
    ) -> Result<(), SourceError> {
        self.library.reorder_items(folder, ids, before)
    }

    async fn rename_tag(&self, from: &str, to: &str) -> Result<(), SourceError> {
        self.library.rename_tag(from, to)
    }

    async fn merge_tags(&self, source: &str, target: &str) -> Result<(), SourceError> {
        self.library.merge_tags(source, target)
    }

    async fn delete_tag(&self, name: &str) -> Result<(), SourceError> {
        self.library.delete_tag(name)
    }

    async fn save_tag_group(&self, change: TagGroupChange) -> Result<Vec<TagGroup>, SourceError> {
        self.library.save_tag_group(&change)?;
        self.library.tag_groups()
    }

    async fn delete_tag_group(&self, id: &str) -> Result<Vec<TagGroup>, SourceError> {
        self.library.delete_tag_group(id)?;
        self.library.tag_groups()
    }

    async fn extensions(&self) -> Result<Vec<ExtensionCount>, SourceError> {
        self.library.extensions()
    }

    async fn save_smart_folder(
        &self,
        change: SmartFolderChange,
    ) -> Result<Vec<SmartFolder>, SourceError> {
        self.library.save_smart_folder(&change)?;
        self.library.smart_folders()
    }

    async fn delete_smart_folder(&self, id: &str) -> Result<Vec<SmartFolder>, SourceError> {
        self.library.delete_smart_folder(id)?;
        self.library.smart_folders()
    }
}
