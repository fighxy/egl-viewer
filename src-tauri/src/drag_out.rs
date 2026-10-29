//! Dragging originals out of the window into other apps (docs/spec.md §6): an OS drag of the
//! files, started on the main thread when the WebView reports a drag from the details panel.

use std::path::PathBuf;

use tauri::WebviewWindow;

/// Starts the drag and returns at once on Linux; on Windows it returns after the drop.
pub fn start(
    window: &WebviewWindow,
    paths: Vec<PathBuf>,
    image: Option<Vec<u8>>,
) -> Result<(), String> {
    let image = drag::Image::Raw(image.unwrap_or_default());
    let item = drag::DragItem::Files(paths);
    let options = drag::Options::default();
    #[cfg(target_os = "linux")]
    let target = window.gtk_window().map_err(|error| error.to_string())?;
    #[cfg(not(target_os = "linux"))]
    let target = window.clone();
    drag::start_drag(&target, item, image, |_, _| {}, options).map_err(|error| error.to_string())
}
