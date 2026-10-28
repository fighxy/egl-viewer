//! The window's size and position across restarts, and showing it after the first paint
//! (docs/architecture.md §8, D-029).

use std::thread;
use std::time::Duration;

use app_store::{AppStore, WindowState};
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow, Window};

/// Puts the window where it was, if that place is still on a monitor.
pub fn restore(window: &WebviewWindow, store: &AppStore) {
    let Some(state) = store.window_state() else {
        return;
    };
    let position = PhysicalPosition::new(state.x, state.y);
    if is_on_a_monitor(window, position) {
        let _ = window.set_position(position);
    }
    let _ = window.set_size(PhysicalSize::new(state.width, state.height));
    if state.maximized {
        let _ = window.maximize();
    }
}

/// Saves the window's place. A maximized window keeps the size it had before.
pub fn remember(window: &Window, store: &AppStore) {
    let maximized = window.is_maximized().unwrap_or(false);
    let current = window
        .outer_position()
        .ok()
        .zip(window.inner_size().ok())
        .map(|(position, size)| WindowState {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
            maximized,
        });
    let state = match (maximized, store.window_state(), current) {
        (true, Some(before), _) => Some(WindowState {
            maximized: true,
            ..before
        }),
        (_, _, current) => current,
    };
    if let Some(state) = state
        && let Err(error) = store.save_window_state(state)
    {
        tracing::warn!("cannot save the window position: {error}");
    }
}

/// The UI shows the window after its first paint; this shows it anyway if the UI never does.
pub fn show_eventually(window: WebviewWindow) {
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(3));
        if !window.is_visible().unwrap_or(true) {
            let _ = window.show();
        }
    });
}

fn is_on_a_monitor(window: &WebviewWindow, position: PhysicalPosition<i32>) -> bool {
    let Ok(monitors) = window.available_monitors() else {
        return false;
    };
    monitors.iter().any(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        let right = origin.x + i32::try_from(size.width).unwrap_or(i32::MAX);
        let bottom = origin.y + i32::try_from(size.height).unwrap_or(i32::MAX);
        // At least the title bar's left part must be reachable.
        position.x + 100 > origin.x
            && position.x < right - 100
            && position.y >= origin.y - 8
            && position.y < bottom - 50
    })
}
