//! The link between the audio decode thread and the audio device callback, taken from the
//! Scrobb player engine (`player/core/crates/player-engine/src/rt.rs`, same author).
//!
//! Two lock-free ring buffers (rtrb): samples and chunk headers. The decode thread writes a
//! chunk's samples, then its header; the callback reads a header and exactly that many samples.
//! Everything else is atomics in [`Shared`].
//!
//! A seek never touches the buffer from the writing side: the player raises the generation and
//! the callback drops chunks of older generations itself. The callback takes no locks,
//! allocates nothing and does no I/O.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use rtrb::{Consumer, Producer, RingBuffer};

/// Sample rate and channel count of the PCM going to the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PcmFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

/// What the device callback pulls from.
pub(crate) trait PcmSource: Send {
    /// Fills all of `out` with interleaved samples, silence where there is no data. Returns how
    /// many samples at the start of `out` came from the stream.
    fn fill(&mut self, out: &mut [f32]) -> usize;
}

/// The header of a chunk of samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Chunk {
    /// Chunks of generations below `Shared::flush_before` are dropped.
    pub generation: u32,
    /// A segment is a continuous run of the file (a seek starts a new one).
    pub segment: u32,
    pub frames: u32,
    /// Ramp the volume up from zero at this chunk (after a seek).
    pub fade_in: bool,
}

/// How long a volume change ramps.
const RAMP_SECONDS: f32 = 0.005;
const MAX_CHUNKS: usize = 4096;
const FRAME_BITS: u32 = 40;
const FRAME_MASK: u64 = (1 << FRAME_BITS) - 1;
const NO_PLAYHEAD: u64 = u64::MAX;

/// State shared by the player, the decode thread and the callback.
pub(crate) struct Shared {
    pub flush_before: AtomicU32,
    pub paused: AtomicBool,
    volume_bits: AtomicU32,
    /// What plays now: the segment (24 bits) and the frame inside it (40 bits).
    playhead: AtomicU64,
    /// How often the callback ran out of data while not paused (the end included).
    pub underruns: AtomicU64,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            flush_before: AtomicU32::new(0),
            paused: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1.0f32.to_bits()),
            playhead: AtomicU64::new(NO_PLAYHEAD),
            underruns: AtomicU64::new(0),
        }
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume_bits.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// The segment and the frame inside it that play now.
    pub fn playhead(&self) -> Option<(u32, u64)> {
        let v = self.playhead.load(Ordering::Acquire);
        (v != NO_PLAYHEAD).then_some(((v >> FRAME_BITS) as u32, v & FRAME_MASK))
    }

    fn set_playhead(&self, segment: u32, frames: u64) {
        let v = (u64::from(segment & 0xFF_FFFF) << FRAME_BITS) | (frames & FRAME_MASK);
        self.playhead.store(v, Ordering::Release);
    }

    fn target_gain(&self) -> f32 {
        if self.paused.load(Ordering::Relaxed) {
            0.0
        } else {
            self.volume()
        }
    }
}

/// The segment number as the playhead stores it.
pub(crate) fn segment_id(n: u32) -> u32 {
    n & 0xFF_FFFF
}

/// A buffer pair for `format` holding `buffer_ms` of sound.
pub(crate) fn channel(
    format: PcmFormat,
    buffer_ms: u32,
    shared: Arc<Shared>,
) -> (Writer, RtSource) {
    let channels = usize::from(format.channels.max(1));
    let frames = (u64::from(format.sample_rate) * u64::from(buffer_ms.max(100)) / 1000) as usize;
    let (samples_tx, samples_rx) = RingBuffer::new(frames * channels);
    let (chunks_tx, chunks_rx) = RingBuffer::new(MAX_CHUNKS);
    let gain = shared.target_gain();
    let writer = Writer {
        samples: samples_tx,
        chunks: chunks_tx,
        channels,
    };
    let source = RtSource {
        samples: samples_rx,
        chunks: chunks_rx,
        shared,
        channels,
        chunk: None,
        remaining: 0,
        segment: None,
        played: 0,
        gain,
        step: 1.0 / (format.sample_rate as f32 * RAMP_SECONDS).max(1.0),
    };
    (writer, source)
}

