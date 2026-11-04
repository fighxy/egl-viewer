//! C ABI over [`app_service::LibraryService`] for the iOS and Android shells (D-082, D-083,
//! D-092): the same open library, views, undo and tasks as the desktop, without Tauri and
//! without Eagle.
//!
//! Conventions: strings are UTF-8, NUL-terminated; JSON goes in and out as such strings. Rust
//! allocates every string and byte buffer it hands out; free them with `egl_string_free` and
//! `egl_bytes_free`. Every call returns an [`EglStatus`]; the message of a failure is in
//! `egl_last_error(lib)` (or `egl_last_error_global()` before a handle exists). A panic inside
//! Rust never crosses the boundary: the call returns `EGL_ERR_OTHER`.
//!
//! `unsafe` is confined to this crate: it is the boundary. Every function that takes a pointer
//! is `unsafe` for its caller and says what it needs (the SAFETY notes).

#![allow(unsafe_code)]

mod decoder;
mod events;
mod maintenance;
mod manage;
mod read;

pub use events::*;
pub use maintenance::*;
pub use manage::*;
pub use read::*;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::ptr;
use std::sync::{Arc, Mutex};

use app_service::{LibraryService, lock};
use library_core::SourceError;
use library_native::create_library;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Opaque handle for Swift / C callers: one open library.
pub struct EglLibrary {
    service: LibraryService,
    last_error: Mutex<String>,
    /// Where the service's events go; set with `egl_set_event_callback`.
    events: Arc<events::Sink>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EglStatus {
    Ok = 0,
    Invalid = 1,
    NotFound = 2,
    ReadOnly = 3,
    NotALibrary = 4,
    Storage = 5,
    Unsupported = 6,
    Other = 7,
    WrongPassword = 8,
    Locked = 9,
}

static GLOBAL_ERROR: Mutex<String> = Mutex::new(String::new());

fn set_global_error(message: impl Into<String>) {
    *lock(&GLOBAL_ERROR) = message.into();
}

pub(crate) fn set_error(lib: &EglLibrary, message: impl Into<String>) {
    *lock(&lib.last_error) = message.into();
}

pub(crate) fn map_error(err: &SourceError) -> EglStatus {
    match err {
        SourceError::Invalid(_) | SourceError::BadRequest => EglStatus::Invalid,
        SourceError::NotFound => EglStatus::NotFound,
        SourceError::ReadOnly => EglStatus::ReadOnly,
        SourceError::NotALibrary { .. } => EglStatus::NotALibrary,
        SourceError::Storage(_) => EglStatus::Storage,
        SourceError::WrongPassword => EglStatus::WrongPassword,
        SourceError::Locked { .. } => EglStatus::Locked,
        SourceError::NotSupported(_)
        | SourceError::NewerFormat { .. }
        | SourceError::UnsupportedLocation(_) => EglStatus::Unsupported,
        _ => EglStatus::Other,
    }
}

pub(crate) fn fail(lib: &EglLibrary, status: EglStatus, message: impl Into<String>) -> EglStatus {
    set_error(lib, message);
    status
}

pub(crate) fn fail_source(lib: &EglLibrary, err: SourceError) -> EglStatus {
    let status = map_error(&err);
    set_error(lib, err.to_string());
    status
}

/// Borrows a C string.
///
/// # Safety
/// `ptr` is null or points at a NUL-terminated string that outlives the returned borrow.
pub(crate) unsafe fn c_str<'a>(ptr: *const c_char) -> Result<&'a str, EglStatus> {
    if ptr.is_null() {
        return Err(EglStatus::Invalid);
    }
    // SAFETY: non-null, and the caller promises a NUL-terminated string.
    let raw = unsafe { CStr::from_ptr(ptr) };
    raw.to_str().map_err(|_| EglStatus::Invalid)
}

/// Hands a string to the caller in `*out`.
///
/// # Safety
/// `out` is null or a valid place for one `*mut c_char`.
pub(crate) unsafe fn write_string(
    out: *mut *mut c_char,
    value: impl Into<String>,
) -> Result<(), EglStatus> {
    if out.is_null() {
        return Err(EglStatus::Invalid);
    }
    let value = CString::new(value.into()).map_err(|_| EglStatus::Invalid)?;
    // SAFETY: non-null, and the caller promises it is writable.
    unsafe { *out = value.into_raw() };
    Ok(())
}

/// Hands `value` as JSON to the caller in `*out`.
///
/// # Safety
/// As [`write_string`].
pub(crate) unsafe fn write_json<T: Serialize>(
    out: *mut *mut c_char,
    value: &T,
) -> Result<(), EglStatus> {
    let json = serde_json::to_string(value).map_err(|_| EglStatus::Other)?;
    // SAFETY: forwarded from the caller.
    unsafe { write_string(out, json) }
}

/// Writes a count to `*out` when the caller asked for it.
///
/// # Safety
/// `out` is null or a valid place for one `u32`.
pub(crate) unsafe fn write_count(out: *mut u32, value: u32) {
    if !out.is_null() {
        // SAFETY: non-null, and the caller promises it is writable.
        unsafe { *out = value };
    }
}

/// A required string argument.
///
/// # Safety
/// As [`c_str`].
pub(crate) unsafe fn arg<'a>(
    lib: &EglLibrary,
    raw: *const c_char,
    what: &str,
) -> Result<&'a str, EglStatus> {
    // SAFETY: forwarded from the caller.
    unsafe { c_str(raw) }.map_err(|status| fail(lib, status, format!("{what} is required")))
}

