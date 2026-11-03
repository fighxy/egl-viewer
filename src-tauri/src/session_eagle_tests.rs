//! Session tests against a fake Eagle (`wiremock` serving the captures in `fixtures/eagle-api/`):
//! edits, unlocks and library switches while a running Eagle is connected, exports into it,
//! and the paths an export may reveal (D-089).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use app_store::{AppStore, MemoryStore, SecretStore, eagle_token_name};
use library_core::{ItemEdit, ItemId};
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::views::{Event, ItemsTarget};

const ITEM: &str = "MVQTEQPE8FAQM";

/// A fake Eagle on its own runtime: the session's blocking calls never run on it.
struct FakeEagle {
    runtime: tokio::runtime::Runtime,
    server: MockServer,
}

impl FakeEagle {
    fn start() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let server = runtime.block_on(async {
            let server = MockServer::start().await;
            for (route, capture) in [
                ("/api/v2/app/info", "app-info.json"),
                ("/api/v2/library/info", "library-info.with-folders.json"),
                ("/api/v2/item/countAll", "item-countAll.json"),
                ("/api/v2/item/get", "item-get.by-id.1.json"),
                ("/api/v2/item/update", "item-update.star.json"),
            ] {
                Mock::given(path(route))
                    .respond_with(captured(capture))
                    .mount(&server)
                    .await;
            }
            Mock::given(path("/api/folder/unlock"))
                .respond_with(json(serde_json::json!({ "status": "success" })))
                .mount(&server)
                .await;
            server
        });
        Self { runtime, server }
    }

    fn settings(&self) -> ConnectionSettings {
        ConnectionSettings {
            host: "127.0.0.1".into(),
            port: self.server.address().port(),
        }
    }

    fn requests_to(&self, route: &str) -> usize {
        self.runtime
            .block_on(self.server.received_requests())
            .unwrap_or_default()
            .iter()
            .filter(|request| request.url.path() == route)
            .count()
    }
}

fn captured(name: &str) -> ResponseTemplate {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures/eagle-api")
        .join(name);
    let capture: serde_json::Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
    let http = capture["response"]["http"].as_u64().unwrap() as u16;
    ResponseTemplate::new(http)
        .insert_header("content-type", "application/json")
        .set_body_string(capture["response"]["body"].to_string())
}

fn json(body: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/json")
        .set_body_string(body.to_string())
}

struct Fixture {
    _data: tempfile::TempDir,
    home: tempfile::TempDir,
    secrets: Arc<MemoryStore>,
    events: Arc<std::sync::Mutex<Vec<Event>>>,
    session: Arc<Session>,
}

fn fixture() -> Fixture {
    let data = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemoryStore::default());
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let heard = Arc::clone(&events);
    let session = Arc::new(Session::with_secrets(
        AppStore::new(data.path()),
        home.path().to_path_buf(),
        "0.1.0-test",
        SyncEnvironment::default(),
        Arc::new(move |event: Event| heard.lock().unwrap().push(event)),
        Arc::clone(&secrets) as Arc<dyn SecretStore>,
    ));
    Fixture {
        _data: data,
        home,
        secrets,
        events,
        session,
    }
}

/// Runs `call` on another thread and fails the test if it does not return: a deadlock hangs
/// the call instead of failing it.
fn within<T: Send + 'static>(
    session: &Arc<Session>,
    call: impl FnOnce(&Session) -> T + Send + 'static,
) -> T {
    let session = Arc::clone(session);
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(call(&session));
    });
    receiver
        .recv_timeout(Duration::from_secs(20))
        .expect("the session call did not return: deadlock")
}

#[test]
fn edits_and_unlocks_while_eagle_is_connected_return() {
    let eagle = FakeEagle::start();
    let f = fixture();
    let settings = eagle.settings();
    within(&f.session, move |session| {
        session.connect_eagle(settings, None)
    })
    .unwrap();

    let item = within(&f.session, |session| {
        session.apply_item_edit(ITEM, ItemEdit::SetStar(3))
    })
    .unwrap();
    assert_eq!(item.star, 3);

    let edited = within(&f.session, |session| {
        session.apply_batch_edit(
            ItemsTarget {
                view_id: None,
                all: false,
                ids: vec![ItemId(ITEM.into())],
            },
            ItemEdit::SetStar(3),
        )
    })
    .unwrap();
    assert_eq!(edited, 1);

    within(&f.session, |session| {
        session.unlock_folder("LOCKED1", "secret".into())
    })
    .unwrap();
    assert_eq!(eagle.requests_to("/api/folder/unlock"), 1);
    assert!(within(&f.session, |session| session.counts()).is_ok());
}

#[test]
fn a_token_is_stored_only_after_the_connection_succeeds() {
    let eagle = FakeEagle::start();
    let f = fixture();
    let refused = ConnectionSettings {
        host: "127.0.0.1".into(),
        port: 9, // discard: nothing listens there
    };
    assert!(
        f.session
            .connect_eagle(refused, Some("typo".into()))
            .is_err()
    );
    assert_eq!(
        f.secrets.get(&eagle_token_name("127.0.0.1", 9)).unwrap(),
        None
    );

    let settings = eagle.settings();
    let name = eagle_token_name(&settings.host, settings.port);
    f.session
        .connect_eagle(settings, Some("right".into()))
        .unwrap();
    assert_eq!(f.secrets.get(&name).unwrap().as_deref(), Some("right"));
}

