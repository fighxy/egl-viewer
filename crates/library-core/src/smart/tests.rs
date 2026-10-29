//! Smart folder rules: Eagle's semantics for the v1 subset, and rules made from a screen.

use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::model::{Folder, ItemId};
use crate::query::filter::{Context, filter};

const DAY: i64 = 86_400_000;

fn item(id: &str) -> ItemLite {
    ItemLite {
        id: ItemId::from(id),
        name: id.to_owned(),
        ext: "jpg".into(),
        folders: Some(Vec::new()),
        ..ItemLite::default()
    }
}

fn tagged(id: &str, tags: &[&str]) -> ItemLite {
    ItemLite {
        tags: tags.iter().map(|tag| Arc::from(*tag)).collect(),
        ..item(id)
    }
}

fn rules(conditions: Value) -> SmartRules {
    SmartRules::parse(&conditions).unwrap()
}

/// One AND condition with these rules.
fn one(rules_json: Value) -> SmartRules {
    rules(json!([{ "match": "AND", "boolean": "TRUE", "rules": rules_json }]))
}

fn hits(rules: &SmartRules, item: &ItemLite) -> bool {
    rules.matches(
        &RuleItem {
            item,
            annotation: "",
            url: "",
        },
        0,
    )
}

#[test]
fn conditions_and_rules_combine_as_in_eagle() {
    let poster = tagged("a", &["Poster"]);
    let logo = tagged("b", &["Logo"]);
    let both = tagged("c", &["Poster", "Logo"]);
    let any = rules(json!([{ "match": "OR", "rules": [
        { "property": "tags", "method": "union", "value": ["Poster"] },
        { "property": "tags", "method": "union", "value": ["Logo"] }
    ]}]));
    assert!(hits(&any, &poster) && hits(&any, &logo));
    // A missing `match` means AND.
    let every = rules(json!([{ "rules": [
        { "property": "tags", "method": "union", "value": ["Poster"] },
        { "property": "tags", "method": "union", "value": ["Logo"] }
    ]}]));
    assert!(!hits(&every, &poster) && hits(&every, &both));
    // `boolean: "FALSE"` negates the condition; conditions are AND-ed.
    let negated = rules(json!([
        { "match": "AND", "boolean": "TRUE", "rules": [{ "property": "tags", "method": "not-empty", "value": [] }] },
        { "match": "AND", "boolean": "FALSE", "rules": [{ "property": "tags", "method": "union", "value": ["Logo"] }] }
    ]));
    assert!(hits(&negated, &poster));
    assert!(!hits(&negated, &logo) && !hits(&negated, &both));
}

#[test]
fn tag_and_folder_sets_follow_eagle() {
    let item_ab = tagged("x", &["a", "b"]);
    let empty = tagged("y", &[]);
    let set = |method: &str, value: Value| {
        one(json!([{ "property": "tags", "method": method, "value": value }]))
    };
    assert!(hits(&set("union", json!(["a", "z"])), &item_ab));
    assert!(hits(&set("intersection", json!(["a", "b"])), &item_ab));
    assert!(!hits(&set("intersection", json!(["a", "z"])), &item_ab));
    // An empty intersection never matches tags.
    assert!(!hits(&set("intersection", json!([])), &item_ab));
    assert!(hits(&set("equal", json!(["b", "a"])), &item_ab));
    assert!(!hits(&set("equal", json!(["a"])), &item_ab));
    assert!(hits(&set("identity", json!(["z"])), &item_ab));
    assert!(!hits(&set("identity", json!(["a"])), &item_ab));
    assert!(hits(&set("empty", json!([])), &empty));
    assert!(hits(&set("not-empty", json!([])), &item_ab));
    // Exact strings: "Poster" is not "poster".
    assert!(!hits(&set("union", json!(["A"])), &item_ab));

    let mut filed = item("f");
    filed.folders = Some(vec!["F1".into()]);
    let folders = |method: &str, value: Value| {
        one(json!([{ "property": "folders", "method": method, "value": value }]))
    };
    assert!(hits(&folders("union", json!(["F1"])), &filed));
    // For folders an empty intersection matches every item that has the key.
    assert!(hits(&folders("intersection", json!([])), &filed));
    // Without the key nothing matches, not even `empty`.
    let mut keyless = item("k");
    keyless.folders = None;
    assert!(!hits(&folders("empty", json!([])), &keyless));
    assert!(hits(&folders("empty", json!([])), &item("e")));
}

