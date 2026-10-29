//! The sorter (docs/spec.md §5 "Sort", docs/architecture.md §6.4 step 7).

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use icu_collator::options::{CollatorOptions, Strength};
use icu_collator::preferences::CollationNumericOrdering;
use icu_collator::{Collator, CollatorBorrowed, CollatorPreferences};

use crate::model::ItemLite;
use crate::query::{LibraryQuery, SortKey};

/// Natural, case-insensitive order for names, extensions and tags: `img2` before `img10`.
fn collator() -> &'static CollatorBorrowed<'static> {
    static COLLATOR: OnceLock<CollatorBorrowed<'static>> = OnceLock::new();
    COLLATOR.get_or_init(|| {
        let mut preferences = CollatorPreferences::default();
        preferences.numeric_ordering = Some(CollationNumericOrdering::True);
        let mut options = CollatorOptions::default();
        options.strength = Some(Strength::Secondary);
        Collator::try_new(preferences, options).expect("the collation data is compiled in")
    })
}

/// Compares two strings the way names are sorted.
pub fn compare_names(a: &str, b: &str) -> Ordering {
    collator().compare(a, b)
}

/// Tag order: like names, and a total order for tags that differ only in case (`"Poster"`
/// before `"poster"`), so lists of tags never depend on where they came from.
pub fn compare_tags(a: &str, b: &str) -> Ordering {
    compare_names(a, b).then_with(|| a.cmp(b))
}

/// The collation sort key of `text`: comparing keys as bytes orders like [`compare_names`].
fn sort_key(text: &str) -> Vec<u8> {
    let mut key = Vec::new();
    let Ok(()) = collator().write_sort_key_to(text, &mut key);
    key
}

/// Keys worked out once per item instead of once per comparison.
enum Keys {
    None,
    Random(Vec<u64>),
    /// Collation keys; `None` sorts first (an item without tags).
    Text(Vec<Option<Vec<u8>>>),
}

/// Sorts `order` (positions in `items`) by the query's key and direction. Missing values count
/// as the smallest. Ties fall back to date added, then the id, so the order is total: it stays
/// the same while scrolling and between runs.
pub fn sort(items: &[ItemLite], order: &mut [u32], query: &LibraryQuery) {
    let keys = match query.sort {
        SortKey::Random => {
            let seed = random_seed(query);
            Keys::Random(
                items
                    .iter()
                    .map(|item| mix(seed ^ fnv(item.id.0.as_bytes())))
                    .collect(),
            )
        }
        SortKey::Title => Keys::Text(
            items
                .iter()
                .map(|item| Some(sort_key(&item.name)))
                .collect(),
        ),
        SortKey::Extension => {
            Keys::Text(items.iter().map(|item| Some(sort_key(&item.ext))).collect())
        }
        SortKey::FirstTag => Keys::Text(
            items
                .iter()
                .map(|item| item.tags.iter().map(|tag| sort_key(tag)).min())
                .collect(),
        ),
        _ => Keys::None,
    };
    order.sort_unstable_by(|&a, &b| {
        let (ia, ib) = (a as usize, b as usize);
        let (x, y) = (&items[ia], &items[ib]);
        let primary = match &keys {
            Keys::Random(keys) => keys[ia].cmp(&keys[ib]),
            // `None` < `Some`: items without tags come first.
            Keys::Text(keys) => keys[ia].cmp(&keys[ib]),
            Keys::None => compare_by(query.sort, x, y),
        };
        let ordering = primary
            .then_with(|| x.added_at.cmp(&y.added_at))
            .then_with(|| x.id.cmp(&y.id));
        if query.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    });
}

fn compare_by(key: SortKey, x: &ItemLite, y: &ItemLite) -> Ordering {
    let area =
        |item: &ItemLite| u64::from(item.width.unwrap_or(0)) * u64::from(item.height.unwrap_or(0));
    match key {
        SortKey::DateAdded => x.added_at.cmp(&y.added_at),
        SortKey::DateCreated => x
            .file_created_at
            .or(x.added_at)
            .cmp(&y.file_created_at.or(y.added_at)),
        SortKey::DateModified => x
            .file_modified_at
            .or(x.added_at)
            .cmp(&y.file_modified_at.or(y.added_at)),
        SortKey::FileSize => x.size.unwrap_or(0).cmp(&y.size.unwrap_or(0)),
        SortKey::Resolution => area(x).cmp(&area(y)),
        SortKey::Rating => x.star.cmp(&y.star),
        SortKey::Duration => x
            .duration
            .unwrap_or(0.0)
            .total_cmp(&y.duration.unwrap_or(0.0)),
        // Items without a position in the open folder (from its subfolders, or outside a
        // folder) come after the ordered ones.
        SortKey::Manual => x
            .position
            .unwrap_or(f64::INFINITY)
            .total_cmp(&y.position.unwrap_or(f64::INFINITY)),
        SortKey::Title | SortKey::Extension | SortKey::FirstTag | SortKey::Random => {
            unreachable!("keyed above")
        }
    }
}

/// One shuffle per query: the seed comes from the query without its sort, so random order stays
/// the same until a scope, chip or search changes.
fn random_seed(query: &LibraryQuery) -> u64 {
    let identity = LibraryQuery {
        sort: SortKey::Random,
        ascending: true,
        ..query.clone()
    };
    let mut hasher = Fnv::default();
    identity.hash(&mut hasher);
    hasher.finish()
}

/// FNV-1a: a fixed hash, unlike `std`'s randomly keyed one.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut hasher = Fnv::default();
    hasher.write(bytes);
    hasher.finish()
}

/// The SplitMix64 finalizer: spreads similar ids over the whole range.
fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
