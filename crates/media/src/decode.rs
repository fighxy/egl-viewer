//! Raster images through `image`, SVG through `resvg`, PSD composites through `psd`.

use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbaImage};
use resvg::{tiny_skia, usvg};

use crate::{Decoded, MediaError};

/// Decoding stops above this many bytes of pixel memory instead of exhausting RAM.
const MAX_ALLOC: u64 = 1 << 30;
/// PSD files are read whole; larger ones keep the placeholder.
const MAX_PSD: u64 = 1 << 30;

fn decode_error(error: impl std::fmt::Display) -> MediaError {
    MediaError::Decode(error.to_string())
}

/// JPEG, PNG, GIF (first frame), WebP, TIFF, BMP, with the EXIF orientation applied.
pub(crate) fn raster(path: &Path) -> Result<Decoded, MediaError> {
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(too_large_or_decode)?;
    let orientation = decoder.orientation().map_err(decode_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(too_large_or_decode)?;
    image.apply_orientation(orientation);
    let pixels = image.into_rgba8();
    Ok(Decoded {
        width: pixels.width(),
        height: pixels.height(),
        pixels,
    })
}

fn too_large_or_decode(error: image::ImageError) -> MediaError {
    match error {
        image::ImageError::Limits(_) => MediaError::TooLarge,
        other => decode_error(other),
    }
}

/// System fonts for text in SVG files, loaded once per process.
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut fonts = usvg::fontdb::Database::new();
            fonts.load_system_fonts();
            Arc::new(fonts)
        })
        .clone()
}

/// Renders the SVG so that its long edge is `size` pixels; vectors scale up cleanly. External
/// files and the network are never read.
pub(crate) fn svg(path: &Path, size: u32) -> Result<Decoded, MediaError> {
    let data = fs::read(path)?;
    let options = usvg::Options {
        fontdb: fonts(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(&data, &options).map_err(decode_error)?;
    let svg_size = tree.size();
    let width = svg_size.width().ceil().max(1.0) as u32;
    let height = svg_size.height().ceil().max(1.0) as u32;
    let scale = size as f32 / svg_size.width().max(svg_size.height()).max(1.0);
    let pixel_width = (svg_size.width() * scale).round().max(1.0) as u32;
    let pixel_height = (svg_size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(pixel_width, pixel_height)
        .ok_or_else(|| MediaError::Decode("the SVG has no area".into()))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let straight: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    let pixels = RgbaImage::from_raw(pixel_width, pixel_height, straight)
        .ok_or_else(|| MediaError::Decode("the SVG rendered to a wrong size".into()))?;
    Ok(Decoded {
        width,
        height,
        pixels,
    })
}

/// The composite image that Photoshop stores in the file ("Maximize compatibility").
pub(crate) fn psd(path: &Path) -> Result<Decoded, MediaError> {
    if fs::metadata(path)?.len() > MAX_PSD {
        return Err(MediaError::TooLarge);
    }
    let bytes = fs::read(path)?;
    let psd = psd::Psd::from_bytes(&bytes).map_err(decode_error)?;
    let (width, height) = (psd.width(), psd.height());
    let pixels = RgbaImage::from_raw(width, height, psd.rgba())
        .ok_or_else(|| MediaError::Decode("the PSD composite has a wrong size".into()))?;
    Ok(Decoded {
        width,
        height,
        pixels,
    })
}
