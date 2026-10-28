//! Errors of every library source, as the UI sees them: `{ kind, detail }`.
//!
//! One flat set of kinds for all sources (docs/decisions.md, D-004), so the UI maps errors
//! without knowing which kind of library is open.

use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export)]
pub enum SourceError {
    #[error("not connected")]
    NotConnected,
    /// HTTP 401 from Eagle: wrong or missing token.
    #[error("unauthorized")]
    Unauthorized,
    /// 404 on `/api/v2/app/info`: Eagle is older than 4.0.
    #[error("Eagle is older than 4.0")]
    ApiTooOld,
    /// `library/info` without folders: no library is open on the PC.
    #[error("no library is open")]
    NoLibraryOpen,
    /// The source rejected the call (Eagle: JSend fail/error with HTTP 200 or 500).
    #[error("{message}")]
    Rejected { http: u16, message: String },
    #[error("not found")]
    NotFound,
    /// The source could not parse what we sent (Eagle: HTTP 400).
    #[error("bad request")]
    BadRequest,
    /// Rejected by our own checks before any request.
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("network: {0}")]
    Network(NetworkError),
    #[error("this library is read-only")]
    ReadOnly,
    #[error("not supported: {0}")]
    NotSupported(String),
    /// Database or file system errors of local sources.
    #[error("storage: {0}")]
    Storage(String),
    /// The folder is not an egl-viewer library. `eagle` when it looks like an Eagle library.
    #[error("not an egl-viewer library")]
    NotALibrary { eagle: bool },
    /// Made by a newer egl-viewer: the format version is above the newest this build knows.
    #[error("made by a newer egl-viewer (format version {version})")]
    NewerFormat { version: u32 },
    /// A library cannot live there (D-021).
    #[error("{0}")]
    UnsupportedLocation(LocationProblem),
    #[error("already exists")]
    AlreadyExists,
    /// The password or recovery key of a locked folder does not match.
    #[error("wrong password")]
    WrongPassword,
    /// The write needs a locked folder unlocked first; `folder` is its name.
    #[error("unlock {folder} first")]
    Locked { folder: String },
}

/// Why a folder cannot hold a library (docs/spec.md §3, D-021).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LocationProblem {
    /// Inside a folder that a cloud sync tool copies (OneDrive, Dropbox, Syncthing, …).
    #[error("the folder is synced by a cloud service")]
    SyncFolder,
    /// A network share: SQLite's WAL mode needs a local disk.
    #[error("the folder is on a network drive")]
    NetworkPath,
    /// Inside another egl-viewer library.
    #[error("the folder is inside another library")]
    InsideLibrary,
    /// Inside an Eagle `.library` folder, which only Eagle writes.
    #[error("the folder is inside an Eagle library")]
    InsideEagleLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NetworkError {
    /// Almost always the firewall on the Eagle PC.
    #[error("connect timeout")]
    ConnectTimeout,
    /// Eagle is not running, or the port is wrong.
    #[error("connection refused")]
    Refused,
    /// Wrong network or address.
    #[error("unreachable")]
    Unreachable,
    /// The operating system denied local network access (later: Android 17+, iOS).
    #[error("local network access denied")]
    LocalNetworkDenied,
    #[error("{0}")]
    Other(String),
}

impl From<NetworkError> for SourceError {
    fn from(error: NetworkError) -> Self {
        Self::Network(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unit_kinds_have_no_detail() {
        assert_eq!(
            serde_json::to_value(SourceError::Unauthorized).unwrap(),
            json!({ "kind": "unauthorized" })
        );
        assert_eq!(
            serde_json::to_value(SourceError::ApiTooOld).unwrap(),
            json!({ "kind": "apiTooOld" })
        );
    }

    #[test]
    fn rejected_carries_status_and_message() {
        let error = SourceError::Rejected {
            http: 500,
            message: "name too long (max 10000)".into(),
        };
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({ "kind": "rejected", "detail": { "http": 500, "message": "name too long (max 10000)" } })
        );
        assert_eq!(error.to_string(), "name too long (max 10000)");
    }

    #[test]
    fn library_errors_carry_their_detail() {
        assert_eq!(
            serde_json::to_value(SourceError::NotALibrary { eagle: true }).unwrap(),
            json!({ "kind": "notALibrary", "detail": { "eagle": true } })
        );
        assert_eq!(
            serde_json::to_value(SourceError::NewerFormat { version: 3 }).unwrap(),
            json!({ "kind": "newerFormat", "detail": { "version": 3 } })
        );
        let error = SourceError::UnsupportedLocation(LocationProblem::SyncFolder);
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({ "kind": "unsupportedLocation", "detail": "syncFolder" })
        );
        assert_eq!(error.to_string(), "the folder is synced by a cloud service");
    }

    #[test]
    fn network_errors_nest_under_detail() {
        let error: SourceError = NetworkError::Refused.into();
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({ "kind": "network", "detail": "refused" })
        );
        assert_eq!(error.to_string(), "network: connection refused");
    }
}
