//! What the native library learns from a file: its dimensions, a 512 px WebP thumbnail, a colour
//! palette and EXIF (docs/architecture.md §15.4). Plain functions on a path; no database, no
//! Tauri. Formats without a preview are reported as [`MediaError::Unsupported`] and get an
//! extension placeholder (D-015). HEIC/HEIF decode through libheif (D-063).

mod decode;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
mod metadata;
mod palette;
mod thumbnail;

use std::path::Path;

use image::RgbaImage;
use library_core::Palette;
use thiserror::Error;

pub use palette::palette;

/// The long edge of a thumbnail, in pixels.
pub const THUMBNAIL_SIZE: u32 = 512;

/// How a file is read, by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Raster,
    Svg,
    Psd,
    Heic,
    /// Stored with a placeholder: no decoder in this version (AVIF, video, documents, …).
    Other,
}

const RASTER: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "gif", "webp", "tif", "tiff", "bmp", "dib",
];
const SVG: &[&str] = &["svg", "svgz"];
const PSD: &[&str] = &["psd"];
const HEIC: &[&str] = &["heic", "heif", "hif"];

/// Every lowercase extension that gets a preview.
pub fn preview_extensions() -> impl Iterator<Item = &'static str> {
    RASTER.iter().chain(SVG).chain(PSD).chain(HEIC).copied()
}

/// The kind for a lowercase or mixed-case extension without the dot.
pub fn kind(ext: &str) -> Kind {
    let ext = ext.to_ascii_lowercase();
    if RASTER.contains(&ext.as_str()) {
        Kind::Raster
    } else if SVG.contains(&ext.as_str()) {
        Kind::Svg
    } else if PSD.contains(&ext.as_str()) {
        Kind::Psd
    } else if HEIC.contains(&ext.as_str()) {
        Kind::Heic
    } else {
        Kind::Other
    }
}

/// Whether a file with this extension gets a preview (docs/architecture.md §15.4).
pub fn has_preview(ext: &str) -> bool {
    !matches!(kind(ext), Kind::Other)
}

/// A decoded image, ready for the pipeline.
pub struct Rendered {
    pub width: u32,
    pub height: u32,
    pub thumbnail: Thumbnail,
    pub palette: Option<Palette>,
    pub exif: Option<metadata::Exif>,
}

/// The 512 px lossy WebP thumbnail of a file (THUMBNAIL_SIZE).
pub struct Thumbnail {
    pub webp: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("decode: {0}")]
    Decode(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}

/// Render a file: dimensions, a THUMBNAIL_SIZE lossy WebP thumbnail, a palette and EXIF.
///
/// Formats without a decoder (Kind::Other) are rejected with MediaError::Unsupported before
/// any decoding is attempted; the caller stores an extension placeholder instead.
pub fn render(path: &Path, ext: &str) -> Result<Rendered, MediaError> {
    let bytes = std::fs::read(path)?;
    render_bytes(&bytes, ext)
}

/// The byte-slice variant of [`render`].
pub fn render_bytes(bytes: &[u8], ext: &str) -> Result<Rendered, MediaError> {
    if let Kind::Other = kind(ext) {
        return Err(MediaError::Unsupported(format!("no decoder for {ext}")));
    }
    let decoded = decode::decode(bytes, ext)?;
    let thumbnail = thumbnail::thumbnail(&decoded)?;
    let palette = palette::palette(&decoded);
    let exif = metadata::exif(bytes, ext);
    Ok(Rendered {
        width: decoded.width(),
        height: decoded.height(),
        thumbnail,
        palette,
        exif,
    })
}
