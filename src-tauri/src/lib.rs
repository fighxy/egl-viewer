//! The Tauri app crate: a thin layer of commands over the core crates (docs/architecture.md §3).

mod commands;

use tracing_subscriber::EnvFilter;

pub fn run() {
    init_logging();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::connection_defaults,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start egl-viewer");
}

/// `RUST_LOG` controls the level (default `info`). Never log the token or raw bodies of
/// Eagle's `library/info` and `folder/get`: they contain folder passwords (§10).
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
