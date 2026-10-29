//! The pipeline, the sorter and views (docs/spec.md §10, core table).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::sync::Arc;

use super::filter::{Context, FolderIndex, filter, matches_tags, shape_of, type_matches};
use super::search::SearchMatch;
use super::sort::sort;
use super::view::{MAX_WINDOW, ViewSource, Views};
use super::{LibraryQuery, Scope, ScopeHints, SortKey, TagMode};
use crate::error::SourceError;
use crate::model::{Folder, ItemId, ItemLite};
use crate::source::Progress;

fn item(id: &str) -> ItemLite {
    ItemLite {
        id: ItemId::from(id),
        name: id.to_owned(),
        ext: "jpg".into(),
        ..ItemLite::default()
    }
}

fn tags(names: &[&str]) -> Vec<Arc<str>> {
    names.iter().map(|name| Arc::from(*name)).collect()
}

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn folder(id: &str, children: Vec<Folder>) -> Folder {
    Folder {
        id: id.into(),
        name: id.into(),
        children,
        ..Folder::default()
    }
}

fn kept(items: &[ItemLite], query: &LibraryQuery, folders: &FolderIndex) -> Vec<String> {
    filter(
        items,
        query,
        Context {
            folders,
            text: None,
        },
    )
    .into_iter()
    .map(|index| items[index as usize].id.0.clone())
    .collect()
}

fn sorted(items: &[ItemLite], key: SortKey, ascending: bool) -> Vec<String> {
    let query = LibraryQuery {
        sort: key,
        ascending,
        ..LibraryQuery::default()
    };
    let mut order: Vec<u32> = (0..items.len() as u32).collect();
    sort(items, &mut order, &query);
    order
        .into_iter()
        .map(|index| items[index as usize].id.0.clone())
        .collect()
}

#[test]
fn a_parent_shows_its_childs_items_only_with_subfolders_on() {
    let tree = [folder("parent", vec![folder("child", vec![])])];
    let index = FolderIndex::new(&tree);
    let items = [ItemLite {
        folders: Some(strings(&["child"])),
        ..item("a")
    }];
    let open_parent = |include_descendants| LibraryQuery {
        scope: Scope::Folder {
            id: "parent".into(),
            include_descendants,
        },
        ..LibraryQuery::default()
    };
    assert!(kept(&items, &open_parent(false), &index).is_empty());
    assert_eq!(kept(&items, &open_parent(true), &index), ["a"]);
}

#[test]
fn tag_modes_compare_exact_strings() {
    let ab = tags(&["a", "b"]);
    assert!(matches_tags(&ab, &strings(&["a"]), TagMode::Any));
    assert!(!matches_tags(
        &tags(&["a"]),
        &strings(&["a", "b"]),
        TagMode::All
    ));
    assert!(matches_tags(&ab, &strings(&["a", "b"]), TagMode::All));
    assert!(!matches_tags(
        &tags(&["a", "b", "c"]),
        &strings(&["a", "b"]),
        TagMode::Exact
    ));
    assert!(matches_tags(&ab, &strings(&["a", "b"]), TagMode::Exact));
    assert!(!matches_tags(&ab, &strings(&["a"]), TagMode::None));
    assert!(matches_tags(&ab, &strings(&["c"]), TagMode::None));
    assert!(!matches_tags(
        &tags(&["Poster"]),
        &strings(&["poster"]),
        TagMode::Any
    ));
}

#[test]
fn an_empty_tag_selection_filters_nothing_in_any_mode() {
    let items = [ItemLite {
        tags: tags(&["a"]),
        ..item("tagged")
    }];
    for mode in [TagMode::Any, TagMode::All, TagMode::Exact, TagMode::None] {
        let query = LibraryQuery {
            tag_mode: mode,
            ..LibraryQuery::default()
        };
        assert_eq!(
            kept(&items, &query, &FolderIndex::default()),
            ["tagged"],
            "{mode:?}"
        );
    }
}

