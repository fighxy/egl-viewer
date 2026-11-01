//! Adding files to a running Eagle (docs/spec.md §8.3 "Adding files to a running Eagle").
//!
//! On the same PC as Eagle, `item/add` runs in the batch form with `path` entries. From
//! another computer, one data URL per request. The id comes back before the item exists
//! (correction 2), so the client polls `item/get {id}` until it appears.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use library_core::{AddTarget, NewFile};
use serde_json::{Map, Value};

use crate::api::EagleApi;
use crate::dto::ItemDto;
use crate::error::EagleError;

/// The size limit for one file sent as a data URL (docs/spec.md §8.3).
const DATA_URL_MAX_BYTES: u64 = 50 * 1024 * 1024;
/// `items` per `item/add` batch call (docs/spec.md §8.3).
const BATCH_MAX: usize = 1000;
/// Poll `item/get {id}` with 0.25 → 0.5 → 1 → 2 → 4 s steps, about 20 s in total.
const POLL_TOTAL: Duration = Duration::from_secs(20);

/// One file Eagle has accepted but that does not exist yet: the grid shows an
/// "Importing…" cell for it until `wait_for_item` sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAdd {
    pub id: String,
    pub name: String,
    pub ext: String,
}

/// Adds `files` to `target`, reporting each accepted file through `on_pending` as soon as
/// Eagle answers. Only the ids are known at that point: the items appear later.
pub(crate) async fn start_add(
    api: &EagleApi,
    same_pc: bool,
    files: &[NewFile],
    target: &AddTarget,
    mut on_pending: impl FnMut(PendingAdd),
) -> Result<Vec<PendingAdd>, EagleError> {
    if same_pc {
        start_add_paths(api, files, target, &mut on_pending).await
    } else {
        start_add_data_urls(api, files, target, &mut on_pending).await
    }
}

/// The same-PC form: one `item/add` batch with `path` entries, even for one file (it keeps
/// `star` and `modificationTime`, correction 35).
async fn start_add_paths(
    api: &EagleApi,
    files: &[NewFile],
    target: &AddTarget,
    on_pending: &mut impl FnMut(PendingAdd),
) -> Result<Vec<PendingAdd>, EagleError> {
    let mut pendings = Vec::with_capacity(files.len());
    for chunk in files.chunks(BATCH_MAX) {
        let mut entries = Vec::with_capacity(chunk.len());
        for file in chunk {
            if !Path::new(&file.path).is_file() {
                return Err(EagleError::Invalid(format!(
                    "not a file on this PC: {}",
                    file.path
                )));
            }
            let mut entry = Map::from_iter([("path".to_owned(), Value::from(file.path.clone()))]);
            if let Some(name) = &file.name {
                entry.insert("name".into(), Value::from(name.clone()));
            }
            add_target_fields(&mut entry, target);
            entries.push(entry);
        }
        let ids = api.item_add_batch(entries).await?;
        if ids.len() != chunk.len() {
            return Err(EagleError::Rejected {
                http: 200,
                message: format!("Eagle accepted {} of {} files", ids.len(), chunk.len()),
            });
        }
        for (file, id) in chunk.iter().zip(ids) {
            let pending = PendingAdd {
                id,
                name: file.name.clone().unwrap_or_else(|| base_name(&file.path)),
                ext: extension(&file.path),
            };
            on_pending(pending.clone());
            pendings.push(pending);
        }
    }
    Ok(pendings)
}

