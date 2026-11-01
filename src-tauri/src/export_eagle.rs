//! Exporting the open native library into a running Eagle (docs/spec.md §8.4,
//! docs/architecture.md §15.7). Eagle stays the writer of its library: everything goes
//! through its API, and every new Eagle id is stored in the `eagle_id` columns so a second
//! export updates instead of duplicating.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use eagle_api::{EagleApi, page_all};
use library_core::{EagleExportTarget, ExportProgress, ExportRequest, ExportState, ExportSummary};
use library_native::{
    EagleExportItem, Library, item_bytes, item_path, load_eagle_export, store_eagle_ids,
};
use secrecy::SecretString;
use serde_json::{Map, Value};

/// Runs one export into Eagle to the end and returns the final progress. `cancel` stops it
/// between steps; the ids stored so far stay stored.
pub fn export_into_eagle(
    library: &Library,
    id: u64,
    request: &ExportRequest,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(ExportProgress),
) -> ExportProgress {
    let target = request
        .eagle
        .clone()
        .expect("start_export checks the target first");
    let mut progress = ExportProgress {
        id,
        state: ExportState::Preparing,
        total: 0,
        done: 0,
        bytes_total: 0,
        bytes_done: 0,
        current: None,
        summary: None,
        error: None,
    };
    let result = run(
        library,
        &target,
        request,
        cancel,
        &mut progress,
        on_progress,
    );
    match result {
        Ok(summary) => {
            progress.state = ExportState::Done;
            progress.current = None;
            progress.summary = Some(summary);
        }
        Err(_message) if cancel.load(Ordering::Relaxed) => {
            progress.state = ExportState::Cancelled;
            progress.current = None;
        }
        Err(message) => {
            progress.state = ExportState::Failed;
            progress.current = None;
            progress.error = Some(message);
        }
    }
    progress
}

