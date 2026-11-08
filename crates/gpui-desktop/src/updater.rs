//! GPUI desktop update check (D-133) — not `tauri-plugin-updater`.
//!
//! Fetches `latest-gpui.json` from GitHub Releases, compares SemVer, and returns
//! a download URL. Install is currently “open the download URL” (signed
//! download/install/relaunch and minisign verification are deferred).

use serde_json::Value;

/// Default endpoint for the GPUI portable channel (parallel to Tauri `latest.json`).
pub const DEFAULT_ENDPOINT: &str =
    "https://github.com/fighxy/egl-viewer/releases/latest/download/latest-gpui.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub current_version: String,
    pub version: String,
    pub notes: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    UpToDate { current: String },
    Update(AvailableUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    Network(String),
    Parse(String),
    MissingPlatform,
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(msg) => write!(f, "{msg}"),
            Self::Parse(msg) => write!(f, "{msg}"),
            Self::MissingPlatform => write!(f, "No Windows GPUI download in the update manifest"),
        }
    }
}

/// Parse `x.y.z` (optional leading `v`); returns None when not a simple SemVer triple.
pub fn parse_version(raw: &str) -> Option<(u64, u64, u64)> {
    let s = raw.trim().trim_start_matches('v');
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        // Refuse pre-release / build metadata for the first GPUI channel.
        return None;
    }
    Some((major, minor, patch))
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_version(remote), parse_version(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

/// Interprets a `latest-gpui.json` (or same-shaped) body for Windows x86_64.
pub fn outcome_from_manifest(current: &str, body: &str) -> Result<CheckOutcome, CheckError> {
    let value: Value =
        serde_json::from_str(body).map_err(|e| CheckError::Parse(format!("JSON: {e}")))?;
    let version = value
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CheckError::Parse("missing version".into()))?
        .to_string();
    let notes = value
        .get("notes")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let platform = value
        .pointer("/platforms/windows-x86_64")
        .or_else(|| value.pointer("/platforms/windows-x86_64-gpui"));
    let url = platform
        .and_then(|p| p.get("url"))
        .and_then(|u| u.as_str())
        .filter(|u| !u.is_empty())
        .ok_or(CheckError::MissingPlatform)?
        .to_string();

    if !is_newer(&version, current) {
        return Ok(CheckOutcome::UpToDate {
            current: current.to_string(),
        });
    }
    Ok(CheckOutcome::Update(AvailableUpdate {
        current_version: current.to_string(),
        version,
        notes,
        url,
    }))
}

/// Blocking HTTPS GET of the update manifest. Call from a worker thread.
pub fn check_latest(current: &str, endpoint: &str) -> Result<CheckOutcome, CheckError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("egl-viewer-gpui/{current}"))
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| CheckError::Network(e.to_string()))?;
    let response = client
        .get(endpoint)
        .send()
        .map_err(|e| CheckError::Network(e.to_string()))?;
    if !response.status().is_success() {
        return Err(CheckError::Network(format!(
            "HTTP {}",
            response.status()
        )));
    }
    let body = response
        .text()
        .map_err(|e| CheckError::Network(e.to_string()))?;
    outcome_from_manifest(current, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_semver() {
        assert_eq!(parse_version("0.2.77"), Some((0, 2, 77)));
        assert_eq!(parse_version("v1.0.0"), Some((1, 0, 0)));
        assert!(parse_version("1.0.0-beta").is_none());
    }

    #[test]
    fn newer_compares_patch() {
        assert!(is_newer("0.2.78", "0.2.77"));
        assert!(!is_newer("0.2.77", "0.2.77"));
        assert!(!is_newer("0.2.76", "0.2.77"));
    }

    #[test]
    fn manifest_update_and_uptodate() {
        let body = r#"{
          "version": "0.2.80",
          "notes": "GPUI portable",
          "platforms": {
            "windows-x86_64": { "url": "https://example.test/gpui.zip", "signature": "" }
          }
        }"#;
        match outcome_from_manifest("0.2.77", body).unwrap() {
            CheckOutcome::Update(u) => {
                assert_eq!(u.version, "0.2.80");
                assert_eq!(u.url, "https://example.test/gpui.zip");
                assert_eq!(u.notes, "GPUI portable");
            }
            other => panic!("expected update, got {other:?}"),
        }
        assert!(matches!(
            outcome_from_manifest("0.2.80", body).unwrap(),
            CheckOutcome::UpToDate { .. }
        ));
    }

    #[test]
    fn missing_platform_errors() {
        let body = r#"{"version":"9.0.0","platforms":{}}"#;
        assert!(matches!(
            outcome_from_manifest("0.1.0", body),
            Err(CheckError::MissingPlatform)
        ));
    }
}