/// The remote form: one data URL per request, files one after another.
async fn start_add_data_urls(
    api: &EagleApi,
    files: &[NewFile],
    target: &AddTarget,
    on_pending: &mut impl FnMut(PendingAdd),
) -> Result<Vec<PendingAdd>, EagleError> {
    let mut pendings = Vec::with_capacity(files.len());
    for file in files {
        let path = PathBuf::from(&file.path);
        let bytes = std::fs::metadata(&path)
            .and_then(|meta| {
                if meta.len() > DATA_URL_MAX_BYTES {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "over the 50 MB limit",
                    ))
                } else {
                    std::fs::read(&path)
                }
            })
            .map_err(|error| EagleError::Invalid(format!("cannot read {}: {error}", file.path)))?;
        let ext = extension(&file.path);
        let mime = mime_of(&ext).unwrap_or("application/octet-stream");
        let mut body = Map::from_iter([
            (
                "base64".to_owned(),
                Value::from(format!("data:{mime};base64,{}", STANDARD.encode(&bytes))),
            ),
            (
                "name".to_owned(),
                Value::from(file.name.clone().unwrap_or_else(|| base_name(&file.path))),
            ),
        ]);
        add_target_fields(&mut body, target);
        let id = api.item_add_single(body).await?;
        let pending = PendingAdd {
            id,
            name: file.name.clone().unwrap_or_else(|| base_name(&file.path)),
            ext,
        };
        on_pending(pending.clone());
        pendings.push(pending);
    }
    Ok(pendings)
}

/// What `item/add` and `item/update` share: folders, tags, star, note.
fn add_target_fields(body: &mut Map<String, Value>, target: &AddTarget) {
    if !target.folders.is_empty() {
        body.insert("folders".into(), Value::from(target.folders.clone()));
    }
    if !target.tags.is_empty() {
        body.insert("tags".into(), Value::from(target.tags.clone()));
    }
    if let Some(star) = target.star {
        body.insert("star".into(), Value::from(star));
    }
    if let Some(annotation) = &target.annotation {
        body.insert("annotation".into(), Value::from(annotation.clone()));
    }
}

/// Polls `item/get {id}` until the item exists or about [`POLL_TOTAL`] passes. The single
/// mode ignores `star` (correction 11), so a chosen rating is sent separately after the
/// item appears.
pub(crate) async fn wait_for_item(
    api: &EagleApi,
    id: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<Option<ItemDto>, EagleError> {
    let mut delay = Duration::from_millis(250);
    let mut waited = Duration::ZERO;
    loop {
        if let Some(item) = api.item_by_id(id).await? {
            return Ok(Some(item));
        }
        if waited >= POLL_TOTAL || cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        // Yield on the EagleSource runtime (callers `block_on` this); never `thread::sleep`.
        tokio::time::sleep(delay).await;
        waited += delay;
        delay = (delay * 2).min(Duration::from_secs(4));
    }
}

/// The file name without its extension and directory.
fn base_name(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

/// The lowercase extension without the dot.
fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// The MIME type for a data URL, from the extension. Verified live for JPEG only; the rest
/// follows the standard mapping (docs/spec.md §8.3).
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
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "pdf" => "application/pdf",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_extensions_come_from_the_path() {
        assert_eq!(base_name("C:\\Photos\\IMG 1042.JPG"), "IMG 1042");
        assert_eq!(base_name("/home/me/pic.png"), "pic");
        assert_eq!(extension("C:\\Photos\\IMG.JPG"), "jpg");
        assert_eq!(extension("no-extension"), "");
    }

    #[test]
    fn common_extensions_map_to_mime_types() {
        assert_eq!(mime_of("jpg"), Some("image/jpeg"));
        assert_eq!(mime_of("png"), Some("image/png"));
        assert_eq!(mime_of("mp4"), Some("video/mp4"));
        assert_eq!(mime_of("unknown"), None);
    }

    #[test]
    fn data_url_limit_is_50_mb() {
        assert_eq!(DATA_URL_MAX_BYTES, 50 * 1024 * 1024);
    }

    #[test]
    fn target_fields_are_omitted_when_empty() {
        let mut body = Map::new();
        add_target_fields(&mut body, &AddTarget::default());
        assert!(body.is_empty());
        let mut body = Map::new();
        add_target_fields(
            &mut body,
            &AddTarget {
                folders: vec!["F1".into()],
                tags: vec!["t".into()],
                star: Some(3),
                annotation: Some("n".into()),
            },
        );
        assert_eq!(body["folders"], Value::from(vec!["F1"]));
        assert_eq!(body["star"], Value::from(3));
    }
}
