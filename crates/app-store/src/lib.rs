//! Settings, recent libraries and secrets (docs/architecture.md §3).
//!
//! Phase 0 holds the Eagle connection settings. Recent libraries arrive in phase 1, the secret
//! store for the Eagle token (Windows Credential Manager) in phase 9 (docs/spec.md §9).

pub mod settings;

pub use settings::ConnectionSettings;
