//! Load a preview image from Session originals / thumbs / media (HEIC).
//!
//! The full-size preview reads and decodes off the UI thread (D-143): [`preview_payload`] reads
//! the bytes, [`decode_payload`] turns them into a ready-to-upload [`RenderImage`] (EXIF
//! orientation applied, BGRA, capped at [`MAX_EDGE`]), so GPUI never decodes a large original
//! while painting. Animated GIFs decode into all their frames, which the preview's player
//! steps through itself (D-154).

use std::io::Cursor;
use std::sync::Arc;

use desktop_session::Session;
use gpui_kit::{Image, ImageFormat, ImageSource, RenderImage};
use image::{DynamicImage, ImageDecoder as _, ImageReader};
use library_core::{ItemCell, Thumbnail};

/// Extensions whose original bytes the preview reads directly (no `media` decode).
const SHOWN_AS_IS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg"];

/// The longest edge of a decoded preview. Larger originals are scaled down once, on the
/// background thread: GPU textures stop at 16384 and 8192 still leaves room to zoom.
pub const MAX_EDGE: u32 = 8192;

/// Pixel memory for the frames of one animated GIF in the preview; longer or larger GIFs
/// play at a smaller size (`media::decode_gif` halves them until they fit).
const PREVIEW_ANIMATION_BYTES: u64 = 384 << 20;
/// The same for a GIF playing under the pointer in the grid.
const HOVER_ANIMATION_BYTES: u64 = 64 << 20;

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

/// Raw preview bytes, kept so a rotation can decode again (D-058).
#[derive(Clone)]
pub struct PreviewPayload {
    pub bytes: Arc<Vec<u8>>,
    /// How GPUI would read `bytes`; unused when `media_ext` is set.
    pub format: ImageFormat,
    /// The original in a format GPUI cannot read (HEIC, PSD, TIFF): `media` decodes it at
    /// full resolution.
    pub media_ext: Option<String>,
}

impl PreviewPayload {
    fn encoded(bytes: Vec<u8>, format: ImageFormat) -> Self {
        Self {
            bytes: Arc::new(bytes),
            format,
            media_ext: None,
        }
    }
}

/// Originals that `media` decodes for the preview: every previewable kind but SVG, which
/// GPUI draws itself.
fn decodes_through_media(ext: &str) -> bool {
    matches!(
        media::kind(ext),
        media::Kind::Raster | media::Kind::Psd | media::Kind::Heic
    )
}

/// What the preview paints: pixels decoded by us, every frame of an animated GIF decoded by
/// us, or encoded bytes GPUI decodes itself (SVG, the thumbnail placeholder).
#[derive(Clone)]
pub enum Shown {
    Pixels(Arc<RenderImage>),
    /// Two or more frames with their delays; the preview paints one at a time (D-154).
    Animation(Arc<RenderImage>),
    Encoded(Arc<Image>),
}

impl Shown {
    pub fn source(&self) -> ImageSource {
        match self {
            Self::Pixels(image) | Self::Animation(image) => ImageSource::Render(image.clone()),
            Self::Encoded(image) => ImageSource::Image(image.clone()),
        }
    }

    /// Our own pixels; GPUI must be told to free their texture (`App::drop_image`).
    pub fn pixels(&self) -> Option<&Arc<RenderImage>> {
        match self {
            Self::Pixels(image) | Self::Animation(image) => Some(image),
            Self::Encoded(_) => None,
        }
    }
}

/// Full-size preview source: the original (read by GPUI's formats, or at full resolution
/// through `media` for HEIC, PSD and TIFF); the thumbnail for anything else.
/// Reads files: call it off the UI thread.
pub fn preview_payload(session: &Session, cell: &ItemCell) -> Option<PreviewPayload> {
    let ext = cell.ext.to_ascii_lowercase();

    let original = || session.image(&format!("/{}/original", cell.id.0), None);

    if shows_original(&ext)
        && let Some(original) = original()
        && let Some(format) = format_from_content_type(&original.content_type)
    {
        return Some(PreviewPayload::encoded(original.bytes, format));
    }

    if decodes_through_media(&ext)
        && let Some(original) = original()
    {
        return Some(PreviewPayload {
            bytes: Arc::new(original.bytes),
            format: ImageFormat::Png,
            media_ext: Some(ext),
        });
    }

    // Anything else with a generated preview: its thumbnail.
    if cell.has_thumbnail
        && let Some(thumb) = session.image(&format!("/{}", cell.id.0), Some(cell.thumb_version))
        && let Some(format) = format_from_content_type(&thumb.content_type)
    {
        return Some(PreviewPayload::encoded(thumb.bytes, format));
    }
    None
}

