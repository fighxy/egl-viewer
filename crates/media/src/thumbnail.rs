//! A thumbnail fitted into a square, never enlarged, encoded as lossy WebP.

use fast_image_resize as fr;
use image::RgbaImage;

use crate::{MediaError, Thumbnail};

const QUALITY: f32 = 80.0;

/// The size that fits `(width, height)` into `size` × `size`, keeping the aspect ratio.
pub(crate) fn fit(width: u32, height: u32, size: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= size {
        return (width.max(1), height.max(1));
    }
    let scale = f64::from(size) / f64::from(long);
    let scaled = |side: u32| ((f64::from(side) * scale).round() as u32).clamp(1, size);
    (scaled(width), scaled(height))
}

pub(crate) fn make(pixels: &RgbaImage, size: u32) -> Result<Thumbnail, MediaError> {
    let (width, height) = fit(pixels.width(), pixels.height(), size);
    let resized = resize(pixels, width, height)?;
    let (pixels_rgba, _) = resized.as_chunks::<4>();
    let opaque = pixels_rgba.iter().all(|pixel| pixel[3] == u8::MAX);
    let webp = if opaque {
        let rgb: Vec<u8> = pixels_rgba
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        webp::Encoder::from_rgb(&rgb, width, height)
            .encode(QUALITY)
            .to_vec()
    } else {
        webp::Encoder::from_rgba(&resized, width, height)
            .encode(QUALITY)
            .to_vec()
    };
    Ok(Thumbnail {
        width,
        height,
        webp,
    })
}

/// Lanczos3 with alpha handled by the resizer.
pub(crate) fn resize(pixels: &RgbaImage, width: u32, height: u32) -> Result<Vec<u8>, MediaError> {
    if (width, height) == pixels.dimensions() {
        return Ok(pixels.as_raw().clone());
    }
    let resize_error =
        |error: &dyn std::fmt::Display| MediaError::Decode(format!("resize: {error}"));
    let source = fr::images::ImageRef::new(
        pixels.width(),
        pixels.height(),
        pixels.as_raw(),
        fr::PixelType::U8x4,
    )
    .map_err(|error| resize_error(&error))?;
    let mut target = fr::images::Image::new(width, height, fr::PixelType::U8x4);
    let options =
        fr::ResizeOptions::new().resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3));
    fr::Resizer::new()
        .resize(&source, &mut target, &options)
        .map_err(|error| resize_error(&error))?;
    Ok(target.into_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_into_the_square_without_enlarging() {
        assert_eq!(fit(4000, 3000, 512), (512, 384));
        assert_eq!(fit(3000, 4000, 512), (384, 512));
        assert_eq!(fit(10_000, 10, 512), (512, 1));
        assert_eq!(fit(300, 200, 512), (300, 200));
        assert_eq!(fit(512, 512, 512), (512, 512));
    }
}