/// The error string; cancellation is told apart by the flag.
fn run(
    library: &Library,
    target: &EagleExportTarget,
    request: &ExportRequest,
    cancel: &AtomicBool,
    progress: &mut ExportProgress,
    on_progress: &mut dyn FnMut(ExportProgress),
) -> Result<ExportSummary, String> {
    let plan = load_eagle_export(library, &request.selection, request.include_subfolders)
        .map_err(|error| error.to_string())?;
    progress.total = plan.items.len() as u64;
    progress.bytes_total = plan.items.iter().map(|item| item.size).sum();
    on_progress(progress.clone());

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("runtime: {error}"))?;
    let token = SecretString::from(target.token.clone().unwrap_or_default());
    let api = EagleApi::new(&target.host, target.port, token, tailscale(&target.host))
        .map_err(|error| error.to_string())?;
    let target_library = runtime
        .block_on(async {
            api.app_info().await?;
            api.library_info().await
        })
        .map_err(|error| error.to_string())?;
    if target_library.to_info().is_none() {
        return Err("Eagle has no library open".into());
    }
    let same_pc = loopback(&target.host);

    // Folders top-down: map ids, create or update (docs/spec.md §8.4 step 2). Folder labels
    // are not exported: Eagle keeps its own.
    let target_folders = runtime
        .block_on(page_all(
            |offset| {
                let api = api.clone();
                async move { api.folder_page(offset).await }
            },
            |folder: &eagle_api::dto::FolderDto| folder.id.clone(),
            |_, _| {},
        ))
        .map_err(|error| error.to_string())?;
    let target_folder_ids: HashSet<String> = target_folders
        .iter()
        .map(|folder| folder.id.clone())
        .collect();
    let mut folder_map: HashMap<String, String> = HashMap::new();
    let mut folder_pairs: Vec<(String, String)> = Vec::new();
    for folder in &plan.folders {
        if cancel.load(Ordering::Relaxed) {
            return Err(String::new());
        }
        let parent = folder.parent.as_deref().and_then(|id| folder_map.get(id));
        if let Some(eagle_id) = &folder.eagle_id
            && target_folder_ids.contains(eagle_id)
        {
            // Already there: refresh the full state (rename, colour, move).
            let mut body = Map::from_iter([
                ("id".to_owned(), Value::from(eagle_id.clone())),
                ("name".to_owned(), Value::from(folder.name.clone())),
                (
                    "description".to_owned(),
                    Value::from(folder.description.clone()),
                ),
                ("tags".to_owned(), Value::from(Vec::<String>::new())),
            ]);
            if let Some(color) = &folder.icon_color {
                body.insert("iconColor".into(), Value::from(color.clone()));
            }
            if let Some(parent) = parent {
                body.insert("parent".into(), Value::from(parent.clone()));
            }
            runtime
                .block_on(api.folder_update(body))
                .map_err(|error| error.to_string())?;
            folder_map.insert(folder.id.clone(), eagle_id.clone());
            continue;
        }
        let mut body = Map::from_iter([("name".to_owned(), Value::from(folder.name.clone()))]);
        if !folder.description.is_empty() {
            body.insert(
                "description".into(),
                Value::from(folder.description.clone()),
            );
        }
        if let Some(color) = &folder.icon_color {
            body.insert("iconColor".into(), Value::from(color.clone()));
        }
        if let Some(parent) = parent {
            body.insert("parent".into(), Value::from(parent.clone()));
        }
        let eagle_id = runtime
            .block_on(api.folder_create(body))
            .map_err(|error| error.to_string())?;
        folder_map.insert(folder.id.clone(), eagle_id.clone());
        folder_pairs.push((folder.id.clone(), eagle_id));
    }

    // Items. A stored Eagle id that still exists means "already exported": skip it, so a
    // second export updates folders and adds only what is new (docs/spec.md §8.4 step 6).
    progress.state = ExportState::Copying;
    let mut item_pairs: Vec<(String, String)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut added: HashMap<String, String> = HashMap::new();
    let mut batch: Vec<(EagleExportItem, String)> = Vec::new();
    let mut temp_files: Vec<std::path::PathBuf> = Vec::new();
    let mut added_count = 0u64;
    for item in &plan.items {
        if cancel.load(Ordering::Relaxed) {
            store_ids(library, &folder_pairs, &item_pairs, &[], &[]);
            return Err(String::new());
        }
        if let Some(eagle_id) = &item.eagle_id {
            let exists = runtime
                .block_on(api.item_by_id(eagle_id))
                .map_err(|error| error.to_string())?
                .is_some();
            if exists {
                progress.done += 1;
                progress.bytes_done += item.size;
                continue;
            }
        }
        progress.current = Some(item.name.clone());
        if same_pc {
            let Some(path) = same_pc_path(library, item, &mut temp_files, &mut missing) else {
                progress.done += 1;
                continue;
            };
            batch.push((item.clone(), path));
            if batch.len() >= 1000 {
                flush_batch(
                    &runtime,
                    &api,
                    &folder_map,
                    &mut batch,
                    &mut temp_files,
                    progress,
                    &mut item_pairs,
                    &mut added,
                    &mut added_count,
                    on_progress,
                )?;
            }
            continue;
        }
        export_one_remote(
            &runtime,
            &api,
            library,
            item,
            &folder_map,
            cancel,
            progress,
            &mut item_pairs,
            &mut added,
            &mut added_count,
            &mut missing,
            on_progress,
        )?;
    }
    flush_batch(
        &runtime,
        &api,
        &folder_map,
        &mut batch,
        &mut temp_files,
        progress,
        &mut item_pairs,
        &mut added,
        &mut added_count,
        on_progress,
    )?;

    // Comments of the items exported now (a repeated export must not duplicate them).
    for comment in &plan.comments {
        if cancel.load(Ordering::Relaxed) {
            store_ids(library, &folder_pairs, &item_pairs, &[], &[]);
            return Err(String::new());
        }
        let Some(eagle_item) = added.get(&comment.item_id) else {
            continue;
        };
        let mut body = Map::from_iter([
            ("id".to_owned(), Value::from(eagle_item.clone())),
            (
                "annotation".to_owned(),
                Value::from(comment.annotation.clone()),
            ),
        ]);
        match comment.rect {
            Some([x, y, w, h]) => {
                body.insert("x".into(), Value::from(x));
                body.insert("y".into(), Value::from(y));
                body.insert("width".into(), Value::from(w));
                body.insert("height".into(), Value::from(h));
            }
            None => {
                let Some(seconds) = comment.at_seconds else {
                    continue;
                };
                body.insert("duration".into(), Value::from(seconds));
            }
        }
        runtime
            .block_on(api.item_add_comment(body))
            .map_err(|error| error.to_string())?;
    }

    // Tag groups and smart folders, skipped when their stored Eagle id already exists.
    let target_groups = runtime
        .block_on(page_all(
            |offset| {
                let api = api.clone();
                async move { api.tag_group_page(offset).await }
            },
            |group: &eagle_api::dto::TagGroupDto| group.id.clone(),
            |_, _| {},
        ))
        .map_err(|error| error.to_string())?;
    let target_group_ids: HashSet<String> =
        target_groups.iter().map(|group| group.id.clone()).collect();
    let mut group_pairs: Vec<(String, String)> = Vec::new();
    for group in &plan.groups {
        if cancel.load(Ordering::Relaxed) {
            store_ids(library, &folder_pairs, &item_pairs, &group_pairs, &[]);
            return Err(String::new());
        }
        if let Some(eagle_id) = &group.eagle_id
            && target_group_ids.contains(eagle_id)
        {
            continue;
        }
        let mut body = Map::from_iter([("name".to_owned(), Value::from(group.name.clone()))]);
        if let Some(color) = &group.color {
            body.insert("color".into(), Value::from(color.clone()));
        }
        body.insert("tags".into(), Value::from(group.tags.clone()));
        let eagle_id = runtime
            .block_on(api.tag_group_create(body))
            .map_err(|error| error.to_string())?;
        group_pairs.push((group.id.clone(), eagle_id));
    }

    let target_smart = runtime
        .block_on(page_all(
            |offset| {
                let api = api.clone();
                async move { api.smart_folder_page(offset).await }
            },
            |folder: &eagle_api::dto::SmartFolderDto| folder.id.clone(),
            |_, _| {},
        ))
        .map_err(|error| error.to_string())?;
    let target_smart_ids: HashSet<String> = target_smart
        .iter()
        .map(|folder| folder.id.clone())
        .collect();
    let mut smart_pairs: Vec<(String, String)> = Vec::new();
    for smart in &plan.smart {
        if cancel.load(Ordering::Relaxed) {
            store_ids(
                library,
                &folder_pairs,
                &item_pairs,
                &group_pairs,
                &smart_pairs,
            );
            return Err(String::new());
        }
        if let Some(eagle_id) = &smart.eagle_id
            && target_smart_ids.contains(eagle_id)
        {
            continue;
        }
        let conditions: Value = serde_json::from_str(&smart.conditions)
            .map_err(|error| format!("bad smart folder rules: {error}"))?;
        let eagle_id = runtime
            .block_on(api.smart_folder_create(Map::from_iter([
                ("name".to_owned(), Value::from(smart.name.clone())),
                ("conditions".to_owned(), conditions),
            ])))
            .map_err(|error| error.to_string())?;
        smart_pairs.push((smart.id.clone(), eagle_id));
    }

    store_ids(
        library,
        &folder_pairs,
        &item_pairs,
        &group_pairs,
        &smart_pairs,
    );
    Ok(ExportSummary {
        items: progress.total,
        files: added_count,
        bytes: progress.bytes_done,
        missing,
        path: format!("Eagle on {}:{}", target.host, target.port),
    })
}

