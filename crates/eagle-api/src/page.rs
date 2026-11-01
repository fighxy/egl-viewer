//! Pagination: the nested wire shape and the loop that follows it (docs/spec.md §8.3
//! "Pagination", docs/architecture.md §4.5).

use std::collections::HashSet;
use std::future::Future;

use serde::Deserialize;
use serde_json::Value;

use crate::error::EagleError;

/// The slice the server re-wraps: `body.data` is `{ data, total, offset, limit }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub data: Vec<T>,
    pub total: usize,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub limit: usize,
}

/// Decodes a `Page<T>` from the `data` value of a JSend body.
pub fn decode_page<T: for<'de> Deserialize<'de>>(data: Value) -> Result<Page<T>, EagleError> {
    serde_json::from_value(data)
        .map_err(|error| EagleError::Invalid(format!("unexpected page shape: {error}")))
}

/// Pages through a list endpoint with `limit=1000`, sequentially, restarting from offset 0
/// when `total` changes and deduplicating by id: the server recomputes every page, so a
/// concurrent import shifts offsets (docs/architecture.md §4.5).
pub async fn page_all<T, F, Fut, K>(
    mut fetch: F,
    key: impl Fn(&T) -> K,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<T>, EagleError>
where
    F: FnMut(usize) -> Fut,
    Fut: Future<Output = Result<Page<T>, EagleError>>,
    K: AsRef<str> + Eq + std::hash::Hash,
{
    const ATTEMPTS: usize = 3;
    for _attempt in 0..ATTEMPTS {
        let mut order: Vec<T> = Vec::new();
        let mut seen: HashSet<K> = HashSet::new();
        let mut offset = 0usize;
        let mut total = None;
        loop {
            let page = fetch(offset).await?;
            match total {
                None => total = Some(page.total),
                Some(expected) if expected != page.total => break, // changed while loading
                _ => {}
            }
            let received = page.data.len();
            for item in page.data {
                if seen.insert(key(&item)) {
                    order.push(item);
                }
            }
            offset += received;
            progress(order.len(), page.total);
            if offset >= page.total || received == 0 {
                return Ok(order);
            }
        }
    }
    Err(EagleError::Rejected {
        http: 200,
        message: "the library keeps changing while it loads".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(ids: &[u32], total: usize, offset: usize) -> Page<u32> {
        Page {
            data: ids.to_vec(),
            total,
            offset,
            limit: 1000,
        }
    }

    fn ids_of(items: &[u32]) -> Vec<u32> {
        items.to_vec()
    }

    #[tokio::test]
    async fn loops_until_offset_reaches_total() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let fetch = {
            let calls = std::sync::Arc::clone(&calls);
            move |offset: usize| {
                let calls = std::sync::Arc::clone(&calls);
                async move {
                    *calls.lock().unwrap() += 1;
                    Ok(match offset {
                        0 => page(&[1, 2], 4, 0),
                        2 => page(&[3, 4], 4, 2),
                        _ => panic!("unexpected offset {offset}"),
                    })
                }
            }
        };
        let loaded = page_all(fetch, |id| id.to_string(), |_, _| {})
            .await
            .unwrap();
        assert_eq!(ids_of(&loaded), [1, 2, 3, 4]);
        assert_eq!(*calls.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn deduplicates_a_shifted_offset_within_one_pass() {
        // The server recomputes every page: a concurrent import shifts offsets and item 2
        // arrives twice.
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let fetch = {
            let calls = std::sync::Arc::clone(&calls);
            move |offset: usize| {
                let calls = std::sync::Arc::clone(&calls);
                async move {
                    let call = {
                        let mut calls = calls.lock().unwrap();
                        *calls += 1;
                        *calls
                    };
                    assert_eq!(offset, if call == 1 { 0 } else { 2 });
                    Ok(match call {
                        1 => page(&[1, 2], 4, 0),
                        _ => page(&[2, 3], 4, 2), // shifted: item 2 again
                    })
                }
            }
        };
        let loaded = page_all(fetch, |id| id.to_string(), |_, _| {})
            .await
            .unwrap();
        assert_eq!(ids_of(&loaded), [1, 2, 3]);
    }

    #[tokio::test]
    async fn restarts_when_total_changes() {
        // First pass: offset 0 says total 4, offset 2 sees total 5 → restart. Second pass
        // returns a stable one-item library.
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let fetch = {
            let calls = std::sync::Arc::clone(&calls);
            move |_offset: usize| {
                let calls = std::sync::Arc::clone(&calls);
                async move {
                    let call = {
                        let mut calls = calls.lock().unwrap();
                        *calls += 1;
                        *calls
                    };
                    Ok(match call {
                        1 => page(&[1, 2], 4, 0),
                        2 => page(&[1, 2, 3], 5, 2), // total changed: start over
                        3 => page(&[7], 1, 0),
                        _ => page(&[], 1, 1),
                    })
                }
            }
        };
        let mut reported = Vec::new();
        let loaded = page_all(
            fetch,
            |id| id.to_string(),
            |done, total| {
                reported.push((done, total));
            },
        )
        .await
        .unwrap();
        assert_eq!(ids_of(&loaded), [7]);
        // Pass 1 reported (2, 4); the changed total restarted the loop before its page was
        // counted, and pass 2 reported the stable (1, 1).
        assert_eq!(reported, [(2, 4), (1, 1)]);
        assert!(*calls.lock().unwrap() >= 3);
    }

    #[tokio::test]
    async fn gives_up_after_three_passes() {
        let fetch = |offset: usize| async move {
            Ok(match offset {
                0 => page(&[1], 2, 0),
                _ => page(&[2], 3, 1), // total keeps changing
            })
        };
        let error = page_all(fetch, |id| id.to_string(), |_, _| {})
            .await
            .unwrap_err();
        assert!(matches!(error, EagleError::Rejected { http: 200, .. }));
    }

    #[tokio::test]
    async fn reports_progress() {
        let fetch = |offset: usize| async move {
            Ok(match offset {
                0 => page(&[1, 2], 3, 0),
                _ => page(&[3], 3, 2),
            })
        };
        let mut progress = Vec::new();
        page_all(
            fetch,
            |id| id.to_string(),
            |done, total| {
                progress.push((done, total));
            },
        )
        .await
        .unwrap();
        assert_eq!(progress, [(2, 3), (3, 3)]);
    }
}