/// An optional string argument: null is `None`.
///
/// # Safety
/// As [`c_str`].
pub(crate) unsafe fn optional<'a>(
    lib: &EglLibrary,
    raw: *const c_char,
    what: &str,
) -> Result<Option<&'a str>, EglStatus> {
    if raw.is_null() {
        return Ok(None);
    }
    // SAFETY: forwarded from the caller.
    unsafe { arg(lib, raw, what) }.map(Some)
}

/// A required JSON argument.
///
/// # Safety
/// As [`c_str`].
pub(crate) unsafe fn parse<T: DeserializeOwned>(
    lib: &EglLibrary,
    raw: *const c_char,
    what: &str,
) -> Result<T, EglStatus> {
    // SAFETY: forwarded from the caller.
    let text = unsafe { arg(lib, raw, what) }?;
    serde_json::from_str(text)
        .map_err(|err| fail(lib, EglStatus::Invalid, format!("invalid {what}: {err}")))
}

/// A JSON argument where null or empty means the default.
///
/// # Safety
/// As [`c_str`].
pub(crate) unsafe fn parse_or_default<T: DeserializeOwned + Default>(
    lib: &EglLibrary,
    raw: *const c_char,
    what: &str,
) -> Result<T, EglStatus> {
    // SAFETY: forwarded from the caller.
    let text = unsafe { c_str(raw) }.unwrap_or("");
    if text.trim().is_empty() {
        return Ok(T::default());
    }
    serde_json::from_str(text)
        .map_err(|err| fail(lib, EglStatus::Invalid, format!("invalid {what}: {err}")))
}

/// The answer of a call: `value` as JSON in `*out`, or the error.
///
/// # Safety
/// As [`write_string`].
pub(crate) unsafe fn reply<T: Serialize>(
    lib: &EglLibrary,
    out: *mut *mut c_char,
    result: Result<T, SourceError>,
    what: &str,
) -> EglStatus {
    match result {
        // SAFETY: forwarded from the caller.
        Ok(value) => match unsafe { write_json(out, &value) } {
            Ok(()) => EglStatus::Ok,
            Err(status) => fail(lib, status, format!("failed to hand over the {what}")),
        },
        Err(err) => fail_source(lib, err),
    }
}

/// The answer of a call that returns a count.
///
/// # Safety
/// As [`write_count`].
pub(crate) unsafe fn reply_count(
    lib: &EglLibrary,
    out: *mut u32,
    result: Result<u32, SourceError>,
) -> EglStatus {
    match result {
        Ok(count) => {
            // SAFETY: forwarded from the caller.
            unsafe { write_count(out, count) };
            EglStatus::Ok
        }
        Err(err) => fail_source(lib, err),
    }
}

pub(crate) fn reply_unit(lib: &EglLibrary, result: Result<(), SourceError>) -> EglStatus {
    match result {
        Ok(()) => EglStatus::Ok,
        Err(err) => fail_source(lib, err),
    }
}

/// `try`-style early return for `Result<_, EglStatus>` inside `with_lib` bodies.
macro_rules! tri {
    ($expr:expr) => {
        match $expr {
            Ok(value) => value,
            Err(status) => return status,
        }
    };
}
pub(crate) use tri;

/// Runs `body` and turns a panic into `EGL_ERR_OTHER`: unwinding never crosses into C.
pub(crate) fn guarded(
    on_panic: impl FnOnce(String),
    body: impl FnOnce() -> EglStatus,
) -> EglStatus {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(status) => status,
        Err(panic) => {
            let detail = panic
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            on_panic(format!("internal error: {detail}"));
            EglStatus::Other
        }
    }
}

/// Runs `body` with the handle behind `lib`.
///
/// # Safety
/// `lib` is null or a live handle from `egl_open`, not closed yet.
pub(crate) unsafe fn with_lib(
    lib: *mut EglLibrary,
    body: impl FnOnce(&EglLibrary) -> EglStatus,
) -> EglStatus {
    if lib.is_null() {
        set_global_error("library handle is null");
        return EglStatus::Invalid;
    }
    // SAFETY: non-null, and the caller promises a live handle.
    let handle = unsafe { &*lib };
    guarded(|message| set_error(handle, message), || body(handle))
}

/// A thread-local copy of `message` that stays valid until this thread asks again.
fn tls_cstr(message: String) -> *const c_char {
    thread_local! {
        static BUF: std::cell::RefCell<CString> = std::cell::RefCell::new(CString::default());
    }
    BUF.with(|buf| {
        let mut slot = buf.borrow_mut();
        *slot = CString::new(message).unwrap_or_default();
        slot.as_ptr()
    })
}