#[test]
fn items_in_locked_folders_are_hidden_in_every_scope() {
    let locked = Folder {
        is_locked: true,
        ..folder("secret", vec![folder("inside", vec![])])
    };
    let tree = [locked, folder("open", vec![])];
    let index = FolderIndex::new(&tree);
    let items = [
        ItemLite {
            folders: Some(strings(&["secret"])),
            ..item("direct")
        },
        ItemLite {
            folders: Some(strings(&["inside"])),
            ..item("under")
        },
        ItemLite {
            folders: Some(strings(&["open", "secret"])),
            ..item("both")
        },
        ItemLite {
            folders: Some(strings(&["open"])),
            ..item("visible")
        },
    ];
    for scope in [
        Scope::All,
        Scope::Untagged,
        Scope::Trash,
        Scope::Folder {
            id: "open".into(),
            include_descendants: false,
        },
        Scope::Folder {
            id: "secret".into(),
            include_descendants: true,
        },
    ] {
        let query = LibraryQuery {
            scope: scope.clone(),
            ..LibraryQuery::default()
        };
        let shown = kept(&items, &query, &index);
        assert!(
            shown.iter().all(|id| id == "visible"),
            "{scope:?} showed {shown:?}"
        );
    }
}

#[test]
fn unlocking_one_folder_unlocks_no_other() {
    let tree = [Folder {
        is_locked: true,
        is_unlocked: true,
        ..folder(
            "outer",
            vec![Folder {
                is_locked: true,
                ..folder("inner", vec![])
            }],
        )
    }];
    let index = FolderIndex::new(&tree);
    let items = [
        ItemLite {
            folders: Some(strings(&["outer"])),
            ..item("outer-item")
        },
        ItemLite {
            folders: Some(strings(&["inner"])),
            ..item("inner-item")
        },
    ];
    assert_eq!(
        kept(&items, &LibraryQuery::default(), &index),
        ["outer-item"]
    );
}

#[test]
fn uncategorized_and_untagged_check_the_item() {
    let items = [
        ItemLite {
            folders: None,
            ..item("no-folders")
        },
        ItemLite {
            folders: Some(vec![]),
            ..item("empty-folders")
        },
        ItemLite {
            folders: Some(strings(&["f"])),
            tags: tags(&["t"]),
            ..item("filed")
        },
    ];
    let index = FolderIndex::default();
    let query = |scope| LibraryQuery {
        scope,
        ..LibraryQuery::default()
    };
    assert_eq!(
        kept(&items, &query(Scope::Unfiled), &index),
        ["no-folders", "empty-folders"]
    );
    assert_eq!(
        kept(&items, &query(Scope::Untagged), &index),
        ["no-folders", "empty-folders"]
    );
}

#[test]
fn type_rating_shape_and_text_combine() {
    let items = [
        ItemLite {
            ext: "PNG".into(),
            width: Some(100),
            height: Some(100),
            star: 3,
            ..item("square-png")
        },
        ItemLite {
            ext: "jpg".into(),
            width: Some(300),
            height: Some(200),
            ..item("landscape-jpg")
        },
        ItemLite {
            ext: "pdf".into(),
            ..item("document")
        },
    ];
    let index = FolderIndex::default();
    let with = |change: fn(&mut LibraryQuery)| {
        let mut query = LibraryQuery::default();
        change(&mut query);
        kept(&items, &query, &index)
    };
    assert_eq!(
        with(|q| q.ext = Some("image".into())),
        ["square-png", "landscape-jpg"]
    );
    assert_eq!(with(|q| q.ext = Some("png".into())), ["square-png"]);
    assert_eq!(with(|q| q.rating = Some(3)), ["square-png"]);
    assert_eq!(with(|q| q.rating = Some(0)), ["landscape-jpg", "document"]);
    assert_eq!(
        with(|q| q.shape = Some("landscape".into())),
        ["landscape-jpg"]
    );

    let found = SearchMatch::Only(HashSet::from([ItemId::from("document")]));
    let text = filter(
        &items,
        &LibraryQuery::default(),
        Context {
            folders: &index,
            text: Some(&found),
        },
    );
    assert_eq!(text, [2]);
    let excluded = SearchMatch::Except(HashSet::from([ItemId::from("document")]));
    let text = filter(
        &items,
        &LibraryQuery::default(),
        Context {
            folders: &index,
            text: Some(&excluded),
        },
    );
    assert!(!text.contains(&2) && !text.is_empty());
}