#[test]
fn a_native_library_and_a_running_eagle_are_never_open_together() {
    let eagle = FakeEagle::start();
    let f = fixture();
    let parent = f.home.path().join("Libraries");
    std::fs::create_dir_all(&parent).unwrap();
    let native = f
        .session
        .create_library(&parent.to_string_lossy(), "Native")
        .unwrap();

    f.session.connect_eagle(eagle.settings(), None).unwrap();
    assert!(f.session.current_library().is_none());
    assert!(f.session.eagle_session().is_some());

    f.session
        .open_library(native.path.as_deref().unwrap())
        .unwrap();
    assert!(f.session.eagle_session().is_none());
    assert_eq!(f.session.current().unwrap().name, "Native");
}

#[test]
fn an_export_into_eagle_that_fails_keeps_what_eagle_created() {
    let f = fixture();
    let parent = f.home.path().join("Libraries");
    std::fs::create_dir_all(&parent).unwrap();
    f.session
        .create_library(&parent.to_string_lossy(), "Native")
        .unwrap();
    let tree = f
        .session
        .save_folder(library_core::FolderChange::Create {
            name: "Posters".into(),
            parent: None,
            description: String::new(),
            icon_color: None,
        })
        .unwrap();
    let posters = tree[0].id.clone();

    // A fake Eagle that creates the folder, then fails on the tag groups.
    let eagle = FakeEagle::start();
    eagle.runtime.block_on(async {
        Mock::given(path("/api/v2/folder/get"))
            .respond_with(captured("folder-get.json"))
            .mount(&eagle.server)
            .await;
        Mock::given(path("/api/v2/folder/create"))
            .respond_with(captured("folder-create.parent.json"))
            .mount(&eagle.server)
            .await;
        Mock::given(path("/api/v2/tagGroup/get"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&eagle.server)
            .await;
    });
    let settings = eagle.settings();
    // The token Connect saved for this Eagle is the one the export sends.
    f.secrets
        .set(&eagle_token_name(&settings.host, settings.port), "saved")
        .unwrap();
    let request = library_core::ExportRequest {
        selection: library_core::ExportSelection {
            whole: true,
            folders: Vec::new(),
            tags: Vec::new(),
        },
        include_subfolders: true,
        format: library_core::ExportFormat::Mirror,
        destination: String::new(),
        encrypt: None,
        eagle: Some(library_core::EagleExportTarget {
            host: settings.host.clone(),
            port: settings.port,
            token: None,
        }),
    };
    f.session.start_export(request).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let last = loop {
        let finished = f
            .events
            .lock()
            .unwrap()
            .iter()
            .find_map(|event| match event {
                Event::ExportProgress(progress)
                    if !matches!(
                        progress.state,
                        library_core::ExportState::Preparing
                            | library_core::ExportState::Copying
                            | library_core::ExportState::Encrypting
                    ) =>
                {
                    Some(progress.clone())
                }
                _ => None,
            });
        if let Some(progress) = finished {
            break progress;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the export never ended"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(last.state, library_core::ExportState::Failed, "{last:?}");
    // The ids are stored as the run ends, before its last progress goes out.

    let library = f.session.current_library().unwrap();
    let plan = library_native::load_eagle_export(
        &library,
        &library_core::ExportSelection {
            whole: true,
            folders: Vec::new(),
            tags: Vec::new(),
        },
        true,
    )
    .unwrap();
    let folder = plan.folders.iter().find(|f| f.id == posters).unwrap();
    assert_eq!(folder.eagle_id.as_deref(), Some("MVQTKKFBN2JVC"));
    let requests = eagle
        .runtime
        .block_on(eagle.server.received_requests())
        .unwrap();
    let create = requests
        .iter()
        .find(|request| request.url.path() == "/api/v2/folder/create")
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&create.body).unwrap();
    assert_eq!(body["token"], "saved");
}

#[test]
fn show_in_explorer_reveals_only_what_an_export_wrote() {
    let f = fixture();
    let elsewhere = f.home.path().to_string_lossy().into_owned();
    assert!(matches!(
        f.session.may_reveal(&elsewhere),
        Err(SourceError::Invalid(_))
    ));
    f.session
        .service()
        .exports()
        .remember_written(PathBuf::from(&elsewhere));
    assert_eq!(
        f.session.may_reveal(&elsewhere).unwrap(),
        PathBuf::from(&elsewhere)
    );
}

#[test]
fn a_window_holds_at_most_max_window_cells_with_the_pending_ones_in_order() {
    let f = fixture();
    let parent = f.home.path().join("Libraries");
    std::fs::create_dir_all(&parent).unwrap();
    f.session
        .create_library(&parent.to_string_lossy(), "Native")
        .unwrap();
    let opened = f
        .session
        .open_view(library_core::LibraryQuery::default(), &|_, _| {})
        .unwrap();
    let cells: Vec<_> = (0..300)
        .map(|n| {
            eagle_api::EagleSource::pending_cell(&eagle_api::PendingAdd {
                id: format!("P{n:03}"),
                name: format!("file {n}"),
                ext: "png".into(),
            })
        })
        .collect();
    lock(&f.session.pending).extend(cells);
    let window = f.session.view_range(opened.view_id, 0, 500).unwrap();
    assert_eq!(window.len(), library_core::MAX_WINDOW);
    assert_eq!(window[0].id.0, "P000");
    assert_eq!(
        window.last().unwrap().id.0,
        format!("P{:03}", library_core::MAX_WINDOW - 1)
    );

    // A refresh tells the UI totals with the pending cells, as opening the view did.
    f.events.lock().unwrap().clear();
    f.session.refresh_views();
    let heard = f
        .events
        .lock()
        .unwrap()
        .iter()
        .find_map(|event| match event {
            Event::ViewsInvalidated(views) => Some(views.views[0].total),
            _ => None,
        });
    assert_eq!(heard, Some(300));
}
