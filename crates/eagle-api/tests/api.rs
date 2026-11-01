//! Integration tests over the captures of a live Eagle session (`fixtures/eagle-api/`):
//! the wire shapes recorded there are what these calls must parse.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use eagle_api::{EagleApi, EagleError, EagleSource, ServerFilter};
use library_core::{LibrarySource, Scope, ScopeHints, SortKey};
use secrecy::SecretString;
use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// `{ request, response: { http, body } }` from the captures folder.
fn capture(name: &str) -> Value {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/eagle-api")
        .join(name);
    serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap()
}

fn http_of(capture: &Value) -> u16 {
    capture["response"]["http"].as_u64().unwrap() as u16
}

fn body_of(capture: &Value) -> Value {
    capture["response"]["body"].clone()
}

fn respond(http: u16, body: Value) -> ResponseTemplate {
    ResponseTemplate::new(http)
        .insert_header("content-type", "application/json")
        .set_body_string(body.to_string())
}

/// The app's invariant (docs/architecture.md §6.5): a source called on a blocking thread is
/// immediately ready, because `EagleSource` drives its own runtime internally.
fn blocking<T>(future: impl Future<Output = T>) -> T {
    use std::task::{Context, Poll, Waker};
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("an EagleSource call must be immediately ready"),
    }
}

/// Runs a source call the way the app does: on a blocking thread (docs/architecture.md §6.5).
async fn on_blocking<T>(
    source: Arc<EagleSource>,
    call: impl FnOnce(&EagleSource) -> T + Send + 'static,
) -> T
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || call(&source))
        .await
        .expect("blocking task")
}

fn api_at(server: &MockServer) -> EagleApi {
    EagleApi::at(
        url::Url::parse(&format!("{}/", server.uri())).unwrap(),
        SecretString::from("capture-token"),
    )
    .unwrap()
}

#[tokio::test]
async fn connect_probes_app_info_library_info_and_count() {
    let server = MockServer::start().await;
    let shot = capture("app-info.json");
    let (http, body) = (http_of(&shot), body_of(&shot));
    Mock::given(method("GET"))
        .and(path("/api/v2/app/info"))
        .and(query_param("token", "capture-token"))
        .respond_with(respond(http, body))
        .mount(&server)
        .await;
    let shot = capture("library-info.with-folders.json");
    let (http, body) = (http_of(&shot), body_of(&shot));
    Mock::given(method("GET"))
        .and(path("/api/v2/library/info"))
        .respond_with(respond(http, body))
        .mount(&server)
        .await;
    let shot = capture("item-countAll.json");
    let (http, body) = (http_of(&shot), body_of(&shot));
    Mock::given(method("GET"))
        .and(path("/api/v2/item/countAll"))
        .respond_with(respond(http, body))
        .mount(&server)
        .await;

    let source = EagleSource::at(
        url::Url::parse(&format!("{}/", server.uri())).unwrap(),
        SecretString::from("capture-token"),
    )
    .unwrap();
    // `EagleSource::at` skips the probe; exercise the same calls the connect path makes.
    let api = api_at(&server);
    let app = api.app_info().await.unwrap();
    assert_eq!(app.version, "4.0.0");
    let library = api.library_info().await.unwrap();
    let info = library.to_info().unwrap();
    assert_eq!(info.name, "ggwp");
    assert!(info.path.as_deref().unwrap().ends_with("ggwp.library"));
    assert_eq!(api.item_count().await.unwrap(), 2);
    let _ = source;
}

#[tokio::test]
async fn library_info_without_folders_is_no_library_open() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/library/info"))
        .respond_with(respond(
            200,
            serde_json::json!({ "status": "success", "data": {} }),
        ))
        .mount(&server)
        .await;
    let api = api_at(&server);
    let library = api.library_info().await.unwrap();
    assert!(library.to_info().is_none());
}

#[tokio::test]
async fn folder_tree_comes_from_paged_folder_get() {
    let server = MockServer::start().await;
    let capture = capture("folder-get.tree.json");
    let (http, body) = (http_of(&capture), body_of(&capture));
    Mock::given(method("GET"))
        .and(path("/api/v2/folder/get"))
        .and(query_param("limit", "1000"))
        .respond_with(respond(http, body))
        .mount(&server)
        .await;
    let source = Arc::new(
        EagleSource::at(
            url::Url::parse(&format!("{}/", server.uri())).unwrap(),
            SecretString::new("".into()),
        )
        .unwrap(),
    );
    let tree = on_blocking(source, |source| blocking(source.folder_tree()))
        .await
        .unwrap();
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].name, "eagle-api-test-parent");
    assert_eq!(tree[0].children.len(), 1);
    assert_eq!(tree[0].children[0].icon_color.as_deref(), Some("red"));
}

