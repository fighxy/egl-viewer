//! The service as an app drives it: open, views, edits with undo, imports, the trash, exports.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use library_core::{
    Compression, ExportFormat, ExportRequest, ExportSelection, ExportState, FolderChange,
    ImportState, ItemEdit, LibraryQuery, Scope, SourceError,
};
use library_native::ImportFilter;

use crate::{Event, ItemsTarget, LibraryService, lock};

struct Fixture {
    dir: tempfile::TempDir,
    events: Arc<Mutex<Vec<Event>>>,
    service: LibraryService,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let heard = Arc::clone(&events);
    let service = LibraryService::new(
        "0.1.0-test",
        Arc::new(move |event: Event| lock(&heard).push(event)),
    );
    service.create_library(dir.path(), "Service").unwrap();
    Fixture {
        dir,
        events,
        service,
    }
}

fn images(dir: &Path, count: u32) -> Vec<PathBuf> {
    let source = dir.join("source");
    std::fs::create_dir_all(&source).unwrap();
    (0..count)
        .map(|n| {
            let path = source.join(format!("image-{n}.jpg"));
            std::fs::write(&path, media::fixtures::jpeg(32, 24, n)).unwrap();
            path
        })
        .collect()
}

fn wait<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < deadline, "{what} did not happen");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Adds `count` images and waits for the import and its previews.
fn add(f: &Fixture, count: u32) {
    let ticket = f
        .service
        .add_files(images(f.dir.path(), count), None, ImportFilter::default())
        .unwrap();
    wait("the import", || {
        f.service
            .imports()
            .list()
            .into_iter()
            .find(|progress| progress.id == ticket.id && progress.state == ImportState::Done)
    });
    wait("the previews", || {
        (!f.service
            .imports()
            .is_running(library_core::ImportKind::Previews))
        .then_some(())
    });
}

fn all(f: &Fixture) -> (u64, u32) {
    let opened = f
        .service
        .open_view(LibraryQuery::default(), &|_, _| {})
        .unwrap();
    (opened.view_id, opened.total)
}

#[test]
fn added_files_reach_the_views_and_the_app_hears_progress() {
    let f = fixture();
    let (view, before) = all(&f);
    assert_eq!(before, 0);
    add(&f, 3);
    let events = lock(&f.events);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::ImportProgress(progress) if progress.added == 3))
    );
    // The view opened before the import follows it under the same id.
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ViewsInvalidated(views) if views.views.iter().any(|v| v.view_id == view && v.total == 3)
    )));
    drop(events);
    assert_eq!(f.service.view_range(view, 0, 10).unwrap().len(), 3);
    assert_eq!(f.service.view_dimensions(view).unwrap(), vec![[32, 24]; 3]);
}

#[test]
fn edits_go_back_with_undo_and_a_selection_is_resolved_from_its_view() {
    let f = fixture();
    add(&f, 3);
    let (view, _) = all(&f);
    let ids = f.service.view_ids(view, 0, 3).unwrap();
    let edited = f
        .service
        .apply_batch_edit(
            ItemsTarget {
                view_id: Some(view),
                all: true,
                ids: vec![ids[0].clone()],
            },
            ItemEdit::SetStar(4),
        )
        .unwrap();
    assert_eq!(edited, 2);
    let item = f
        .service
        .apply_item_edit(&ids[0].0, ItemEdit::SetStar(2))
        .unwrap();
    assert_eq!(item.star, 2);
    assert_eq!(f.service.undo_status().unwrap().items, 1);
    f.service.undo().unwrap();
    assert_eq!(f.service.item(&ids[0].0).unwrap().unwrap().star, 0);
    f.service.undo().unwrap();
    assert_eq!(f.service.item(&ids[1].0).unwrap().unwrap().star, 0);
    assert_eq!(f.service.undo().unwrap(), None);
}

#[test]
fn the_trash_restores_with_undo_and_deleting_for_good_forgets_the_undo() {
    let f = fixture();
    add(&f, 2);
    let (view, _) = all(&f);
    let ids = f.service.view_ids(view, 0, 2).unwrap();
    f.service
        .apply_batch_edit(ItemsTarget::ids(ids.clone()), ItemEdit::Trash)
        .unwrap();
    assert_eq!(f.service.trash_info().unwrap().items, 2);
    assert_eq!(f.service.restore_trash().unwrap(), 2);
    f.service.undo().unwrap();
    assert_eq!(f.service.trash_info().unwrap().items, 2);
    f.service
        .delete_permanently(ItemsTarget::ids(vec![ids[0].clone()]))
        .unwrap();
    // The trash edit of the deleted item leaves the stack; the other one stays.
    assert_eq!(f.service.undo_status().unwrap().items, 1);
    assert_eq!(f.service.empty_trash(&|_, _| {}).unwrap(), 1);
    assert_eq!(f.service.trash_info().unwrap().items, 0);
}