#[test]
fn names_notes_and_links_compare_without_case() {
    let mut named = item("x");
    named.name = "Плакат Swiss".into();
    let text = |property: &str, method: &str, value: &str| {
        one(json!([{ "property": property, "method": method, "value": value }]))
    };
    let check = |rules: &SmartRules, annotation: &str, url: &str| {
        rules.matches(
            &RuleItem {
                item: &named,
                annotation,
                url,
            },
            0,
        )
    };
    assert!(check(&text("name", "contain", "ПЛАК"), "", ""));
    assert!(check(&text("name", "startWith", "плакат"), "", ""));
    assert!(check(&text("name", "endWith", "SWISS"), "", ""));
    assert!(check(&text("name", "equal", "плакат swiss"), "", ""));
    assert!(check(&text("name", "uncontain", "logo"), "", ""));
    // Prefixes are literal here, not regular expressions (D-048).
    assert!(!check(&text("name", "startWith", "пл.к"), "", ""));
    // An empty value matches nothing, except with empty and not-empty.
    assert!(!check(&text("name", "contain", ""), "", ""));
    assert!(check(&text("annotation", "empty", ""), "", ""));
    assert!(check(
        &text("annotation", "contain", "grid"),
        "Swiss GRID",
        ""
    ));
    assert!(check(
        &text("url", "startWith", "https://"),
        "",
        "https://example.com"
    ));
    assert!(check(
        &text("url", "not-empty", ""),
        "",
        "https://example.com"
    ));
}

#[test]
fn dates_count_the_day_of_the_value() {
    let mut dated = item("d");
    dated.added_at = Some(10 * DAY + 5_000);
    dated.file_modified_at = None;
    dated.file_created_at = Some(3 * DAY);
    let date = |property: &str, method: &str, value: Value| {
        one(json!([{ "property": property, "method": method, "value": value }]))
    };
    // before: t <= value + 1 day, so the day itself counts.
    assert!(hits(
        &date("createTime", "before", json!([10 * DAY])),
        &dated
    ));
    assert!(!hits(
        &date("createTime", "before", json!([8 * DAY])),
        &dated
    ));
    assert!(hits(
        &date("createTime", "after", json!([10 * DAY])),
        &dated
    ));
    assert!(hits(&date("createTime", "on", json!([10 * DAY])), &dated));
    assert!(!hits(&date("createTime", "on", json!([9 * DAY])), &dated));
    assert!(hits(
        &date("createTime", "between", json!([9 * DAY, 10 * DAY])),
        &dated
    ));
    // mtime falls back to the date added; btime is its own.
    assert!(hits(&date("mtime", "on", json!([10 * DAY])), &dated));
    assert!(hits(&date("btime", "before", json!([3 * DAY])), &dated));
    let within = date("createTime", "within", json!([7]));
    let check = |now: i64| {
        within.matches(
            &RuleItem {
                item: &dated,
                annotation: "",
                url: "",
            },
            now,
        )
    };
    assert!(check(15 * DAY) && !check(18 * DAY));
}

#[test]
fn sizes_compare_in_their_unit() {
    let mut big = item("b");
    big.width = Some(4000);
    big.height = Some(3000);
    big.size = Some(3 * 1024 * 1024);
    let number = |property: &str, method: &str, value: Value| {
        one(json!([{ "property": property, "method": method, "value": value }]))
    };
    assert!(hits(&number("width", ">=", json!([4000])), &big));
    assert!(!hits(&number("width", ">", json!([4000])), &big));
    assert!(hits(
        &number("height", "between", json!([3000, 3000])),
        &big
    ));
    assert!(hits(
        &one(json!([{ "property": "fileSize", "method": "=", "value": [3], "unit": "mb" }])),
        &big
    ));
    assert!(hits(
        &one(json!([{ "property": "fileSize", "method": "<", "value": ["3073"], "unit": "kb" }])),
        &big
    ));
    // A missing value never matches.
    assert!(!hits(&number("width", "<", json!([10])), &item("none")));
}

