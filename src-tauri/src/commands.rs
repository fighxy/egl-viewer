//! IPC commands. Each one parses input, calls the core and returns a UI-shaped result.

use app_store::ConnectionSettings;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME").to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}

/// The values the Connect screen starts with: Eagle on this PC.
#[tauri::command]
pub fn connection_defaults() -> ConnectionSettings {
    ConnectionSettings::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_reports_the_crate() {
        let info = app_info();
        assert_eq!(info.name, "egl-viewer");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn connection_defaults_point_at_this_pc() {
        let defaults = connection_defaults();
        assert_eq!(defaults.port, 41595);
        assert!(defaults.is_same_pc());
    }

    #[test]
    fn app_info_is_camel_case_on_the_wire() {
        let json = serde_json::to_value(app_info()).unwrap();
        assert!(json.get("name").is_some() && json.get("version").is_some());
    }
}