#[tokio::test]
async fn grid_scope_loads_item_get_with_the_grid_fields() {
    let server = MockServer::start().await;
    let capture = capture("item-get.list.grid-fields.json");
    let (http, body) = (http_of(&capture), body_of(&capture));
    let scoped = wiremock::Mock::given(method("POST"))
        .and(path("/api/v2/item/get"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "folders": ["MVQTKKFBN2JVC"], "offset": 0, "limit": 1000
        })))
        .respond_with(respond(http, body))
        .mount_as_scoped(&server)
        .await;

    let source = Arc::new(
        EagleSource::at(
            url::Url::parse(&format!("{}/", server.uri())).unwrap(),
            SecretString::new("".into()),
        )
        .unwrap(),
    );
    // A folder without descendants asks for exactly that folder.
    let scope = Scope::Folder {
        id: "MVQTKKFBN2JVC".into(),
        include_descendants: false,
    };
    let loaded = on_blocking(source, move |source| {
        blocking(source.load_scope(&scope, &ScopeHints::default(), &|_, _| {}))
    })
    .await
    .unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].id, library_core::ItemId::from("MVQTEQPE8FAQM"));
    assert_eq!(loaded[0].star, 0);
    assert_eq!(loaded[0].palette0, Some([142, 83, 55]));
    assert_eq!(loaded[1].thumb_version, 1700000000000);
    // The fields were the grid fields.
    let requests = scoped.received_requests().await;
    let sent: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(sent["fields"].as_array().unwrap().len(), 15);
    assert!(sent.get("token").is_some());
}

#[tokio::test]
async fn search_uses_item_query_and_takes_only_ids() {
    let server = MockServer::start().await;
    let capture = capture("item-query.json");
    let (http, body) = (http_of(&capture), body_of(&capture));
    Mock::given(method("POST"))
        .and(path("/api/v2/item/query"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "query": "eagle-api-test"
        })))
        .respond_with(respond(http, body))
        .mount(&server)
        .await;
    let source = Arc::new(
        EagleSource::at(
            url::Url::parse(&format!("{}/", server.uri())).unwrap(),
            SecretString::new("".into()),
        )
        .unwrap(),
    );
    let matched = on_blocking(source, |source| {
        blocking(source.search_ids("eagle-api-test"))
    })
    .await
    .unwrap();
    assert!(matched.contains(&library_core::ItemId::from("MVQTEQPE8FAQM")));
    assert!(matched.contains(&library_core::ItemId::from("MVQTEQPM2IAAG")));
}

#[tokio::test]
async fn tags_and_groups_come_from_their_endpoints() {
    let server = MockServer::start().await;
    let capture = capture("tag-get.json");
    let (http, body) = (http_of(&capture), body_of(&capture));
    Mock::given(method("GET"))
        .and(path("/api/v2/tag/get"))
        .respond_with(respond(http, body))
        .mount(&server)
        .await;
    let source = Arc::new(
        EagleSource::at(
            url::Url::parse(&format!("{}/", server.uri())).unwrap(),
            SecretString::new("".into()),
        )
        .unwrap(),
    );
    let tags = on_blocking(source, |source| blocking(source.tags()))
        .await
        .unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "eagle-api-test");
    assert_eq!(tags[0].image_count, Some(2));
}

#[tokio::test]
async fn trash_scope_is_not_supported() {
    let source = Arc::new(
        EagleSource::at(
            url::Url::parse("http://127.0.0.1:9/").unwrap(),
            SecretString::new("".into()),
        )
        .unwrap(),
    );
    let error = on_blocking(source, |source| {
        blocking(source.load_scope(&Scope::Trash, &ScopeHints::default(), &|_, _| {}))
    })
    .await
    .unwrap_err();
    assert!(matches!(error, library_core::SourceError::NotSupported(_)));
}

#[tokio::test]
async fn item_detail_reads_one_full_item() {
    let server = MockServer::start().await;
    let capture = capture("item-get.by-id.1.json");
    let (http, body) = (http_of(&capture), body_of(&capture));
    Mock::given(method("POST"))
        .and(path("/api/v2/item/get"))
        .and(wiremock::matchers::body_partial_json(serde_json::json!({
            "id": "MVQTEQPE8FAQM"
        })))
        .respond_with(respond(http, body))
        .mount(&server)
        .await;
    let source = Arc::new(
        EagleSource::at(
            url::Url::parse(&format!("{}/", server.uri())).unwrap(),
            SecretString::new("".into()),
        )
        .unwrap(),
    );
    let item = on_blocking(source, |source| {
        blocking(source.item(&library_core::ItemId::from("MVQTEQPE8FAQM")))
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(item.name, "eagle-api-test-1");
    assert_eq!(item.width, Some(640));
}

#[test]
fn capabilities_are_read_only_with_locked_folders() {
    let source = EagleSource::at(
        url::Url::parse("http://127.0.0.1:9/").unwrap(),
        SecretString::new("".into()),
    )
    .unwrap();
    let capabilities = source.capabilities();
    assert!(!capabilities.writable);
    assert!(!capabilities.change_feed);
    assert!(!capabilities.delete_items);
    assert!(!capabilities.delete_folders);
    assert!(capabilities.locked_folders);
}

#[test]
fn server_filter_wire_shape_matches_the_captures() {
    // item-get.is-unfiled.json shows exactly what the unfiled count sends.
    let capture = capture("item-get.is-unfiled.json");
    assert_eq!(http_of(&capture), 200);
    let sent = &capture["request"]["body"];
    let filter = ServerFilter {
        is_unfiled: true,
        ..ServerFilter::default()
    };
    let filter = serde_json::to_value(&filter).unwrap();
    assert_eq!(filter["isUnfiled"], sent["isUnfiled"]);
    assert!(filter.get("folders").is_none());
    assert!(filter.get("tags").is_none());
    let _ = SortKey::DateAdded;
}

#[test]
fn error_enum_is_send_for_the_blocking_adapter() {
    fn assert_send<T: Send>() {}
    assert_send::<EagleError>();
}
