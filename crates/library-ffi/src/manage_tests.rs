//! Host tests for the management exports.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;
use std::time::{Duration, Instant};

use image::ImageFormat;
use media::fixtures;
use serde_json::Value;

use super::*;
use crate::{
    egl_apply_item_edit, egl_close, egl_create_library, egl_folder_tree, egl_item, egl_last_error,
    egl_open, egl_string_free, egl_tags, egl_thumbnail_path,
};

fn cstring(value: &str) -> CString {
    CString::new(value).unwrap()
}

/// A temp dir and an open library in it.
struct Fixture {
    dir: tempfile::TempDir,
    lib: *mut EglLibrary,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        unsafe { egl_close(self.lib) };
    }
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let parent = cstring(dir.path().to_str().unwrap());
    let name = cstring("Manage");
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
    unsafe { egl_string_free(out_path) };
    assert!(!lib.is_null());
    Fixture { dir, lib }
}

fn take(ptr: *mut c_char) -> Value {
    assert!(!ptr.is_null());
    let text = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_owned();
    unsafe { egl_string_free(ptr) };
    serde_json::from_str(&text).unwrap()
}

fn last_error(lib: *mut EglLibrary) -> String {
    unsafe { CStr::from_ptr(egl_last_error(lib)) }
        .to_str()
        .unwrap()
        .to_owned()
}

/// Calls `f(out)` and returns the parsed JSON, asserting `Ok`.
fn json(lib: *mut EglLibrary, f: impl FnOnce(*mut *mut c_char) -> EglStatus) -> Value {
    let mut out: *mut c_char = ptr::null_mut();
    let status = f(&mut out);
    assert_eq!(status, EglStatus::Ok, "{}", last_error(lib));
    take(out)
}

fn count(lib: *mut EglLibrary, f: impl FnOnce(*mut u32) -> EglStatus) -> u32 {
    let mut out = u32::MAX;
    let status = f(&mut out);
    assert_eq!(status, EglStatus::Ok, "{}", last_error(lib));
    out
}

fn png(dir: &Path, name: &str, seed: u8) -> String {
    let path = dir.join(name);
    std::fs::write(
        &path,
        fixtures::raster(ImageFormat::Png, 40, 20, seed.into()),
    )
    .unwrap();
    path.to_str().unwrap().to_owned()
}

fn status(lib: *mut EglLibrary, id: u64) -> Value {
    json(lib, |out| unsafe { egl_import_status(lib, id, out) })
}

fn wait_for(lib: *mut EglLibrary, id: u64, states: &[&str]) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let progress = status(lib, id);
        if let Some(state) = progress.get("state").and_then(Value::as_str)
            && states.contains(&state)
        {
            return progress;
        }
        assert!(Instant::now() < deadline, "import {id} stuck at {progress}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn add(lib: *mut EglLibrary, request: &Value) -> u64 {
    let request = cstring(&request.to_string());
    let ticket = json(lib, |out| unsafe {
        egl_add_files(lib, request.as_ptr(), out)
    });
    ticket["id"].as_u64().unwrap()
}

fn query_window(lib: *mut EglLibrary, query: &Value) -> Value {
    let query = cstring(&query.to_string());
    json(lib, |out| unsafe {
        egl_query_window(lib, query.as_ptr(), 0, 200, out)
    })
}

