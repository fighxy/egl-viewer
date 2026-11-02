//! C ABI over [`library_native`] for the experimental iOS shell (D-082).
//!
//! Native `.eglib` only. Eagle is not linked or exposed. `unsafe` is confined to
//! the FFI boundary (same pattern as `src-tauri` WebAuthn).

#![allow(unsafe_code)]
#![allow(clippy::missing_safety_doc)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::ptr;
use std::sync::Mutex;

use library_core::{ItemCell, ItemEdit, MAX_WINDOW, Scope, SourceError};
use library_native::{Library, OpenOptions, create_library};
use serde::Serialize;

/// Opaque handle for Swift / C callers.
pub struct EglLibrary {
    library: Library,
    last_error: Mutex<String>,
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewWindow<'a> {
    total: usize,
    start: u32,
    items: &'a [ItemCell],
}

static GLOBAL_ERROR: Mutex<String> = Mutex::new(String::new());

fn set_global_error(message: impl Into<String>) {
    if let Ok(mut slot) = GLOBAL_ERROR.lock() {
        *slot = message.into();
    }
}

fn set_error(lib: &EglLibrary, message: impl Into<String>) {
    if let Ok(mut slot) = lib.last_error.lock() {
        *slot = message.into();
    }
}

fn map_error(err: &SourceError) -> EglStatus {
    match err {
        SourceError::Invalid(_) | SourceError::BadRequest => EglStatus::Invalid,
        SourceError::NotFound => EglStatus::NotFound,
        SourceError::ReadOnly => EglStatus::ReadOnly,
        SourceError::NotALibrary { .. } => EglStatus::NotALibrary,
        SourceError::Storage(_) => EglStatus::Storage,
        SourceError::NotSupported(_)
        | SourceError::NewerFormat { .. }
        | SourceError::UnsupportedLocation(_) => EglStatus::Unsupported,
        _ => EglStatus::Other,
    }
}

fn fail(lib: &EglLibrary, status: EglStatus, message: impl Into<String>) -> EglStatus {
    set_error(lib, message);
    status
}

fn fail_source(lib: &EglLibrary, err: SourceError) -> EglStatus {
    let status = map_error(&err);
    set_error(lib, err.to_string());
    status
}

fn c_str<'a>(ptr: *const c_char) -> Result<&'a str, EglStatus> {
    if ptr.is_null() {
        return Err(EglStatus::Invalid);
    }
    // SAFETY: caller promises a valid NUL-terminated UTF-8 C string.
    let raw = unsafe { CStr::from_ptr(ptr) };
    raw.to_str().map_err(|_| EglStatus::Invalid)
}

fn alloc_string(value: impl Into<String>) -> Result<*mut c_char, EglStatus> {
    let c = CString::new(value.into()).map_err(|_| EglStatus::Invalid)?;
    Ok(c.into_raw())
}

fn write_string(out: *mut *mut c_char, value: impl Into<String>) -> Result<(), EglStatus> {
    if out.is_null() {
        return Err(EglStatus::Invalid);
    }
    let ptr = alloc_string(value)?;
    // SAFETY: out is a non-null pointer provided by the caller.
    unsafe {
        *out = ptr;
    }
    Ok(())
}

fn write_json<T: Serialize>(out: *mut *mut c_char, value: &T) -> Result<(), EglStatus> {
    let json = serde_json::to_string(value).map_err(|_| EglStatus::Other)?;
    write_string(out, json)
}

fn tls_cstr(message: String) -> *const c_char {
    thread_local! {
        static BUF: std::cell::RefCell<CString> =
            std::cell::RefCell::new(CString::new("").expect("empty CString"));
    }
    BUF.with(|buf| {
        let mut slot = buf.borrow_mut();
        *slot = CString::new(message).unwrap_or_else(|_| CString::new("").expect("empty CString"));
        slot.as_ptr()
    })
}

