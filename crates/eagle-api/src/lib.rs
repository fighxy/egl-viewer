//! A running Eagle as a `LibrarySource`, through its HTTP API (docs/architecture.md §4).
//!
//! Transport, JSend error model, pagination and the query planner live here; the session
//! connects in phase 9 (docs/spec.md §9).

mod add;
mod api;
mod dto;
mod error;
mod http;
mod page;
mod plan;
mod source;
mod write;

/// Eagle's API port.
pub const DEFAULT_PORT: u16 = 41595;

/// Prefix of the v2 API.
pub const API_V2: &str = "api/v2/";

pub use add::PendingAdd;
pub use api::{EagleApi, GRID_FIELDS};
pub use error::EagleError;
pub use http::{EagleHttp, Endpoint, escape_non_ascii};
pub use page::{Page, decode_page, page_all};
pub use plan::{FetchPlan, LARGE_LIBRARY, ServerFilter, plan};
pub use source::EagleSource;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_eagle() {
        assert_eq!(DEFAULT_PORT, 41595);
        assert!(API_V2.ends_with('/'));
    }
}