/// The writing side; owned by the audio decode thread.
pub(crate) struct Writer {
    samples: Producer<f32>,
    chunks: Producer<Chunk>,
    channels: usize,
}

impl Writer {
    /// How many frames fit right now.
    pub fn free_frames(&self) -> usize {
        if self.chunks.slots() == 0 {
            0
        } else {
            self.samples.slots() / self.channels
        }
    }

    /// Writes the start of `data` as one chunk; returns how many frames went in.
    pub fn write(&mut self, data: &[f32], header: Chunk) -> usize {
        let frames = (data.len() / self.channels).min(self.free_frames());
        if frames == 0 {
            return 0;
        }
        let n = frames * self.channels;
        let Ok(chunk) = self.samples.write_chunk_uninit(n) else {
            return 0;
        };
        chunk.fill_from_iter(data[..n].iter().copied());
        let header = Chunk {
            frames: frames as u32,
            ..header
        };
        // Room was checked through `free_frames`.
        let _ = self.chunks.push(header);
        frames
    }

    /// The callback has played everything: both buffers are empty.
    pub fn is_drained(&self) -> bool {
        self.samples.slots() == self.samples.buffer().capacity()
            && self.chunks.slots() == self.chunks.buffer().capacity()
    }
}

/// The callback's side.
pub(crate) struct RtSource {
    samples: Consumer<f32>,
    chunks: Consumer<Chunk>,
    shared: Arc<Shared>,
    channels: usize,
    chunk: Option<Chunk>,
    remaining: u32,
    segment: Option<u32>,
    played: u64,
    gain: f32,
    step: f32,
}

impl RtSource {
    fn discard(&mut self, frames: u32) {
        let n = (frames as usize * self.channels).min(self.samples.slots());
        if let Ok(chunk) = self.samples.read_chunk(n) {
            chunk.commit_all();
        }
    }

    /// The next current chunk; stale ones are dropped. The generation is read again for every
    /// chunk, so a flush in the middle of a callback cuts the old sound right after this chunk.
    fn next_chunk(&mut self) -> Option<Chunk> {
        while let Ok(chunk) = self.chunks.pop() {
            if chunk.generation < self.shared.flush_before.load(Ordering::Acquire) {
                self.discard(chunk.frames);
                continue;
            }
            return Some(chunk);
        }
        None
    }
}

/// Moves the gain one frame toward the target; the last step lands exactly on it.
fn step_gain(gain: &mut f32, step: f32, target: f32) {
    let delta = target - *gain;
    if delta.abs() <= step * 1.001 {
        *gain = target;
    } else {
        *gain += step.copysign(delta);
    }
}

impl PcmSource for RtSource {
    fn fill(&mut self, out: &mut [f32]) -> usize {
        let flush_before = self.shared.flush_before.load(Ordering::Acquire);
        if self.chunk.is_some_and(|c| c.generation < flush_before) {
            self.discard(self.remaining);
            self.chunk = None;
            self.remaining = 0;
        }
        let paused = self.shared.paused.load(Ordering::Relaxed);
        let target = self.shared.target_gain();
        let ch = self.channels;
        let total_frames = out.len() / ch;
        let mut frame = 0;
        let mut consumed_any = false;

        'outer: while frame < total_frames {
            if self.remaining == 0 {
                let Some(chunk) = self.next_chunk() else {
                    if !paused {
                        self.shared.underruns.fetch_add(1, Ordering::Relaxed);
                    }
                    break;
                };
                if self.segment != Some(chunk.segment) {
                    self.segment = Some(chunk.segment);
                    self.played = 0;
                }
                if chunk.fade_in {
                    self.gain = 0.0;
                }
                self.chunk = Some(chunk);
                self.remaining = chunk.frames;
            }
            let run = (self.remaining as usize).min(total_frames - frame);
            let Ok(chunk) = self.samples.read_chunk(run * ch) else {
                break;
            };
            let (a, b) = chunk.as_slices();
            for i in 0..run {
                step_gain(&mut self.gain, self.step, target);
                if paused && self.gain == 0.0 {
                    // Paused and faded out: keep the rest of the buffer. A volume of zero
                    // (mute) still plays on, silently, so the playhead keeps moving.
                    chunk.commit(i * ch);
                    self.remaining -= i as u32;
                    self.played += i as u64;
                    frame += i;
                    consumed_any |= i > 0;
                    break 'outer;
                }
                for c in 0..ch {
                    let idx = i * ch + c;
                    let s = if idx < a.len() {
                        a[idx]
                    } else {
                        b[idx - a.len()]
                    };
                    out[(frame + i) * ch + c] = s * self.gain;
                }
            }
            chunk.commit_all();
            self.remaining -= run as u32;
            self.played += run as u64;
            frame += run;
            consumed_any = true;
        }