fn ids_of(window: &Value) -> Vec<String> {
    window["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Adds `n` distinct PNGs and waits for the import (with thumbnails) to finish.
fn seeded(n: u8) -> (Fixture, Vec<String>) {
    let f = fixture();
    let paths: Vec<String> = (0..n)
        .map(|i| png(f.dir.path(), &format!("img{i}.png"), i + 1))
        .collect();
    let id = add(f.lib, &serde_json::json!({ "paths": paths }));
    let done = wait_for(f.lib, id, &["done", "cancelled"]);
    assert_eq!(done["state"], "done");
    assert_eq!(done["added"], u64::from(n));
    let ids = ids_of(&query_window(f.lib, &serde_json::json!({})));
    assert_eq!(ids.len(), usize::from(n));
    (f, ids)
}

#[test]
fn add_files_imports_makes_thumbnails_and_lists_imports() {
    let (f, ids) = seeded(2);
    let id_c = cstring(&ids[0]);
    let mut thumb: *mut c_char = ptr::null_mut();
    assert_eq!(
        unsafe { egl_thumbnail_path(f.lib, id_c.as_ptr(), &mut thumb) },
        EglStatus::Ok
    );
    let thumb = unsafe { CStr::from_ptr(thumb).to_str().unwrap().to_owned() };
    assert!(Path::new(&thumb).is_file(), "{thumb}");

    let imports = json(f.lib, |out| unsafe { egl_imports(f.lib, out) });
    assert_eq!(imports.as_array().unwrap().len(), 1);
    assert_eq!(imports[0]["kind"], "files");
}

#[test]
fn add_files_rejects_bad_requests() {
    let f = fixture();
    let empty = cstring(r#"{"paths":[]}"#);
    let mut out: *mut c_char = ptr::null_mut();
    assert_eq!(
        unsafe { egl_add_files(f.lib, empty.as_ptr(), &mut out) },
        EglStatus::Invalid
    );
    let bad_folder = cstring(r#"{"paths":["/nope.png"],"folder":"missing"}"#);
    assert_eq!(
        unsafe { egl_add_files(f.lib, bad_folder.as_ptr(), &mut out) },
        EglStatus::Invalid
    );
    assert!(last_error(f.lib).contains("folder"));
    assert_eq!(
        unsafe { egl_import_status(f.lib, 999, &mut out) },
        EglStatus::NotFound
    );
}

#[test]
fn duplicates_wait_for_an_answer() {
    let (f, _) = seeded(1);
    let path = f.dir.path().join("img0.png");
    let id = add(f.lib, &serde_json::json!({ "paths": [path] }));
    let waiting = wait_for(f.lib, id, &["waitingForDecision", "done"]);
    assert_eq!(waiting["state"], "waitingForDecision");
    let bad = cstring("maybe");
    assert_eq!(
        unsafe { egl_answer_import(f.lib, id, bad.as_ptr()) },
        EglStatus::Invalid
    );
    let skip = cstring("skip");
    assert_eq!(
        unsafe { egl_answer_import(f.lib, id, skip.as_ptr()) },
        EglStatus::Ok
    );
    let done = wait_for(f.lib, id, &["done", "cancelled"]);
    assert_eq!(done["duplicates"], 1);
    assert_eq!(done["added"], 0);
}

#[test]
fn cancel_import_stops_a_waiting_import() {
    let (f, _) = seeded(1);
    let path = f.dir.path().join("img0.png");
    let id = add(f.lib, &serde_json::json!({ "paths": [path] }));
    wait_for(f.lib, id, &["waitingForDecision"]);
    assert_eq!(unsafe { egl_cancel_import(f.lib, id) }, EglStatus::Ok);
    let done = wait_for(f.lib, id, &["done", "cancelled"]);
    assert_eq!(done["state"], "cancelled");
    // The final progress can be seen just before the thread leaves the running set.
    let deadline = Instant::now() + Duration::from_secs(10);
    while unsafe { egl_cancel_import(f.lib, id) } != EglStatus::NotFound {
        assert!(Instant::now() < deadline, "import {id} never finished");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn closing_with_a_waiting_import_does_not_hang() {
    let (f, _) = seeded(1);
    let path = f.dir.path().join("img0.png");
    let id = add(f.lib, &serde_json::json!({ "paths": [path] }));
    wait_for(f.lib, id, &["waitingForDecision"]);
    drop(f); // egl_close cancels and joins.
}

#[test]
fn generate_previews_rebuilds_thumbnails() {
    let (f, _) = seeded(1);
    let ticket = json(f.lib, |out| unsafe { egl_generate_previews(f.lib, 1, out) });
    assert_eq!(ticket["waiting"], 1);
    let id = ticket["id"].as_u64().unwrap();
    let done = wait_for(f.lib, id, &["done", "cancelled"]);
    assert_eq!(done["kind"], "previews");
}

#[test]
fn original_path_points_at_the_stored_file() {
    let (f, ids) = seeded(1);
    let id = cstring(&ids[0]);
    let mut out: *mut c_char = ptr::null_mut();
    assert_eq!(
        unsafe { egl_original_path(f.lib, id.as_ptr(), &mut out) },
        EglStatus::Ok
    );
    let path = unsafe { CStr::from_ptr(out).to_str().unwrap().to_owned() };
    unsafe { egl_string_free(out) };
    assert!(Path::new(&path).is_file(), "{path}");
    assert!(path.ends_with(".png"));
    let missing = cstring("01NOTANITEM000000000000000");
    assert_ne!(
        unsafe { egl_original_path(f.lib, missing.as_ptr(), &mut out) },
        EglStatus::Ok
    );
}

#[test]
fn trash_restore_undo_and_delete_permanently() {
    let (f, ids) = seeded(3);
    let lib = f.lib;
    let target = cstring(&serde_json::json!({ "ids": [ids[0], ids[1]] }).to_string());
    assert_eq!(
        count(lib, |out| unsafe {
            egl_trash_items(lib, target.as_ptr(), out)
        }),
        2
    );
    let info = json(lib, |out| unsafe { egl_trash_info(lib, out) });
    assert_eq!(info["items"], 2);
    assert!(info["bytes"].as_u64().unwrap() > 0);
    assert_eq!(query_window(lib, &serde_json::json!({}))["total"], 1);

    let status = json(lib, |out| unsafe { egl_undo_status(lib, out) });
    assert_eq!(status["kind"], "trash");
    assert_eq!(status["items"], 2);
    let undone = json(lib, |out| unsafe { egl_undo(lib, out) });
    assert_eq!(undone["kind"], "trash");
    assert_eq!(query_window(lib, &serde_json::json!({}))["total"], 3);
    assert!(json(lib, |out| unsafe { egl_undo(lib, out) }).is_null());

    assert_eq!(
        count(lib, |out| unsafe {
            egl_trash_items(lib, target.as_ptr(), out)
        }),
        2
    );
    assert_eq!(count(lib, |out| unsafe { egl_restore_trash(lib, out) }), 2);
    assert_eq!(
        json(lib, |out| unsafe { egl_trash_info(lib, out) })["items"],
        0
    );

    // Select All in All except ids[2], then delete one of them for good.
    let all =
        cstring(&serde_json::json!({ "all": true, "ids": [ids[2]], "query": {} }).to_string());
    assert_eq!(
        count(lib, |out| unsafe {
            egl_trash_items(lib, all.as_ptr(), out)
        }),
        2
    );
    let one = cstring(&serde_json::json!({ "ids": [ids[0]] }).to_string());
    assert_eq!(
        count(lib, |out| unsafe {
            egl_delete_permanently(lib, one.as_ptr(), out)
        }),
        1
    );
    let id0 = cstring(&ids[0]);
    let mut out: *mut c_char = ptr::null_mut();
    assert_eq!(
        unsafe { egl_item(lib, id0.as_ptr(), &mut out) },
        EglStatus::NotFound
    );

    let trash = query_window(lib, &serde_json::json!({ "scope": { "kind": "trash" } }));
    assert_eq!(trash["total"], 1);
    assert_eq!(unsafe { egl_cancel_empty_trash(lib) }, EglStatus::Ok);
    assert_eq!(count(lib, |out| unsafe { egl_empty_trash(lib, out) }), 1);
    assert_eq!(
        json(lib, |out| unsafe { egl_trash_info(lib, out) })["items"],
        0
    );
}

#[test]
fn folders_create_rename_reorder_and_delete() {
    let (f, ids) = seeded(2);
    let lib = f.lib;
    let create = cstring(
        r#"{"type":"create","name":"Refs","parent":null,"description":"","iconColor":null}"#,
    );
    let tree = json(lib, |out| unsafe {
        egl_save_folder(lib, create.as_ptr(), out)
    });
    let folder = tree[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(tree[0]["name"], "Refs");

    let rename = cstring(
        &serde_json::json!({
            "type": "update", "id": folder, "name": "References", "description": "",
            "tags": [], "iconColor": null, "orderBy": null, "sortAscending": null
        })
        .to_string(),
    );
    let tree = json(lib, |out| unsafe {
        egl_save_folder(lib, rename.as_ptr(), out)
    });
    assert_eq!(tree[0]["name"], "References");

    let blank =
        cstring(r#"{"type":"create","name":"  ","parent":null,"description":"","iconColor":null}"#);
    let mut out: *mut c_char = ptr::null_mut();
    assert_eq!(
        unsafe { egl_save_folder(lib, blank.as_ptr(), &mut out) },
        EglStatus::Invalid
    );

    // Both items into the folder, then reverse their manual order.
    let target = cstring(&serde_json::json!({ "ids": ids }).to_string());
    let edit =
        cstring(&serde_json::json!({ "type": "addToFolders", "value": [folder] }).to_string());
    assert_eq!(
        count(lib, |out| unsafe {
            egl_apply_batch_edit(lib, target.as_ptr(), edit.as_ptr(), out)
        }),
        2
    );
    let folder_c = cstring(&folder);
    let moved = cstring(&serde_json::json!([ids[1]]).to_string());
    let before = cstring(&ids[0]);
    assert_eq!(
        unsafe { egl_reorder_items(lib, folder_c.as_ptr(), moved.as_ptr(), before.as_ptr()) },
        EglStatus::Ok,
        "{}",
        last_error(lib)
    );
    let manual = query_window(
        lib,
        &serde_json::json!({
            "scope": { "kind": "folder", "id": folder, "includeDescendants": false },
            "sort": "manual", "ascending": true
        }),
    );
    assert_eq!(ids_of(&manual), vec![ids[1].clone(), ids[0].clone()]);
    let moved_end = cstring(&serde_json::json!([ids[1]]).to_string());
    assert_eq!(
        unsafe { egl_reorder_items(lib, folder_c.as_ptr(), moved_end.as_ptr(), ptr::null()) },
        EglStatus::Ok
    );

    let tree = json(lib, |out| unsafe {
        egl_delete_folder(lib, folder_c.as_ptr(), 1, out)
    });
    assert_eq!(tree.as_array().unwrap().len(), 0);
    assert_eq!(
        json(lib, |out| unsafe { egl_trash_info(lib, out) })["items"],
        2
    );
}

#[test]
fn tags_rename_merge_delete_and_groups() {
    let (f, ids) = seeded(2);
    let lib = f.lib;
    let target = cstring(&serde_json::json!({ "ids": ids }).to_string());
    let edit = cstring(r#"{"type":"addTags","value":["Sky","Blue"]}"#);
    assert_eq!(
        count(lib, |out| unsafe {
            egl_apply_batch_edit(lib, target.as_ptr(), edit.as_ptr(), out)
        }),
        2
    );
    let one = cstring(&ids[0]);
    let red = cstring(r#"{"type":"addTags","value":["Red"]}"#);
    let item = json(lib, |out| unsafe {
        egl_apply_item_edit_undoable(lib, one.as_ptr(), red.as_ptr(), out)
    });
    assert!(item["tags"].as_array().unwrap().iter().any(|t| t == "Red"));
    assert_eq!(
        json(lib, |out| unsafe { egl_undo_status(lib, out) })["items"],
        1
    );
    json(lib, |out| unsafe { egl_undo(lib, out) });
    let item = json(lib, |out| unsafe { egl_item(lib, one.as_ptr(), out) });
    assert!(!item["tags"].as_array().unwrap().iter().any(|t| t == "Red"));

    let selection = json(lib, |out| unsafe {
        egl_selection_tags(lib, target.as_ptr(), out)
    });
    assert_eq!(selection["items"], 2);
    assert_eq!(selection["tags"].as_array().unwrap().len(), 2);

    let (sky, heaven, blue) = (cstring("Sky"), cstring("Heaven"), cstring("Blue"));
    assert_eq!(
        unsafe { egl_rename_tag(lib, sky.as_ptr(), heaven.as_ptr()) },
        EglStatus::Ok
    );
    assert_eq!(
        unsafe { egl_merge_tags(lib, blue.as_ptr(), heaven.as_ptr()) },
        EglStatus::Ok
    );
    let tags = json(lib, |out| unsafe { egl_tags(lib, out) });
    let names: Vec<&str> = tags
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Heaven"]);
    assert_eq!(
        unsafe { egl_delete_tag(lib, heaven.as_ptr()) },
        EglStatus::Ok
    );
    assert_eq!(
        json(lib, |out| unsafe { egl_tags(lib, out) })
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let create = cstring(r#"{"type":"create","name":"Colors","color":null,"tags":["Red"]}"#);
    let groups = json(lib, |out| unsafe {
        egl_save_tag_group(lib, create.as_ptr(), out)
    });
    assert_eq!(groups[0]["name"], "Colors");
    let group = cstring(groups[0]["id"].as_str().unwrap());
    assert_eq!(
        json(lib, |out| unsafe { egl_tag_groups(lib, out) })
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let left = json(lib, |out| unsafe {
        egl_delete_tag_group(lib, group.as_ptr(), out)
    });
    assert_eq!(left.as_array().unwrap().len(), 0);
}

#[test]
fn batch_edit_rejects_invalid_edits() {
    let (f, ids) = seeded(1);
    let target = cstring(&serde_json::json!({ "ids": ids }).to_string());
    let star = cstring(r#"{"type":"setStar","value":9}"#);
    let mut n = 0u32;
    assert_eq!(
        unsafe { egl_apply_batch_edit(f.lib, target.as_ptr(), star.as_ptr(), &mut n) },
        EglStatus::Invalid
    );
    let all_without_query = cstring(r#"{"all":true}"#);
    let ok = cstring(r#"{"type":"setStar","value":3}"#);
    assert_eq!(
        unsafe { egl_apply_batch_edit(f.lib, all_without_query.as_ptr(), ok.as_ptr(), &mut n) },
        EglStatus::Invalid
    );
    let _ = egl_apply_item_edit; // the non-undoable export stays as it was
}

#[test]
fn query_window_filters_sorts_and_smart_folders_round_trip() {
    let (f, ids) = seeded(3);
    let lib = f.lib;
    let target = cstring(&serde_json::json!({ "ids": [ids[0]] }).to_string());
    let tag = cstring(r#"{"type":"addTags","value":["Poster"]}"#);
    count(lib, |out| unsafe {
        egl_apply_batch_edit(lib, target.as_ptr(), tag.as_ptr(), out)
    });
    let star = cstring(r#"{"type":"setStar","value":5}"#);
    let two = cstring(&serde_json::json!({ "ids": [ids[1]] }).to_string());
    count(lib, |out| unsafe {
        egl_apply_batch_edit(lib, two.as_ptr(), star.as_ptr(), out)
    });

    let tagged = query_window(lib, &serde_json::json!({ "tags": ["Poster"] }));
    assert_eq!(ids_of(&tagged), vec![ids[0].clone()]);
    let rated = query_window(lib, &serde_json::json!({ "rating": 5 }));
    assert_eq!(ids_of(&rated), vec![ids[1].clone()]);
    let named = query_window(
        lib,
        &serde_json::json!({ "sort": "name", "ascending": true }),
    );
    assert_eq!(named["total"], 3);
    let search = query_window(lib, &serde_json::json!({ "text": "img2" }));
    assert_eq!(search["total"], 1);
    let bad = cstring(r#"{"sort":"nope"}"#);
    let mut out: *mut c_char = ptr::null_mut();
    assert_eq!(
        unsafe { egl_query_window(lib, bad.as_ptr(), 0, 10, &mut out) },
        EglStatus::Invalid
    );

    let exts = json(lib, |out| unsafe { egl_extensions(lib, out) });
    assert!(exts.to_string().contains("png"));

    let create = cstring(
        &serde_json::json!({
            "type": "create", "name": "Posters", "parent": null,
            "conditions": [{ "match": "AND", "boolean": "TRUE", "rules": [
                { "property": "tags", "method": "union", "value": ["Poster"] }
            ]}]
        })
        .to_string(),
    );
    let tree = json(lib, |out| unsafe {
        egl_save_smart_folder(lib, create.as_ptr(), out)
    });
    let smart = tree[0]["id"].as_str().unwrap().to_owned();
    let view = query_window(
        lib,
        &serde_json::json!({ "scope": { "kind": "smart", "id": smart } }),
    );
    assert_eq!(ids_of(&view), vec![ids[0].clone()]);
    assert_eq!(
        json(lib, |out| unsafe { egl_smart_folders(lib, out) })
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let smart_c = cstring(&smart);
    let left = json(lib, |out| unsafe {
        egl_delete_smart_folder(lib, smart_c.as_ptr(), out)
    });
    assert_eq!(left.as_array().unwrap().len(), 0);
    let _ = egl_folder_tree;
}

#[test]
fn null_handles_are_invalid() {
    let mut out: *mut c_char = ptr::null_mut();
    let mut n = 0u32;
    unsafe {
        assert_eq!(egl_imports(ptr::null_mut(), &mut out), EglStatus::Invalid);
        assert_eq!(
            egl_trash_info(ptr::null_mut(), &mut out),
            EglStatus::Invalid
        );
        assert_eq!(
            egl_restore_trash(ptr::null_mut(), &mut n),
            EglStatus::Invalid
        );
        assert_eq!(egl_undo(ptr::null_mut(), &mut out), EglStatus::Invalid);
        assert_eq!(egl_cancel_import(ptr::null_mut(), 1), EglStatus::Invalid);
    }
}

// ---------------------------------------------------------------------------------------------
// The service-backed API (D-092)

use std::ffi::c_void;
use std::sync::Mutex;

/// Events the callback heard, as JSON.
static HEARD: Mutex<Vec<Value>> = Mutex::new(Vec::new());

unsafe extern "C" fn record(ctx: *mut c_void, event: *const c_char) {
    assert_eq!(ctx as usize, 42);
    let text = unsafe { CStr::from_ptr(event) }.to_str().unwrap();
    HEARD
        .lock()
        .unwrap()
        .push(serde_json::from_str(text).unwrap());
}

#[test]
fn views_by_id_follow_writes_and_the_callback_hears_them() {
    let (f, ids) = seeded(3);
    assert_eq!(
        unsafe { egl_set_event_callback(f.lib, Some(record), 42 as *mut c_void) },
        EglStatus::Ok
    );
    let opened = json(f.lib, |out| unsafe {
        egl_open_view(f.lib, ptr::null(), out)
    });
    let view = opened["viewId"].as_u64().unwrap();
    assert_eq!(opened["total"], 3);
    let cells = json(f.lib, |out| unsafe {
        egl_view_range(f.lib, view, 0, 10, out)
    });
    assert_eq!(cells.as_array().unwrap().len(), 3);
    let dims = json(f.lib, |out| unsafe {
        egl_view_dimensions(f.lib, view, out)
    });
    assert_eq!(dims[0], serde_json::json!([40, 20]));

    // Select All minus one, trashed through the view: the view shrinks under its id.
    let target =
        cstring(&serde_json::json!({"viewId": view, "all": true, "ids": [ids[0]]}).to_string());
    let edit = cstring(r#"{"type":"trash"}"#);
    let trashed = count(f.lib, |out| unsafe {
        egl_edit_items(f.lib, target.as_ptr(), edit.as_ptr(), out)
    });
    assert_eq!(trashed, 2);
    let heard = HEARD.lock().unwrap().clone();
    assert!(
        heard.iter().any(|event| event["type"] == "viewsInvalidated"
            && event["views"][0]["viewId"] == view
            && event["views"][0]["total"] == 1),
        "{heard:?}"
    );
    let counts = json(f.lib, |out| unsafe { egl_counts(f.lib, out) });
    assert_eq!(counts["trash"], 2);
    assert_eq!(
        unsafe { egl_set_event_callback(f.lib, None, ptr::null_mut()) },
        EglStatus::Ok
    );
    assert_eq!(unsafe { egl_close_view(f.lib, view) }, EglStatus::Ok);
    let mut out: *mut c_char = ptr::null_mut();
    assert_eq!(
        unsafe { egl_view_range(f.lib, view, 0, 1, &mut out) },
        EglStatus::NotFound
    );
}

#[test]
fn a_locked_folder_hides_its_items_until_unlocked() {
    let (f, ids) = seeded(1);
    let change = cstring(
        r#"{"type":"create","name":"Private","parent":null,"description":"","iconColor":null}"#,
    );
    let tree = json(f.lib, |out| unsafe {
        egl_save_folder(f.lib, change.as_ptr(), out)
    });
    let folder = tree[0]["id"].as_str().unwrap().to_owned();
    let target = cstring(&serde_json::json!({"ids": ids}).to_string());
    let edit = cstring(&serde_json::json!({"type":"addToFolders","value":[folder]}).to_string());
    count(f.lib, |out| unsafe {
        egl_apply_batch_edit(f.lib, target.as_ptr(), edit.as_ptr(), out)
    });

    let folder_c = cstring(&folder);
    let request = cstring(r#"{"password":"pässwörd 🔒"}"#);
    let key = json(f.lib, |out| unsafe {
        egl_lock_folder(f.lib, folder_c.as_ptr(), request.as_ptr(), out)
    });
    assert_eq!(key["phrase"].as_str().unwrap().split(' ').count(), 12);
    assert_eq!(
        ids_of(&query_window(f.lib, &serde_json::json!({}))).len(),
        0
    );

    let wrong = cstring("nope");
    assert_eq!(
        unsafe { egl_unlock_folder(f.lib, folder_c.as_ptr(), wrong.as_ptr()) },
        EglStatus::WrongPassword
    );
    let right = cstring("pässwörd 🔒");
    assert_eq!(
        unsafe { egl_unlock_folder(f.lib, folder_c.as_ptr(), right.as_ptr()) },
        EglStatus::Ok
    );
    assert_eq!(ids_of(&query_window(f.lib, &serde_json::json!({}))), ids);
    // The original of an unlocked item comes decrypted as bytes; its file is ciphertext.
    let id = cstring(&ids[0]);
    let mut bytes: *mut u8 = ptr::null_mut();
    let mut len = 0usize;
    assert_eq!(
        unsafe { egl_original_bytes(f.lib, id.as_ptr(), &mut bytes, &mut len, ptr::null_mut()) },
        EglStatus::Ok,
        "{}",
        last_error(f.lib)
    );
    assert_eq!(
        &unsafe { std::slice::from_raw_parts(bytes, len) }[1..4],
        b"PNG"
    );
    unsafe { egl_bytes_free(bytes, len) };
    // Null relocks every open folder.
    assert_eq!(
        unsafe { egl_relock_folder(f.lib, ptr::null()) },
        EglStatus::Ok
    );
    assert_eq!(
        ids_of(&query_window(f.lib, &serde_json::json!({}))).len(),
        0
    );
    assert_eq!(
        unsafe { egl_remove_folder_lock(f.lib, folder_c.as_ptr(), right.as_ptr()) },
        EglStatus::Ok
    );
    assert_eq!(ids_of(&query_window(f.lib, &serde_json::json!({}))), ids);
}

#[test]
fn backups_checks_and_an_export_into_a_zip() {
    let (f, _) = seeded(2);
    let backup = json(f.lib, |out| unsafe { egl_back_up_now(f.lib, out) });
    let backups = json(f.lib, |out| unsafe { egl_backups(f.lib, out) });
    assert!(
        backups
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["name"] == backup["name"])
    );
    let check = json(f.lib, |out| unsafe { egl_check_library(f.lib, out) });
    assert_eq!(check["databaseOk"], true);
    assert_eq!(check["missingCount"], 0);
    let stats = json(f.lib, |out| unsafe { egl_library_stats(f.lib, out) });
    assert!(stats["originals"].as_u64().unwrap() > 0);
    let renamed_to = cstring("Renamed 📚");
    let info = json(f.lib, |out| unsafe {
        egl_rename_library(f.lib, renamed_to.as_ptr(), out)
    });
    assert_eq!(info["name"], "Renamed 📚");

    let selection = cstring(r#"{"whole":true,"folders":[],"tags":[]}"#);
    let preview = json(f.lib, |out| unsafe {
        egl_export_preview(f.lib, selection.as_ptr(), 1, out)
    });
    assert_eq!(preview["items"], 2);
    let destination = f.dir.path().join("out");
    std::fs::create_dir_all(&destination).unwrap();
    let request = cstring(
        &serde_json::json!({
            "selection": {"whole": true, "folders": [], "tags": []},
            "includeSubfolders": true,
            "format": {"kind": "zip", "compression": "fast"},
            "destination": destination.to_str().unwrap(),
            "encrypt": null,
            "eagle": null
        })
        .to_string(),
    );
    let first = json(f.lib, |out| unsafe {
        egl_start_export(f.lib, request.as_ptr(), out)
    });
    assert!(first["id"].as_u64().is_some());
    let deadline = Instant::now() + Duration::from_secs(30);
    while !json(f.lib, |out| unsafe { egl_exports(f.lib, out) })
        .as_array()
        .unwrap()
        .is_empty()
    {
        assert!(Instant::now() < deadline, "the export never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
    let written: Vec<_> = std::fs::read_dir(&destination).unwrap().flatten().collect();
    assert_eq!(written.len(), 1);
    assert!(written[0].file_name().to_string_lossy().ends_with(".zip"));
}

#[test]
fn a_panic_never_crosses_the_boundary() {
    let status = crate::guarded(|_| {}, || panic!("boom"));
    assert_eq!(status, EglStatus::Other);
}
