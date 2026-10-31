//! Source-agnostic core of egl-viewer.
//!
//! Models, the library query, item edits, errors and the [`LibrarySource`] trait that every
//! kind of library implements: a running Eagle, an Eagle library folder, the native library.
//! This crate has no IO and no async runtime (docs/architecture.md §3, §15.2).

pub mod edit;
pub mod error;
pub mod export;
pub mod folder;
pub mod maintenance;
pub mod model;
pub mod query;
pub mod smart;
pub mod source;
pub mod tag;
pub mod undo;

pub use edit::{ItemEdit, is_web_link};
pub use error::{LocationProblem, NetworkError, SourceError};
pub use export::{
    Compression, ExportFormat, ExportPreview, ExportProgress, ExportRequest, ExportSelection,
    ExportState, ExportSummary,
};
pub use folder::{FOLDER_COLORS, FolderChange, order_key_between};
pub use maintenance::{
    Backup, BackupKind, LibraryCheck, LibraryStats, MissingOriginal, RepairOptions,
};
pub use model::{
    AddTarget, ChangeToken, Changes, DuplicateNotice, ExtensionCount, Folder, ImportFailure,
    ImportKind, ImportPreview, ImportProgress, ImportState, ImportTicket, Item, ItemId, ItemLite,
    LibraryCounts, LibraryHolder, LibraryInfo, LibraryKind, NewFile, Palette, PivPolicy, PivPrompt,
    PivWrap, PreviewEntry, ReadOnlyReason, RecoveryKey, SmartFolder, Tag, TagGroup, Thumbnail,
    TrashInfo,
};
pub use query::filter::{FolderIndex, matches_tags, shape_of, type_matches};
pub use query::search::{SearchExpr, SearchMatch, fold, parse_search};
pub use query::sort::{compare_names, compare_tags};
pub use query::view::{ItemCell, MAX_WINDOW, View, ViewSource, Views};
pub use query::{LibraryQuery, Scope, ScopeHints, SortKey, TagMode};
pub use smart::{RuleItem, SmartFolderChange, SmartRules, rules_from_query};
pub use source::{Capabilities, LibrarySource, Progress};
pub use tag::{SelectionTags, TagGroupChange, validate_tag_names};
pub use undo::{EditKind, ItemState, UndoStack, UndoStatus, UndoStep};
