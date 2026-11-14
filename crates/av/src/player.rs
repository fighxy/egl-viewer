//! The player engine: one file, a video thread and an audio thread, one clock.
//!
//! - The **video thread** owns a [`FrameReader`] and keeps a short queue of upright, scaled
//!   BGRA frames ahead of the clock. The UI asks [`Player::take_frame`] for the frame due now.
//! - The **audio thread** owns its own demuxer (two readers of one local file are simpler than
//!   sharing packets), decodes, runs FFmpeg's filter graph (`atempo` for the speed, then the
//!   device's rate, channels and `f32`) and writes into the lock-free ring buffer of [`rt`].
//! - The **clock** runs on wall time and follows the audio playhead while sound plays, so the
//!   sound card's clock leads and video follows it. Without sound it is the wall clock.
//!
//! Seeks raise a generation: frames and sound of older generations are dropped, never waited
//! for. The UI thread only takes locks held for a few instructions.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use ffmpeg_next::media::Type as MediaType;
use ffmpeg_next::util::frame;
use ffmpeg_next::{ChannelLayout, Rational, codec, filter, format};

use crate::AvError;
use crate::clock::Clock;
use crate::frames::{FrameReader, PixelOrder, VideoFrame, stream_start};
use crate::output::{self, AudioOutput};
use crate::probe::{self, MediaInfo};
use crate::rt::{self, Chunk, PcmFormat, RtSource, Writer};

/// Frames decoded ahead of the clock.
const FRAME_QUEUE: usize = 6;
/// Sound decoded ahead of the device.
const AUDIO_BUFFER_MS: u32 = 400;
/// The audio thread writes when at least this many frames fit.
const MIN_WRITE_FRAMES: usize = 1024;
/// How long an idle decode thread waits for a command before looking again.
const IDLE: Duration = Duration::from_millis(5);
/// No generation has ended yet.
const NONE: u32 = u32::MAX;
/// A position this close to the end counts as the end.
const END_SLACK: f64 = 0.25;

/// Whether sound goes to the default output device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    Device,
    /// No sound: the clock runs on wall time (tests, and files played muted from the start).
    Off,
}

/// How exact a seek is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekMode {
    /// The frame at the time: decodes from the key frame before it.
    Exact,
    /// The key frame at or before the time; sound waits for the next exact seek or play.
    /// For dragging along the seek bar.
    Key,
}

#[derive(Debug, Clone)]
pub struct PlayerOptions {
    /// The long edge of video frames, at most the video's own size.
    pub max_edge: u32,
    pub audio: AudioMode,
    pub paused: bool,
    pub volume: f32,
    pub muted: bool,
    pub speed: f64,
}

impl Default for PlayerOptions {
    fn default() -> Self {
        Self {
            max_edge: 3840,
            audio: AudioMode::Device,
            paused: false,
            volume: 1.0,
            muted: false,
            speed: 1.0,
        }
    }
}

/// A run of sound from one seek: where it starts in the file and how fast it plays.
#[derive(Debug, Clone, Copy)]
struct Segment {
    id: u32,
    start: Option<f64>,
    speed: f64,
}

struct State {
    clock: Clock,
    video_generation: u32,
    /// The generation whose first frame was handed out (shown at once, due or not).
    shown_generation: Option<u32>,
    audio_generation: u32,
    audio_segment: u32,
    segments: VecDeque<Segment>,
    /// The sound no longer matches the clock (a key seek, a frame step): it restarts at the
    /// clock on the next play or exact seek.
    audio_stale: bool,
}

struct Shared {
    state: Mutex<State>,
    frames: Mutex<VecDeque<(u32, VideoFrame)>>,
    /// The generation whose video reached the end of the file.
    video_end: AtomicU32,
    /// The generation whose sound was all played.
    audio_end: AtomicU32,
}

enum VideoCommand {
    Seek {
        at: f64,
        mode: SeekMode,
        generation: u32,
    },
    Stop,
}

enum AudioCommand {
    Seek {
        at: f64,
        generation: u32,
        segment: u32,
        speed: f64,
    },
    Stop,
}

