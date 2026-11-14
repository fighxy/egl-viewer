//! What the native library learns from a file: its dimensions, a 512 px WebP thumbnail, a colour
//! palette and EXIF (docs/architecture.md §15.4). Plain functions on a path; no database, no
//! Tauri. Formats without a preview are reported as [`MediaError::Unsupported`] and get an
//! extension placeholder (D-015). HEIC/HEIF decode through libheif (D-063).

mod animation;
mod decode;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
mod metadata;
mod palette;
mod platform;
mod thumbnail;

use std::path::Path;

use image::RgbaImage;
use library_core::Palette;
use thiserror::Error;

pub use animation::{Animation, AnimationFrame, AnimationLimits, MIN_ANIMATION_EDGE, decode_gif};
pub use palette::palette;
pub use platform::{PlatformDecoder, PlatformImage, set_platform_decoder};

/// The long edge of a thumbnail, in pixels.
pub const THUMBNAIL_SIZE: u32 = 512;

/// How a file is read, by its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Raster,
    Svg,
    Psd,
    Heic,
    /// A poster frame through FFmpeg (D-169); desktop builds only.
    Video,
    /// Stored with a placeholder: no decoder in this version (AVIF, audio, documents, …).
    Other,
}

const RASTER: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "gif", "webp", "tif", "tiff", "bmp", "dib",
];
const SVG: &[&str] = &["svg", "svgz"];
const PSD: &[&str] = &["psd"];
const HEIC: &[&str] = &["heic", "heif", "hif"];
/// Video files with a poster frame: desktop builds have FFmpeg, iOS and Android do not.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
const VIDEO: &[&str] = av::VIDEO;
#[cfg(any(target_os = "ios", target_os = "android"))]
const VIDEO: &[&str] = &[];

/// Every lowercase extension that gets a preview.
pub fn preview_extensions() -> impl Iterator<Item = &'static str> {
    RASTER
        .iter()
        .chain(SVG)
        .chain(PSD)
        .chain(HEIC)
        .chain(VIDEO)
        .copied()
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
    } else if VIDEO.contains(&ext.as_str()) {
        Kind::Video
    } else {
        Kind::Other
    }
}

/// Audio files whose length is read when they are added (D-169); none on iOS and Android.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
const AUDIO: &[&str] = av::AUDIO;
#[cfg(any(target_os = "ios", target_os = "android"))]
const AUDIO: &[&str] = &[];

/// Every lowercase extension whose length is read without a thumbnail (audio).
pub fn duration_extensions() -> impl Iterator<Item = &'static str> {
    AUDIO.iter().copied()
}

/// Whether a file of this extension gets its length read without a thumbnail.
pub fn has_duration_only(ext: &str) -> bool {
    let ext = ext.to_ascii_lowercase();
    AUDIO.contains(&ext.as_str())
}

/// The length in seconds of an audio file, from its headers; `None` when it cannot be read.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn duration(path: &Path) -> Option<f64> {
    av::probe(path)
        .ok()?
        .duration
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
}

#[cfg(any(target_os = "ios", target_os = "android"))]
pub fn duration(_: &Path) -> Option<f64> {
    None
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
    /// Seconds, for video (D-169).
    pub duration: Option<f64>,
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

/// A small PNG of a thumbnail: the image under the cursor while files are dragged out of the
/// window. PNG, because every platform's drag helper decodes it.
pub fn drag_image(thumbnail: &[u8], size: u32) -> Result<Vec<u8>, MediaError> {
    let decoded = decode::raster_bytes(thumbnail)?;
    let (width, height) = thumbnail::fit(decoded.width, decoded.height, size);
    png_from_rgba(
        width,
        height,
        thumbnail::resize(&decoded.pixels, width, height)?,
    )
}

/// Decodes `path`, makes the thumbnail and the palette, and reads EXIF.
pub fn render(path: &Path, ext: &str) -> Result<Rendered, MediaError> {
    if kind(ext) == Kind::Heic {
        // One read serves both the decode and the EXIF item.
        return render_heic(&std::fs::read(path)?);
    }
    if kind(ext) == Kind::Video {
        return render_video(path);
    }
    let decoded = match kind(ext) {
        Kind::Raster => decode::raster(path)?,
        Kind::Svg => decode::svg(path, THUMBNAIL_SIZE)?,
        Kind::Psd => decode::psd(path)?,
        Kind::Heic | Kind::Video | Kind::Other => return Err(MediaError::Unsupported),
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
        duration: None,
    })
}