/// How GPUI reads an original it shows as it is, by extension.
fn format_from_ext(ext: &str) -> Option<ImageFormat> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "gif" => ImageFormat::Gif,
        "webp" => ImageFormat::Webp,
        "bmp" => ImageFormat::Bmp,
        "svg" => ImageFormat::Svg,
        _ => return None,
    })
}

/// Full-size preview source for a loose file outside any library (D-180): the same choice as
/// [`preview_payload`], read by path. A file with nothing better (SVGZ) shows the picture
/// `media` renders for a thumbnail; video plays instead and has none. Reads the file: call it
/// off the UI thread.
pub fn file_payload(path: &std::path::Path, ext: &str) -> Option<PreviewPayload> {
    let ext = ext.to_ascii_lowercase();
    if shows_original(&ext)
        && let Some(format) = format_from_ext(&ext)
    {
        return Some(PreviewPayload::encoded(std::fs::read(path).ok()?, format));
    }
    if decodes_through_media(&ext) {
        return Some(PreviewPayload {
            bytes: Arc::new(std::fs::read(path).ok()?),
            format: ImageFormat::Png,
            media_ext: Some(ext),
        });
    }
    if media::kind(&ext) == media::Kind::Svg {
        let rendered = media::render(path, &ext).ok()?;
        return Some(PreviewPayload::encoded(
            rendered.thumbnail.webp,
            ImageFormat::Webp,
        ));
    }
    None
}

fn raster_format(format: ImageFormat) -> Option<image::ImageFormat> {
    Some(match format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Webp => image::ImageFormat::WebP,
        ImageFormat::Gif => image::ImageFormat::Gif,
        ImageFormat::Bmp => image::ImageFormat::Bmp,
        ImageFormat::Tiff => image::ImageFormat::Tiff,
        ImageFormat::Ico => image::ImageFormat::Ico,
        ImageFormat::Pnm => image::ImageFormat::Pnm,
        ImageFormat::Svg => return None,
    })
}

/// Decodes with the file's EXIF orientation applied, scaled to fit [`MAX_EDGE`].
fn decode_upright(bytes: &[u8], format: image::ImageFormat) -> Option<DynamicImage> {
    let mut decoder = ImageReader::with_format(Cursor::new(bytes), format)
        .into_decoder()
        .ok()?;
    let orientation = decoder.orientation().ok();
    let mut decoded = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(orientation) = orientation {
        decoded.apply_orientation(orientation);
    }
    Some(fit_edge(decoded))
}

/// Scales down to [`MAX_EDGE`] on the long side; smaller images stay as they are.
fn fit_edge(image: DynamicImage) -> DynamicImage {
    if image.width().max(image.height()) > MAX_EDGE {
        image.resize(MAX_EDGE, MAX_EDGE, image::imageops::FilterType::Triangle)
    } else {
        image
    }
}

fn rotate(image: DynamicImage, degrees: u16) -> DynamicImage {
    match degrees % 360 {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    }
}

/// GPUI paints BGRA frames.
fn to_bgra(image: DynamicImage) -> image::RgbaImage {
    let mut rgba = image.into_rgba8();
    let (pixels, _) = rgba.as_chunks_mut::<4>();
    for pixel in pixels {
        pixel.swap(0, 2);
    }
    rgba
}

fn to_render_image(image: DynamicImage) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(to_bgra(image))]))
}

/// Every frame, turned clockwise by `degrees`, with its file delay.
fn animation_image(animation: media::Animation, degrees: u16) -> Arc<RenderImage> {
    let frames: Vec<image::Frame> = animation
        .frames
        .into_iter()
        .map(|frame| {
            let turned = rotate(DynamicImage::ImageRgba8(frame.pixels), degrees);
            let delay = image::Delay::from_saturating_duration(frame.delay);
            image::Frame::from_parts(to_bgra(turned), 0, 0, delay)
        })
        .collect();
    Arc::new(RenderImage::new(frames))
}

