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
    AddTarget, ChangeToken, Changes, DuplicateNotice, Folder, FolderChange, ImportFailure,
    ImportKind, ImportProgress, ImportState, ImportTicket, Item, ItemId, ItemLite, LibraryCounts,
    LibraryHolder, LibraryInfo, LibraryKind, NewFile, Palette, ReadOnlyReason, SmartFolder, Tag,
    TagGroup, Thumbnail,
};
pub use query::filter::{FolderIndex, matches_tags, shape_of, type_matches};
pub use query::sort::compare_names;
pub use query::view::{ItemCell, MAX_WINDOW, View, ViewSource, Views};
pub use query::{LibraryQuery, Scope, ScopeHints, SortKey, TagMode};
pub use source::{Capabilities, LibrarySource, Progress};