#[allow(clippy::too_many_arguments)]
fn flush_batch(
    runtime: &tokio::runtime::Runtime,
    api: &EagleApi,
    folder_map: &HashMap<String, String>,
    batch: &mut Vec<(EagleExportItem, String)>,
    temp_files: &mut Vec<std::path::PathBuf>,
    progress: &mut ExportProgress,
    item_pairs: &mut Vec<(String, String)>,
    added: &mut HashMap<String, String>,
    added_count: &mut u64,
    on_progress: &mut dyn FnMut(ExportProgress),
) -> Result<(), String> {
    if batch.is_empty() {
        return Ok(());
    }
    let entries: Vec<Map<String, Value>> = batch
        .iter()
        .map(|(item, path)| {
            Map::from_iter([
                ("path".to_owned(), Value::from(path.clone())),
                ("name".to_owned(), Value::from(item.name.clone())),
                ("tags".to_owned(), Value::from(item.tags.clone())),
                (
                    "folders".to_owned(),
                    Value::from(
                        item.folder_ids
                            .iter()
                            .filter_map(|id| folder_map.get(id).cloned())
                            .collect::<Vec<_>>(),
                    ),
                ),
                (
                    "annotation".to_owned(),
                    Value::from(item.annotation.clone()),
                ),
                ("website".to_owned(), Value::from(item.url.clone())),
                ("modificationTime".to_owned(), Value::from(item.added_at)),
                ("star".to_owned(), Value::from(item.star)),
            ])
        })
        .collect();
    let ids = runtime
        .block_on(api.item_add_batch(entries))
        .map_err(|error| error.to_string())?;
    if ids.len() != batch.len() {
        return Err(format!(
            "Eagle accepted {} of {} files",
            ids.len(),
            batch.len()
        ));
    }
    for ((item, _), eagle_id) in std::mem::take(batch).into_iter().zip(ids) {
        item_pairs.push((item.id.clone(), eagle_id.clone()));
        added.insert(item.id.clone(), eagle_id);
        *added_count += 1;
        progress.done += 1;
        progress.bytes_done += item.size;
    }
    for temp in temp_files.drain(..) {
        let _ = std::fs::remove_file(temp);
    }
    on_progress(progress.clone());
    Ok(())
}

