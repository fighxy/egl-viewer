//! The Tauri app crate: a thin layer of commands over the core crates (docs/architecture.md §3).

mod commands;
mod session;
mod views;
mod window;

use std::sync::Arc;

use app_store::{AppStore, SyncEnvironment};
use library_core::Thumbnail;
use tauri::http::{Response, StatusCode, header};
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tracing_subscriber::EnvFilter;

use crate::session::Session;
use crate::views::Event;

/// Import progress for the UI (docs/architecture.md §8).
const IMPORT_PROGRESS: &str = "import-progress";
/// Views recomputed after a write; the UI refetches their visible windows.
const VIEWS_INVALIDATED: &str = "views-invalidated";
/// The window's close was held back because files are being added; the UI asks the user.
const CLOSE_REQUESTED: &str = "close-requested";

pub fn run() {
    init_logging();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Thumbnails and originals of the open library (docs/architecture.md §11).
        .register_asynchronous_uri_scheme_protocol("thumb", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            tauri::async_runtime::spawn_blocking(move || {
                // Before `setup` has made the session there is no library to serve.
                let image = app
                    .try_state::<Session>()
                    .and_then(|session| session.image(&path));
                responder.respond(image_response(image));
            });
        })
        .setup(|app| {
            let store = AppStore::new(app.path().app_local_data_dir()?);
            let handle = app.handle().clone();
            let emit = Arc::new(move |event: Event| {
                let _ = match event {
                    Event::ImportProgress(progress) => handle.emit(IMPORT_PROGRESS, progress),
                    Event::ViewsInvalidated(views) => handle.emit(VIEWS_INVALIDATED, views),
                };
            });
            let session = Session::new(
                store,
                app.path().home_dir()?,
                env!("CARGO_PKG_VERSION"),
                SyncEnvironment::from_system(),
                emit,
            );
            if let Some(window) = app.get_webview_window("main") {
                window::restore(&window, session.store());
                window::show_eventually(window);
            }
            app.manage(session);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let session = window.state::<Session>();
                if session.has_running_imports() {
                    // Files are being added: the UI asks, then calls `quit` (arch §9).
                    api.prevent_close();
                    let _ = window.emit(CLOSE_REQUESTED, ());
                } else {
                    window::remember(window, session.store());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::connection_defaults,
            commands::default_library_location,
            commands::create_library,
            commands::open_library,
            commands::close_library,
            commands::current_library,
            commands::recent_libraries,
            commands::forget_recent_library,
            commands::get_preferences,
            commands::set_preferences,
            commands::startup,
            commands::library_counts,
            commands::open_view,
            commands::view_range,
            commands::view_ids,
            commands::close_view,
            commands::item_detail,
            commands::folder_tree,
            commands::save_folder,
            commands::delete_folder,
            commands::apply_item_edit,
            commands::apply_batch_edit,
            commands::reorder_items,
            commands::lock_folder,
            commands::unlock_folder,
            commands::recover_folder,
            commands::relock_folder,
            commands::change_folder_password,
            commands::renew_recovery_key,
            commands::remove_folder_lock,
            commands::set_lock_label,
            commands::save_recovery_file,
            commands::add_files,
            commands::paste_files,
            commands::answer_import,
            commands::cancel_import,
            commands::imports,
            commands::quit,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start egl-viewer");
    app.run(|app, event| {
        // Quitting closes the library (WAL checkpoint, lock released) without marking it
        // closed explicitly, so it reopens on the next start.
        if let RunEvent::Exit = event {
            app.state::<Session>().shutdown();
        }
    });
}

/// A versioned URL never changes its image, so the WebView may keep it. SVG originals are
/// images, never documents: no scripts, no sniffing.
fn image_response(image: Option<Thumbnail>) -> Response<Vec<u8>> {
    let response = match image {
        Some(image) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, image.content_type)
            // A decrypted image of a locked folder stays out of the WebView's cache.
            .header(
                header::CACHE_CONTROL,
                if image.private {
                    "no-store"
                } else {
                    "max-age=31536000, immutable"
                },
            )
            .body(image.bytes),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new()),
    };
    response
        .map(|mut response| {
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_SECURITY_POLICY,
                header::HeaderValue::from_static("default-src 'none'"),
            );
            headers.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                header::HeaderValue::from_static("nosniff"),
            );
            response
        })
        .unwrap_or_default()
}

/// `RUST_LOG` controls the level (default `info`). Never log the token or raw bodies of
/// Eagle's `library/info` and `folder/get`: they contain folder passwords (§10).
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
