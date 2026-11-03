//! Host tests of the handle: create, open, windows, thumbnails.
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
        unsafe { egl_thumbnail_bytes(lib, id_c.as_ptr(), &mut bytes, &mut len, &mut content_type) },
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
