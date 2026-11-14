//! Video frames as pictures: the poster frame of a thumbnail and frames at a time for scrubbing.

use std::path::Path;

use ffmpeg_next::format::Pixel;
use ffmpeg_next::software::scaling::{self, Flags};
use ffmpeg_next::util::frame;
use ffmpeg_next::{Rational, codec, format};
use image::RgbaImage;

use crate::AvError;
use crate::probe::{self, MediaInfo};

/// Byte order of the pixels: RGBA for `image` and thumbnails, BGRA for GPUI textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelOrder {
    Rgba,
    Bgra,
}

/// One decoded picture, upright and scaled.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Seconds from the start of the file.
    pub time: f64,
    /// Four bytes a pixel in the reader's [`PixelOrder`] (the type is only the container).
    pub image: RgbaImage,
}

/// Where the poster frame is taken: a tenth into the video, at most this many seconds, so a
/// fade from black does not become the thumbnail.
const POSTER_MAX_SECONDS: f64 = 10.0;

/// The poster frame of a video for its thumbnail: RGBA, upright, the long edge at most
/// `max_edge`. Falls back to the first frame when the seek finds nothing.
pub fn poster(path: &Path, max_edge: u32) -> Result<(MediaInfo, RgbaImage), AvError> {
    let mut reader = FrameReader::open(path, max_edge, PixelOrder::Rgba)?;
    let info = reader.info().clone();
    let at = info
        .duration
        .map_or(0.0, |duration| (duration * 0.1).min(POSTER_MAX_SECONDS));
    let frame = match reader.frame_at(at) {
        Ok(frame) => frame,
        Err(_) => reader.frame_at(0.0)?,
    };
    Ok((info, frame.image))
}

/// Decodes the frames of one video stream: in order, or from any time on after a seek.
/// Keeps the file open, so scrubbing over a cell seeks without reopening it.
pub struct FrameReader {
    input: format::context::Input,
    stream: usize,
    time_base: f64,
    start: f64,
    decoder: codec::decoder::Video,
    scaler: Option<Scaler>,
    max_edge: u32,
    order: PixelOrder,
    rotation: u16,
    info: MediaInfo,
    flushed: bool,
}

struct Scaler {
    context: scaling::Context,
    /// The source the context was made for; a change (resolution switch) makes a new one.
    from: (Pixel, u32, u32),
    to: (u32, u32),
}

impl FrameReader {
    /// Opens the main video stream of `path`; frames come out with the long edge at most
    /// `max_edge` (never larger than the video itself).
    pub fn open(path: &Path, max_edge: u32, order: PixelOrder) -> Result<Self, AvError> {
        crate::init();
        let input = format::input(path)?;
        let info = probe::info(&input)?;
        let stream = probe::video_stream(&input).ok_or(AvError::NoVideo)?;
        let index = stream.index();
        let time_base = f64::from(stream.time_base());
        let start = stream_start(&stream);
        let rotation = probe::rotation(&stream);
        let mut context = codec::context::Context::from_parameters(stream.parameters())?;
        context.set_threading(codec::threading::Config {
            kind: codec::threading::Type::Frame,
            count: 0,
        });
        let decoder = context.decoder().video()?;
        Ok(Self {
            input,
            stream: index,
            time_base,
            start,
            decoder,
            scaler: None,
            max_edge: max_edge.max(1),
            order,
            rotation,
            info,
            flushed: false,
        })
    }

    pub fn info(&self) -> &MediaInfo {
        &self.info
    }

    /// Moves to the key frame at or before `seconds`; the next [`Self::next_frame`] starts
    /// there. Cheap and coarse: scrubbing uses it as is.
    pub fn seek_key(&mut self, seconds: f64) -> Result<(), AvError> {
        let ts =
            ((seconds.max(0.0) + self.start) * f64::from(ffmpeg_next::ffi::AV_TIME_BASE)) as i64;
        self.input.seek(ts, ..ts)?;
        self.decoder.flush();
        self.flushed = false;
        Ok(())
    }

