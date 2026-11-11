//! GPUI desktop update check and Windows install handoff (D-133 / D-153).
//!
//! Fetches `latest-gpui.json` from the rolling `gpui-latest` prerelease (never
//! GitHub Latest — that channel belongs to Tauri's `latest.json`). On Windows,
//! after the user confirms, downloads the NSIS setup, exits, runs it silently,
//! and relaunches. Minisign verification remains deferred (empty signature).

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Rolling GPUI channel: `gpui-latest` prerelease asset, not `/releases/latest/`.
pub const DEFAULT_ENDPOINT: &str =
    "https://github.com/fighxy/egl-viewer/releases/download/gpui-latest/latest-gpui.json";

/// Per-user install directory (must stay distinct from Tauri's `egl-viewer`).
pub const INSTALL_DIR_NAME: &str = "egl-viewer-gpui";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableUpdate {
    pub current_version: String,
    pub version: String,
    pub notes: String,
    /// Preferred download for this host (NSIS setup on Windows).
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
            Self::MissingPlatform => {
                write!(f, "No download for this platform in the update manifest")
            }
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

/// Platform key used when reading the manifest on this host.
pub fn host_platform_key() -> &'static str {
    #[cfg(windows)]
    {
        "windows-x86_64"
    }
    #[cfg(target_os = "linux")]
    {
        "linux-x86_64"
    }
    #[cfg(target_os = "macos")]
    {
        "macos-aarch64"
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        "windows-x86_64"
    }
}

/// Interprets a `latest-gpui.json` body for the current host platform.
pub fn outcome_from_manifest(current: &str, body: &str) -> Result<CheckOutcome, CheckError> {
    outcome_from_manifest_for(current, body, host_platform_key())
}

pub fn outcome_from_manifest_for(
    current: &str,
    body: &str,
    platform_key: &str,
) -> Result<CheckOutcome, CheckError> {
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
    let platforms = value
        .get("platforms")
        .ok_or_else(|| CheckError::Parse("missing platforms".into()))?;
    let platform = platforms
        .get(platform_key)
        .or_else(|| {
            if platform_key == "windows-x86_64" {
                platforms.get("windows-x86_64-gpui")
            } else {
                None
            }
        })
        .ok_or(CheckError::MissingPlatform)?;
    let url = platform
        .get("url")
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
        return Err(CheckError::Network(format!("HTTP {}", response.status())));
    }
    let body = response
        .text()
        .map_err(|e| CheckError::Network(e.to_string()))?;
    outcome_from_manifest(current, &body)
}

/// Default per-user install directory for the GPUI NSIS layout.
pub fn default_install_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Programs")
            .join(INSTALL_DIR_NAME)
    }
    #[cfg(not(windows))]
    {
        std::env::temp_dir().join(INSTALL_DIR_NAME)
    }
}

/// Whether this host can download the NSIS setup, exit, install silently, and relaunch.
pub fn supports_silent_install() -> bool {
    cfg!(windows)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    /// Built on the Windows download path (`download_to`); kept for all targets.
    #[allow(dead_code)]
    Network(String),
    /// Built on the Windows download / handoff path; kept for all targets.
    #[allow(dead_code)]
    Io(String),
    Spawn(String),
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(msg) | Self::Io(msg) | Self::Spawn(msg) => write!(f, "{msg}"),
        }
    }
}

/// Downloads `url` to `dest`, replacing any existing file.
/// Used by the Windows silent-install handoff; compiled on all targets for tests.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn download_to(url: &str, dest: &Path) -> Result<(), ApplyError> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ApplyError::Io(e.to_string()))?;
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("egl-viewer-gpui/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| ApplyError::Network(e.to_string()))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| ApplyError::Network(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ApplyError::Network(format!("HTTP {}", response.status())));
    }
    let mut file = std::fs::File::create(dest).map_err(|e| ApplyError::Io(e.to_string()))?;
    std::io::copy(&mut response, &mut file).map_err(|e| ApplyError::Io(e.to_string()))?;
    Ok(())
}

/// Windows: download the NSIS setup, spawn a handoff that waits for this process to
/// exit, runs the installer silently, and relaunches. Call `shutdown` + remove the
/// window afterwards. Non-Windows: returns an error — open the URL instead.
pub fn prepare_silent_install_and_relaunch(url: &str) -> Result<PathBuf, ApplyError> {
    #[cfg(windows)]
    {
        prepare_silent_install_and_relaunch_windows(url)
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err(ApplyError::Spawn(
            "silent install is only implemented on Windows".into(),
        ))
    }
}

