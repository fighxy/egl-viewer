//! Tests against the committed synthetic Eagle library (fixtures/eagle-library/). The
//! original and thumbnail files are placeholder bytes: parsing, the manifest and the
//! by-extension original lookup need no decodable pixels (fixtures/eagle-library/README.md).

use std::path::Path;

use eagle_folder::{EagleError, EagleLibrary};

fn fixture() -> EagleLibrary {
    EagleLibrary::open(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/eagle-library/Synthetic.library"),
    )
    .expect("open the synthetic fixture library")
}

#[test]
fn opens_and_summarises_the_fixture() {
    let library = fixture();
    assert_eq!(library.name(), "Synthetic");
    let summary = library.summary().expect("summarise the fixture");
    assert_eq!(summary.name, "Synthetic");
    assert_eq!(summary.item_count, 7);
    assert!(summary.bytes > 0);
}

#[test]
fn refuses_what_is_not_an_eagle_library() {
    let dir = tempfile::tempdir().unwrap();
    let error = EagleLibrary::open(dir.path()).unwrap_err();
    let EagleError::NotAnEagleLibrary(detail) = error else {
        panic!("expected NotAnEagleLibrary, got {error:?}");
    };
    assert!(detail.contains("metadata.json"), "{detail}");

    let missing = dir.path().join("nothing");
    let error = EagleLibrary::open(&missing).unwrap_err();
    assert!(matches!(error, EagleError::NotFound(_)));
}

#[test]
fn reads_the_folder_tree_smart_folders_and_tag_groups() {
    let library = fixture();
    let metadata = library.metadata().expect("read metadata.json");
    assert_eq!(metadata.folders.len(), 3);
    let posters = &metadata.folders[0];
    assert_eq!(posters.id, "FPOSTERS");
    assert_eq!(posters.name, "Posters");
    assert_eq!(posters.icon_color.as_deref(), Some("red"));
    assert_eq!(posters.tags, ["design"]);
    assert_eq!(posters.children.len(), 1);
    assert_eq!(posters.children[0].id, "FPOSTERS24");
    assert!(!posters.is_locked);

    let secret = metadata
        .folders
        .iter()
        .find(|folder| folder.id == "FSECRET")
        .expect("the locked folder");
    assert!(secret.is_locked);
    assert!(!format!("{metadata:?}").contains("synthetic-password"));

    assert_eq!(metadata.smart_folders.len(), 1);
    assert_eq!(metadata.smart_folders[0].id, "SRATED");
    assert!(metadata.smart_folders[0].conditions.is_array());
    assert_eq!(metadata.tag_groups.len(), 1);
    assert_eq!(metadata.tag_groups[0].name, "Moods");
    assert_eq!(metadata.tag_groups[0].tags, ["warm", "cold"]);
}

#[test]
fn reads_the_manifest_of_what_to_import() {
    let library = fixture();
    let manifest = library.manifest().expect("read mtime.json");
    assert_eq!(manifest.total, 7);
    assert_eq!(manifest.items.len(), 7);
    let poster = manifest
        .items
        .iter()
        .find(|entry| entry.id == "EITEMPOSTER1")
        .expect("the poster item");
    assert_eq!(poster.last_modified, 1794000001000);
}

#[test]
fn reads_an_item_and_finds_its_original_by_extension() {
    let library = fixture();
    let item = library.item("EITEMPOSTER1").expect("read the item");
    assert_eq!(item.name, "sunset-poster");
    assert_eq!(item.ext, "jpg");
    assert_eq!(item.star, Some(3));
    assert_eq!(item.tags, ["poster", "warm"]);
    assert_eq!(item.folders, ["FPOSTERS", "FPOSTERS24"]);
    assert_eq!(item.annotation.as_deref(), Some("A sunset poster"));
    assert_eq!(item.palettes.len(), 2);
    assert_eq!(item.last_modified, Some(1794000001000));

    let original = library.original(&item).expect("the original by extension");
    assert!(original.ends_with("sunset-poster.jpg"), "{original:?}");
    let thumbnail = library.thumbnail("EITEMPOSTER1").expect("the thumbnail");
    assert!(
        thumbnail
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("_thumbnail.png"),
        "{thumbnail:?}"
    );

    // Small images have no thumbnail file at all.
    assert!(library.thumbnail("EITEMNOTHUMB1").is_none());

    // An item whose original is missing: metadata parses, no original path.
    let broken = library.item("EITEMBROKEN1").expect("read the broken item");
    assert!(library.original(&broken).is_none());
}

#[test]
fn a_gone_item_metadata_is_an_error_not_a_panic() {
    let library = fixture();
    let error = library.item("ENOSUCHITEM1").unwrap_err();
    assert!(matches!(error, EagleError::Unreadable { .. }));
}
