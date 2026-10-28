//! The Tauri app crate: a thin layer of commands over the core crates (docs/architecture.md §3).

mod commands;
mod session;
mod window;

use app_store::{AppStore, SyncEnvironment};
use tauri::{Manager, RunEvent, WindowEvent};
use tracing_subscriber::EnvFilter;

use crate::session::Session;

pub fn run() {
    init_logging();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let store = AppStore::new(app.path().app_local_data_dir()?);
            let session = Session::new(
                store,
                app.path().home_dir()?,
                env!("CARGO_PKG_VERSION"),
                SyncEnvironment::from_system(),
            );
            if let Some(window) = app.get_webview_window("main") {
                window::restore(&window, session.store());
                window::show_eventually(window);
            }
            app.manage(session);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                window::remember(window, window.state::<Session>().store());
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