/// A GIF for the preview: every frame when it moves, one picture when it is still.
fn decode_gif(bytes: &[u8], degrees: u16) -> Option<Shown> {
    let limits = media::AnimationLimits {
        max_edge: MAX_EDGE,
        max_bytes: PREVIEW_ANIMATION_BYTES,
    };
    match media::decode_gif(bytes, limits) {
        Ok(animation) if animation.frames.len() > 1 => {
            Some(Shown::Animation(animation_image(animation, degrees)))
        }
        Ok(mut still) => {
            let frame = still.frames.pop()?;
            let turned = rotate(DynamicImage::ImageRgba8(frame.pixels), degrees);
            Some(Shown::Pixels(to_render_image(turned)))
        }
        // Too many frames even when small: the first frame still shows.
        Err(_) => {
            let first = decode_upright(bytes, image::ImageFormat::Gif)?;
            Some(Shown::Pixels(to_render_image(rotate(first, degrees))))
        }
    }
}

/// The frames of an animated GIF for a grid cell whose long edge is `edge` pixels; `None` for
/// a still or unreadable GIF (the thumbnail stays). Heavy: background only.
pub fn hover_animation(bytes: &[u8], edge: u32) -> Option<Arc<RenderImage>> {
    let limits = media::AnimationLimits {
        max_edge: edge.clamp(media::MIN_ANIMATION_EDGE, 1024),
        max_bytes: HOVER_ANIMATION_BYTES,
    };
    let animation = media::decode_gif(bytes, limits).ok()?;
    (animation.frames.len() > 1).then(|| animation_image(animation, 0))
}

