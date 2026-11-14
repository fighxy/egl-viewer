//! Video and audio on FFmpeg (D-168): what a media file holds ([`probe`]), a poster frame for
//! the thumbnail ([`poster`]), frames at a given time for scrubbing ([`FrameReader`]) and the
//! player engine ([`Player`]): a decode thread, video frames in a short queue, sound through
//! `cpal` with the lock-free ring buffer and playhead of the Scrobb player engine ([`rt`]).
//!
//! FFmpeg is the LGPL build, linked dynamically (its DLLs ship beside the executable, like
//! libheif's, D-070). Files are read by path; their bytes never leave Rust.

mod clock;
mod frames;
mod output;
mod player;
mod probe;
mod rt;

use std::sync::Once;

use thiserror::Error;

pub use frames::{FrameReader, PixelOrder, VideoFrame, poster};
pub use player::{AudioMode, Player, PlayerOptions, SeekMode};
pub use probe::{AudioInfo, MediaInfo, VideoInfo, probe};

/// Extensions the app treats as video (Eagle's `video` type rule, correction 40, minus formats
/// FFmpeg cannot read without GPL or non-free parts).
pub const VIDEO: &[&str] = &[
    "mp4", "m4v", "mov", "webm", "mkv", "avi", "wmv", "mpg", "mpeg", "mts", "m2ts", "ts", "flv",
    "f4v", "3gp", "ogv",
];

/// Extensions the app treats as audio.
pub const AUDIO: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "oga", "opus", "aac", "m4a", "wma", "aif", "aiff",
];

/// A video extension, in any case.
pub fn is_video(ext: &str) -> bool {
    let ext = ext.to_ascii_lowercase();
    VIDEO.contains(&ext.as_str())
}

/// An audio extension, in any case.
pub fn is_audio(ext: &str) -> bool {
    let ext = ext.to_ascii_lowercase();
    AUDIO.contains(&ext.as_str())
}

/// The license the linked FFmpeg was built under, as `avcodec` reports it (`LGPL version 2.1 or
/// later` for the bundled build), for About and the tests.
pub fn ffmpeg_license() -> &'static str {
    ffmpeg_next::codec::license()
}

/// The FFmpeg version string, for About.
pub fn ffmpeg_version() -> String {
    let v = ffmpeg_next::util::version();
    format!("{}.{}.{}", v >> 16, (v >> 8) & 0xff, v & 0xff)
}

#[derive(Debug, Error)]
pub enum AvError {
    #[error("cannot read the media file: {0}")]
    Ffmpeg(#[from] ffmpeg_next::Error),
    #[error("the file has no video")]
    NoVideo,
    #[error("the file has neither video nor audio")]
    NoStreams,
    #[error("no frame could be decoded")]
    NoFrame,
}

/// Registers FFmpeg once and keeps its log quiet: decoding errors come back as values.
pub(crate) fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = ffmpeg_next::init();
        ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Fatal);
    });
}

#[cfg(test)]
pub(crate) mod fixtures {
    use std::path::PathBuf;

    /// A file from `crates/av/fixtures` (see its README).
    pub fn path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The build that ships on Windows is the LGPL one (D-168): no GPL or non-free parts.
    #[cfg(windows)]
    #[test]
    fn the_windows_build_links_an_lgpl_ffmpeg() {
        let license = ffmpeg_license();
        assert!(license.starts_with("LGPL"), "{license}");
        for library in [
            ffmpeg_next::format::license(),
            ffmpeg_next::filter::license(),
            ffmpeg_next::software::scaling::license(),
        ] {
            assert!(library.starts_with("LGPL"), "{library}");
        }
    }

    #[test]
    fn extensions_are_case_blind() {
        assert!(is_video("MP4") && is_video("webm") && !is_video("mp3"));
        assert!(is_audio("FLAC") && is_audio("m4a") && !is_audio("mov"));
    }
}
