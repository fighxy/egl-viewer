//! Pure helpers for export/backup/decrypt: selection, formats, passphrase rules,
//! Eagle connect validation and the app login gate.

use app_store::{ConnectionSettings, LoginMethod};
use library_core::{
    Compression, ExportFormat, ExportRequest, ExportSelection, ExportState, MIN_PASSPHRASE_CHARS,
    ZipEncryption,
};

/// How Backup / Export was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportPreset {
    /// Library → Backup… — free scope starts as whole library.
    Backup,
    Folder {
        id: String,
        name: String,
    },
    Tag {
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    Mirror,
    Zip,
    SevenZip,
    Eagle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginGate {
    /// Checking `login_status` before first paint of Welcome.
    Pending,
    Locked {
        method: LoginMethod,
    },
    Open,
}

impl LoginGate {
    pub fn from_status(method: LoginMethod) -> Self {
        match method {
            LoginMethod::None => Self::Open,
            other => Self::Locked { method: other },
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }
}

/// Builds the selection the export dialog sends to Session.
pub fn export_selection(preset: &ExportPreset, whole_only: bool) -> ExportSelection {
    match preset {
        ExportPreset::Backup if whole_only => ExportSelection {
            whole: true,
            folders: Vec::new(),
            tags: Vec::new(),
        },
        ExportPreset::Backup => ExportSelection {
            whole: true,
            folders: Vec::new(),
            tags: Vec::new(),
        },
        ExportPreset::Folder { id, .. } => ExportSelection {
            whole: false,
            folders: vec![id.clone()],
            tags: Vec::new(),
        },
        ExportPreset::Tag { name } => ExportSelection {
            whole: false,
            folders: Vec::new(),
            tags: vec![name.clone()],
        },
    }
}

pub fn export_format(
    kind: FormatKind,
    compression: Compression,
    zip_enc: ZipEncryption,
) -> ExportFormat {
    match kind {
        FormatKind::Mirror => ExportFormat::Mirror,
        FormatKind::Zip => ExportFormat::Zip {
            compression,
            encryption: zip_enc,
        },
        FormatKind::SevenZip => ExportFormat::SevenZip { compression },
        FormatKind::Eagle => ExportFormat::Mirror, // ignored when eagle target is set
    }
}

/// Passphrase rules for encrypted archives (D-110 / D-112 / D-113).
pub fn passphrase_problem(encrypt: bool, password: &str, confirm: &str) -> Option<&'static str> {
    if !encrypt {
        return None;
    }
    if password.is_empty() {
        return Some("Type a password.");
    }
    if password.chars().count() < MIN_PASSPHRASE_CHARS {
        return Some("The password needs at least 8 characters.");
    }
    if password != confirm {
        return Some("The passwords do not match.");
    }
    None
}

pub fn validate_connect_host_port(host: &str, port: &str) -> Result<ConnectionSettings, String> {
    let host = host.trim().to_owned();
    if host.is_empty() {
        return Err("Enter the address of the PC that runs Eagle.".into());
    }
    if host.contains(char::is_whitespace) || host.contains('/') {
        return Err("Enter a computer name or an IP address, without spaces or slashes.".into());
    }
    let port: u16 = port
        .trim()
        .parse()
        .map_err(|_| "The port is a number from 1 to 65535.".to_string())?;
    if port == 0 {
        return Err("The port is a number from 1 to 65535.".into());
    }
    let settings = ConnectionSettings { host, port };
    settings.validate().map_err(|e| e.to_string())?;
    Ok(settings)
}

pub fn build_archive_request(
    selection: ExportSelection,
    include_subfolders: bool,
    format: ExportFormat,
    destination: String,
    encrypt: bool,
    password: String,
) -> ExportRequest {
    ExportRequest {
        selection,
        include_subfolders,
        format,
        destination,
        encrypt: encrypt.then_some(password),
        eagle: None,
    }
}

pub fn export_state_label(state: ExportState) -> &'static str {
    match state {
        ExportState::Preparing => "Preparing…",
        ExportState::Copying => "Copying…",
        ExportState::Encrypting => "Encrypting…",
        ExportState::Done => "Done",
        ExportState::Failed => "Failed",
        ExportState::Cancelled => "Cancelled",
    }
}

pub fn is_terminal_export(state: ExportState) -> bool {
    matches!(
        state,
        ExportState::Done | ExportState::Failed | ExportState::Cancelled
    )
}

/// Default destination for a decrypted `.zip.age`: sibling folder of the archive.
pub fn decrypt_default_folder(source: &str) -> String {
    let path = std::path::Path::new(source);
    path.parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| source.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use library_core::Compression;

    #[test]
    fn login_gate_opens_when_none() {
        assert!(LoginGate::from_status(LoginMethod::None).is_open());
        assert!(!LoginGate::from_status(LoginMethod::Password).is_open());
        assert!(!LoginGate::from_status(LoginMethod::WebAuthn).is_open());
    }

    #[test]
    fn selection_from_presets() {
        let whole = export_selection(&ExportPreset::Backup, true);
        assert!(whole.whole);
        let folder = export_selection(
            &ExportPreset::Folder {
                id: "f1".into(),
                name: "A".into(),
            },
            true,
        );
        assert_eq!(folder.folders, vec!["f1"]);
        let tag = export_selection(&ExportPreset::Tag { name: "x".into() }, true);
        assert_eq!(tag.tags, vec!["x"]);
    }

    #[test]
    fn passphrase_rules() {
        assert!(passphrase_problem(false, "", "").is_none());
        assert_eq!(passphrase_problem(true, "", ""), Some("Type a password."));
        assert!(passphrase_problem(true, "short", "short").is_some());
        assert_eq!(
            passphrase_problem(true, "longenough", "different"),
            Some("The passwords do not match.")
        );
        assert!(passphrase_problem(true, "longenough", "longenough").is_none());
    }

    #[test]
    fn connect_validation() {
        assert!(validate_connect_host_port("127.0.0.1", "41595").is_ok());
        assert!(validate_connect_host_port("", "41595").is_err());
        assert!(validate_connect_host_port("bad host", "41595").is_err());
        assert!(validate_connect_host_port("127.0.0.1", "0").is_err());
        assert!(validate_connect_host_port("127.0.0.1", "abc").is_err());
    }

    #[test]
    fn format_builders() {
        assert!(matches!(
            export_format(FormatKind::Zip, Compression::Balanced, ZipEncryption::Age),
            ExportFormat::Zip {
                encryption: ZipEncryption::Age,
                ..
            }
        ));
        assert!(matches!(
            export_format(FormatKind::SevenZip, Compression::Fast, ZipEncryption::Aes),
            ExportFormat::SevenZip { .. }
        ));
    }

    #[test]
    fn decrypt_folder_of_archive() {
        let folder = decrypt_default_folder(r"C:\Exports\lib.zip.age");
        assert!(folder.ends_with("Exports") || folder.contains("Exports"));
    }

    #[test]
    fn terminal_export_states() {
        assert!(is_terminal_export(ExportState::Done));
        assert!(!is_terminal_export(ExportState::Copying));
    }
}