#[test]
fn shapes_follow_eagle() {
    assert_eq!(shape_of(Some(10), Some(10)), Some("square"));
    assert_eq!(shape_of(Some(20), Some(10)), Some("landscape"));
    assert_eq!(shape_of(Some(10), Some(20)), Some("portrait"));
    assert_eq!(shape_of(Some(25), Some(10)), Some("panoramic-landscape"));
    assert_eq!(shape_of(Some(10), Some(25)), Some("panoramic-portrait"));
    assert_eq!(shape_of(None, Some(10)), None);
    assert_eq!(shape_of(Some(0), Some(10)), None);
    assert!(type_matches("MP4", "videos"));
    assert!(!type_matches("jpg", "video"));
}

#[test]
fn every_sort_key_orders_with_missing_values_first() {
    let items = [
        ItemLite {
            name: "img10".into(),
            ext: "png".into(),
            size: Some(300),
            width: Some(10),
            height: Some(10),
            star: 5,
            duration: Some(2.0),
            tags: tags(&["beta", "Alpha"]),
            added_at: Some(30),
            file_created_at: Some(5),
            file_modified_at: None,
            ..item("c")
        },
        ItemLite {
            name: "img2".into(),
            ext: "JPG".into(),
            size: None,
            width: None,
            height: None,
            star: 0,
            duration: None,
            tags: vec![],
            added_at: Some(10),
            file_created_at: None,
            file_modified_at: Some(40),
            ..item("a")
        },
        ItemLite {
            name: "Img3".into(),
            ext: "gif".into(),
            size: Some(100),
            width: Some(100),
            height: Some(50),
            star: 2,
            duration: Some(1.5),
            tags: tags(&["gamma"]),
            added_at: Some(20),
            file_created_at: Some(50),
            file_modified_at: Some(1),
            ..item("b")
        },
    ];
    let cases: &[(SortKey, [&str; 3])] = &[
        (SortKey::DateAdded, ["a", "b", "c"]),
        // c was created at 5; a has no creation time and falls back to added (10).
        (SortKey::DateCreated, ["c", "a", "b"]),
        // c has no modification time and falls back to added (30).
        (SortKey::DateModified, ["b", "c", "a"]),
        // Natural and case-insensitive: img2 < Img3 < img10.
        (SortKey::Title, ["a", "b", "c"]),
        (SortKey::Extension, ["b", "a", "c"]),
        (SortKey::FileSize, ["a", "b", "c"]),
        (SortKey::Resolution, ["a", "c", "b"]),
        (SortKey::Rating, ["a", "b", "c"]),
        (SortKey::Duration, ["a", "b", "c"]),
        // No tags first, then the smallest tag of each item: Alpha < gamma.
        (SortKey::FirstTag, ["a", "c", "b"]),
        // Positions come with folders; until then, date added.
        (SortKey::Manual, ["a", "b", "c"]),
    ];
    for (key, expected) in cases {
        assert_eq!(sorted(&items, *key, true), expected, "{key:?} ascending");
        let mut reversed = expected.to_vec();
        reversed.reverse();
        assert_eq!(sorted(&items, *key, false), reversed, "{key:?} descending");
    }
}

#[test]
fn sort_keys_order_like_the_collator() {
    let names = [
        "img10", "Img2", "img2", "äb", "ab", "Ac", "ад", "аб", "10", "9",
    ];
    let items: Vec<ItemLite> = names
        .iter()
        .enumerate()
        .map(|(n, name)| ItemLite {
            name: (*name).to_owned(),
            added_at: Some(n as i64),
            ..item(&format!("{n:02}"))
        })
        .collect();
    let by_keys: Vec<String> = sorted(&items, SortKey::Title, true)
        .iter()
        .map(|id| items[id.parse::<usize>().unwrap()].name.clone())
        .collect();
    let mut by_compare: Vec<&ItemLite> = items.iter().collect();
    by_compare.sort_by(|a, b| {
        super::sort::compare_names(&a.name, &b.name).then_with(|| a.added_at.cmp(&b.added_at))
    });
    let by_compare: Vec<String> = by_compare.iter().map(|item| item.name.clone()).collect();
    assert_eq!(by_keys, by_compare);
    assert_eq!(&by_keys[..2], ["9", "10"], "numbers by value");
}