/// Like [`render`], from the file's bytes in memory: a decrypted original of a locked folder
/// is never written to disk in plaintext (docs/age-locked-folders.md §2).
pub fn render_bytes(bytes: &[u8], ext: &str) -> Result<Rendered, MediaError> {
    if kind(ext) == Kind::Heic {
        return render_heic(bytes);
    }
    if kind(ext) == Kind::Video {
        // FFmpeg reads files by path; a locked original stays encrypted on disk, so its video
        // keeps the placeholder (D-169).
        return Err(MediaError::Decode(
            "videos in locked folders get no preview".into(),
        ));
    }
    let decoded = match kind(ext) {
        Kind::Raster => decode::raster_bytes(bytes)?,
        Kind::Svg => decode::svg_data(bytes, THUMBNAIL_SIZE)?,
        Kind::Psd => decode::psd_bytes(bytes)?,
        Kind::Heic | Kind::Video | Kind::Other => return Err(MediaError::Unsupported),
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
        duration: None,
    })
}

/// The whole image at its own resolution and display-oriented (EXIF, HEIF transforms): the
/// full-size preview of files a UI toolkit cannot decode by itself, such as HEIC, PSD and
/// TIFF (D-143). SVG has no pixel size of its own and is refused.
pub fn decode_full(bytes: &[u8], ext: &str) -> Result<RgbaImage, MediaError> {
    let decoded = match kind(ext) {
        Kind::Raster => decode::raster_bytes(bytes)?,
        Kind::Psd => decode::psd_bytes(bytes)?,
        Kind::Heic => decode::heic_bytes(bytes)?,
        Kind::Svg | Kind::Video | Kind::Other => return Err(MediaError::Unsupported),
    };
    Ok(decoded.pixels)
}

/// HEIC/HEIF from bytes: the decode and the EXIF item both parse the container.
fn render_heic(bytes: &[u8]) -> Result<Rendered, MediaError> {
    let decoded = decode::heic_bytes(bytes)?;
    let thumbnail = thumbnail::make(&decoded.pixels, THUMBNAIL_SIZE)?;
    let palette = palette(&decoded.pixels);
    let exif = metadata::exif_heic_bytes(bytes);
    Ok(Rendered {
        width: decoded.width,
        height: decoded.height,
        thumbnail,
        palette,
        exif,
        duration: None,
    })
}

/// A video: the poster frame (a tenth in, at most 10 s) as the thumbnail and palette source, the
/// displayed size (rotation applied) and the duration.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn render_video(path: &Path) -> Result<Rendered, MediaError> {
    // Every failure is a decode error, an audio-only MP4 too: the item is marked and the
    // pending pass moves on (an `Unsupported` item would stay pending).
    let (info, pixels) =
        av::poster(path, THUMBNAIL_SIZE).map_err(|error| MediaError::Decode(error.to_string()))?;
    let (width, height) = info
        .video
        .as_ref()
        .map_or(pixels.dimensions(), |video| (video.width, video.height));
    Ok(Rendered {
        width,
        height,
        thumbnail: thumbnail::make(&pixels, THUMBNAIL_SIZE)?,
        palette: palette(&pixels),
        exif: None,
        duration: info.duration,
    })
}

#[cfg(any(target_os = "ios", target_os = "android"))]
fn render_video(_: &Path) -> Result<Rendered, MediaError> {
    Err(MediaError::Unsupported)
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
