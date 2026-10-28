//! A running Eagle as a `LibrarySource`, through its HTTP API (docs/architecture.md §4).
//!
//! Phase 0 only sets the crate up. The transport (`EagleHttp`, ASCII-escaped bodies, the
//! JSend error model, pagination) arrives in phase 9 (docs/spec.md §8.3, §9).

/// Eagle's API port.
pub const DEFAULT_PORT: u16 = 41595;

/// Prefix of the v2 API.
pub const API_V2: &str = "api/v2/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_eagle() {
        assert_eq!(DEFAULT_PORT, 41595);
        assert!(API_V2.ends_with('/'));
    }
}
