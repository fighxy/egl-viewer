//! Where to find Eagle. The token is not here: it lives in the OS secret store (§10).

use std::net::IpAddr;

use library_core::SourceError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ConnectionSettings {
    pub host: String,
    pub port: u16,
}

impl Default for ConnectionSettings {
    /// Eagle on the same PC.
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 41595,
        }
    }
}

impl ConnectionSettings {
    pub fn validate(&self) -> Result<(), SourceError> {
        let host = self.host.trim();
        if host.is_empty() {
            return Err(SourceError::Invalid("host must not be empty".into()));
        }
        if host.contains(char::is_whitespace) || host.contains('/') {
            return Err(SourceError::Invalid(
                "host must be a name or an address".into(),
            ));
        }
        if self.port == 0 {
            return Err(SourceError::Invalid(
                "port must be between 1 and 65535".into(),
            ));
        }
        Ok(())
    }

    /// Eagle runs on this PC: files are added by path and thumbnails are read from disk
    /// (docs/architecture.md §7.4, §11).
    pub fn is_same_pc(&self) -> bool {
        let host = self
            .host
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']');
        host.eq_ignore_ascii_case("localhost")
            || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
    }

    /// Tailscale addresses (100.64.0.0/10) and MagicDNS names get a longer connect timeout
    /// (docs/architecture.md §4.3).
    pub fn is_tailscale(&self) -> bool {
        let host = self.host.trim();
        if host.to_ascii_lowercase().ends_with(".ts.net") {
            return true;
        }
        match host.parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) => {
                let [a, b, ..] = ip.octets();
                a == 100 && (64..=127).contains(&b)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(host: &str) -> ConnectionSettings {
        ConnectionSettings {
            host: host.into(),
            port: 41595,
        }
    }

    #[test]
    fn defaults_to_eagle_on_this_pc() {
        let settings = ConnectionSettings::default();
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 41595);
        assert!(settings.is_same_pc());
        assert_eq!(settings.validate(), Ok(()));
    }

    #[test]
    fn rejects_bad_hosts_and_port_zero() {
        assert!(at("").validate().is_err());
        assert!(at("   ").validate().is_err());
        assert!(at("my pc").validate().is_err());
        assert!(at("http://pc").validate().is_err());
        assert!(
            ConnectionSettings {
                host: "pc".into(),
                port: 0
            }
            .validate()
            .is_err()
        );
        assert_eq!(at("desktop-pc").validate(), Ok(()));
        assert_eq!(at("192.168.1.20").validate(), Ok(()));
    }

    #[test]
    fn recognises_the_same_pc() {
        for host in [
            "127.0.0.1",
            "localhost",
            "LOCALHOST",
            "::1",
            "[::1]",
            " 127.0.0.1 ",
        ] {
            assert!(at(host).is_same_pc(), "{host}");
        }
        for host in ["192.168.1.20", "desktop-pc", "100.101.102.103"] {
            assert!(!at(host).is_same_pc(), "{host}");
        }
    }

    #[test]
    fn recognises_tailscale() {
        for host in ["100.64.0.1", "100.127.255.254", "pc.tail1234.ts.net"] {
            assert!(at(host).is_tailscale(), "{host}");
        }
        for host in ["100.63.0.1", "100.128.0.1", "192.168.1.20", "127.0.0.1"] {
            assert!(!at(host).is_tailscale(), "{host}");
        }
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let settings: ConnectionSettings = serde_json::from_str(r#"{ "host": "pc" }"#).unwrap();
        assert_eq!(settings.port, 41595);
    }
}
