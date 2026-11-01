//! What the network loads for one scope (docs/architecture.md §6.2). `ServerFilter` is exactly
//! what goes into `item/get`, so it is the snapshot cache key.

use library_core::query::{Scope, ScopeHints, TagMode};
use library_core::{ItemLite, type_matches};
use serde::Serialize;

/// Above this, "All" uses safe server prefilters instead of loading everything.
pub const LARGE_LIBRARY: usize = 5000;

/// Exactly the `item/get` filter body. `false` is never serialized: sending `isUnfiled: false`
/// filters to the empty set (spec §8.3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_unfiled: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_untagged: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
}

impl ServerFilter {
    /// Whether `item` still belongs to a snapshot with this filter, checked locally after
    /// a write (docs/architecture.md §6.3). Supports `folders`, `tags`, `isUnfiled`,
    /// `isUntagged` and `ext`.
    pub fn matches_locally(&self, item: &ItemLite) -> bool {
        let folders = item.folders.as_deref().unwrap_or_default();
        if let Some(wanted) = &self.folders
            && !wanted.iter().any(|id| folders.contains(id))
        {
            return false;
        }
        if let Some(wanted) = &self.tags
            && !wanted
                .iter()
                .all(|tag| item.tags.iter().any(|own| **own == **tag))
        {
            return false;
        }
        if self.is_unfiled && !folders.is_empty() {
            return false;
        }
        if self.is_untagged && !item.tags.is_empty() {
            return false;
        }
        if let Some(ext) = &self.ext
            && !type_matches(&item.ext, ext)
        {
            return false;
        }
        true
    }

    /// `rating` and `shape` cannot be re-evaluated locally without extra fields: a snapshot
    /// carrying them is dropped after a write instead of patched.
    pub fn has_unpatchable_filter(&self) -> bool {
        self.rating.is_some() || self.shape.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FetchPlan {
    /// Several filters load several lists, unioned by id in the client.
    ItemGet(Vec<ServerFilter>),
    SmartFolder(String),
}

/// The plan for one scope; the plan is the cache key, so chips, sort and text changes do not
/// touch the network (docs/architecture.md §6.1).
pub fn plan(scope: &Scope, hints: &ScopeHints, library_count: usize) -> FetchPlan {
    let only = |filter: ServerFilter| FetchPlan::ItemGet(vec![filter]);
    match scope {
        Scope::Smart { id } => FetchPlan::SmartFolder(id.clone()),
        Scope::Unfiled => only(ServerFilter {
            is_unfiled: true,
            ..ServerFilter::default()
        }),
        Scope::Untagged => only(ServerFilter {
            is_untagged: true,
            ..ServerFilter::default()
        }),
        // Folder with descendants: the union is computed locally (Eagle's `folders` is an
        // intersection). On a small library that is one unfiltered request.
        Scope::Folder {
            include_descendants: true,
            ..
        } => only(ServerFilter::default()),
        Scope::Folder { id, .. } => only(ServerFilter {
            folders: Some(vec![id.clone()]),
            ..ServerFilter::default()
        }),
        Scope::All if library_count <= LARGE_LIBRARY => only(ServerFilter::default()),
        // Above the threshold only safe prefilters shrink the fetch; without chips the whole
        // library loads (the confirmation dialog of the spec is not built yet, and loading
        // everything stays correct).
        Scope::All => match safe_prefilters(hints) {
            filters if !filters.is_empty() => FetchPlan::ItemGet(filters),
            _ => only(ServerFilter::default()),
        },
        // Rejected with `NotSupported` before planning, as the architecture has it.
        Scope::Trash => only(ServerFilter::default()),
    }
}

/// On a large library, only these prefilters cannot shrink the visible result: a single tag or
/// mode All (intersection is correct), one request per tag in Any mode, or a single
/// ext/rating/shape (docs/spec.md §8.3 "Query plan").
fn safe_prefilters(hints: &ScopeHints) -> Vec<ServerFilter> {
    let mut filters = Vec::new();
    match hints.tags.as_slice() {
        [] => {}
        [only] => filters.push(ServerFilter {
            tags: Some(vec![only.clone()]),
            ..ServerFilter::default()
        }),
        many if hints.tag_mode == TagMode::Any => {
            for tag in many {
                filters.push(ServerFilter {
                    tags: Some(vec![tag.clone()]),
                    ..ServerFilter::default()
                });
            }
        }
        many => filters.push(ServerFilter {
            tags: Some(many.to_vec()),
            ..ServerFilter::default()
        }),
    }
    if let Some(ext) = &hints.ext {
        filters.push(ServerFilter {
            ext: Some(ext.clone()),
            ..ServerFilter::default()
        });
    }
    if let Some(rating) = hints.rating {
        filters.push(ServerFilter {
            rating: Some(rating),
            ..ServerFilter::default()
        });
    }
    if let Some(shape) = &hints.shape {
        filters.push(ServerFilter {
            shape: Some(shape.clone()),
            ..ServerFilter::default()
        });
    }
    filters
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn folder(id: &str, include_descendants: bool) -> Scope {
        Scope::Folder {
            id: id.into(),
            include_descendants,
        }
    }

    #[test]
    fn server_filter_never_sends_false() {
        assert_eq!(
            serde_json::to_value(ServerFilter::default()).unwrap(),
            json!({})
        );
        let filter = ServerFilter {
            is_unfiled: true,
            ..ServerFilter::default()
        };
        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            json!({ "isUnfiled": true })
        );
        let filter = ServerFilter {
            folders: Some(vec!["F1".into()]),
            rating: Some(0),
            ..ServerFilter::default()
        };
        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            json!({ "folders": ["F1"], "rating": 0 })
        );
    }