/// Creates `<parent>/<name>.eglib`. On success `*out_path` is an absolute UTF-8 path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_create_library(
    parent_utf8: *const c_char,
    name_utf8: *const c_char,
    app_version_utf8: *const c_char,
    out_path: *mut *mut c_char,
) -> EglStatus {
    let parent = match c_str(parent_utf8) {
        Ok(value) => value,
        Err(status) => {
            set_global_error("parent path is required");
            return status;
        }
    };
    let name = match c_str(name_utf8) {
        Ok(value) => value,
        Err(status) => {
            set_global_error("library name is required");
            return status;
        }
    };
    let version = c_str(app_version_utf8).unwrap_or("0.0.0-ios");
    match create_library(Path::new(parent), name, version) {
        Ok(path) => match write_string(out_path, path.to_string_lossy()) {
            Ok(()) => EglStatus::Ok,
            Err(status) => {
                set_global_error("failed to copy library path");
                status
            }
        },
        Err(err) => {
            set_global_error(err.to_string());
            map_error(&err)
        }
    }
}

/// Opens a native library. Returns null on failure; see [`egl_last_error_global`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_open(
    path_utf8: *const c_char,
    app_version_utf8: *const c_char,
    sync_folder: c_int,
) -> *mut EglLibrary {
    let path = match c_str(path_utf8) {
        Ok(value) => value,
        Err(_) => {
            set_global_error("path is required");
            return ptr::null_mut();
        }
    };
    let version = c_str(app_version_utf8).unwrap_or("0.0.0-ios");
    let mut options = OpenOptions::new(version);
    options.sync_folder = sync_folder != 0;
    match Library::open(Path::new(path), &options) {
        Ok(library) => Box::into_raw(Box::new(EglLibrary {
            library,
            last_error: Mutex::new(String::new()),
        })),
        Err(err) => {
            set_global_error(err.to_string());
            ptr::null_mut()
        }
    }
}