#[test]
fn folders_and_tags_refresh_the_open_views() {
    let f = fixture();
    add(&f, 2);
    let tree = f
        .service
        .save_folder(FolderChange::Create {
            name: "Posters".into(),
            parent: None,
            description: String::new(),
            icon_color: None,
        })
        .unwrap();
    let folder = tree[0].id.clone();
    let opened = f
        .service
        .open_view(
            LibraryQuery {
                scope: Scope::Folder {
                    id: folder.clone(),
                    include_descendants: false,
                },
                ..LibraryQuery::default()
            },
            &|_, _| {},
        )
        .unwrap();
    assert_eq!(opened.total, 0);
    let (view, _) = all(&f);
    let ids = f.service.view_ids(view, 0, 1).unwrap();
    lock(&f.events).clear();
    f.service
        .apply_batch_edit(
            ItemsTarget::ids(ids.clone()),
            ItemEdit::AddToFolders(vec![folder]),
        )
        .unwrap();
    assert!(lock(&f.events).iter().any(|event| matches!(
        event,
        Event::ViewsInvalidated(views) if views.views.iter().any(|v| v.view_id == opened.view_id && v.total == 1)
    )));
    f.service
        .apply_batch_edit(
            ItemsTarget::ids(ids),
            ItemEdit::AddTags(vec!["Poster".into()]),
        )
        .unwrap();
    f.service.rename_tag("Poster", "poster").unwrap();
    let tags: Vec<String> = f
        .service
        .tags()
        .unwrap()
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert_eq!(tags, ["poster"]);
}

#[test]
fn an_export_reports_its_progress_and_only_its_output_may_be_revealed() {
    let f = fixture();
    add(&f, 2);
    let destination = f.dir.path().join("out");
    std::fs::create_dir_all(&destination).unwrap();
    let first = f
        .service
        .start_export(ExportRequest {
            selection: ExportSelection {
                whole: true,
                folders: Vec::new(),
                tags: Vec::new(),
            },
            include_subfolders: true,
            format: ExportFormat::Zip {
                compression: Compression::Fast,
            },
            destination: destination.to_string_lossy().into_owned(),
            encrypt: None,
            eagle: None,
        })
        .unwrap();
    let done = wait("the export", || {
        lock(&f.events).iter().find_map(|event| match event {
            Event::ExportProgress(progress)
                if progress.id == first.id && progress.state == ExportState::Done =>
            {
                Some(progress.clone())
            }
            _ => None,
        })
    });
    let summary = done.summary.unwrap();
    assert_eq!(summary.items, 2);
    wait("the export to leave the registry", || {
        f.service.exports().list().is_empty().then_some(())
    });
    assert!(f.service.exports().may_reveal(&summary.path).is_ok());
    assert!(matches!(
        f.service
            .exports()
            .may_reveal(&destination.to_string_lossy()),
        Err(SourceError::Invalid(_))
    ));
}

#[test]
fn an_export_into_eagle_is_not_the_services() {
    let f = fixture();
    let request = ExportRequest {
        selection: ExportSelection {
            whole: true,
            folders: Vec::new(),
            tags: Vec::new(),
        },
        include_subfolders: true,
        format: ExportFormat::Mirror,
        destination: String::new(),
        encrypt: None,
        eagle: Some(library_core::EagleExportTarget {
            host: "127.0.0.1".into(),
            port: 41595,
            token: None,
        }),
    };
    assert!(matches!(
        f.service.start_export(request),
        Err(SourceError::NotSupported(_))
    ));
}

#[test]
fn opening_another_library_drops_the_views_and_closing_ends_everything() {
    let f = fixture();
    add(&f, 1);
    let (view, _) = all(&f);
    let first = f.service.info().unwrap();
    // The open library again: nothing changes.
    let again = f
        .service
        .open_library(Path::new(first.path.as_deref().unwrap()), false)
        .unwrap();
    assert_eq!(again.id, first.id);
    assert!(f.service.view_range(view, 0, 1).is_ok());

    let other = tempfile::tempdir().unwrap();
    let second = f.service.create_library(other.path(), "Other").unwrap();
    assert_ne!(second.id, first.id);
    assert!(matches!(
        f.service.view_range(view, 0, 1),
        Err(SourceError::NotFound)
    ));
    f.service.close_library().unwrap();
    assert!(f.service.info().is_none());
    assert!(matches!(f.service.counts(), Err(SourceError::Invalid(_))));
}

#[test]
fn a_synced_folder_opens_read_only_and_refuses_writes() {
    let f = fixture();
    let path = PathBuf::from(f.service.info().unwrap().path.unwrap());
    f.service.close_library().unwrap();
    let info = f.service.open_library(&path, true).unwrap();
    assert!(info.read_only);
    assert!(matches!(
        f.service
            .add_files(images(f.dir.path(), 1), None, ImportFilter::default()),
        Err(SourceError::ReadOnly)
    ));
}