/// Creates `<parent>/<name>.eglib`. On success `*out_path` is its absolute path.
///
/// # Safety
/// The strings are NUL-terminated UTF-8; `out_path` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_create_library(
    parent_utf8: *const c_char,
    name_utf8: *const c_char,
    app_version_utf8: *const c_char,
    out_path: *mut *mut c_char,
) -> EglStatus {
    guarded(set_global_error, || {
        // SAFETY: the caller's strings.
        let (parent, name, version) = unsafe {
            (
                c_str(parent_utf8),
                c_str(name_utf8),
                c_str(app_version_utf8).unwrap_or("0.0.0-mobile"),
            )
        };
        let (Ok(parent), Ok(name)) = (parent, name) else {
            set_global_error("the parent folder and the library name are required");
            return EglStatus::Invalid;
        };
        match create_library(Path::new(parent), name, version) {
            // SAFETY: the caller's out pointer.
            Ok(path) => match unsafe { write_string(out_path, path.to_string_lossy()) } {
                Ok(()) => EglStatus::Ok,
                Err(status) => {
                    set_global_error("failed to hand over the library path");
                    status
                }
            },
            Err(err) => {
                set_global_error(err.to_string());
                map_error(&err)
            }
        }
    })
}

/// Opens a library folder; `sync_folder != 0` opens it read-only (a cloud-synced place,
/// D-021). Previews that a crash or a cancelled import left undone start in the background.
/// Returns null on failure; the reason is in `egl_last_error_global()`.
///
/// # Safety
/// The strings are NUL-terminated UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_open(
    path_utf8: *const c_char,
    app_version_utf8: *const c_char,
    sync_folder: c_int,
) -> *mut EglLibrary {
    let mut opened = ptr::null_mut();
    guarded(set_global_error, || {
        // SAFETY: the caller's strings.
        let (path, version) = unsafe {
            (
                c_str(path_utf8),
                c_str(app_version_utf8).unwrap_or("0.0.0-mobile"),
            )
        };
        let Ok(path) = path else {
            set_global_error("the library path is required");
            return EglStatus::Invalid;
        };
        let events = Arc::new(events::Sink::default());
        let service = LibraryService::new(version, events::emit(&events));
        match service.open_library(Path::new(path), sync_folder != 0) {
            Ok(_) => {
                opened = Box::into_raw(Box::new(EglLibrary {
                    service,
                    last_error: Mutex::new(String::new()),
                    events,
                }));
                EglStatus::Ok
            }
            Err(err) => {
                set_global_error(err.to_string());
                map_error(&err)
            }
        }
    });
    opened
}

/// Closes the library and frees the handle. Running imports and exports are cancelled and
/// joined first; files already added stay.
///
/// # Safety
/// `lib` is null or a live handle from `egl_open`; it is invalid afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_close(lib: *mut EglLibrary) {
    if lib.is_null() {
        return;
    }
    // SAFETY: the caller hands back a live handle, once.
    let handle = unsafe { Box::from_raw(lib) };
    guarded(set_global_error, || {
        handle.events.clear();
        match handle.service.close_library() {
            Ok(()) => EglStatus::Ok,
            Err(err) => {
                set_global_error(err.to_string());
                map_error(&err)
            }
        }
    });
}

/// `LibraryInfo` of the open library.
///
/// # Safety
/// `lib` is a live handle; `out_json` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_library_info(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    // SAFETY: the caller's handle and out pointer.
    unsafe {
        with_lib(lib, |handle| {
            let info = handle.service.info().ok_or_else(app_service::no_library);
            reply(handle, out_json, info, "library info")
        })
    }
}

/// Frees a string from this library. Null is fine.
///
/// # Safety
/// `s` is null or a string this library handed out, not freed yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    // SAFETY: allocated by `CString::into_raw` in `write_string`.
    drop(unsafe { CString::from_raw(s) });
}

/// Frees a byte buffer from this library. Null is fine.
///
/// # Safety
/// `bytes`/`len` are a buffer this library handed out, not freed yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_bytes_free(bytes: *mut u8, len: usize) {
    if bytes.is_null() || len == 0 {
        return;
    }
    // SAFETY: a forgotten `Box<[u8]>` of this length (`read::hand_over_bytes`).
    drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(bytes, len)) });
}

/// The message of the last failure on `lib`; with null, the process-wide one. Valid until
/// this thread calls again; do not free.
///
/// # Safety
/// `lib` is null or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_last_error(lib: *mut EglLibrary) -> *const c_char {
    if lib.is_null() {
        return tls_cstr(lock(&GLOBAL_ERROR).clone());
    }
    // SAFETY: the caller's live handle.
    let handle = unsafe { &*lib };
    tls_cstr(lock(&handle.last_error).clone())
}

/// The message of the last failure before a handle existed (create, open). Valid until this
/// thread calls again; do not free.
#[unsafe(no_mangle)]
pub extern "C" fn egl_last_error_global() -> *const c_char {
    tls_cstr(lock(&GLOBAL_ERROR).clone())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "manage_tests.rs"]
mod manage_tests;

#[cfg(test)]
mod header_tests;