    /// The frame showing at `seconds`: a seek to the key frame before it, then decoding up to
    /// it. The last frame when `seconds` is past the end.
    pub fn frame_at(&mut self, seconds: f64) -> Result<VideoFrame, AvError> {
        self.seek_key(seconds)?;
        self.next_from(seconds)?.ok_or(AvError::NoFrame)
    }

    /// After a key-frame seek: decodes on to the frame showing at `seconds` (the last frame
    /// when the file ends first). Frames before it are not scaled.
    pub(crate) fn next_from(&mut self, seconds: f64) -> Result<Option<VideoFrame>, AvError> {
        let mut last = None;
        while let Some(raw) = self.next_raw()? {
            let wanted = self.time_of(&raw) + self.frame_slack() >= seconds;
            last = Some(raw);
            if wanted {
                break;
            }
        }
        last.map(|raw| self.picture(&raw)).transpose()
    }

    /// Seconds one frame lasts, for stepping back a frame.
    pub fn frame_duration(&self) -> f64 {
        self.frame_slack() * 2.0
    }

    /// The next frame in decoding order after the last one or a seek; `None` at the end.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>, AvError> {
        match self.next_raw()? {
            Some(raw) => self.picture(&raw).map(Some),
            None => Ok(None),
        }
    }

    /// Half a frame: a frame that starts this close before the wanted time is the one shown.
    fn frame_slack(&self) -> f64 {
        self.info
            .video
            .as_ref()
            .and_then(|video| video.frame_rate)
            .map_or(0.02, |rate| 0.5 / rate)
    }

    fn time_of(&self, raw: &frame::Video) -> f64 {
        raw.timestamp()
            .map_or(0.0, |ts| ts as f64 * self.time_base - self.start)
    }

    fn next_raw(&mut self) -> Result<Option<frame::Video>, AvError> {
        let mut raw = frame::Video::empty();
        loop {
            if self.decoder.receive_frame(&mut raw).is_ok() {
                return Ok(Some(raw));
            }
            if self.flushed {
                return Ok(None);
            }
            let mut sent = false;
            for (stream, packet) in self.input.packets() {
                if stream.index() == self.stream {
                    // A broken packet is skipped; the next key frame recovers.
                    let _ = self.decoder.send_packet(&packet);
                    sent = true;
                    break;
                }
            }
            if !sent {
                let _ = self.decoder.send_eof();
                self.flushed = true;
            }
        }
    }

    fn picture(&mut self, raw: &frame::Video) -> Result<VideoFrame, AvError> {
        let time = self.time_of(raw);
        let from = (raw.format(), raw.width(), raw.height());
        if self
            .scaler
            .as_ref()
            .is_none_or(|scaler| scaler.from != from)
        {
            let aspect = raw.aspect_ratio();
            let to = fit(
                display_width(raw.width(), aspect),
                raw.height(),
                self.max_edge,
            );
            let pixel = match self.order {
                PixelOrder::Rgba => Pixel::RGBA,
                PixelOrder::Bgra => Pixel::BGRA,
            };
            let context =
                scaling::Context::get(from.0, from.1, from.2, pixel, to.0, to.1, Flags::BILINEAR)?;
            self.scaler = Some(Scaler { context, from, to });
        }
        let scaler = self.scaler.as_mut().ok_or(AvError::NoFrame)?;
        let mut scaled = frame::Video::empty();
        scaler.context.run(raw, &mut scaled)?;
        let image = packed(&scaled, scaler.to)?;
        Ok(VideoFrame {
            time,
            image: turn(image, self.rotation),
        })
    }
}

/// Seconds where a stream starts; most files start at 0.
pub(crate) fn stream_start(stream: &format::stream::Stream<'_>) -> f64 {
    let start = stream.start_time();
    if start == ffmpeg_next::ffi::AV_NOPTS_VALUE {
        0.0
    } else {
        start as f64 * f64::from(stream.time_base())
    }
}

fn display_width(width: u32, aspect: Rational) -> u32 {
    if aspect.numerator() > 0 && aspect.denominator() > 0 {
        ((f64::from(width) * f64::from(aspect)).round() as u32).max(1)
    } else {
        width
    }
}

