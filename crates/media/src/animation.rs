//! Every frame of an animated GIF, composited onto the canvas, for players that step through
//! the frames themselves (D-154). The thumbnail pipeline keeps using the first frame only.

use std::io::Cursor;
use std::time::Duration;

use image::codecs::gif::GifDecoder;
use image::imageops::{self, FilterType};
use image::{AnimationDecoder, ImageDecoder, Limits, RgbaImage};

use crate::MediaError;

/// The long edge never drops below this while frames are shrunk to fit the memory budget.
pub const MIN_ANIMATION_EDGE: u32 = 128;

/// One frame as it is shown: the whole canvas, earlier frames composited under it.
#[derive(Debug, Clone)]
pub struct AnimationFrame {
    pub pixels: RgbaImage,
    /// How long the frame stays up, as written in the file (0 for frames without a delay).
    pub delay: Duration,
}

#[derive(Debug, Clone)]
pub struct Animation {
    /// The size of every frame after any scaling.
    pub width: u32,
    pub height: u32,
    pub frames: Vec<AnimationFrame>,
}

impl Animation {
    pub fn bytes(&self) -> u64 {
        frame_bytes(self.width, self.height) * self.frames.len() as u64
    }
}

/// How much memory frames may take, and how large each may be.
#[derive(Debug, Clone, Copy)]
pub struct AnimationLimits {
    /// The long edge of a frame; larger canvases are scaled down.
    pub max_edge: u32,
    /// Bytes of RGBA pixels for all frames together. Past it the frames are halved in size,
    /// down to [`MIN_ANIMATION_EDGE`]; past that the animation is refused as too large.
    pub max_bytes: u64,
}

fn frame_bytes(width: u32, height: u32) -> u64 {
    u64::from(width) * u64::from(height) * 4
}

/// `(width, height)` scaled so the long edge is at most `edge`; never enlarged, never 0.
fn fit(width: u32, height: u32, edge: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= edge || long == 0 {
        return (width.max(1), height.max(1));
    }
    let scale = f64::from(edge) / f64::from(long);
    let scaled = |side: u32| ((f64::from(side) * scale).round() as u32).max(1);
    (scaled(width), scaled(height))
}

fn decode_error(error: image::ImageError) -> MediaError {
    match error {
        image::ImageError::Limits(_) => MediaError::TooLarge,
        other => MediaError::Decode(other.to_string()),
    }
}

