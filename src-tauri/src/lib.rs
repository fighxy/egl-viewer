//! The Tauri app crate: a thin layer of commands over the core crates (docs/architecture.md §3).

mod commands;
mod session;
mod window;

use std::sync::Arc;

use app_store::{AppStore, SyncEnvironment};
use library_core::ImportProgress;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tracing_subscriber::EnvFilter;

use crate::session::Session;

/// Import progress for the UI (docs/architecture.md §8).
const IMPORT_PROGRESS: &str = "import-progress";
/// The window's close was held back because files are being added; the UI asks the user.
const CLOSE_REQUESTED: &str = "close-requested";

pub fn run() {
    init_logging();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let store = AppStore::new(app.path().app_local_data_dir()?);
            let handle = app.handle().clone();
            let emit = Arc::new(move |progress: &ImportProgress| {
                let _ = handle.emit(IMPORT_PROGRESS, progress);
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

/// `RUST_LOG` controls the level (default `info`). Never log the token or raw bodies of
/// Eagle's `library/info` and `folder/get`: they contain folder passwords (§10).
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
