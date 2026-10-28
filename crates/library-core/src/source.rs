//! The one interface to every kind of library (docs/architecture.md §15.2).

use std::collections::HashSet;

use async_trait::async_trait;
use serde::Serialize;
use ts_rs::TS;

use crate::edit::ItemEdit;
use crate::error::SourceError;
use crate::model::{
    AddTarget, ChangeToken, Changes, Folder, FolderChange, ImportTicket, Item, ItemId, ItemLite,
    LibraryInfo, NewFile, SmartFolder, Tag, TagGroup, Thumbnail,
};
use crate::query::{Scope, ScopeHints};

/// Called with `(loaded, total)` while a scope loads.
pub type Progress<'a> = &'a (dyn Fn(usize, usize) + Send + Sync);

/// What a source can do. The UI hides what the open library cannot do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Capabilities {
    pub writable: bool,
    /// `changes_since` works (native: yes; Eagle folder: through `mtime.json`; Eagle API: no).
    pub change_feed: bool,
    /// Emptying the trash: native library only.
    pub delete_items: bool,
    /// Native library only: Eagle's API has no folder delete.
    pub delete_folders: bool,
    /// Eagle's API only accepts comments with a rect or a video time.
    pub write_comments: bool,
    pub locked_folders: bool,
}

impl Capabilities {
    /// A source that can only be read.
    pub const READ_ONLY: Self = Self {
        writable: false,
        change_feed: false,
        delete_items: false,
        delete_folders: false,
        write_comments: false,
        locked_folders: false,
    };
}

/// Everything above the sources (commands, views, the pipeline, the UI) talks to this trait.
///
/// Write methods default to [`SourceError::ReadOnly`], so a read-only source implements only
/// the reads. `async_trait` keeps the trait usable as `dyn LibrarySource`.
#[async_trait]
pub trait LibrarySource: Send + Sync {
    fn capabilities(&self) -> Capabilities;

    async fn info(&self) -> Result<LibraryInfo, SourceError>;
    async fn folder_tree(&self) -> Result<Vec<Folder>, SourceError>;
    async fn load_scope(
        &self,
        scope: &Scope,
        hints: &ScopeHints,
        progress: Progress<'_>,
    ) -> Result<Vec<ItemLite>, SourceError>;
    async fn search_ids(&self, text: &str) -> Result<HashSet<ItemId>, SourceError>;
    async fn item(&self, id: &ItemId) -> Result<Option<Item>, SourceError>;
    async fn tags(&self) -> Result<Vec<Tag>, SourceError>;
    async fn tag_groups(&self) -> Result<Vec<TagGroup>, SourceError>;
    async fn smart_folders(&self) -> Result<Vec<SmartFolder>, SourceError>;

    async fn thumbnail(&self, _id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        Ok(None)
    }

    /// The original file, for the full-size preview, when the source can read it.
    async fn original(&self, _id: &ItemId) -> Result<Option<Thumbnail>, SourceError> {
        Ok(None)
    }

    async fn changes_since(&self, _token: &ChangeToken) -> Result<Changes, SourceError> {
        Err(SourceError::NotSupported("change feed".into()))
    }

    async fn apply_item_edit(&self, _id: &ItemId, _edit: ItemEdit) -> Result<Item, SourceError> {
        Err(SourceError::ReadOnly)
    }

    async fn add_files(
        &self,
        _files: Vec<NewFile>,
        _target: AddTarget,
    ) -> Result<Vec<ImportTicket>, SourceError> {
        Err(SourceError::ReadOnly)
    }

    async fn save_folder(&self, _change: FolderChange) -> Result<Vec<Folder>, SourceError> {
        Err(SourceError::ReadOnly)
    }

    async fn rename_tag(&self, _from: &str, _to: &str) -> Result<(), SourceError> {
        Err(SourceError::ReadOnly)
    }

    async fn merge_tags(&self, _source: &str, _target: &str) -> Result<(), SourceError> {
        Err(SourceError::ReadOnly)
    }
}