/// The size that fits `max_edge` on the long side, never larger than the source, even sides
/// avoided only where scalers need them (they do not here).
pub(crate) fn fit(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let long = width.max(height).max(1);
    if long <= max_edge {
        return (width.max(1), height.max(1));
    }
    let scale = f64::from(max_edge) / f64::from(long);
    (
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    )
}

/// Copies the scaler's rows (which may be padded) into a tight buffer.
fn packed(scaled: &frame::Video, (width, height): (u32, u32)) -> Result<RgbaImage, AvError> {
    let stride = scaled.stride(0);
    let data = scaled.data(0);
    let row = width as usize * 4;
    let mut pixels = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        let start = y * stride;
        pixels.extend_from_slice(data.get(start..start + row).ok_or(AvError::NoFrame)?);
    }
    RgbaImage::from_raw(width, height, pixels).ok_or(AvError::NoFrame)
}

pub(crate) fn turn(image: RgbaImage, rotation: u16) -> RgbaImage {
    match rotation {
        90 => image::imageops::rotate90(&image),
        180 => image::imageops::rotate180(&image),
        270 => image::imageops::rotate270(&image),
        _ => image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::path;

    fn is_red(image: &RgbaImage) -> bool {
        let p = image.get_pixel(image.width() / 2, image.height() / 2);
        p[0] > 180 && p[1] < 80 && p[2] < 80
    }

    fn is_blue(image: &RgbaImage) -> bool {
        let p = image.get_pixel(image.width() / 2, image.height() / 2);
        p[2] > 180 && p[0] < 80 && p[1] < 80
    }

    #[test]
    fn poster_is_a_tenth_in() {
        let (info, image) = poster(&path("clip.mp4"), 512).unwrap();
        assert_eq!(image.dimensions(), (64, 36));
        assert!(is_red(&image));
        assert!(info.audio.is_some());
    }

    #[test]
    fn poster_scales_down_and_turns() {
        let (_, image) = poster(&path("clip.webm"), 32).unwrap();
        assert_eq!(image.dimensions(), (32, 18));
        let (_, upright) = poster(&path("rotated.mp4"), 512).unwrap();
        assert_eq!(upright.dimensions(), (36, 64));
    }

    #[test]
    fn audio_has_no_poster() {
        assert!(matches!(
            poster(&path("tone.mp3"), 512),
            Err(AvError::NoVideo)
        ));
    }

    #[test]
    fn frames_at_times() {
        for name in ["clip.mp4", "clip.webm"] {
            let mut reader = FrameReader::open(&path(name), 64, PixelOrder::Rgba).unwrap();
            let blue = reader.frame_at(1.5).unwrap();
            assert!(is_blue(&blue.image), "{name} at 1.5 s");
            assert!((blue.time - 1.5).abs() < 0.05, "{name}: {}", blue.time);
            let red = reader.frame_at(0.3).unwrap();
            assert!(is_red(&red.image), "{name} back at 0.3 s");
            // Past the end: the last frame.
            let last = reader.frame_at(60.0).unwrap();
            assert!(is_blue(&last.image), "{name} past the end");
        }
    }

    #[test]
    fn frames_follow_in_order_and_bgra_swaps_bytes() {
        let mut reader = FrameReader::open(&path("clip.mp4"), 64, PixelOrder::Bgra).unwrap();
        let mut times = Vec::new();
        while let Some(frame) = reader.next_frame().unwrap() {
            times.push(frame.time);
            if times.len() == 1 {
                // Red in BGRA: the third byte is high.
                let p = frame.image.get_pixel(10, 10);
                assert!(p[2] > 180 && p[0] < 80, "{p:?}");
            }
        }
        assert_eq!(times.len(), 50);
        assert!(times.windows(2).all(|pair| pair[1] > pair[0]));
    }

    #[test]
    fn fit_keeps_small_sizes() {
        assert_eq!(fit(64, 36, 512), (64, 36));
        assert_eq!(fit(1920, 1080, 960), (960, 540));
        assert_eq!(fit(1080, 1920, 480), (270, 480));
    }
}
