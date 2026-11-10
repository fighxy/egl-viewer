//! Decoded full-size previews of the open item and its neighbours (D-143).
//!
//! Stepping through the preview finds the next image already decoded. The cache owns the
//! decoded pixels; textures of evicted entries come back to the caller, which frees them with
//! `App::drop_image` (GPUI keeps a texture until told otherwise).

use std::sync::Arc;

use gpui_kit::RenderImage;

use crate::browse::preview_image::{PreviewPayload, Shown};

/// An item at a thumbnail version: a regenerated preview means a changed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewKey {
    pub id: String,
    pub version: i64,
}

pub enum Slot {
    Loading,
    /// `shown` is `None` when the payload could not be read or decoded.
    Ready {
        payload: Option<PreviewPayload>,
        shown: Option<Shown>,
    },
}

#[derive(Default)]
pub struct PreviewCache {
    slots: Vec<(PreviewKey, Slot)>,
}

impl PreviewCache {
    pub fn get(&self, key: &PreviewKey) -> Option<&Slot> {
        self.slots
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, slot)| slot)
    }

    /// Marks `key` as loading; `true` when the caller should start the load.
    pub fn begin(&mut self, key: &PreviewKey) -> bool {
        if self.get(key).is_some() {
            return false;
        }
        self.slots.push((key.clone(), Slot::Loading));
        true
    }

    /// Stores a finished load. A key evicted meanwhile is not stored: its pixels come back so
    /// the caller can drop them.
    pub fn finish(
        &mut self,
        key: &PreviewKey,
        payload: Option<PreviewPayload>,
        shown: Option<Shown>,
    ) -> Option<Shown> {
        match self.slots.iter_mut().find(|(k, _)| k == key) {
            Some((_, slot)) => {
                *slot = Slot::Ready { payload, shown };
                None
            }
            None => shown,
        }
    }

    /// Keeps only `keep`; returns the textures of the evicted entries.
    pub fn retain(&mut self, keep: &[PreviewKey]) -> Vec<Arc<RenderImage>> {
        let mut dropped = Vec::new();
        self.slots.retain(|(key, slot)| {
            if keep.contains(key) {
                return true;
            }
            if let Slot::Ready {
                shown: Some(shown), ..
            } = slot
            {
                dropped.extend(shown.pixels().cloned());
            }
            false
        });
        dropped
    }

    pub fn clear(&mut self) -> Vec<Arc<RenderImage>> {
        self.retain(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Frame, RgbaImage};

    fn key(id: &str) -> PreviewKey {
        PreviewKey {
            id: id.into(),
            version: 1,
        }
    }

    fn pixels() -> Shown {
        Shown::Pixels(Arc::new(RenderImage::new(vec![Frame::new(
            RgbaImage::new(1, 1),
        )])))
    }

    #[test]
    fn begins_each_key_once() {
        let mut cache = PreviewCache::default();
        assert!(cache.begin(&key("a")));
        assert!(!cache.begin(&key("a")));
        assert!(matches!(cache.get(&key("a")), Some(Slot::Loading)));
    }

    #[test]
    fn a_new_version_is_a_new_key() {
        let mut cache = PreviewCache::default();
        cache.begin(&key("a"));
        let newer = PreviewKey {
            id: "a".into(),
            version: 2,
        };
        assert!(cache.begin(&newer));
    }

    #[test]
    fn finish_stores_or_returns_the_pixels() {
        let mut cache = PreviewCache::default();
        cache.begin(&key("a"));
        assert!(cache.finish(&key("a"), None, Some(pixels())).is_none());
        assert!(matches!(
            cache.get(&key("a")),
            Some(Slot::Ready { shown: Some(_), .. })
        ));
        // Evicted while loading: nothing stored, pixels handed back.
        assert!(cache.finish(&key("b"), None, Some(pixels())).is_some());
        assert!(cache.get(&key("b")).is_none());
    }

    #[test]
    fn retain_hands_back_evicted_textures() {
        let mut cache = PreviewCache::default();
        for id in ["a", "b", "c"] {
            cache.begin(&key(id));
            cache.finish(&key(id), None, Some(pixels()));
        }
        cache.begin(&key("d"));
        let dropped = cache.retain(&[key("b"), key("d")]);
        assert_eq!(dropped.len(), 2);
        assert!(cache.get(&key("b")).is_some());
        assert!(cache.get(&key("d")).is_some());
        assert!(cache.get(&key("a")).is_none());
        assert_eq!(cache.clear().len(), 1);
    }
}
