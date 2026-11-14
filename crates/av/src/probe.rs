//! What a media file holds: duration, the displayed size of its video, its sound.

use std::path::Path;

use ffmpeg_next::codec::packet::side_data::Type as SideDataType;
use ffmpeg_next::format::stream::Disposition;
use ffmpeg_next::media::Type as MediaType;
use ffmpeg_next::{codec, format};

use crate::AvError;

#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    /// Displayed size: the display rotation and the sample aspect ratio applied.
    pub width: u32,
    pub height: u32,
    /// Clockwise quarter turn that shows the picture upright: 0, 90, 180 or 270.
    pub rotation: u16,
    /// Average frames per second, when the container knows it.
    pub frame_rate: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioInfo {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// Seconds; `None` when neither the container nor a stream knows it.
    pub duration: Option<f64>,
    /// The main video stream; cover art in an audio file does not count.
    pub video: Option<VideoInfo>,
    pub audio: Option<AudioInfo>,
}

/// Opens `path` and reads its streams. Cheap: no frame is decoded.
pub fn probe(path: &Path) -> Result<MediaInfo, AvError> {
    crate::init();
    let input = format::input(path)?;
    info(&input)
}

pub(crate) fn info(input: &format::context::Input) -> Result<MediaInfo, AvError> {
    let video = video_stream(input).map(|stream| video_info(&stream));
    let audio = input.streams().best(MediaType::Audio).and_then(|stream| {
        let params = codec::context::Context::from_parameters(stream.parameters()).ok()?;
        let decoder = params.decoder().audio().ok()?;
        Some(AudioInfo {
            sample_rate: decoder.rate(),
            channels: decoder.channels(),
        })
    });
    if video.is_none() && audio.is_none() {
        return Err(AvError::NoStreams);
    }
    Ok(MediaInfo {
        duration: duration(input),
        video,
        audio,
    })
}

/// The best video stream that is a moving picture, not attached cover art.
pub(crate) fn video_stream(input: &format::context::Input) -> Option<format::stream::Stream<'_>> {
    let best = input.streams().best(MediaType::Video)?;
    if !best.disposition().contains(Disposition::ATTACHED_PIC) {
        return Some(best);
    }
    input.streams().find(|stream| {
        stream.parameters().medium() == MediaType::Video
            && !stream.disposition().contains(Disposition::ATTACHED_PIC)
    })
}

fn duration(input: &format::context::Input) -> Option<f64> {
    let container = input.duration();
    if container > 0 {
        return Some(container as f64 / f64::from(ffmpeg_next::ffi::AV_TIME_BASE));
    }
    input
        .streams()
        .filter(|stream| stream.duration() > 0)
        .map(|stream| stream.duration() as f64 * f64::from(stream.time_base()))
        .reduce(f64::max)
}

fn video_info(stream: &format::stream::Stream<'_>) -> VideoInfo {
    let rotation = rotation(stream);
    let (width, height) = codec::context::Context::from_parameters(stream.parameters())
        .ok()
        .and_then(|params| params.decoder().video().ok())
        .map(|decoder| {
            let aspect = decoder.aspect_ratio();
            let width = if aspect.numerator() > 0 && aspect.denominator() > 0 {
                (f64::from(decoder.width()) * f64::from(aspect)).round() as u32
            } else {
                decoder.width()
            };
            (width.max(1), decoder.height().max(1))
        })
        .unwrap_or((0, 0));
    let (width, height) = if rotation % 180 == 90 {
        (height, width)
    } else {
        (width, height)
    };
    let rate = stream.avg_frame_rate();
    let frame_rate = (rate.numerator() > 0 && rate.denominator() > 0).then(|| f64::from(rate));
    VideoInfo {
        width,
        height,
        rotation,
        frame_rate,
    }
}

/// The clockwise turn from the stream's display matrix, as `ffmpeg -autorotate` reads it.
pub(crate) fn rotation(stream: &format::stream::Stream<'_>) -> u16 {
    stream
        .side_data()
        .find(|data| data.kind() == SideDataType::DisplayMatrix)
        .and_then(|data| rotation_from_matrix(data.data()))
        .unwrap_or(0)
}

/// `av_display_rotation_get` turned into a clockwise quarter turn: a 3×3 matrix of 32-bit
/// fixed-point numbers. Odd angles round to the nearest quarter.
fn rotation_from_matrix(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 36 {
        return None;
    }
    let m = |i: usize| {
        let raw = i32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap_or_default());
        f64::from(raw) / 65536.0
    };
    let scale0 = m(0).hypot(m(3));
    let scale1 = m(1).hypot(m(4));
    if scale0 == 0.0 || scale1 == 0.0 {
        return None;
    }
    // av_display_rotation_get is counter-clockwise; the player turns the picture back.
    let counter_clockwise = -(m(1) / scale1).atan2(m(0) / scale0).to_degrees();
    let clockwise = (-counter_clockwise).rem_euclid(360.0);
    let quarter = ((clockwise / 90.0).round() as u16 % 4) * 90;
    Some(quarter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::path;

    #[test]
    fn video_with_sound() {
        let info = probe(&path("clip.mp4")).unwrap();
        let video = info.video.unwrap();
        assert_eq!((video.width, video.height, video.rotation), (64, 36, 0));
        assert!((video.frame_rate.unwrap() - 25.0).abs() < 0.01);
        assert!((info.duration.unwrap() - 2.0).abs() < 0.05);
        assert_eq!(info.audio.unwrap().sample_rate, 48_000);
        let webm = probe(&path("clip.webm")).unwrap();
        assert_eq!(webm.video.unwrap().width, 64);
        assert!(webm.audio.is_some());
    }

    #[test]
    fn rotated_video_reports_the_displayed_size() {
        let video = probe(&path("rotated.mp4")).unwrap().video.unwrap();
        assert_eq!((video.width, video.height), (36, 64));
        assert_eq!(video.rotation, 270);
    }

    #[test]
    fn audio_files_have_no_video() {
        for name in ["tone.mp3", "tone.flac", "tone.wav", "tone.ogg"] {
            let info = probe(&path(name)).unwrap();
            assert!(info.video.is_none(), "{name}");
            assert!(info.audio.is_some(), "{name}");
            assert!((info.duration.unwrap() - 1.5).abs() < 0.1, "{name}");
        }
        assert!(probe(&path("silent.mp4")).unwrap().audio.is_none());
    }

    #[test]
    fn broken_files_are_errors() {
        assert!(probe(&path("README.md")).is_err());
        assert!(probe(&path("missing.mp4")).is_err());
    }

    #[test]
    fn matrix_angles() {
        let matrix = |a: i32, b: i32, c: i32, d: i32| {
            let values = [a, b, 0, c, d, 0, 0, 0, 1 << 30];
            values
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<u8>>()
        };
        let one = 1 << 16;
        assert_eq!(rotation_from_matrix(&matrix(one, 0, 0, one)), Some(0));
        // av_display_rotation_set(90): counter-clockwise 90°, shown turned clockwise 270°.
        assert_eq!(rotation_from_matrix(&matrix(0, -one, one, 0)), Some(270));
        assert_eq!(rotation_from_matrix(&matrix(0, one, -one, 0)), Some(90));
        assert_eq!(rotation_from_matrix(&matrix(-one, 0, 0, -one)), Some(180));
        assert_eq!(rotation_from_matrix(&[0; 8]), None);
    }
}