#[test]
fn manual_order_follows_positions_and_puts_unplaced_items_last() {
    let items = [
        ItemLite {
            position: Some(2.0),
            added_at: Some(1),
            ..item("second")
        },
        ItemLite {
            position: None,
            added_at: Some(0),
            ..item("from-a-subfolder")
        },
        ItemLite {
            position: Some(0.5),
            added_at: Some(9),
            ..item("first")
        },
    ];
    assert_eq!(
        sorted(&items, SortKey::Manual, true),
        ["first", "second", "from-a-subfolder"]
    );
}

#[test]
fn the_cycle_guard_refuses_a_move_under_itself() {
    let tree = [folder(
        "a",
        vec![folder("b", vec![folder("c", vec![])]), folder("d", vec![])],
    )];
    let index = FolderIndex::new(&tree);
    assert!(!index.can_move("a", Some("a")));
    assert!(!index.can_move("a", Some("c")), "c is a grandchild of a");
    assert!(!index.can_move("b", Some("c")));
    assert!(index.can_move("c", Some("d")));
    assert!(index.can_move("c", None), "to the root");
    assert!(index.can_move("d", Some("b")));
    assert_eq!(index.parent("c"), Some("b"));
    assert_eq!(index.parent("a"), None);
}

#[test]
fn ties_fall_back_to_date_added_then_id() {
    let items = [
        ItemLite {
            added_at: Some(2),
            ..item("y")
        },
        ItemLite {
            added_at: Some(2),
            ..item("x")
        },
        ItemLite {
            added_at: Some(1),
            ..item("z")
        },
    ];
    assert_eq!(sorted(&items, SortKey::Rating, true), ["z", "x", "y"]);
}

#[test]
fn random_order_is_stable_for_one_query_and_changes_with_it() {
    let items: Vec<ItemLite> = (0..50).map(|n| item(&format!("{n:02}"))).collect();
    let order = |query: &LibraryQuery| {
        let mut order: Vec<u32> = (0..items.len() as u32).collect();
        sort(&items, &mut order, query);
        order
    };
    let random = LibraryQuery {
        sort: SortKey::Random,
        ..LibraryQuery::default()
    };
    let first = order(&random);
    assert_eq!(order(&random), first);
    let natural: Vec<u32> = (0..50).collect();
    assert_ne!(first, natural);
    let other = LibraryQuery {
        tags: strings(&["x"]),
        ..random.clone()
    };
    assert_ne!(order(&other), first);
    // The direction is not part of the seed: descending is the same shuffle reversed.
    let mut reversed = order(&LibraryQuery {
        ascending: true,
        ..random
    });
    reversed.reverse();
    assert_eq!(reversed, first);
}

/// A source with a fixed list per scope that counts its loads.
struct FakeSource {
    items: RefCell<Vec<ItemLite>>,
    loads: Cell<usize>,
    /// A folder whose scope answers `NotFound`.
    missing: RefCell<Option<String>>,
}

impl FakeSource {
    fn new(count: usize) -> Self {
        Self {
            items: RefCell::new(
                (0..count)
                    .map(|n| ItemLite {
                        added_at: Some(n as i64),
                        ..item(&format!("{n:04}"))
                    })
                    .collect(),
            ),
            loads: Cell::new(0),
            missing: RefCell::new(None),
        }
    }
}