/// Decodes every frame of a GIF. A still GIF gives one frame. Frames are scaled to
/// `limits.max_edge` and, if they still need more than `limits.max_bytes`, halved until they
/// fit (or refused with [`MediaError::TooLarge`]).
pub fn decode_gif(bytes: &[u8], limits: AnimationLimits) -> Result<Animation, MediaError> {
    let mut decoder = GifDecoder::new(Cursor::new(bytes)).map_err(decode_error)?;
    let mut decoder_limits = Limits::default();
    // One canvas at a time: the frame buffers are what the budget below counts.
    decoder_limits.max_alloc = Some(1 << 30);
    decoder.set_limits(decoder_limits).map_err(decode_error)?;
    let (canvas_w, canvas_h) = decoder.dimensions();
    let mut edge = limits.max_edge.max(MIN_ANIMATION_EDGE);
    let (mut width, mut height) = fit(canvas_w, canvas_h, edge);
    let mut frames: Vec<AnimationFrame> = Vec::new();

    for frame in decoder.into_frames() {
        let frame = frame.map_err(decode_error)?;
        let delay = Duration::from(frame.delay());
        let mut pixels = frame.into_buffer();
        if pixels.dimensions() != (width, height) {
            pixels = imageops::resize(&pixels, width, height, FilterType::Triangle);
        }
        frames.push(AnimationFrame { pixels, delay });

        while frame_bytes(width, height) * frames.len() as u64 > limits.max_bytes {
            if edge / 2 < MIN_ANIMATION_EDGE {
                return Err(MediaError::TooLarge);
            }
            edge /= 2;
            (width, height) = fit(canvas_w, canvas_h, edge);
            for frame in &mut frames {
                frame.pixels = imageops::resize(&frame.pixels, width, height, FilterType::Triangle);
            }
        }
    }
    if frames.is_empty() {
        return Err(MediaError::Decode("the GIF has no frames".into()));
    }
    Ok(Animation {
        width,
        height,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    const ROOMY: AnimationLimits = AnimationLimits {
        max_edge: 4096,
        max_bytes: 1 << 30,
    };

    #[test]
    fn decodes_every_frame_with_its_delay() {
        let gif = fixtures::animated_gif(40, 20, &[30, 50, 70]);
        let animation = decode_gif(&gif, ROOMY).unwrap();
        assert_eq!((animation.width, animation.height), (40, 20));
        assert_eq!(animation.frames.len(), 3);
        let delays: Vec<_> = animation.frames.iter().map(|f| f.delay).collect();
        assert_eq!(
            delays,
            [30, 50, 70].map(Duration::from_millis).to_vec(),
            "GIF delays are in hundredths of a second, the fixture's are whole ones"
        );
        // Each fixture frame has its own colour.
        assert_ne!(
            animation.frames[0].pixels.get_pixel(0, 0),
            animation.frames[1].pixels.get_pixel(0, 0)
        );
    }

    #[test]
    fn a_still_gif_is_one_frame() {
        let gif = fixtures::raster(image::ImageFormat::Gif, 12, 8, 3);
        let animation = decode_gif(&gif, ROOMY).unwrap();
        assert_eq!(animation.frames.len(), 1);
        assert_eq!((animation.width, animation.height), (12, 8));
    }

    #[test]
    fn large_canvases_shrink_to_the_edge() {
        let gif = fixtures::animated_gif(400, 200, &[10, 10]);
        let limits = AnimationLimits {
            max_edge: 200,
            max_bytes: 1 << 30,
        };
        let animation = decode_gif(&gif, limits).unwrap();
        assert_eq!((animation.width, animation.height), (200, 100));
        assert!(
            animation
                .frames
                .iter()
                .all(|f| f.pixels.dimensions() == (200, 100))
        );
    }

    #[test]
    fn frames_are_halved_to_fit_the_budget() {
        let gif = fixtures::animated_gif(512, 256, &[10; 6]);
        // Six frames at 512×256 need 3 MiB; allow a quarter of that.
        let limits = AnimationLimits {
            max_edge: 4096,
            max_bytes: 6 * 256 * 128 * 4,
        };
        let animation = decode_gif(&gif, limits).unwrap();
        assert_eq!(animation.frames.len(), 6);
        assert_eq!((animation.width, animation.height), (256, 128));
        assert!(animation.bytes() <= limits.max_bytes);
        assert!(
            animation
                .frames
                .iter()
                .all(|f| f.pixels.dimensions() == (256, 128))
        );
    }

    #[test]
    fn refuses_what_cannot_fit_even_when_small() {
        let gif = fixtures::animated_gif(256, 256, &[10; 4]);
        let limits = AnimationLimits {
            max_edge: 256,
            max_bytes: 1024,
        };
        assert!(matches!(
            decode_gif(&gif, limits),
            Err(MediaError::TooLarge)
        ));
    }

    #[test]
    fn broken_bytes_are_a_decode_error() {
        assert!(matches!(
            decode_gif(b"GIF89a broken", ROOMY),
            Err(MediaError::Decode(_))
        ));
    }

    #[test]
    fn fit_keeps_the_aspect_and_never_enlarges() {
        assert_eq!(fit(1000, 500, 100), (100, 50));
        assert_eq!(fit(50, 20, 100), (50, 20));
        assert_eq!(fit(1000, 1, 100), (100, 1));
    }
}