        out[frame * ch..].fill(0.0);
        if consumed_any && let Some(segment) = self.segment {
            self.shared.set_playhead(segment, self.played);
        }
        frame * ch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEREO: PcmFormat = PcmFormat {
        sample_rate: 1000,
        channels: 2,
    };

    fn header(generation: u32, segment: u32) -> Chunk {
        Chunk {
            generation,
            segment,
            frames: 0,
            fade_in: false,
        }
    }

    #[test]
    fn passes_samples_through_and_reports_playhead() {
        let shared = Arc::new(Shared::new());
        let (mut w, mut rt) = channel(STEREO, 1000, shared.clone());
        assert_eq!(w.write(&[0.1, 0.2, 0.3, 0.4], header(0, 7)), 2);
        let mut out = [9.0; 6];
        assert_eq!(rt.fill(&mut out), 4);
        assert_eq!(out, [0.1, 0.2, 0.3, 0.4, 0.0, 0.0]);
        assert_eq!(shared.playhead(), Some((7, 2)));
        assert!(w.is_drained());
    }

    #[test]
    fn flush_drops_stale_generations() {
        let shared = Arc::new(Shared::new());
        let (mut w, mut rt) = channel(STEREO, 1000, shared.clone());
        w.write(&[1.0; 8], header(0, 1));
        shared.flush_before.store(1, Ordering::Release);
        w.write(&[0.5, 0.5], header(1, 2));
        let mut out = [0.0; 4];
        assert_eq!(rt.fill(&mut out), 2);
        assert_eq!(out, [0.5, 0.5, 0.0, 0.0]);
    }

    #[test]
    fn pause_fades_out_then_keeps_buffer() {
        let shared = Arc::new(Shared::new());
        let (mut w, mut rt) = channel(STEREO, 1000, shared.clone());
        w.write(&[1.0; 40], header(0, 1));
        shared.paused.store(true, Ordering::Relaxed);
        let mut out = [0.0; 40];
        // At 1000 Hz the ramp lasts 5 frames: 0.8, 0.6, 0.4, 0.2, then silence.
        assert_eq!(rt.fill(&mut out), 8);
        assert!((out[0] - 0.8).abs() < 1e-6 && out[8] == 0.0);
        assert_eq!(rt.fill(&mut out), 0);
        shared.paused.store(false, Ordering::Relaxed);
        assert_eq!(rt.fill(&mut out), 32);
    }

    #[test]
    fn mute_plays_on_silently() {
        let shared = Arc::new(Shared::new());
        let (mut w, mut rt) = channel(STEREO, 1000, shared.clone());
        w.write(&[1.0; 40], header(0, 3));
        shared.set_volume(0.0);
        let mut out = [9.0; 40];
        // Unlike a pause, the buffer is consumed and the playhead moves.
        assert_eq!(rt.fill(&mut out), 40);
        assert_eq!(shared.playhead(), Some((3, 20)));
        assert!(out[10..].iter().all(|s| *s == 0.0));
    }

    #[test]
    fn volume_is_clamped() {
        let shared = Shared::new();
        shared.set_volume(3.0);
        assert_eq!(shared.volume(), 1.0);
        shared.set_volume(-1.0);
        assert_eq!(shared.volume(), 0.0);
    }
}
