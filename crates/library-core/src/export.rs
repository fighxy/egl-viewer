//! Exporting originals to a folder or a ZIP archive (docs/spec.md §7.4 item 12): the selection, the
//! request, the live progress and the result. The engine lives in `library-native`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Minimum length of an age passphrase that wraps an export archive (D-102).
pub const ARCHIVE_PASSWORD_MIN_LEN: usize = 8;

/// Refuses an empty or too-short archive passphrase. Match and non-empty checks stay in the UI;
/// this is the hard floor shared by export and decrypt.
pub fn check_archive_password(password: &str) -> Result<(), crate::SourceError> {
    if password.is_empty() {
        return Err(crate::SourceError::Invalid("Type a password.".into()));
    }
    if password.chars().count() < ARCHIVE_PASSWORD_MIN_LEN {
        return Err(crate::SourceError::Invalid(format!(
            "The password must be at least {ARCHIVE_PASSWORD_MIN_LEN} characters."
        )));
    }
    Ok(())
}

/// Soft warning for the UI: short, one character class, or a well-known weak string. Export is
/// still allowed once [`check_archive_password`] passes (D-102).
pub fn archive_password_is_weak(password: &str) -> bool {
    let len = password.chars().count();
    if len < 12 {
        return true;
    }
    let lower = password.chars().any(|c| c.is_lowercase());
    let upper = password.chars().any(|c| c.is_uppercase());
    let digit = password.chars().any(|c| c.is_ascii_digit());
    let other = password.chars().any(|c| !c.is_alphanumeric());
    let classes = usize::from(lower) + usize::from(upper) + usize::from(digit) + usize::from(other);
    if classes < 2 {
        return true;
    }
    const COMMON: &[&str] = &[
        "password",
        "password1",
        "password12",
        "password123",
        "12345678",
        "123456789",
        "1234567890",
        "qwertyui",
        "qwerty123",
        "abcdefgh",
        "letmein1",
        "iloveyou",
        "admin123",
        "welcome1",
        "monkey12",
        "dragon12",
        "master12",
        "login123",
        "passw0rd",
        "changeme",
    ];
    let folded = password.to_ascii_lowercase();
    COMMON.iter().any(|word| folded == *word)
}

#[cfg(test)]
mod password_tests {
    use super::{ARCHIVE_PASSWORD_MIN_LEN, archive_password_is_weak, check_archive_password};

    #[test]
    fn short_passwords_are_refused() {
        assert!(check_archive_password("").is_err());
        assert!(check_archive_password("1234567").is_err());
        assert!(check_archive_password("12345678").is_ok());
        assert_eq!(ARCHIVE_PASSWORD_MIN_LEN, 8);
    }

    #[test]
    fn weak_passwords_are_flagged_but_allowed() {
        assert!(archive_password_is_weak("12345678"));
        assert!(archive_password_is_weak("password12"));
        assert!(archive_password_is_weak("abcdefghijkl"));
        assert!(!archive_password_is_weak("Correct-Horse-1"));
    }
}

/// What an export covers: the whole library, some folders, some tags, or any mix.
/// An item is exported once even when several selections cover it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ExportSelection {
    pub whole: bool,
    pub folders: Vec<String>,
    pub tags: Vec<String>,
}

impl ExportSelection {
    /// Nothing selected; the request would export zero items.
    pub fn is_empty(&self) -> bool {
        !self.whole && self.folders.is_empty() && self.tags.is_empty()
    }
}

/// How hard a ZIP archive squeezes its entries (deflate levels 0/1/6/9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Compression {
    /// Stored, no compression: fastest, for already-compressed media.
    None,
    /// Deflate 1.
    Fast,
    /// Deflate 6.
    #[default]
    Balanced,
    /// Deflate 9.
    Smallest,
}

impl Compression {
    /// The deflate level, `None` for stored entries.
    pub fn level(self) -> Option<i32> {
        match self {
            Self::None => None,
            Self::Fast => Some(1),
            Self::Balanced => Some(6),
            Self::Smallest => Some(9),
        }
    }
}

/// The output format: a plain folder tree, or a ZIP archive with a compression level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ExportFormat {
    /// The originals copied into a folder tree that mirrors the library's folders.
    #[default]
    Mirror,
    /// One ZIP archive; the entries mirror the same tree.
    Zip { compression: Compression },
}

/// One export: what, in which format, where, and with what protection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportRequest {
    pub selection: ExportSelection,
    /// A folder selection includes the items of its subfolders.
    pub include_subfolders: bool,
    pub format: ExportFormat,
    /// An existing folder the export is written into.
    pub destination: String,
    /// An age passphrase protecting the archive; only valid with [`ExportFormat::Zip`].
    /// At least [`ARCHIVE_PASSWORD_MIN_LEN`] characters (D-102). The UI sends it like a
    /// lock password and is never stored.
    pub encrypt: Option<String>,
    /// A running Eagle to export into instead of a folder or archive (docs/spec.md §8.4).
    /// When set, `format`, `destination` and `encrypt` are ignored.
    #[serde(default)]
    pub eagle: Option<EagleExportTarget>,
}

/// A running Eagle as the export destination (docs/spec.md §8.4): the app connects, keeps
/// Eagle the writer of its library, and stores every new Eagle id for the next export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EagleExportTarget {
    pub host: String,
    pub port: u16,
    /// API token; `None` or empty only works when Eagle runs on the same PC.
    pub token: Option<String>,
}

/// What an export would write, computed from the database without touching the files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportPreview {
    /// Visible items the selection covers, each counted once.
    pub items: u64,
    /// The sum of their stored sizes: the export cannot be smaller.
    pub bytes: u64,
    /// Items the selection covers but a locked folder hides; they are not exported.
    pub hidden: u64,
}

/// What one export task is doing right now (`export-progress` events).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExportState {
    /// Listing the items and planning the layout.
    Preparing,
    /// Copying files into the folder tree or the archive.
    Copying,
    /// Writing the ZIP through an age passphrase (no plaintext archive on disk).
    Encrypting,
    /// Everything written; `ExportProgress::summary` is set.
    Done,
    /// Gave up; `ExportProgress::error` says why. The partial output is removed.
    Failed,
    /// The user cancelled; the partial output is removed.
    Cancelled,
}

/// The result of a finished export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportSummary {
    /// Visible items the export covered.
    pub items: u64,
    /// Files written, the manifest included. An item in two exported folders is written twice.
    pub files: u64,
    /// Bytes written (plaintext; an encrypted archive is a little larger).
    pub bytes: u64,
    /// The items whose original was missing, by name; they were skipped.
    pub missing: Vec<String>,
    /// The written folder or archive, as an absolute path.
    pub path: String,
}

/// The state of one export task, sent to the UI as it changes (`export-progress` events).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportProgress {
    pub id: u64,
    pub state: ExportState,
    /// Items to write; known once preparing ends.
    pub total: u64,
    /// Items handled so far (written or found missing).
    pub done: u64,
    /// Sum of the stored sizes of `total` items.
    pub bytes_total: u64,
    /// Source bytes read so far.
    pub bytes_done: u64,
    /// The file being written now.
    pub current: Option<String>,
    /// Set when `state` becomes `done`.
    pub summary: Option<ExportSummary>,
    /// The message when `state` is `failed`.
    pub error: Option<String>,
}