/// The path of an item's original for the same-PC batch: the stored file, or a temp file
/// with the decrypted bytes of a locked folder's item. `None` (counted in `missing`) when
/// the original is gone.
fn same_pc_path(
    library: &Library,
    item: &EagleExportItem,
    temp_files: &mut Vec<std::path::PathBuf>,
    missing: &mut Vec<String>,
) -> Option<String> {
    if !item.encrypted {
        return match item_path(library, item) {
            Some(path) => Some(path.to_string_lossy().into_owned()),
            None => {
                missing.push(item.name.clone());
                None
            }
        };
    }
    match item_bytes(library, item) {
        Ok(Some(bytes)) => {
            let temp = std::env::temp_dir().join(format!(
                "egl-export-{}-{}.{}",
                std::process::id(),
                item.id,
                item.ext
            ));
            match std::fs::write(&temp, bytes) {
                Ok(()) => {
                    temp_files.push(temp.clone());
                    Some(temp.to_string_lossy().into_owned())
                }
                Err(error) => {
                    tracing::warn!("cannot stage {}: {error}", item.name);
                    missing.push(item.name.clone());
                    None
                }
            }
        }
        _ => {
            missing.push(item.name.clone());
            None
        }
    }
}

/// One data-URL item for a client on another computer (docs/spec.md §8.4 step 3).
#[allow(clippy::too_many_arguments)]
fn export_one_remote(
    runtime: &tokio::runtime::Runtime,
    api: &EagleApi,
    library: &Library,
    item: &EagleExportItem,
    folder_map: &HashMap<String, String>,
    cancel: &AtomicBool,
    progress: &mut ExportProgress,
    item_pairs: &mut Vec<(String, String)>,
    added: &mut HashMap<String, String>,
    added_count: &mut u64,
    missing: &mut Vec<String>,
    on_progress: &mut dyn FnMut(ExportProgress),
) -> Result<(), String> {
    let bytes = item_bytes(library, item).map_err(|error| error.to_string())?;
    let Some(bytes) = bytes else {
        missing.push(item.name.clone());
        progress.done += 1;
        return Ok(());
    };
    let mime = mime_of(&item.ext).unwrap_or("application/octet-stream");
    let body = Map::from_iter([
        (
            "base64".to_owned(),
            Value::from(format!("data:{mime};base64,{}", STANDARD.encode(&bytes))),
        ),
        ("name".to_owned(), Value::from(item.name.clone())),
        ("tags".to_owned(), Value::from(item.tags.clone())),
        (
            "folders".to_owned(),
            Value::from(
                item.folder_ids
                    .iter()
                    .filter_map(|id| folder_map.get(id).cloned())
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "annotation".to_owned(),
            Value::from(item.annotation.clone()),
        ),
        ("website".to_owned(), Value::from(item.url.clone())),
    ]);
    let eagle_id = runtime
        .block_on(api.item_add_single(body))
        .map_err(|error| error.to_string())?;
    // The single form ignores `star` (correction 11): set it once the item exists.
    wait_for_item(runtime, api, &eagle_id, cancel);
    if item.star > 0 {
        let update = Map::from_iter([
            ("id".to_owned(), Value::from(eagle_id.clone())),
            ("star".to_owned(), Value::from(item.star)),
        ]);
        let _ = runtime.block_on(api.item_update(update));
    }
    item_pairs.push((item.id.clone(), eagle_id.clone()));
    added.insert(item.id.clone(), eagle_id);
    *added_count += 1;
    progress.done += 1;
    progress.bytes_done += item.size;
    on_progress(progress.clone());
    Ok(())
}

fn store_ids(
    library: &Library,
    folders: &[(String, String)],
    items: &[(String, String)],
    groups: &[(String, String)],
    smart: &[(String, String)],
) {
    if let Err(error) = store_eagle_ids(library, folders, items, groups, smart) {
        tracing::warn!("cannot store the Eagle ids: {error}");
    }
}

/// Polls `item/get {id}` until the item exists (docs/architecture.md §15.7 step 3).
fn wait_for_item(runtime: &tokio::runtime::Runtime, api: &EagleApi, id: &str, cancel: &AtomicBool) {
    let mut delay = Duration::from_millis(250);
    let mut waited = Duration::ZERO;
    loop {
        match runtime.block_on(api.item_by_id(id)) {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(_) => return,
        }
        if waited >= Duration::from_secs(20) || cancel.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(delay);
        waited += delay;
        delay = (delay * 2).min(Duration::from_secs(4));
    }
}

fn loopback(host: &str) -> bool {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

fn tailscale(host: &str) -> bool {
    let host = host.trim();
    if host.to_ascii_lowercase().ends_with(".ts.net") {
        return true;
    }
    matches!(host.parse::<std::net::IpAddr>(), Ok(std::net::IpAddr::V4(ip)) if ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
}

/// The MIME type of a data URL by extension (live-verified for JPEG only, docs/spec.md §8.4).
fn mime_of(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_and_tailscale_match_the_connection_settings() {
        for host in ["127.0.0.1", "localhost", "[::1]"] {
            assert!(loopback(host), "{host}");
        }
        assert!(!loopback("eagle-pc"));
        assert!(tailscale("100.64.0.1"));
        assert!(tailscale("pc.tail1234.ts.net"));
        assert!(!tailscale("192.168.1.20"));
    }

    #[test]
    fn mime_table_covers_the_common_formats() {
        assert_eq!(mime_of("jpg"), Some("image/jpeg"));
        assert_eq!(mime_of("png"), Some("image/png"));
        assert_eq!(mime_of("xyz"), None);
    }
}