#[test]
fn type_rating_and_shape() {
    let mut photo = item("p");
    photo.ext = "PNG".into();
    photo.width = Some(30);
    photo.height = Some(10);
    photo.star = 4;
    let rule = |property: &str, method: &str, value: Value| {
        one(json!([{ "property": property, "method": method, "value": value }]))
    };
    assert!(hits(&rule("type", "equal", json!("png")), &photo));
    assert!(hits(&rule("type", "equal", json!("image")), &photo));
    assert!(hits(&rule("type", "unequal", json!("video")), &photo));
    assert!(hits(&rule("rating", "equal", json!("4")), &photo));
    assert!(hits(&rule("rating", "contain", json!("3,4,5")), &photo));
    assert!(hits(&rule("rating", "equal", json!("none")), &item("u")));
    // A value with "none" in `contain` matches every item (Eagle).
    assert!(hits(&rule("rating", "contain", json!("none,5")), &photo));
    assert!(hits(
        &rule("shape", "equal", json!("panoramic-landscape")),
        &photo
    ));
    // Without dimensions there is no shape: equal fails, unequal matches.
    assert!(hits(&rule("shape", "unequal", json!("square")), &item("n")));
    assert!(!hits(&rule("shape", "equal", json!("square")), &item("n")));
}

#[test]
fn rules_outside_the_subset_are_listed_and_never_match() {
    let parsed = rules(json!([{ "match": "OR", "rules": [
        { "property": "color", "method": "similar", "value": "#ff0000" },
        { "property": "name", "method": "regex", "value": "^a" },
        { "property": "color", "method": "similar", "value": "#00ff00" },
        { "property": "tags", "method": "not-empty", "value": [] }
    ]}]));
    assert_eq!(parsed.unsupported(), ["color similar", "name regex"]);
    assert!(hits(&parsed, &tagged("t", &["a"])));
    assert!(!hits(&parsed, &tagged("u", &[])));
    assert!(SmartRules::parse(&json!({ "rules": [] })).is_err());
    assert!(SmartRules::parse(&json!([{ "match": "AND" }])).is_err());
}

fn tree() -> FolderIndex {
    let child = Folder {
        id: "F2".into(),
        name: "Child".into(),
        ..Folder::default()
    };
    FolderIndex::new(&[Folder {
        id: "F1".into(),
        name: "Parent".into(),
        children: vec![child],
        ..Folder::default()
    }])
}

fn library() -> Vec<ItemLite> {
    let mut items = vec![
        tagged("poster", &["Poster"]),
        tagged("both", &["Poster", "Swiss"]),
        tagged("lower", &["poster"]),
        tagged("none", &[]),
        tagged("extra", &["Poster", "Swiss", "Print"]),
    ];
    items[0].folders = Some(vec!["F1".into()]);
    items[1].folders = Some(vec!["F2".into()]);
    items[1].star = 5;
    items[2].ext = "png".into();
    items[2].width = Some(20);
    items[2].height = Some(10);
    items[4].folders = Some(vec!["F1".into(), "F3".into()]);
    items
}

