//! Events to the app: import and export progress and recomputed views, as JSON through a C
//! callback (D-092). Without a callback, events are dropped and the app polls
//! (`egl_imports`, `egl_exports`).

use std::ffi::{CString, c_void};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};

use app_service::{Emit, Event, lock};
use serde_json::json;

use crate::{EglLibrary, EglStatus, with_lib};

/// Called with one event: `event_json` is valid only during the call; do not free it. Called
/// from a background thread (an import or export) or from the thread of the call that wrote;
/// hop to the main thread before touching UI. `ctx` is what `egl_set_event_callback` got.
pub type EglEventCallback =
    Option<unsafe extern "C" fn(ctx: *mut c_void, event_json: *const c_char)>;

#[derive(Clone, Copy)]
struct Callback {
    call: unsafe extern "C" fn(*mut c_void, *const c_char),
    /// The caller's context pointer, kept as an address: Rust never dereferences it.
    ctx: usize,
}

/// Where a handle's events go.
#[derive(Default)]
pub(crate) struct Sink {
    callback: Mutex<Option<Callback>>,
}

impl Sink {
    pub(crate) fn clear(&self) {
        *lock(&self.callback) = None;
    }

    fn send(&self, event: &Event) {
        let Some(callback) = *lock(&self.callback) else {
            return;
        };
        let Ok(text) = CString::new(event_json(event).to_string()) else {
            return;
        };
        // SAFETY: the app registered `call` for `ctx` and keeps both valid until it clears
        // the callback or closes the handle; the text lives for the call.
        unsafe { (callback.call)(callback.ctx as *mut c_void, text.as_ptr()) };
    }
}

/// The service's `Emit`, forwarding to `sink`.
pub(crate) fn emit(sink: &Arc<Sink>) -> Emit {
    let sink = Arc::clone(sink);
    Arc::new(move |event: Event| sink.send(&event))
}

/// `{"type":"importProgress","progress":ImportProgress}`,
/// `{"type":"exportProgress","progress":ExportProgress}` or
/// `{"type":"viewsInvalidated","views":[{"viewId":N,"total":N}]}`.
pub(crate) fn event_json(event: &Event) -> serde_json::Value {
    match event {
        Event::ImportProgress(progress) => json!({"type": "importProgress", "progress": progress}),
        Event::ExportProgress(progress) => json!({"type": "exportProgress", "progress": progress}),
        Event::ViewsInvalidated(views) => json!({"type": "viewsInvalidated", "views": views.views}),
    }
}

/// Sets (or with a null `callback`, clears) where the events of `lib` go. The callback may be
/// called on any thread until it is cleared or the handle is closed.
///
/// # Safety
/// `lib` is a live handle; `callback` and `ctx` stay valid until cleared or `egl_close`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_set_event_callback(
    lib: *mut EglLibrary,
    callback: EglEventCallback,
    ctx: *mut c_void,
) -> EglStatus {
    // SAFETY: the caller's live handle.
    unsafe {
        with_lib(lib, |handle| {
            *lock(&handle.events.callback) = callback.map(|call| Callback {
                call,
                ctx: ctx as usize,
            });
            EglStatus::Ok
        })
    }
}