impl ViewSource for FakeSource {
    fn load_scope(
        &self,
        scope: &Scope,
        _hints: &ScopeHints,
        progress: Progress<'_>,
    ) -> Result<Vec<ItemLite>, SourceError> {
        if let Scope::Folder { id, .. } = scope
            && self.missing.borrow().as_deref() == Some(id.as_str())
        {
            return Err(SourceError::NotFound);
        }
        self.loads.set(self.loads.get() + 1);
        let items = self.items.borrow().clone();
        progress(items.len(), items.len());
        Ok(items)
    }

    fn folder_index(&self) -> Result<FolderIndex, SourceError> {
        Ok(FolderIndex::default())
    }

    fn search_ids(&self, _text: &str) -> Result<SearchMatch, SourceError> {
        Err(SourceError::NotSupported("search".into()))
    }
}

fn open(views: &mut Views, source: &FakeSource, query: LibraryQuery) -> (u64, usize) {
    let view = views.open(query, source, &|_, _| {}).unwrap();
    (view.id(), view.total())
}

#[test]
fn a_new_sort_or_chip_on_the_same_scope_does_not_load_again() {
    let source = FakeSource::new(10);
    let mut views = Views::default();
    open(&mut views, &source, LibraryQuery::default());
    open(
        &mut views,
        &source,
        LibraryQuery {
            sort: SortKey::Title,
            ..LibraryQuery::default()
        },
    );
    open(
        &mut views,
        &source,
        LibraryQuery {
            rating: Some(0),
            ..LibraryQuery::default()
        },
    );
    assert_eq!(source.loads.get(), 1);
    open(
        &mut views,
        &source,
        LibraryQuery {
            scope: Scope::Untagged,
            ..LibraryQuery::default()
        },
    );
    assert_eq!(source.loads.get(), 2);
}

#[test]
fn each_open_is_its_own_view() {
    let source = FakeSource::new(3);
    let mut views = Views::default();
    let first = open(&mut views, &source, LibraryQuery::default()).0;
    let second = open(&mut views, &source, LibraryQuery::default()).0;
    assert_ne!(first, second);
    views.close(first);
    assert!(views.get(second).is_some());
    assert_eq!(source.loads.get(), 1);
}

#[test]
fn view_range_never_returns_more_than_the_window() {
    let source = FakeSource::new(500);
    let mut views = Views::default();
    let (id, total) = open(&mut views, &source, LibraryQuery::default());
    assert_eq!(total, 500);
    let view = views.get(id).unwrap();
    assert_eq!(view.range(0, 10).len(), 10);
    assert_eq!(view.range(0, 1000).len(), MAX_WINDOW);
    assert_eq!(view.range(495, 10).len(), 5);
    assert!(view.range(600, 10).is_empty());
    // Newest first by default.
    assert_eq!(view.range(0, 1)[0].id, ItemId::from("0499"));
    assert_eq!(
        view.ids(498, 10),
        [ItemId::from("0001"), ItemId::from("0000")]
    );
}

#[test]
fn old_views_are_dropped_and_closed_views_are_gone() {
    let source = FakeSource::new(3);
    let mut views = Views::default();
    let first = open(&mut views, &source, LibraryQuery::default()).0;
    for key in [SortKey::Title, SortKey::FileSize, SortKey::Rating] {
        open(
            &mut views,
            &source,
            LibraryQuery {
                sort: key,
                ..LibraryQuery::default()
            },
        );
    }
    assert!(views.get(first).is_some(), "four views are kept");
    let fifth = open(
        &mut views,
        &source,
        LibraryQuery {
            sort: SortKey::Extension,
            ..LibraryQuery::default()
        },
    )
    .0;
    // `get` above made the first view the most recent, so the Title view went.
    assert!(views.get(first).is_some());
    views.close(fifth);
    assert!(views.get(fifth).is_none());
}

#[test]
fn a_refresh_recomputes_views_under_the_same_ids() {
    let source = FakeSource::new(3);
    let mut views = Views::default();
    let (id, total) = open(&mut views, &source, LibraryQuery::default());
    assert_eq!(total, 3);
    source.items.borrow_mut().push(ItemLite {
        added_at: Some(100),
        ..item("new")
    });
    assert_eq!(views.refresh(&source).unwrap(), [(id, 4)]);
    let view = views.get(id).unwrap();
    assert_eq!(view.range(0, 1)[0].id, ItemId::from("new"));
}