    #[test]
    fn plans_match_the_spec_table() {
        let none = ScopeHints::default();
        // Folder with descendants: the whole library, union computed locally.
        let planned = plan(&folder("F1", true), &none, 100);
        assert_eq!(planned, FetchPlan::ItemGet(vec![ServerFilter::default()]));
        // Folder without descendants: one id. Never several ids (intersection trap).
        let planned = plan(&folder("F1", false), &none, 100);
        assert_eq!(
            planned,
            FetchPlan::ItemGet(vec![ServerFilter {
                folders: Some(vec!["F1".into()]),
                ..ServerFilter::default()
            }])
        );
        // Smart folders load through smartFolder/getItems.
        let planned = plan(&Scope::Smart { id: "S1".into() }, &none, 100);
        assert_eq!(planned, FetchPlan::SmartFolder("S1".into()));
    }

    #[test]
    fn small_library_all_is_one_unfiltered_request() {
        let planned = plan(&Scope::All, &ScopeHints::default(), LARGE_LIBRARY);
        assert_eq!(planned, FetchPlan::ItemGet(vec![ServerFilter::default()]));
    }

    #[test]
    fn large_library_all_uses_safe_prefilters() {
        let hints = ScopeHints {
            tags: vec!["a".into(), "b".into()],
            tag_mode: TagMode::Any,
            ext: Some("jpg".into()),
            rating: Some(4),
            shape: None,
        };
        let planned = plan(&Scope::All, &hints, LARGE_LIBRARY + 1);
        let FetchPlan::ItemGet(filters) = planned else {
            panic!("expected item/get filters");
        };
        // Any of two tags: one request per tag; plus ext and rating.
        assert_eq!(filters.len(), 4);
        assert!(filters.contains(&ServerFilter {
            tags: Some(vec!["a".into()]),
            ..ServerFilter::default()
        }));
        assert!(filters.contains(&ServerFilter {
            ext: Some("jpg".into()),
            ..ServerFilter::default()
        }));
        assert!(filters.iter().all(|filter| filter.folders.is_none()));
    }

    #[test]
    fn large_library_all_without_chips_loads_everything() {
        let planned = plan(&Scope::All, &ScopeHints::default(), 100_000);
        assert_eq!(planned, FetchPlan::ItemGet(vec![ServerFilter::default()]));
    }

    #[test]
    fn chips_change_the_large_all_plan() {
        let none = ScopeHints::default();
        let tagged = ScopeHints {
            tags: vec!["a".into()],
            ..ScopeHints::default()
        };
        assert_ne!(
            plan(&Scope::All, &none, LARGE_LIBRARY + 1),
            plan(&Scope::All, &tagged, LARGE_LIBRARY + 1)
        );
    }

    #[test]
    fn matches_locally_supports_the_patchable_filters() {
        use library_core::ItemId;
        let item = || ItemLite {
            id: ItemId::from("A1"),
            ext: "jpg".into(),
            tags: ["a".into(), "b".into()].into(),
            folders: Some(vec!["F1".into()]),
            ..ItemLite::default()
        };
        let filter = ServerFilter::default();
        assert!(filter.matches_locally(&item()));
        assert!(
            ServerFilter {
                folders: Some(vec!["F1".into()]),
                ..ServerFilter::default()
            }
            .matches_locally(&item())
        );
        assert!(
            !ServerFilter {
                folders: Some(vec!["F9".into()]),
                ..ServerFilter::default()
            }
            .matches_locally(&item())
        );
        assert!(
            ServerFilter {
                tags: Some(vec!["a".into(), "b".into()]),
                ..ServerFilter::default()
            }
            .matches_locally(&item())
        );
        assert!(
            !ServerFilter {
                tags: Some(vec!["a".into(), "z".into()]),
                ..ServerFilter::default()
            }
            .matches_locally(&item())
        );
        assert!(
            !ServerFilter {
                is_unfiled: true,
                ..ServerFilter::default()
            }
            .matches_locally(&item())
        );
        assert!(
            ServerFilter {
                ext: Some("jpg".into()),
                ..ServerFilter::default()
            }
            .matches_locally(&item())
        );
        assert!(
            !ServerFilter {
                ext: Some("png".into()),
                ..ServerFilter::default()
            }
            .matches_locally(&item())
        );
        // Missing folders (not []) is unfiled for Eagle's API-added items.
        let mut no_folders = item();
        no_folders.folders = None;
        assert!(
            ServerFilter {
                is_unfiled: true,
                ..ServerFilter::default()
            }
            .matches_locally(&no_folders)
        );
        // Untagged and unpatched filters.
        assert!(
            ServerFilter {
                is_untagged: true,
                ..ServerFilter::default()
            }
            .matches_locally(&ItemLite::default())
        );
        assert!(
            ServerFilter {
                rating: Some(0),
                ..ServerFilter::default()
            }
            .has_unpatchable_filter()
        );
        assert!(
            !ServerFilter {
                tags: Some(vec!["a".into()]),
                ..ServerFilter::default()
            }
            .has_unpatchable_filter()
        );
    }
}