/// Closes the library and frees the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_close(lib: *mut EglLibrary) {
    if lib.is_null() {
        return;
    }
    // SAFETY: lib was produced by egl_open and not closed yet.
    let handle = unsafe { Box::from_raw(lib) };
    let _ = handle.library.close();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_library_info(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        match write_json(out_json, &handle.library.info()) {
            Ok(()) => EglStatus::Ok,
            Err(status) => fail(handle, status, "failed to serialize library info"),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_view_window(
    lib: *mut EglLibrary,
    scope_json: *const c_char,
    start: u32,
    len: u32,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let raw = c_str(scope_json).unwrap_or("{}");
        let scope: Scope = if raw.trim().is_empty() || raw == "{}" {
            Scope::All
        } else {
            match serde_json::from_str(raw) {
                Ok(scope) => scope,
                Err(err) => {
                    return fail(
                        handle,
                        EglStatus::Invalid,
                        format!("invalid scope JSON: {err}"),
                    );
                }
            }
        };
        let items = match handle.library.load_scope(&scope) {
            Ok(items) => items,
            Err(err) => return fail_source(handle, err),
        };
        let total = items.len();
        let take = (len as usize).min(MAX_WINDOW);
        let window: Vec<ItemCell> = items
            .iter()
            .skip(start as usize)
            .take(take)
            .map(ItemCell::from)
            .collect();
        let payload = ViewWindow {
            total,
            start,
            items: &window,
        };
        match write_json(out_json, &payload) {
            Ok(()) => EglStatus::Ok,
            Err(status) => fail(handle, status, "failed to serialize view window"),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_item(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let id = match c_str(id_utf8) {
            Ok(id) => id,
            Err(status) => return fail(handle, status, "item id is required"),
        };
        match handle.library.item(id) {
            Ok(Some(item)) => match write_json(out_json, &item) {
                Ok(()) => EglStatus::Ok,
                Err(status) => fail(handle, status, "failed to serialize item"),
            },
            Ok(None) => fail(handle, EglStatus::NotFound, "item not found"),
            Err(err) => fail_source(handle, err),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_folder_tree(
    lib: *mut EglLibrary,
    out_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| match handle.library.folder_tree() {
        Ok(tree) => match write_json(out_json, &tree) {
            Ok(()) => EglStatus::Ok,
            Err(status) => fail(handle, status, "failed to serialize folder tree"),
        },
        Err(err) => fail_source(handle, err),
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_tags(lib: *mut EglLibrary, out_json: *mut *mut c_char) -> EglStatus {
    with_lib(lib, |handle| match handle.library.tags() {
        Ok(tags) => match write_json(out_json, &tags) {
            Ok(()) => EglStatus::Ok,
            Err(status) => fail(handle, status, "failed to serialize tags"),
        },
        Err(err) => fail_source(handle, err),
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_thumbnail_path(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_path: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let id = match c_str(id_utf8) {
            Ok(id) => id,
            Err(status) => return fail(handle, status, "item id is required"),
        };
        match handle.library.thumbnail_path(id) {
            Ok(path) => match write_string(out_path, path.to_string_lossy()) {
                Ok(()) => EglStatus::Ok,
                Err(status) => fail(handle, status, "failed to copy thumbnail path"),
            },
            Err(err) => fail_source(handle, err),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_thumbnail_bytes(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    out_bytes: *mut *mut u8,
    out_len: *mut usize,
    out_content_type: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        if out_bytes.is_null() || out_len.is_null() {
            return fail(
                handle,
                EglStatus::Invalid,
                "out_bytes and out_len are required",
            );
        }
        let id = match c_str(id_utf8) {
            Ok(id) => id,
            Err(status) => return fail(handle, status, "item id is required"),
        };
        let thumb = match handle.library.thumbnail(id) {
            Ok(Some(thumb)) => thumb,
            Ok(None) => return fail(handle, EglStatus::NotFound, "thumbnail not found"),
            Err(err) => return fail_source(handle, err),
        };
        let len = thumb.bytes.len();
        let mut boxed = thumb.bytes.into_boxed_slice();
        let ptr = boxed.as_mut_ptr();
        std::mem::forget(boxed);
        // SAFETY: out pointers are non-null (checked above).
        unsafe {
            *out_bytes = ptr;
            *out_len = len;
        }
        if !out_content_type.is_null()
            && let Err(status) = write_string(out_content_type, thumb.content_type)
        {
            return fail(handle, status, "failed to copy content type");
        }
        EglStatus::Ok
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_apply_item_edit(
    lib: *mut EglLibrary,
    id_utf8: *const c_char,
    edit_json: *const c_char,
    out_item_json: *mut *mut c_char,
) -> EglStatus {
    with_lib(lib, |handle| {
        let id = match c_str(id_utf8) {
            Ok(id) => id,
            Err(status) => return fail(handle, status, "item id is required"),
        };
        let raw = match c_str(edit_json) {
            Ok(raw) => raw,
            Err(status) => return fail(handle, status, "edit JSON is required"),
        };
        let edit: ItemEdit = match serde_json::from_str(raw) {
            Ok(edit) => edit,
            Err(err) => {
                return fail(
                    handle,
                    EglStatus::Invalid,
                    format!("invalid edit JSON: {err}"),
                );
            }
        };
        if let Err(err) = edit.validate() {
            return fail_source(handle, err);
        }
        match handle.library.apply_item_edit(id, &edit) {
            Ok(item) => match write_json(out_item_json, &item) {
                Ok(()) => EglStatus::Ok,
                Err(status) => fail(handle, status, "failed to serialize item"),
            },
            Err(err) => fail_source(handle, err),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    // SAFETY: s was allocated by alloc_string / CString::into_raw.
    drop(unsafe { CString::from_raw(s) });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_bytes_free(bytes: *mut u8, len: usize) {
    if bytes.is_null() || len == 0 {
        return;
    }
    // SAFETY: bytes/len pair from egl_thumbnail_bytes (forgotten Box<[u8]>).
    drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(bytes, len)) });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_last_error(lib: *mut EglLibrary) -> *const c_char {
    if lib.is_null() {
        // SAFETY: egl_last_error_global only reads process state and returns a TLS C string.
        return unsafe { egl_last_error_global() };
    }
    // SAFETY: lib is a live handle from egl_open.
    let handle = unsafe { &*lib };
    let message = handle
        .last_error
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_default();
    tls_cstr(message)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn egl_last_error_global() -> *const c_char {
    let message = GLOBAL_ERROR
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_default();
    tls_cstr(message)
}

fn with_lib(lib: *mut EglLibrary, body: impl FnOnce(&EglLibrary) -> EglStatus) -> EglStatus {
    if lib.is_null() {
        set_global_error("library handle is null");
        return EglStatus::Invalid;
    }
    // SAFETY: lib is a live handle from egl_open.
    let handle = unsafe { &*lib };
    body(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn cstring(value: &str) -> CString {
        CString::new(value).unwrap()
    }

    #[test]
    fn create_open_empty_window_and_close() {
        let dir = tempfile::tempdir().unwrap();
        let parent = cstring(dir.path().to_str().unwrap());
        let name = cstring("Mobile");
        let version = cstring("0.0.0-test");
        let mut out_path: *mut c_char = ptr::null_mut();

        let status = unsafe {
            egl_create_library(
                parent.as_ptr(),
                name.as_ptr(),
                version.as_ptr(),
                &mut out_path,
            )
        };
        assert_eq!(status, EglStatus::Ok);
        assert!(!out_path.is_null());

        let lib = unsafe { egl_open(out_path, version.as_ptr(), 0) };
        assert!(!lib.is_null());

        let mut info_json: *mut c_char = ptr::null_mut();
        assert_eq!(
            unsafe { egl_library_info(lib, &mut info_json) },
            EglStatus::Ok
        );
        let info = unsafe { CStr::from_ptr(info_json) }.to_str().unwrap();
        assert!(info.contains("\"kind\":\"native\""));
        assert!(info.contains("Mobile"));
        unsafe { egl_string_free(info_json) };

        let scope = cstring(r#"{"kind":"all"}"#);
        let mut window_json: *mut c_char = ptr::null_mut();
        assert_eq!(
            unsafe { egl_view_window(lib, scope.as_ptr(), 0, 50, &mut window_json) },
            EglStatus::Ok
        );
        let window = unsafe { CStr::from_ptr(window_json) }.to_str().unwrap();
        assert!(window.contains("\"total\":0"));
        assert!(window.contains("\"items\":[]"));
        unsafe { egl_string_free(window_json) };

        let mut tags_json: *mut c_char = ptr::null_mut();
        assert_eq!(unsafe { egl_tags(lib, &mut tags_json) }, EglStatus::Ok);
        unsafe { egl_string_free(tags_json) };

        let mut folders_json: *mut c_char = ptr::null_mut();
        assert_eq!(
            unsafe { egl_folder_tree(lib, &mut folders_json) },
            EglStatus::Ok
        );
        unsafe { egl_string_free(folders_json) };

        unsafe {
            egl_close(lib);
            egl_string_free(out_path);
        }
    }

    #[test]
    fn rejects_null_open_path() {
        let version = cstring("0.0.0-test");
        let lib = unsafe { egl_open(ptr::null(), version.as_ptr(), 0) };
        assert!(lib.is_null());
        let message = unsafe { CStr::from_ptr(egl_last_error_global()) }
            .to_str()
            .unwrap();
        assert!(!message.is_empty());
    }

    #[test]
    fn thumbnail_path_and_bytes_after_preview() {
        use image::ImageFormat;
        use library_native::{Duplicates, Library, OpenOptions};
        use media::fixtures;

        let dir = tempfile::tempdir().unwrap();
        let parent = cstring(dir.path().to_str().unwrap());
        let name = cstring("Thumbs");
        let version = cstring("0.0.0-test");
        let mut out_path: *mut c_char = ptr::null_mut();
        assert_eq!(
            unsafe {
                egl_create_library(
                    parent.as_ptr(),
                    name.as_ptr(),
                    version.as_ptr(),
                    &mut out_path,
                )
            },
            EglStatus::Ok
        );
        let root = unsafe { CStr::from_ptr(out_path) }
            .to_str()
            .unwrap()
            .to_owned();
        unsafe { egl_string_free(out_path) };

        // Writer lock: seed the library through library-native, then reopen via FFI.
        let id = {
            let library = Library::open(Path::new(&root), &OpenOptions::new("0.0.0-test")).unwrap();
            let png = dir.path().join("seed.png");
            std::fs::write(&png, fixtures::raster(ImageFormat::Png, 40, 20, 7)).unwrap();
            let library_native::AddOutcome::Added { id, file } =
                library.add_file(&png, Duplicates::Report).unwrap()
            else {
                panic!("expected a new item");
            };
            let rendered = media::render(&file, "png").unwrap();
            library.store_preview(&id, &rendered).unwrap();
            library.close().unwrap();
            id
        };

        let root_c = cstring(&root);
        let lib = unsafe { egl_open(root_c.as_ptr(), version.as_ptr(), 0) };
        assert!(!lib.is_null());

        let id_c = cstring(&id);
        let mut thumb_path: *mut c_char = ptr::null_mut();
        assert_eq!(
            unsafe { egl_thumbnail_path(lib, id_c.as_ptr(), &mut thumb_path) },
            EglStatus::Ok
        );
        let path = unsafe { CStr::from_ptr(thumb_path) }.to_str().unwrap();
        assert!(path.ends_with(".webp"), "{path}");
        assert!(std::path::Path::new(path).is_file());
        unsafe { egl_string_free(thumb_path) };

        let mut bytes: *mut u8 = ptr::null_mut();
        let mut len: usize = 0;
        let mut content_type: *mut c_char = ptr::null_mut();
        assert_eq!(
            unsafe {
                egl_thumbnail_bytes(lib, id_c.as_ptr(), &mut bytes, &mut len, &mut content_type)
            },
            EglStatus::Ok
        );
        assert!(len > 12);
        assert!(!bytes.is_null());
        let slice = unsafe { std::slice::from_raw_parts(bytes, len) };
        assert_eq!(&slice[..4], b"RIFF");
        let ctype = unsafe { CStr::from_ptr(content_type) }.to_str().unwrap();
        assert_eq!(ctype, "image/webp");
        unsafe {
            egl_bytes_free(bytes, len);
            egl_string_free(content_type);
            egl_close(lib);
        }
    }

    #[test]
    fn thumbnail_bytes_missing_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let parent = cstring(dir.path().to_str().unwrap());
        let name = cstring("EmptyThumbs");
        let version = cstring("0.0.0-test");
        let mut out_path: *mut c_char = ptr::null_mut();
        assert_eq!(
            unsafe {
                egl_create_library(
                    parent.as_ptr(),
                    name.as_ptr(),
                    version.as_ptr(),
                    &mut out_path,
                )
            },
            EglStatus::Ok
        );
        let lib = unsafe { egl_open(out_path, version.as_ptr(), 0) };
        assert!(!lib.is_null());
        let missing = cstring("01NOTANITEM000000000000000");
        let mut bytes: *mut u8 = ptr::null_mut();
        let mut len: usize = 0;
        let mut content_type: *mut c_char = ptr::null_mut();
        assert_eq!(
            unsafe {
                egl_thumbnail_bytes(
                    lib,
                    missing.as_ptr(),
                    &mut bytes,
                    &mut len,
                    &mut content_type,
                )
            },
            EglStatus::NotFound
        );
        unsafe {
            egl_close(lib);
            egl_string_free(out_path);
        }
    }
}