#[test]
fn a_view_of_a_folder_that_is_gone_becomes_empty_on_refresh() {
    let source = FakeSource::new(3);
    let mut views = Views::default();
    let all = open(&mut views, &source, LibraryQuery::default()).0;
    let folder = open(
        &mut views,
        &source,
        LibraryQuery {
            scope: Scope::Folder {
                id: "gone".into(),
                include_descendants: false,
            },
            ..LibraryQuery::default()
        },
    )
    .0;
    *source.missing.borrow_mut() = Some("gone".into());
    let totals = views.refresh(&source).unwrap();
    assert!(totals.contains(&(folder, 0)));
    assert!(totals.contains(&(all, 3)));
}

#[test]
fn short_search_text_is_ignored_and_longer_text_asks_the_source() {
    let source = FakeSource::new(3);
    let mut views = Views::default();
    let short = LibraryQuery {
        text: Some(" a ".into()),
        ..LibraryQuery::default()
    };
    assert_eq!(open(&mut views, &source, short).1, 3);
    let long = LibraryQuery {
        text: Some("ab".into()),
        ..LibraryQuery::default()
    };
    assert!(matches!(
        views.open(long, &source, &|_, _| {}),
        Err(SourceError::NotSupported(_))
    ));
}

/// Sorting a large view: 100 000 items by title (the collator) and at random. Slow in debug
/// builds; run with `cargo test --release -p library-core -- --ignored sorting_100k`.
#[test]
#[ignore]
fn sorting_100k_items() {
    let items: Vec<ItemLite> = (0..100_000)
        .map(|n| ItemLite {
            name: format!("IMG_{:05} copy {}", (n * 7919) % 100_000, n % 3),
            added_at: Some(i64::from(n)),
            ..item(&format!("{n:06}"))
        })
        .collect();
    for key in [SortKey::Title, SortKey::Random, SortKey::DateAdded] {
        let started = std::time::Instant::now();
        let order = sorted(&items, key, true);
        eprintln!("{key:?}: {:?}", started.elapsed());
        assert_eq!(order.len(), 100_000);
    }
}

#[test]
fn selection_tags_count_each_tag_over_the_selected_items() {
    let source = FakeSource::new(4);
    {
        let mut items = source.items.borrow_mut();
        items[0].tags = tags(&["Poster", "Swiss"]);
        items[1].tags = tags(&["Poster"]);
        items[2].tags = tags(&["poster"]);
        for (n, item) in items.iter_mut().enumerate() {
            item.size = Some(100 * (n as u64 + 1));
        }
    }
    let mut views = Views::default();
    let (id, _) = open(&mut views, &source, LibraryQuery::default());
    let view = views.get(id).unwrap();

    let picked: HashSet<ItemId> = ["0000", "0001"].map(ItemId::from).into_iter().collect();
    let counted = view.selection_tags(false, &picked);
    assert_eq!(counted.items, 2);
    let pairs: Vec<(&str, u32)> = counted
        .tags
        .iter()
        .map(|tag| (tag.name.as_str(), tag.image_count.unwrap()))
        .collect();
    assert_eq!(pairs, [("Poster", 2), ("Swiss", 1)]);

    // Select All except one: "poster" and "Poster" stay apart.
    let except: HashSet<ItemId> = ["0001"].map(ItemId::from).into_iter().collect();
    let counted = view.selection_tags(true, &except);
    assert_eq!(counted.items, 3);
    let names: Vec<&str> = counted.tags.iter().map(|tag| tag.name.as_str()).collect();
    assert_eq!(names, ["Poster", "poster", "Swiss"]);
    // The total size, and a stack of the first three in view order.
    assert_eq!(counted.size, 100 + 300 + 400);
    assert_eq!(counted.stack.len(), 3);
    assert!(
        counted
            .stack
            .iter()
            .all(|cell| cell.id != ItemId::from("0001"))
    );
}
