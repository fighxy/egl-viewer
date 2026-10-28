//! What the native library learns from a file: its dimensions, a 512 px WebP thumbnail, a colour
//! palette and EXIF (docs/architecture.md §15.4). Plain functions on a path; no database, no
//! Tauri. Formats without a preview are reported as [`MediaError::Unsupported`] and get an
//! extension placeholder (D-015).

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
    /// Stored with a placeholder: no decoder in this version (AVIF, video, documents, …).
    Other,
}

const RASTER: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "gif", "webp", "tif", "tiff", "bmp", "dib",
];
const SVG: &[&str] = &["svg", "svgz"];
const PSD: &[&str] = &["psd"];

/// Every lowercase extension that gets a preview.
pub fn preview_extensions() -> impl Iterator<Item = &'static str> {
    RASTER.iter().chain(SVG).chain(PSD).copied()
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
    } else {
        Kind::Other
    }
}

/// Files with this extension get a thumbnail.
pub fn has_preview(ext: &str) -> bool {
    kind(ext) != Kind::Other
}

#[derive(Debug, Clone, PartialEq)]
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    /// Lossy WebP.
    pub webp: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// The image as it is shown: EXIF orientation applied; CSS pixels for SVG.
    pub width: u32,
    pub height: u32,
    pub thumbnail: Thumbnail,
    /// Largest share first, in Eagle's palette shape.
    pub palette: Vec<Palette>,
    /// Eagle `rawMetas` keys: `camera`, `isoSpeed`, `aperture`, `focalLength`, `shutter`,
    /// `timestamp` (correction 38). `None` without EXIF.
    pub exif: Option<serde_json::Value>,
}

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("this file type has no preview yet")]
    Unsupported,
    #[error("cannot read the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot decode the image: {0}")]
    Decode(String),
    #[error("the image is too large to preview")]
    TooLarge,
}

/// A PNG of RGBA pixels, for images pasted from the clipboard.
pub fn png_from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Vec<u8>, MediaError> {
    let image = RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| MediaError::Decode("the pasted image has a wrong size".into()))?;
    let mut png = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| MediaError::Decode(error.to_string()))?;
    Ok(png.into_inner())
}

/// Decodes `path`, makes the thumbnail and the palette, and reads EXIF.
pub fn render(path: &Path, ext: &str) -> Result<Rendered, MediaError> {
    let decoded = match kind(ext) {
        Kind::Raster => decode::raster(path)?,
        Kind::Svg => decode::svg(path, THUMBNAIL_SIZE)?,
        Kind::Psd => decode::psd(path)?,
        Kind::Other => return Err(MediaError::Unsupported),
    };
    let thumbnail = thumbnail::make(&decoded.pixels, THUMBNAIL_SIZE)?;
    let palette = palette(&decoded.pixels);
    let exif = match kind(ext) {
        Kind::Raster => metadata::exif(path),
        _ => None,
    };
    Ok(Rendered {
        width: decoded.width,
        height: decoded.height,
        thumbnail,
        palette,
        exif,
    })
}

/// Like [`render`], from the file's bytes in memory: a decrypted original of a locked folder
/// is never written to disk in plaintext (docs/age-locked-folders.md §2).
pub fn render_bytes(bytes: &[u8], ext: &str) -> Result<Rendered, MediaError> {
    let decoded = match kind(ext) {
        Kind::Raster => decode::raster_bytes(bytes)?,
        Kind::Svg => decode::svg_data(bytes, THUMBNAIL_SIZE)?,
        Kind::Psd => decode::psd_bytes(bytes)?,
        Kind::Other => return Err(MediaError::Unsupported),
    };
    let thumbnail = thumbnail::make(&decoded.pixels, THUMBNAIL_SIZE)?;
    let palette = palette(&decoded.pixels);
    let exif = match kind(ext) {
        Kind::Raster => metadata::exif_bytes(bytes),
        _ => None,
    };
    Ok(Rendered {
        width: decoded.width,
        height: decoded.height,
        thumbnail,
        palette,
        exif,
    })
}

/// A decoded image: its displayed size, and pixels that may be smaller (SVG is rendered at
/// thumbnail size).
pub(crate) struct Decoded {
    pub width: u32,
    pub height: u32,
    pub pixels: RgbaImage,
}

#[cfg(test)]
mod tests;