#[cfg(windows)]
fn prepare_silent_install_and_relaunch_windows(url: &str) -> Result<PathBuf, ApplyError> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;

    let temp = std::env::temp_dir();
    let setup = temp.join(format!(
        "egl-viewer-gpui-setup-{}-{}.exe",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    download_to(url, &setup)?;

    let install_dir = default_install_dir();
    let exe = install_dir.join("gpui-desktop.exe");
    let bat = temp.join(format!(
        "egl-viewer-gpui-relaunch-{}.cmd",
        std::process::id()
    ));
    // Wait until this PID is gone so the running exe can be overwritten, then
    // silent-install and relaunch. Delete the helper script afterwards.
    let script = format!(
        "@echo off\r\n\
setlocal\r\n\
set \"PID={pid}\"\r\n\
set \"SETUP={setup}\"\r\n\
set \"EXE={exe}\"\r\n\
:wait\r\n\
tasklist /FI \"PID eq %PID%\" 2>nul | find \"%PID%\" >nul\r\n\
if not errorlevel 1 (\r\n\
  ping 127.0.0.1 -n 2 >nul\r\n\
  goto wait\r\n\
)\r\n\
\"%SETUP%\" /S\r\n\
if errorlevel 1 exit /b 1\r\n\
if exist \"%EXE%\" start \"\" \"%EXE%\"\r\n\
del \"%SETUP%\" >nul 2>&1\r\n\
del \"%~f0\" >nul 2>&1\r\n",
        pid = std::process::id(),
        setup = setup.display(),
        exe = exe.display(),
    );
    std::fs::write(&bat, script).map_err(|e| ApplyError::Io(e.to_string()))?;

    // Quote the .cmd path so usernames with spaces still work under `start`.
    let bat_arg = format!("\"{}\"", bat.display());
    Command::new("cmd")
        .args(["/C", "start", "", "/MIN", &bat_arg])
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map_err(|e| ApplyError::Spawn(e.to_string()))?;

    Ok(setup)
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
    fn default_endpoint_uses_gpui_latest_not_github_latest() {
        assert!(DEFAULT_ENDPOINT.contains("/releases/download/gpui-latest/"));
        assert!(!DEFAULT_ENDPOINT.contains("/releases/latest/"));
    }

    #[test]
    fn install_dir_name_is_not_tauri() {
        assert_eq!(INSTALL_DIR_NAME, "egl-viewer-gpui");
        let dir = default_install_dir();
        assert!(dir.ends_with(INSTALL_DIR_NAME));
        assert!(!dir.ends_with(std::path::Path::new("Programs").join("egl-viewer")));
    }

    #[test]
    fn manifest_update_and_uptodate() {
        let body = r#"{
          "version": "0.2.80",
          "notes": "GPUI setup",
          "platforms": {
            "windows-x86_64": { "url": "https://example.test/setup.exe", "signature": "" }
          }
        }"#;
        match outcome_from_manifest_for("0.2.77", body, "windows-x86_64").unwrap() {
            CheckOutcome::Update(u) => {
                assert_eq!(u.version, "0.2.80");
                assert_eq!(u.url, "https://example.test/setup.exe");
                assert_eq!(u.notes, "GPUI setup");
            }
            other => panic!("expected update, got {other:?}"),
        }
        assert!(matches!(
            outcome_from_manifest_for("0.2.80", body, "windows-x86_64").unwrap(),
            CheckOutcome::UpToDate { .. }
        ));
    }

    #[test]
    fn missing_platform_errors() {
        let body = r#"{"version":"9.0.0","platforms":{}}"#;
        assert!(matches!(
            outcome_from_manifest_for("0.1.0", body, "windows-x86_64"),
            Err(CheckError::MissingPlatform)
        ));
    }

    #[test]
    fn linux_and_macos_keys_are_read() {
        let body = r#"{
          "version": "1.0.0",
          "platforms": {
            "linux-x86_64": { "url": "https://example.test/linux.tgz", "signature": "" },
            "macos-aarch64": { "url": "https://example.test/mac.zip", "signature": "" }
          }
        }"#;
        match outcome_from_manifest_for("0.1.0", body, "linux-x86_64").unwrap() {
            CheckOutcome::Update(u) => assert!(u.url.ends_with("linux.tgz")),
            other => panic!("{other:?}"),
        }
        match outcome_from_manifest_for("0.1.0", body, "macos-aarch64").unwrap() {
            CheckOutcome::Update(u) => assert!(u.url.ends_with("mac.zip")),
            other => panic!("{other:?}"),
        }
    }
}
