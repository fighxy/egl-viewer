//! Source-agnostic core of egl-viewer.
//!
//! Models, the library query, item edits, errors and the [`LibrarySource`] trait that every
//! kind of library implements: a running Eagle, an Eagle library folder, the native library.
//! This crate has no IO and no async runtime (docs/architecture.md §3, §15.2).

pub mod edit;
pub mod error;
pub mod model;
pub mod query;
pub mod source;

pub use edit::ItemEdit;
pub use error::{LocationProblem, NetworkError, SourceError};
pub use model::{
    AddTarget, ChangeToken, Changes, Folder, FolderChange, ImportTicket, Item, ItemId, ItemLite,
    LibraryHolder, LibraryInfo, LibraryKind, NewFile, Palette, ReadOnlyReason, SmartFolder, Tag,
    TagGroup, Thumbnail,
};
pub use query::{LibraryQuery, Scope, ScopeHints, SortKey, TagMode};
pub use source::{Capabilities, LibrarySource, Progress};
