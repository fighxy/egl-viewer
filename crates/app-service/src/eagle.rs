//! Importing an Eagle library folder into the open native library (docs/spec.md §8.2): what the
//! wizard shows for a picked `.library`, and the import task. Shared by the desktop and the
//! phones (D-116). Reading the Eagle folder is read-only; Eagle stays the only writer of it.

use std::path::Path;

use library_core::{ImportKind, ImportTicket, SourceError};
use library_native::{EagleImportOptions, import_eagle};
use serde::{Deserialize, Serialize};

use crate::service::LibraryService;

/// What the import wizard shows for a picked Eagle library (docs/spec.md §8.2 step 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EagleLibrarySummary {
    pub path: String,
    pub name: String,
    /// Items including the trash (`mtime.json` `all`).
    pub item_count: u32,
    /// Bytes of everything under `images/`.
    pub bytes: u64,
}

/// Reads the name, the item count and the size of the Eagle library at `path`, without
/// writing anything. `NotALibrary { eagle: true }` when the folder is not one.
pub fn eagle_library_info(path: &Path) -> Result<EagleLibrarySummary, SourceError> {
    let eagle = eagle_folder::EagleLibrary::open(path).map_err(eagle_error)?;
    let summary = eagle.summary().map_err(eagle_error)?;
    Ok(EagleLibrarySummary {
        path: eagle.root().to_string_lossy().into_owned(),
        name: summary.name,
        item_count: summary.item_count,
        bytes: summary.bytes,
    })
}

/// What a person can act on, for an Eagle folder the reader refuses.
pub fn eagle_error(error: eagle_folder::EagleError) -> SourceError {
    match error {
        eagle_folder::EagleError::NotFound(_) => SourceError::NotFound,
        eagle_folder::EagleError::NotAnEagleLibrary(_) => SourceError::NotALibrary { eagle: true },
        other => SourceError::Invalid(format!("reading the Eagle library: {other}")),
    }
}

impl LibraryService {
    /// Imports the Eagle library at `eagle_path` into the open library in a background task,
    /// with `importProgress` events like adding files; the final progress carries the report.
    /// With `under_root_folder` the Eagle folders go under a new root folder named after the
    /// Eagle library (an import into a library already in use); without it they go at the
    /// root (a library made for the import). Cancelling keeps what was imported; a second
    /// import of the same Eagle library skips the items it already has.
    pub fn import_eagle(
        &self,
        eagle_path: &Path,
        import_trash: bool,
        under_root_folder: bool,
    ) -> Result<ImportTicket, SourceError> {
        let eagle = eagle_folder::EagleLibrary::open(eagle_path).map_err(eagle_error)?;
        let library = self.writable_library()?;
        let options = EagleImportOptions {
            import_trash,
            root_folder: under_root_folder.then(|| eagle.name().to_owned()),
        };
        let id = self.start(
            library,
            ImportKind::Eagle,
            move |library, id, observer, cancel| {
                import_eagle(library, &eagle, &options, id, observer, cancel)
            },
        )?;
        Ok(ImportTicket { id })
    }
}