/// Sound playing on a device (or, in tests, pulled by hand).
struct AudioPart {
    commands: Sender<AudioCommand>,
    rt: Arc<rt::Shared>,
    rate: u32,
    _output: Option<AudioOutput>,
}

/// Plays one video or audio file. Dropping it stops its threads.
pub struct Player {
    info: MediaInfo,
    shared: Arc<Shared>,
    video: Option<Sender<VideoCommand>>,
    audio: Option<AudioPart>,
    frame_duration: f64,
    volume: f32,
    muted: bool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Player {
    /// Opens `path` and starts decoding. Probing reads only the headers; the threads do the rest.
    pub fn open(path: &Path, options: PlayerOptions) -> Result<Self, AvError> {
        let audio = match options.audio {
            AudioMode::Device => output::device_format(),
            AudioMode::Off => None,
        };
        let (player, source) = Self::start(path, &options, audio)?;
        let mut player = player;
        if let (Some(source), Some(format)) = (source, audio)
            && let Some(part) = player.audio.as_mut()
        {
            match output::open(format, Box::new(source)) {
                Ok(out) => part._output = Some(out),
                // No usable device after all: play on without sound.
                Err(_) => {
                    let _ = part.commands.send(AudioCommand::Stop);
                    player.audio = None;
                }
            }
        }
        Ok(player)
    }

    /// Everything but the device: the threads, and the callback side of the ring buffer.
    fn start(
        path: &Path,
        options: &PlayerOptions,
        audio_format: Option<PcmFormat>,
    ) -> Result<(Self, Option<RtSource>), AvError> {
        crate::init();
        let info = probe::probe(path)?;
        let speed = options.speed.clamp(0.25, 4.0);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                clock: Clock::new(Instant::now(), options.paused, speed),
                video_generation: 0,
                shown_generation: None,
                audio_generation: 0,
                audio_segment: 0,
                segments: VecDeque::from([Segment {
                    id: 0,
                    start: None,
                    speed,
                }]),
                audio_stale: false,
            }),
            frames: Mutex::new(VecDeque::new()),
            video_end: AtomicU32::new(NONE),
            audio_end: AtomicU32::new(NONE),
        });

        let mut frame_duration = 0.04;
        let video = if info.video.is_some() {
            // The scaler cannot move between threads: the reader is made on its own.
            let (tx, rx) = mpsc::channel();
            let (opened_tx, opened) = mpsc::channel();
            let shared = shared.clone();
            let file = path.to_path_buf();
            let max_edge = options.max_edge;
            thread::Builder::new()
                .name("av-video".into())
                .spawn(
                    move || match FrameReader::open(&file, max_edge, PixelOrder::Bgra) {
                        Ok(reader) => {
                            let _ = opened_tx.send(Ok(reader.frame_duration()));
                            video_loop(reader, &rx, &shared);
                        }
                        Err(error) => {
                            let _ = opened_tx.send(Err(error));
                        }
                    },
                )
                .map_err(|_| AvError::NoFrame)?;
            frame_duration = opened.recv().map_err(|_| AvError::NoFrame)??;
            Some(tx)
        } else {
            None
        };

        let mut source = None;
        let audio = match (info.audio.is_some(), audio_format) {
            (true, Some(format)) => match AudioDecoder::open(path, format, speed) {
                Ok(decoder) => {
                    let rt = Arc::new(rt::Shared::new());
                    rt.paused.store(options.paused, Ordering::Relaxed);
                    rt.set_volume(if options.muted { 0.0 } else { options.volume });
                    let (writer, rt_source) = rt::channel(format, AUDIO_BUFFER_MS, rt.clone());
                    source = Some(rt_source);
                    let (tx, rx) = mpsc::channel();
                    let shared = shared.clone();
                    thread::Builder::new()
                        .name("av-audio".into())
                        .spawn(move || audio_loop(decoder, writer, &rx, &shared))
                        .map_err(|_| AvError::NoFrame)?;
                    Some(AudioPart {
                        commands: tx,
                        rt,
                        rate: format.sample_rate,
                        _output: None,
                    })
                }
                Err(_) => None,
            },
            _ => None,
        };

        Ok((
            Self {
                info,
                shared,
                video,
                audio,
                frame_duration,
                volume: options.volume.clamp(0.0, 1.0),
                muted: options.muted,
            },
            source,
        ))
    }

    pub fn info(&self) -> &MediaInfo {
        &self.info
    }

    /// Whether sound is playing through a device (false for silent files and without one).
    pub fn has_sound(&self) -> bool {
        self.audio.is_some()
    }

    pub fn duration(&self) -> Option<f64> {
        self.info.duration
    }

    /// Seconds one video frame lasts (0.04 without video).
    pub fn frame_duration(&self) -> f64 {
        self.frame_duration
    }

    /// The playback position in seconds.
    pub fn position(&self) -> f64 {
        let now = Instant::now();
        let mut state = lock(&self.shared.state);
        if let Some(audio) = &self.audio
            && !state.clock.is_paused()
            && !state.audio_stale
            && self.shared.audio_end.load(Ordering::Acquire) != state.audio_generation
            && let Some((segment, frames)) = audio.rt.playhead()
            && segment == rt::segment_id(state.audio_segment)
            && let Some(current) = state
                .segments
                .iter()
                .find(|s| s.id == state.audio_segment)
                .copied()
            && let Some(start) = current.start
        {
            let heard = start + frames as f64 / f64::from(audio.rate) * current.speed;
            state.clock.follow(heard, now);
        }
        let position = state.clock.position(now);
        self.info
            .duration
            .map_or(position, |duration| position.min(duration))
    }

    pub fn is_paused(&self) -> bool {
        lock(&self.shared.state).clock.is_paused()
    }

    pub fn speed(&self) -> f64 {
        lock(&self.shared.state).clock.speed()
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Plays from the position; from the start again once the end was reached.
    pub fn play(&self) {
        if self.ended() {
            self.seek(0.0, SeekMode::Exact);
        }
        let now = Instant::now();
        let restart = {
            let mut state = lock(&self.shared.state);
            state.clock.play(now);
            std::mem::take(&mut state.audio_stale).then(|| state.clock.position(now))
        };
        if let Some(at) = restart {
            self.restart_audio(at);
        }
        if let Some(audio) = &self.audio {
            audio.rt.paused.store(false, Ordering::Relaxed);
        }
    }

    pub fn pause(&self) {
        lock(&self.shared.state).clock.pause(Instant::now());
        if let Some(audio) = &self.audio {
            audio.rt.paused.store(true, Ordering::Relaxed);
        }
    }

    pub fn toggle(&self) {
        if self.is_paused() || self.ended() {
            self.play();
        } else {
            self.pause();
        }
    }

    /// Jumps to `seconds` (clamped to the file).
    pub fn seek(&self, seconds: f64, mode: SeekMode) {
        let at = self
            .info
            .duration
            .map_or(seconds.max(0.0), |d| seconds.clamp(0.0, d));
        let generation = {
            let mut state = lock(&self.shared.state);
            state.clock.seek(at, Instant::now());
            state.video_generation = state.video_generation.wrapping_add(1) % NONE;
            state.video_generation
        };
        lock(&self.shared.frames).clear();
        if let Some(video) = &self.video {
            let _ = video.send(VideoCommand::Seek {
                at,
                mode,
                generation,
            });
        }
        match mode {
            SeekMode::Exact => {
                lock(&self.shared.state).audio_stale = false;
                self.restart_audio(at);
            }
            SeekMode::Key => self.silence_audio(),
        }
    }

    /// Steps one frame forward (the next decoded frame) and pauses.
    pub fn step_forward(&self) {
        self.pause();
        let generation = lock(&self.shared.state).video_generation;
        let next = lock(&self.shared.frames)
            .iter()
            .find(|(g, _)| *g == generation)
            .map(|(_, frame)| frame.time);
        if let Some(time) = next {
            lock(&self.shared.state).clock.seek(time, Instant::now());
            self.silence_audio();
        }
    }

    /// Steps one frame back and pauses.
    pub fn step_back(&self) {
        self.pause();
        let at = (self.position() - self.frame_duration).max(0.0);
        self.seek(at, SeekMode::Exact);
    }

    pub fn set_speed(&self, speed: f64) {
        let speed = speed.clamp(0.25, 4.0);
        let now = Instant::now();
        let at = {
            let mut state = lock(&self.shared.state);
            state.clock.set_speed(speed, now);
            state.clock.position(now)
        };
        if !lock(&self.shared.state).audio_stale {
            self.restart_audio(at);
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.apply_volume();
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.apply_volume();
    }

    fn apply_volume(&self) {
        if let Some(audio) = &self.audio {
            audio
                .rt
                .set_volume(if self.muted { 0.0 } else { self.volume });
        }
    }

    /// The frame to show now, if it changed: the first frame after a seek at once, then each
    /// frame when the clock reaches it (frames the clock already passed are skipped).
    pub fn take_frame(&self) -> Option<VideoFrame> {
        let position = self.position();
        let mut state = lock(&self.shared.state);
        let generation = state.video_generation;
        let mut frames = lock(&self.shared.frames);
        frames.retain(|(g, _)| *g == generation);
        if state.shown_generation != Some(generation) {
            let (_, first) = frames.pop_front()?;
            state.shown_generation = Some(generation);
            return Some(first);
        }
        let mut due = None;
        while frames
            .front()
            .is_some_and(|(_, frame)| frame.time <= position + 0.002)
        {
            due = frames.pop_front().map(|(_, frame)| frame);
        }
        due
    }

    /// How long until the next decoded frame is due; `None` when none is waiting.
    pub fn next_frame_in(&self) -> Option<Duration> {
        let position = self.position();
        let speed = self.speed();
        let generation = lock(&self.shared.state).video_generation;
        let frames = lock(&self.shared.frames);
        let (_, next) = frames.iter().find(|(g, _)| *g == generation)?;
        Some(Duration::from_secs_f64(
            ((next.time - position) / speed).max(0.0),
        ))
    }

    /// Everything played: the end of the file was decoded and shown or heard.
    pub fn ended(&self) -> bool {
        let position = self.position();
        let state = lock(&self.shared.state);
        let video_done = self.video.is_none()
            || (self.shared.video_end.load(Ordering::Acquire) == state.video_generation
                && !lock(&self.shared.frames)
                    .iter()
                    .any(|(g, _)| *g == state.video_generation));
        let audio_done = self.audio.is_none()
            || state.audio_stale
            || self.shared.audio_end.load(Ordering::Acquire) == state.audio_generation;
        video_done
            && audio_done
            && self
                .info
                .duration
                .is_none_or(|duration| position >= duration - END_SLACK)
    }

    /// Starts the sound again at `at`, with the clock's speed.
    fn restart_audio(&self, at: f64) {
        let Some(audio) = &self.audio else {
            return;
        };
        let (generation, segment, speed) = {
            let mut state = lock(&self.shared.state);
            state.audio_generation = state.audio_generation.wrapping_add(1) % NONE;
            state.audio_segment = rt::segment_id(state.audio_segment.wrapping_add(1));
            let speed = state.clock.speed();
            let id = state.audio_segment;
            state.segments.push_back(Segment {
                id,
                start: None,
                speed,
            });
            while state.segments.len() > 8 {
                state.segments.pop_front();
            }
            (state.audio_generation, state.audio_segment, speed)
        };
        audio.rt.flush_before.store(generation, Ordering::Release);
        let _ = audio.commands.send(AudioCommand::Seek {
            at,
            generation,
            segment,
            speed,
        });
    }

    /// Cuts the sound until the next restart (dragging along the seek bar, frame steps).
    fn silence_audio(&self) {
        let Some(audio) = &self.audio else {
            return;
        };
        let generation = {
            let mut state = lock(&self.shared.state);
            state.audio_stale = true;
            state.audio_generation = state.audio_generation.wrapping_add(1) % NONE;
            state.audio_generation
        };
        audio.rt.flush_before.store(generation, Ordering::Release);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Some(video) = &self.video {
            let _ = video.send(VideoCommand::Stop);
        }
        if let Some(audio) = &self.audio {
            let _ = audio.commands.send(AudioCommand::Stop);
        }
    }
}

/// Receives a command: at once when there is work, else after a short wait.
fn next_command<T>(commands: &Receiver<T>, wait: bool) -> Result<Option<T>, ()> {
    if wait {
        match commands.recv_timeout(IDLE) {
            Ok(command) => Ok(Some(command)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(()),
        }
    } else {
        match commands.try_recv() {
            Ok(command) => Ok(Some(command)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(()),
        }
    }
}

fn video_loop(mut reader: FrameReader, commands: &Receiver<VideoCommand>, shared: &Shared) {
    let mut generation = 0;
    let mut exact: Option<f64> = None;
    let mut ended = false;
    loop {
        let full = lock(&shared.frames).len() >= FRAME_QUEUE;
        match next_command(commands, full || ended) {
            Err(()) | Ok(Some(VideoCommand::Stop)) => return,
            Ok(Some(VideoCommand::Seek {
                at,
                mode,
                generation: g,
            })) => {
                generation = g;
                ended = reader.seek_key(at).is_err();
                exact = (mode == SeekMode::Exact).then_some(at);
                continue;
            }
            Ok(None) => {}
        }
        if full || ended {
            continue;
        }
        let next = match exact.take() {
            Some(at) => reader.next_from(at),
            None => reader.next_frame(),
        };
        match next {
            Ok(Some(frame)) => lock(&shared.frames).push_back((generation, frame)),
            // The end, or a file broken past repair: what was shown stays.
            Ok(None) | Err(_) => {
                ended = true;
                shared.video_end.store(generation, Ordering::Release);
            }
        }
    }
}

fn audio_loop(
    mut decoder: AudioDecoder,
    mut writer: Writer,
    commands: &Receiver<AudioCommand>,
    shared: &Shared,
) {
    let mut generation = 0;
    let mut segment = 0;
    let mut fade_in = false;
    let mut started = false;
    let mut pending: Vec<f32> = Vec::new();
    let mut offset = 0;
    let mut ended = false;
    let channels = usize::from(decoder.out.channels);
    loop {
        let blocked = offset < pending.len() || ended || writer.free_frames() < MIN_WRITE_FRAMES;
        match next_command(commands, blocked) {
            Err(()) | Ok(Some(AudioCommand::Stop)) => return,
            Ok(Some(AudioCommand::Seek {
                at,
                generation: g,
                segment: s,
                speed,
            })) => {
                generation = g;
                segment = s;
                fade_in = true;
                started = false;
                pending.clear();
                offset = 0;
                ended = decoder.seek(at, speed).is_err();
                continue;
            }
            Ok(None) => {}
        }
        if offset < pending.len() {
            let header = Chunk {
                generation,
                segment,
                frames: 0,
                fade_in,
            };
            let written = writer.write(&pending[offset..], header);
            if written > 0 {
                fade_in = false;
                offset += written * channels;
            }
            if offset >= pending.len() {
                pending.clear();
                offset = 0;
            }
            continue;
        }
        if ended {
            if writer.is_drained() {
                shared.audio_end.store(generation, Ordering::Release);
            }
            continue;
        }
        if writer.free_frames() < MIN_WRITE_FRAMES {
            continue;
        }
        match decoder.next_samples(&mut pending) {
            Ok(Some(start)) => {
                if !started {
                    started = true;
                    let mut state = lock(&shared.state);
                    if let Some(s) = state.segments.iter_mut().find(|s| s.id == segment) {
                        s.start = Some(start);
                    }
                }
            }
            Ok(None) | Err(_) => ended = true,
        }
    }
}

/// Decodes one audio stream and converts it for the device.
struct AudioDecoder {
    input: format::context::Input,
    stream: usize,
    time_base: Rational,
    start: f64,
    decoder: codec::decoder::Audio,
    graph: Option<filter::Graph>,
    speed: f64,
    out: PcmFormat,
    /// Frames ending before this are dropped (after an exact seek).
    skip_before: Option<f64>,
    /// When the first sample handed out since the last seek starts.
    first: Option<f64>,
    demuxed: bool,
    drained: bool,
}

impl AudioDecoder {
    fn open(path: &Path, out: PcmFormat, speed: f64) -> Result<Self, AvError> {
        let input = format::input(path)?;
        let stream = input
            .streams()
            .best(MediaType::Audio)
            .ok_or(AvError::NoStreams)?;
        let index = stream.index();
        let time_base = stream.time_base();
        let start = stream_start(&stream);
        let decoder = codec::context::Context::from_parameters(stream.parameters())?
            .decoder()
            .audio()?;
        Ok(Self {
            input,
            stream: index,
            time_base,
            start,
            decoder,
            graph: None,
            speed,
            out,
            skip_before: None,
            first: None,
            demuxed: false,
            drained: false,
        })
    }

    fn seek(&mut self, at: f64, speed: f64) -> Result<(), AvError> {
        let ts = ((at + self.start) * f64::from(ffmpeg_next::ffi::AV_TIME_BASE)) as i64;
        self.input.seek(ts, ..ts)?;
        self.decoder.flush();
        self.graph = None;
        self.speed = speed;
        self.skip_before = Some(at);
        self.first = None;
        self.demuxed = false;
        self.drained = false;
        Ok(())
    }

    /// Appends the next converted samples to `out` and returns when the first sample since the
    /// last seek starts; `None` at the end.
    fn next_samples(&mut self, out: &mut Vec<f32>) -> Result<Option<f64>, AvError> {
        let mut decoded = frame::Audio::empty();
        loop {
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                let time = decoded
                    .timestamp()
                    .map_or(0.0, |ts| ts as f64 * f64::from(self.time_base) - self.start);
                let end = time + decoded.samples() as f64 / f64::from(decoded.rate().max(1));
                if self.skip_before.is_some_and(|at| end <= at) {
                    continue;
                }
                self.skip_before = None;
                self.first.get_or_insert(time);
                if self.graph.is_none() {
                    self.graph = Some(graph(&decoded, self.time_base, self.speed, self.out)?);
                }
                if let Some(graph) = self.graph.as_mut()
                    && let Some(mut source) = graph.get("in")
                {
                    source.source().add(&decoded)?;
                }
                self.pull(out);
                if !out.is_empty() {
                    return Ok(self.first);
                }
                continue;
            }
            if self.drained {
                return Ok(None);
            }
            if self.demuxed {
                // The decoder is empty after the end: flush the filters too.
                self.drained = true;
                if let Some(graph) = self.graph.as_mut()
                    && let Some(mut source) = graph.get("in")
                {
                    let _ = source.source().flush();
                }
                self.pull(out);
                if !out.is_empty() {
                    return Ok(Some(self.first.unwrap_or(0.0)));
                }
                return Ok(None);
            }
            let mut sent = false;
            for (stream, packet) in self.input.packets() {
                if stream.index() == self.stream {
                    let _ = self.decoder.send_packet(&packet);
                    sent = true;
                    break;
                }
            }
            if !sent {
                let _ = self.decoder.send_eof();
                self.demuxed = true;
            }
        }
    }

    /// Moves every frame waiting at the end of the filter graph into `out`.
    fn pull(&mut self, out: &mut Vec<f32>) {
        let Some(graph) = self.graph.as_mut() else {
            return;
        };
        let Some(mut sink) = graph.get("out") else {
            return;
        };
        let channels = usize::from(self.out.channels);
        let mut converted = frame::Audio::empty();
        while sink.sink().frame(&mut converted).is_ok() {
            let bytes = converted.samples() * channels * 4;
            if let Some(data) = converted.data(0).get(..bytes) {
                out.extend(
                    data.as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| f32::from_ne_bytes(*b)),
                );
            }
        }
    }
}

/// `atempo` takes 0.5–100 per instance; slower speeds chain two.
fn tempo(speed: f64) -> String {
    if (speed - 1.0).abs() < 1e-6 {
        String::new()
    } else if speed < 0.5 {
        let half = speed.sqrt();
        format!("atempo={half:.6},atempo={half:.6},")
    } else {
        format!("atempo={speed:.6},")
    }
}

fn layout_name(channels: u16) -> String {
    match channels {
        1 => "mono".into(),
        2 => "stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n => format!("{n}c"),
    }
}

/// `abuffer` (the decoder's format) → tempo → the device's rate, layout and packed `f32`.
fn graph(
    decoded: &frame::Audio,
    time_base: Rational,
    speed: f64,
    out: PcmFormat,
) -> Result<filter::Graph, AvError> {
    let mut graph = filter::Graph::new();
    let mut mask = decoded.channel_layout().bits();
    if mask == 0 {
        mask = ChannelLayout::default(i32::from(decoded.channels().max(1))).bits();
    }
    let args = format!(
        "time_base={}/{}:sample_rate={}:sample_fmt={}:channel_layout=0x{mask:x}",
        time_base.numerator(),
        time_base.denominator(),
        decoded.rate(),
        decoded.format().name(),
    );
    let abuffer = filter::find("abuffer").ok_or(ffmpeg_next::Error::FilterNotFound)?;
    let sink = filter::find("abuffersink").ok_or(ffmpeg_next::Error::FilterNotFound)?;
    graph.add(&abuffer, "in", &args)?;
    graph.add(&sink, "out", "")?;
    let spec = format!(
        "{}aresample={},aformat=sample_fmts=flt:channel_layouts={}",
        tempo(speed),
        out.sample_rate,
        layout_name(out.channels)
    );
    graph.output("in", 0)?.input("out", 0)?.parse(&spec)?;
    graph.validate()?;
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::path;
    use crate::rt::PcmSource;

    fn silent(paused: bool) -> PlayerOptions {
        PlayerOptions {
            audio: AudioMode::Off,
            paused,
            ..PlayerOptions::default()
        }
    }

    fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(value) = probe() {
                return value;
            }
            assert!(Instant::now() < deadline, "timed out");
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn is_blue_bgra(frame: &VideoFrame) -> bool {
        let p = frame.image.get_pixel(10, 10);
        p[0] > 180 && p[2] < 80
    }

    #[test]
    fn paused_player_shows_the_first_frame() {
        let player = Player::open(&path("clip.mp4"), silent(true)).unwrap();
        let first = wait_for(|| player.take_frame());
        assert!(first.time < 0.05);
        assert!(!is_blue_bgra(&first));
        assert_eq!(player.position(), 0.0);
        assert!(!player.has_sound());
        // Paused: no further frame is due.
        thread::sleep(Duration::from_millis(50));
        assert!(player.take_frame().is_none());
    }

    #[test]
    fn exact_seek_shows_the_frame_at_once_even_paused() {
        let player = Player::open(&path("clip.webm"), silent(true)).unwrap();
        wait_for(|| player.take_frame());
        player.seek(1.5, SeekMode::Exact);
        let frame = wait_for(|| player.take_frame());
        assert!(is_blue_bgra(&frame));
        assert!((frame.time - 1.5).abs() < 0.05, "{}", frame.time);
        assert!((player.position() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn playing_runs_to_the_end() {
        let options = PlayerOptions {
            speed: 4.0,
            ..silent(false)
        };
        let player = Player::open(&path("clip.mp4"), options).unwrap();
        let mut shown = 0;
        wait_for(|| {
            if player.take_frame().is_some() {
                shown += 1;
            }
            player.ended().then_some(())
        });
        assert!(shown > 3, "{shown} frames");
        assert!(player.position() > 1.7, "{}", player.position());
        // Play again starts over.
        player.play();
        assert!(player.position() < 0.5);
        assert!(!player.ended());
    }

    #[test]
    fn steps_frame_by_frame() {
        let player = Player::open(&path("clip.mp4"), silent(true)).unwrap();
        wait_for(|| player.take_frame());
        wait_for(|| player.next_frame_in());
        player.step_forward();
        let next = wait_for(|| player.take_frame());
        assert!((next.time - 0.04).abs() < 0.005, "{}", next.time);
        player.seek(1.0, SeekMode::Exact);
        wait_for(|| player.take_frame());
        player.step_back();
        let back = wait_for(|| player.take_frame());
        assert!((back.time - 0.96).abs() < 0.005, "{}", back.time);
        assert!(player.is_paused());
    }

    #[test]
    fn key_seeks_land_on_a_key_frame() {
        let player = Player::open(&path("clip.mp4"), silent(true)).unwrap();
        player.seek(1.7, SeekMode::Key);
        let frame = wait_for(|| player.take_frame());
        // Key frames every 25 frames: the one at 1.0 s.
        assert!(frame.time <= 1.7 && frame.time >= 0.99, "{}", frame.time);
    }

    #[test]
    fn audio_only_files_keep_time_without_a_device() {
        let player = Player::open(&path("tone.flac"), silent(true)).unwrap();
        assert!(player.take_frame().is_none());
        player.seek(1.0, SeekMode::Exact);
        assert!((player.position() - 1.0).abs() < 1e-9);
        player.seek(99.0, SeekMode::Exact);
        assert!((player.position() - 1.5).abs() < 0.1);
        assert!(player.ended());
    }

    /// The device side pulled by hand: decoded sound arrives, the clock follows it.
    fn pulled(name: &str, speed: f64) -> (Player, RtSource) {
        let format = PcmFormat {
            sample_rate: 48_000,
            channels: 2,
        };
        let options = PlayerOptions {
            speed,
            ..PlayerOptions::default()
        };
        let (player, source) = Player::start(&path(name), &options, Some(format)).unwrap();
        (player, source.expect("a ring buffer"))
    }

    fn pull(source: &mut RtSource, seconds: f64) -> (usize, f32) {
        let mut out = vec![0.0; (48_000.0 * seconds) as usize * 2];
        let mut taken = 0;
        let deadline = Instant::now() + Duration::from_secs(10);
        while taken == 0 && Instant::now() < deadline {
            taken = source.fill(&mut out);
            if taken == 0 {
                thread::sleep(Duration::from_millis(2));
            }
        }
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        (taken, peak)
    }

    #[test]
    fn sound_is_decoded_converted_and_drives_the_clock() {
        for name in ["clip.mp4", "clip.webm", "tone.mp3", "tone.ogg", "tone.wav"] {
            let (player, mut source) = pulled(name, 1.0);
            assert!(player.has_sound());
            let (taken, peak) = pull(&mut source, 0.2);
            assert!(taken > 0, "{name}: no sound");
            assert!(peak > 0.05, "{name}: silent ({peak})");
            let heard = taken as f64 / 2.0 / 48_000.0;
            let position = player.position();
            assert!(
                position >= 0.0 && position <= heard + 0.2,
                "{name}: {position}"
            );
        }
    }

    #[test]
    fn audio_seeks_flush_and_restart_at_the_target() {
        let (player, mut source) = pulled("tone.flac", 1.0);
        pull(&mut source, 0.05);
        player.seek(1.0, SeekMode::Exact);
        let (taken, _) = pull(&mut source, 0.1);
        assert!(taken > 0);
        let position = player.position();
        assert!((position - 1.0).abs() < 0.25, "{position}");
        // Muted: after the 5 ms ramp the samples are silence.
        let mut player = player;
        player.set_muted(true);
        pull(&mut source, 0.01);
        let (_, peak) = pull(&mut source, 0.1);
        assert!(peak < 1e-6, "{peak}");
    }

    #[test]
    fn tempo_filters() {
        assert_eq!(tempo(1.0), "");
        assert_eq!(tempo(2.0), "atempo=2.000000,");
        assert_eq!(tempo(0.25), "atempo=0.500000,atempo=0.500000,");
        assert_eq!(layout_name(2), "stereo");
        assert_eq!(layout_name(3), "3c");
    }

    #[test]
    fn faster_speed_plays_less_sound() {
        let (player, mut source) = pulled("tone.wav", 2.0);
        assert_eq!(player.speed(), 2.0);
        // 1.5 s at 2× is about 0.75 s of samples.
        let mut total = 0;
        let mut out = vec![0.0; 4800];
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let taken = source.fill(&mut out);
            total += taken;
            if taken == 0 && player.shared.audio_end.load(Ordering::Acquire) == 0 {
                break;
            }
            if taken == 0 {
                thread::sleep(Duration::from_millis(1));
            }
        }
        let seconds = total as f64 / 2.0 / 48_000.0;
        assert!((seconds - 0.75).abs() < 0.1, "{seconds}");
    }
}
