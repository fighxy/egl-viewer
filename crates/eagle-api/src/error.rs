//! Errors of the Eagle transport, mapped one to one onto `SourceError` (docs/architecture.md
//! §4.4, D-004).

use library_core::{NetworkError, SourceError};
use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, TS)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
#[ts(export)]
pub enum EagleError {
    #[error("not connected")]
    NotConnected,
    /// HTTP 401: wrong or missing token.
    #[error("unauthorized")]
    Unauthorized,
    /// 404 on `/api/v2/app/info`: Eagle is older than 4.0.
    #[error("Eagle is older than 4.0")]
    ApiTooOld,
    /// `library/info` without folders: no library is open on the PC.
    #[error("no library is open")]
    NoLibraryOpen,
    /// JSend `fail`/`error` with HTTP 200 or 500.
    #[error("{message}")]
    Rejected { http: u16, message: String },
    /// `success`, but `data` is `false` or missing: the object is not there.
    #[error("not found")]
    NotFound,
    /// Eagle could not parse our JSON (HTTP 400).
    #[error("bad request")]
    BadRequest,
    /// Rejected by our own checks before any request.
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("network: {0}")]
    Network(NetworkError),
}

impl From<EagleError> for SourceError {
    fn from(error: EagleError) -> Self {
        match error {
            EagleError::NotConnected => SourceError::NotConnected,
            EagleError::Unauthorized => SourceError::Unauthorized,
            EagleError::ApiTooOld => SourceError::ApiTooOld,
            EagleError::NoLibraryOpen => SourceError::NoLibraryOpen,
            EagleError::Rejected { http, message } => SourceError::Rejected { http, message },
            EagleError::NotFound => SourceError::NotFound,
            EagleError::BadRequest => SourceError::BadRequest,
            EagleError::Invalid(detail) => SourceError::Invalid(detail),
            EagleError::Network(detail) => SourceError::Network(detail),
        }
    }
}

/// A `reqwest` error as the connect screen explains it (docs/architecture.md §4.4).
pub(crate) fn network_error(error: &reqwest::Error) -> NetworkError {
    use std::io::ErrorKind;
    if error.is_connect() && error.is_timeout() {
        return NetworkError::ConnectTimeout;
    }
    let mut source: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(error);
    while let Some(error) = source {
        if let Some(io) = error.downcast_ref::<std::io::Error>() {
            return match io.kind() {
                ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset => NetworkError::Refused,
                ErrorKind::HostUnreachable | ErrorKind::NetworkUnreachable => {
                    NetworkError::Unreachable
                }
                ErrorKind::PermissionDenied => NetworkError::LocalNetworkDenied,
                ErrorKind::TimedOut => NetworkError::ConnectTimeout,
                _ => NetworkError::Other(io.to_string()),
            };
        }
        source = std::error::Error::source(error);
    }
    if error.is_timeout() {
        return NetworkError::ConnectTimeout;
    }
    if error.is_connect() {
        return NetworkError::Other(error.to_string());
    }
    NetworkError::Other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_one_to_one_onto_source_error() {
        assert_eq!(
            SourceError::from(EagleError::Unauthorized),
            SourceError::Unauthorized
        );
        assert_eq!(
            SourceError::from(EagleError::ApiTooOld),
            SourceError::ApiTooOld
        );
        assert_eq!(
            SourceError::from(EagleError::NoLibraryOpen),
            SourceError::NoLibraryOpen
        );
        assert_eq!(
            SourceError::from(EagleError::NotFound),
            SourceError::NotFound
        );
        assert_eq!(
            SourceError::from(EagleError::BadRequest),
            SourceError::BadRequest
        );
        assert_eq!(
            SourceError::from(EagleError::Rejected {
                http: 500,
                message: "name too long (max 10000)".into()
            }),
            SourceError::Rejected {
                http: 500,
                message: "name too long (max 10000)".into()
            }
        );
        assert_eq!(
            SourceError::from(EagleError::Network(NetworkError::Refused)),
            SourceError::Network(NetworkError::Refused)
        );
        let rejected = EagleError::Rejected {
            http: 500,
            message: "name too long (max 10000)".into(),
        };
        assert_eq!(
            serde_json::to_value(&rejected).unwrap(),
            json!({ "kind": "rejected", "detail": { "http": 500, "message": "name too long (max 10000)" } })
        );
    }
}
