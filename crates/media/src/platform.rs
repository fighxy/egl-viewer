//! A decoder the host platform lends for formats this build cannot read itself: HEIC on iOS,
//! where there is no libheif, goes through ImageIO (D-096). The host sets it once; it runs on
//! the import's threads, so it must be thread-safe.

use std::sync::{Arc, RwLock};

use image::RgbaImage;

use crate::{Decoded, MediaError};

/// What a platform decoder hands back: the image's displayed size and RGBA pixels, which may
/// be smaller (a decoder may scale down to save memory; the thumbnail is small anyway).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformImage {
    pub width: u32,
    pub height: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub rgba: Vec<u8>,
}

/// Decodes the bytes of a file with extension `ext`; `None` when it cannot.
pub type PlatformDecoder = Arc<dyn Fn(&[u8], &str) -> Option<PlatformImage> + Send + Sync>;

static DECODER: RwLock<Option<PlatformDecoder>> = RwLock::new(None);

/// Sets (or with `None` clears) the process's platform decoder.
pub fn set_platform_decoder(decoder: Option<PlatformDecoder>) {
    *DECODER
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = decoder;
}

/// Pixels memory a platform image may take, as for the built-in decoders.
const MAX_PIXEL_BYTES: u64 = 1 << 30;

/// The platform decoder's answer for `bytes`, checked: sizes that do not match the pixels are
/// refused rather than trusted.
pub(crate) fn decode(bytes: &[u8], ext: &str) -> Option<Result<Decoded, MediaError>> {
    let decoder = DECODER
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()?;
    let image = decoder(bytes, ext)?;
    Some(checked(image))
}

fn checked(image: PlatformImage) -> Result<Decoded, MediaError> {
    let expected = u64::from(image.pixel_width) * u64::from(image.pixel_height) * 4;
    if image.width == 0 || image.height == 0 || image.pixel_width == 0 || image.pixel_height == 0 {
        return Err(MediaError::Decode(
            "the platform decoder gave an empty image".into(),
        ));
    }
    if expected > MAX_PIXEL_BYTES {
        return Err(MediaError::TooLarge);
    }
    if expected != image.rgba.len() as u64 {
        return Err(MediaError::Decode(
            "the platform decoder gave pixels of the wrong size".into(),
        ));
    }
    let pixels = RgbaImage::from_raw(image.pixel_width, image.pixel_height, image.rgba)
        .ok_or_else(|| MediaError::Decode("the platform decoder gave a wrong size".into()))?;
    Ok(Decoded {
        width: image.width,
        height: image.height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(pixel_width: u32, pixel_height: u32, len: usize) -> PlatformImage {
        PlatformImage {
            width: pixel_width * 4,
            height: pixel_height * 4,
            pixel_width,
            pixel_height,
            rgba: vec![200; len],
        }
    }

    #[test]
    fn a_platform_image_keeps_its_displayed_size_over_smaller_pixels() {
        let decoded = checked(image(4, 2, 32)).unwrap();
        assert_eq!((decoded.width, decoded.height), (16, 8));
        assert_eq!(decoded.pixels.dimensions(), (4, 2));
    }

    /// The decoder answers only for its own marker, so the other tests of this process (which
    /// share the global decoder) still reach libheif.
    #[test]
    fn heic_goes_through_the_platform_decoder_when_one_is_set() {
        set_platform_decoder(Some(Arc::new(|bytes: &[u8], ext: &str| {
            (ext == "heic" && bytes.starts_with(b"FAKEHEIC")).then(|| PlatformImage {
                width: 640,
                height: 480,
                pixel_width: 64,
                pixel_height: 48,
                rgba: vec![90; 64 * 48 * 4],
            })
        })));
        let rendered = crate::render_bytes(b"FAKEHEIC and more", "heic").unwrap();
        assert_eq!((rendered.width, rendered.height), (640, 480));
        assert!(!rendered.thumbnail.webp.is_empty());
        // Other bytes fall through to the built-in path (libheif off iOS), which refuses junk.
        assert!(crate::render_bytes(b"not an image", "heic").is_err());
    }

    #[test]
    fn wrong_or_empty_sizes_are_refused() {
        assert!(checked(image(4, 2, 31)).is_err());
        assert!(checked(image(0, 2, 0)).is_err());
    }
}