/// The done check: the rules made from a screen keep exactly what the screen showed.
#[test]
fn a_smart_folder_made_from_chips_shows_what_the_chips_showed() {
    let items = library();
    let index = tree();
    let screens = [
        LibraryQuery {
            tags: vec!["Poster".into(), "Swiss".into()],
            ..LibraryQuery::default()
        },
        LibraryQuery {
            tags: vec!["Poster".into(), "Swiss".into()],
            tag_mode: TagMode::Exact,
            ..LibraryQuery::default()
        },
        LibraryQuery {
            tags: vec!["Swiss".into()],
            tag_mode: TagMode::None,
            ..LibraryQuery::default()
        },
        LibraryQuery {
            scope: Scope::Folder {
                id: "F1".into(),
                include_descendants: true,
            },
            tags: vec!["Poster".into()],
            tag_mode: TagMode::All,
            ..LibraryQuery::default()
        },
        LibraryQuery {
            scope: Scope::Folder {
                id: "F1".into(),
                include_descendants: false,
            },
            extra_folder_ids: vec!["F3".into()],
            folder_mode: TagMode::None,
            ..LibraryQuery::default()
        },
        LibraryQuery {
            scope: Scope::Unfiled,
            ext: Some("image".into()),
            ..LibraryQuery::default()
        },
        LibraryQuery {
            scope: Scope::Untagged,
            ..LibraryQuery::default()
        },
        LibraryQuery {
            rating: Some(0),
            shape: Some("landscape".into()),
            ..LibraryQuery::default()
        },
        LibraryQuery {
            rating: Some(5),
            ..LibraryQuery::default()
        },
    ];
    for query in screens {
        let shown: Vec<&str> = filter(
            &items,
            &query,
            Context {
                folders: &index,
                text: None,
            },
        )
        .into_iter()
        .map(|at| items[at as usize].id.0.as_str())
        .collect();
        let (parent, conditions) = rules_from_query(&query, &index).unwrap();
        assert_eq!(parent, None);
        let saved = SmartRules::parse(&conditions).unwrap();
        assert!(saved.unsupported().is_empty());
        let kept: Vec<&str> = items
            .iter()
            .filter(|item| hits(&saved, item))
            .map(|item| item.id.0.as_str())
            .collect();
        assert_eq!(kept, shown, "{query:?}");
    }
}

#[test]
fn what_rules_cannot_hold_is_refused() {
    let index = tree();
    let refused = |query: LibraryQuery| rules_from_query(&query, &index).unwrap_err();
    assert!(matches!(
        refused(LibraryQuery {
            text: Some("poster".into()),
            tags: vec!["a".into()],
            ..LibraryQuery::default()
        }),
        SourceError::Invalid(message) if message.contains("Search text")
    ));
    assert!(matches!(
        refused(LibraryQuery {
            scope: Scope::Trash,
            ..LibraryQuery::default()
        }),
        SourceError::Invalid(_)
    ));
    assert!(matches!(
        refused(LibraryQuery::default()),
        SourceError::Invalid(_)
    ));
    // Inside a smart folder the new one goes under it, so its rules apply too.
    let (parent, _) = rules_from_query(
        &LibraryQuery {
            scope: Scope::Smart { id: "S1".into() },
            rating: Some(3),
            ..LibraryQuery::default()
        },
        &index,
    )
    .unwrap();
    assert_eq!(parent.as_deref(), Some("S1"));
}

#[test]
fn changes_need_a_name_and_one_to_thirty_rules() {
    let rule = json!({ "property": "tags", "method": "not-empty", "value": [] });
    let change = |name: &str, conditions: Value| SmartFolderChange::Create {
        name: name.into(),
        parent: None,
        conditions,
    };
    let ok = change("Tagged", json!([{ "match": "AND", "rules": [rule] }]));
    assert!(ok.validate().is_ok());
    assert!(
        change(" ", json!([{ "match": "AND", "rules": [rule] }]))
            .validate()
            .is_err()
    );
    assert!(change("x", json!([])).validate().is_err());
    assert!(
        change("x", json!([{ "match": "AND", "rules": [] }]))
            .validate()
            .is_err()
    );
    let many: Vec<Value> = (0..31).map(|_| rule.clone()).collect();
    assert!(
        change("x", json!([{ "match": "AND", "rules": many }]))
            .validate()
            .is_err()
    );
    assert_eq!(serde_json::to_value(&ok).unwrap()["type"], json!("create"));
}
