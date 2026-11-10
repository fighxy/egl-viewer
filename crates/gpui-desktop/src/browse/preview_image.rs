//! Load a preview `Image` from Session originals / thumbs / media (HEIC).

use std::io::Cursor;
use std::sync::Arc;

use desktop_session::Session;
use gpui_kit::{Image, ImageFormat};
use library_core::{ItemCell, Thumbnail};

/// Extensions GPUI can show from original file bytes (aligned with shipping WebView set).
const SHOWN_AS_IS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg"];

/// Whether the full-size preview should try the original file bytes first.
pub fn shows_original(ext: &str) -> bool {
    let ext = ext.to_ascii_lowercase();
    SHOWN_AS_IS.iter().any(|e| *e == ext)
}

fn format_from_content_type(content_type: &str) -> Option<ImageFormat> {
    match content_type {
        "image/webp" => Some(ImageFormat::Webp),
        "image/png" => Some(ImageFormat::Png),
        "image/jpeg" | "image/jpg" => Some(ImageFormat::Jpeg),
        "image/gif" => Some(ImageFormat::Gif),
        "image/bmp" => Some(ImageFormat::Bmp),
        "image/svg+xml" => Some(ImageFormat::Svg),
        "image/tiff" => Some(ImageFormat::Tiff),
        _ => None,
    }
}

fn image_from_bytes(format: ImageFormat, bytes: Vec<u8>) -> Arc<Image> {
    Arc::new(Image::from_bytes(format, bytes))
}

fn image_from_thumb(thumb: Thumbnail) -> Option<Arc<Image>> {
    let format = format_from_content_type(&thumb.content_type)?;
    Some(image_from_bytes(format, thumb.bytes))
}

/// Thumbnail bytes already used in the grid / details.
pub fn thumb_image(session: &Session, cell: &ItemCell) -> Option<Arc<Image>> {
    if !cell.has_thumbnail {
        return None;
    }
    let thumb = session.image(&format!("/{}", cell.id.0), Some(cell.thumb_version))?;
    image_from_thumb(thumb)
}

/// Raw preview payload before session-only rotation (D-058).
#[derive(Clone)]
pub struct PreviewPayload {
    pub bytes: Vec<u8>,
    pub format: ImageFormat,
}

impl PreviewPayload {
    pub fn to_image(&self) -> Arc<Image> {
        image_from_bytes(self.format, self.bytes.clone())
    }

    /// Clockwise rotation; returns a PNG so every angle is displayable.
    pub fn rotated(&self, degrees: u16) -> Option<Arc<Image>> {
        let degrees = degrees % 360;
        if degrees == 0 {
            return Some(self.to_image());
        }
        let dyn_img = image::load_from_memory(&self.bytes).ok()?;
        let rotated = match degrees {
            90 => dyn_img.rotate90(),
            180 => dyn_img.rotate180(),
            270 => dyn_img.rotate270(),
            _ => dyn_img,
        };
        let mut png = Cursor::new(Vec::new());
        rotated.write_to(&mut png, image::ImageFormat::Png).ok()?;
        Some(image_from_bytes(ImageFormat::Png, png.into_inner()))
    }
}

/// Full-size preview source: original when showable; HEIC via `media`; else 512 thumb.
pub fn preview_payload(session: &Session, cell: &ItemCell) -> Option<PreviewPayload> {
    let ext = cell.ext.to_ascii_lowercase();

    if shows_original(&ext) {
        if let Some(original) = session.image(&format!("/{}/original", cell.id.0), None) {
            if let Some(format) = format_from_content_type(&original.content_type) {
                return Some(PreviewPayload {
                    bytes: original.bytes,
                    format,
                });
            }
        }
    }

    // HEIC/HEIF: decode through the media pipeline (webp of the original).
    if matches!(ext.as_str(), "heic" | "heif") {
        if let Some(original) = session.image(&format!("/{}/original", cell.id.0), None) {
            if let Ok(rendered) = media::render_bytes(&original.bytes, &ext) {
                return Some(PreviewPayload {
                    bytes: rendered.thumbnail.webp,
                    format: ImageFormat::Webp,
                });
            }
        }
    }

    // TIFF / PSD / other: 512 thumb when present (shipping parity).
    if cell.has_thumbnail {
        if let Some(thumb) = session.image(&format!("/{}", cell.id.0), Some(cell.thumb_version)) {
            if let Some(format) = format_from_content_type(&thumb.content_type) {
                return Some(PreviewPayload {
                    bytes: thumb.bytes,
                    format,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_common_rasters_as_original() {
        assert!(shows_original("JPG"));
        assert!(shows_original("png"));
        assert!(shows_original("webp"));
        assert!(!shows_original("heic"));
        assert!(!shows_original("psd"));
        assert!(!shows_original("tiff"));
    }
}