/// Decodes the payload for painting, rotated clockwise by `degrees`. Heavy: background only.
/// A GIF keeps its animation at any rotation; SVG stays encoded (no rotation).
pub fn decode_payload(payload: &PreviewPayload, degrees: u16) -> Option<Shown> {
    let degrees = degrees % 360;
    if let Some(ext) = &payload.media_ext {
        let pixels = media::decode_full(&payload.bytes, ext).ok()?;
        let decoded = fit_edge(DynamicImage::ImageRgba8(pixels));
        return Some(Shown::Pixels(to_render_image(rotate(decoded, degrees))));
    }
    let encoded = || Shown::Encoded(image_from_bytes(payload.format, (*payload.bytes).clone()));
    let Some(format) = raster_format(payload.format) else {
        return Some(encoded());
    };
    if payload.format == ImageFormat::Gif {
        return decode_gif(&payload.bytes, degrees);
    }
    let decoded = decode_upright(&payload.bytes, format)?;
    Some(Shown::Pixels(to_render_image(rotate(decoded, degrees))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn png(width: u32, height: u32) -> PreviewPayload {
        let mut pixels = RgbaImage::from_pixel(width, height, Rgba([10, 20, 30, 255]));
        pixels.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(pixels)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        PreviewPayload::encoded(bytes.into_inner(), ImageFormat::Png)
    }

    fn size(shown: &Shown) -> (i32, i32) {
        let image = shown.pixels().expect("decoded pixels");
        let size = image.size(0);
        (size.width.0, size.height.0)
    }

    #[test]
    fn loose_files_are_read_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("p.PNG");
        std::fs::write(&photo, png(3, 2).bytes.as_slice()).unwrap();
        let payload = file_payload(&photo, "PNG").expect("a PNG shows as it is");
        assert!(payload.format == ImageFormat::Png && payload.media_ext.is_none());
        assert_eq!(size(&decode_payload(&payload, 0).unwrap()), (3, 2));

        // TIFF, PSD and HEIC go through `media` at full size.
        let tiff = dir.path().join("t.tif");
        std::fs::write(&tiff, b"not read yet").unwrap();
        let payload = file_payload(&tiff, "tif").unwrap();
        assert_eq!(payload.media_ext.as_deref(), Some("tif"));

        // Video plays instead; a missing file gives nothing.
        assert!(file_payload(&dir.path().join("v.mp4"), "mp4").is_none());
        assert!(file_payload(&dir.path().join("gone.jpg"), "jpg").is_none());
    }

    #[test]
    fn shows_common_rasters_as_original() {
        assert!(shows_original("JPG"));
        assert!(shows_original("png"));
        assert!(shows_original("webp"));
        assert!(!shows_original("heic"));
        assert!(!shows_original("psd"));
        assert!(!shows_original("tiff"));
    }

    #[test]
    fn decodes_to_bgra_pixels() {
        let shown = decode_payload(&png(4, 2), 0).unwrap();
        assert_eq!(size(&shown), (4, 2));
        let image = shown.pixels().unwrap();
        // Red in RGBA is [255, 0, 0]; GPUI wants it as BGRA.
        assert_eq!(&image.as_bytes(0).unwrap()[..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn quarter_turns_swap_the_sides() {
        assert_eq!(size(&decode_payload(&png(4, 2), 90).unwrap()), (2, 4));
        assert_eq!(size(&decode_payload(&png(4, 2), 180).unwrap()), (4, 2));
        assert_eq!(size(&decode_payload(&png(4, 2), 270).unwrap()), (2, 4));
    }

    #[test]
    fn huge_originals_are_capped() {
        let shown = decode_payload(&png(MAX_EDGE * 2, 4), 0).unwrap();
        assert_eq!(size(&shown).0, MAX_EDGE as i32);
    }

    #[test]
    fn broken_bytes_give_nothing() {
        let payload = PreviewPayload::encoded(vec![1, 2, 3], ImageFormat::Jpeg);
        assert!(decode_payload(&payload, 0).is_none());
        let heic = PreviewPayload {
            media_ext: Some("heic".into()),
            ..PreviewPayload::encoded(b"not a heic".to_vec(), ImageFormat::Png)
        };
        assert!(decode_payload(&heic, 0).is_none());
    }

    #[test]
    fn media_formats_decode_at_full_resolution() {
        // TIFF goes through `media`, like HEIC and PSD: larger than a 512 px thumbnail.
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::new(1200, 700))
            .write_to(&mut bytes, image::ImageFormat::Tiff)
            .unwrap();
        let payload = PreviewPayload {
            media_ext: Some("tiff".into()),
            ..PreviewPayload::encoded(bytes.into_inner(), ImageFormat::Png)
        };
        assert_eq!(size(&decode_payload(&payload, 0).unwrap()), (1200, 700));
        assert_eq!(size(&decode_payload(&payload, 90).unwrap()), (700, 1200));
        assert!(decodes_through_media("HEIC"));
        assert!(decodes_through_media("psd"));
        assert!(!decodes_through_media("svg"));
    }

    fn gif(width: u32, height: u32, delays_ms: &[u32]) -> PreviewPayload {
        PreviewPayload::encoded(
            media::fixtures::animated_gif(width, height, delays_ms),
            ImageFormat::Gif,
        )
    }

    #[test]
    fn animated_gifs_decode_every_frame_at_any_rotation() {
        for degrees in [0, 90, 180, 270] {
            let Some(Shown::Animation(image)) = decode_payload(&gif(6, 4, &[40, 80, 120]), degrees)
            else {
                panic!("an animation at {degrees}°");
            };
            assert_eq!(image.frame_count(), 3);
            let size = image.size(2);
            let expected = if degrees % 180 == 0 { (6, 4) } else { (4, 6) };
            assert_eq!((size.width.0, size.height.0), expected, "{degrees}°");
            assert_eq!(
                std::time::Duration::from(image.delay(1)),
                std::time::Duration::from_millis(80)
            );
        }
    }

    #[test]
    fn a_still_gif_is_plain_pixels() {
        let still = PreviewPayload::encoded(
            media::fixtures::raster(image::ImageFormat::Gif, 5, 3, 1),
            ImageFormat::Gif,
        );
        assert!(matches!(decode_payload(&still, 0), Some(Shown::Pixels(_))));
        assert!(decode_payload(&gif(5, 3, &[50]), 90).is_some());
    }

    #[test]
    fn hover_frames_fit_the_cell_and_skip_stills() {
        let bytes = media::fixtures::animated_gif(800, 400, &[50, 50]);
        let frames = hover_animation(&bytes, 200).expect("frames");
        assert_eq!(frames.frame_count(), 2);
        assert_eq!(frames.size(0).width.0, 200);
        let still = media::fixtures::animated_gif(10, 10, &[50]);
        assert!(hover_animation(&still, 200).is_none());
        assert!(hover_animation(b"nope", 200).is_none());
    }

    #[test]
    fn svg_stays_encoded() {
        let payload = PreviewPayload::encoded(
            b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec(),
            ImageFormat::Svg,
        );
        assert!(matches!(
            decode_payload(&payload, 90),
            Some(Shown::Encoded(_))
        ));
    }
}
