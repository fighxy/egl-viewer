//! Playback of an animated GIF in the full-size preview: play / pause, frame steps, seeking
//! and speed (D-154). Pure state; the preview paints [`GifPlayer::frame`] and a ticker calls
//! [`GifPlayer::advance_to`] when [`GifPlayer::until_next`] has passed.

use std::time::{Duration, Instant};

/// Playback speeds, slowest first; [`DEFAULT_SPEED`] indexes 1×.
pub const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 1.5, 2.0];
const DEFAULT_SPEED: usize = 2;

/// Delays this short are shown at [`SHORT_DELAY_FALLBACK`], as browsers and Eagle do: many
/// GIFs carry 0 or 10 ms by mistake and would otherwise race.
const SHORTEST_DELAY: Duration = Duration::from_millis(10);
const SHORT_DELAY_FALLBACK: Duration = Duration::from_millis(100);

/// The scrubber has at most this many hit targets; longer GIFs share one between frames.
pub const MAX_SEGMENTS: usize = 240;

/// How long a frame with this file delay stays up at 1×.
pub fn shown_delay(delay: Duration) -> Duration {
    if delay <= SHORTEST_DELAY {
        SHORT_DELAY_FALLBACK
    } else {
        delay
    }
}

/// How many scrubber segments a GIF of `frames` frames gets.
pub fn segment_count(frames: usize) -> usize {
    frames.min(MAX_SEGMENTS)
}

/// The frame a click or drag on `segment` (of [`segment_count`]) seeks to: the first frame
/// that segment stands for, so the track shows the seek as reached.
pub fn frame_for_segment(segment: usize, frames: usize) -> usize {
    let segments = segment_count(frames).max(1);
    (segment.min(segments - 1) * frames)
        .div_ceil(segments)
        .min(frames.saturating_sub(1))
}

/// The segment that shows `frame` as reached.
pub fn segment_for_frame(frame: usize, frames: usize) -> usize {
    let segments = segment_count(frames).max(1);
    (frame * segments / frames.max(1)).min(segments - 1)
}

#[derive(Debug, Clone)]
pub struct GifPlayer {
    /// Shown delays at 1×, one per frame.
    delays: Vec<Duration>,
    frame: usize,
    playing: bool,
    speed: usize,
    /// Time spent on the current frame, at playback speed.
    into_frame: Duration,
    /// The last tick while playing; `None` when paused, so a pause never counts.
    last_tick: Option<Instant>,
}

impl GifPlayer {
    /// A player for frames with these file delays, at the first frame. `autoplay` is off when
    /// the system asks for reduced motion.
    pub fn new(delays: impl IntoIterator<Item = Duration>, autoplay: bool, now: Instant) -> Self {
        let delays: Vec<Duration> = delays.into_iter().map(shown_delay).collect();
        let playing = autoplay && delays.len() > 1;
        Self {
            delays,
            frame: 0,
            playing,
            speed: DEFAULT_SPEED,
            into_frame: Duration::ZERO,
            last_tick: playing.then_some(now),
        }
    }

    pub fn frame(&self) -> usize {
        self.frame
    }

    pub fn frame_count(&self) -> usize {
        self.delays.len()
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn speed(&self) -> f32 {
        SPEEDS[self.speed]
    }

    /// The whole loop at 1×.
    pub fn duration(&self) -> Duration {
        self.delays.iter().sum()
    }

    pub fn play(&mut self, now: Instant) {
        if self.frame_count() > 1 && !self.playing {
            self.playing = true;
            self.last_tick = Some(now);
        }
    }

    pub fn pause(&mut self) {
        self.playing = false;
        self.last_tick = None;
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.playing {
            self.pause();
        } else {
            self.play(now);
        }
    }

    /// One frame back or forward, wrapping around; pauses, as frame steps do in players.
    pub fn step(&mut self, delta: isize) {
        self.pause();
        let count = self.frame_count() as isize;
        if count == 0 {
            return;
        }
        self.frame = (self.frame as isize + delta).rem_euclid(count) as usize;
        self.into_frame = Duration::ZERO;
    }

    /// Jumps to `frame` (clamped); a playing GIF keeps playing from there.
    pub fn seek(&mut self, frame: usize, now: Instant) {
        self.frame = frame.min(self.frame_count().saturating_sub(1));
        self.into_frame = Duration::ZERO;
        if self.playing {
            self.last_tick = Some(now);
        }
    }

    /// The next speed, wrapping from the fastest back to the slowest.
    pub fn cycle_speed(&mut self, now: Instant) {
        self.advance_to(now);
        self.speed = (self.speed + 1) % SPEEDS.len();
    }

    /// Moves playback on to `now`; `true` when another frame is now shown.
    pub fn advance_to(&mut self, now: Instant) -> bool {
        let Some(last) = self.last_tick else {
            return false;
        };
        if !self.playing || self.frame_count() < 2 {
            return false;
        }
        self.last_tick = Some(now);
        let mut elapsed =
            self.into_frame + now.saturating_duration_since(last).mul_f32(self.speed());
        // After a long stall (a sleeping PC) skip whole loops instead of walking them.
        let total = self.duration();
        if !total.is_zero() && elapsed > total {
            elapsed = Duration::from_nanos((elapsed.as_nanos() % total.as_nanos()) as u64);
        }
        let mut moved = false;
        while elapsed >= self.delays[self.frame] {
            elapsed -= self.delays[self.frame];
            self.frame = (self.frame + 1) % self.frame_count();
            moved = true;
        }
        self.into_frame = elapsed;
        moved
    }

    /// Wall-clock time until the next frame is due; `None` while paused.
    pub fn until_next(&self) -> Option<Duration> {
        if !self.playing || self.frame_count() < 2 {
            return None;
        }
        let left = self.delays[self.frame].saturating_sub(self.into_frame);
        Some(left.div_f32(self.speed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn player(delays: &[u64]) -> (GifPlayer, Instant) {
        let start = Instant::now();
        (
            GifPlayer::new(delays.iter().copied().map(ms), true, start),
            start,
        )
    }

    #[test]
    fn plays_through_the_frames_and_loops() {
        let (mut gif, t0) = player(&[100, 200, 300]);
        assert!(gif.is_playing());
        assert!(!gif.advance_to(t0 + ms(99)));
        assert_eq!(gif.frame(), 0);
        assert!(gif.advance_to(t0 + ms(100)));
        assert_eq!(gif.frame(), 1);
        assert_eq!(gif.until_next(), Some(ms(200)));
        assert!(gif.advance_to(t0 + ms(300)));
        assert_eq!(gif.frame(), 2);
        assert!(gif.advance_to(t0 + ms(600)));
        assert_eq!(gif.frame(), 0, "loops back to the first frame");
    }

    #[test]
    fn short_delays_play_at_a_tenth_of_a_second() {
        assert_eq!(shown_delay(Duration::ZERO), ms(100));
        assert_eq!(shown_delay(ms(10)), ms(100));
        assert_eq!(shown_delay(ms(20)), ms(20));
        let (gif, _) = player(&[0, 0]);
        assert_eq!(gif.duration(), ms(200));
    }

    #[test]
    fn a_pause_does_not_count() {
        let (mut gif, t0) = player(&[100, 100]);
        gif.advance_to(t0 + ms(60));
        gif.pause();
        assert_eq!(gif.until_next(), None);
        assert!(!gif.advance_to(t0 + ms(5000)));
        gif.play(t0 + ms(5000));
        assert!(!gif.advance_to(t0 + ms(5030)));
        assert_eq!(gif.frame(), 0);
        assert!(
            gif.advance_to(t0 + ms(5040)),
            "the 60 ms before the pause still count"
        );
        assert_eq!(gif.frame(), 1);
    }

    #[test]
    fn steps_pause_and_wrap() {
        let (mut gif, _) = player(&[100, 100, 100]);
        gif.step(-1);
        assert!(!gif.is_playing());
        assert_eq!(gif.frame(), 2);
        gif.step(1);
        assert_eq!(gif.frame(), 0);
        gif.step(1);
        assert_eq!(gif.frame(), 1);
    }

    #[test]
    fn seeking_keeps_playing_from_the_new_frame() {
        let (mut gif, t0) = player(&[100, 100, 100, 100]);
        gif.advance_to(t0 + ms(50));
        gif.seek(2, t0 + ms(50));
        assert_eq!(gif.frame(), 2);
        assert!(gif.is_playing());
        assert!(!gif.advance_to(t0 + ms(140)));
        assert!(gif.advance_to(t0 + ms(150)));
        assert_eq!(gif.frame(), 3);
        gif.seek(99, t0 + ms(150));
        assert_eq!(gif.frame(), 3, "clamped to the last frame");
    }

    #[test]
    fn speed_scales_time_and_cycles() {
        let (mut gif, t0) = player(&[100, 100]);
        assert_eq!(gif.speed(), 1.0);
        gif.cycle_speed(t0);
        gif.cycle_speed(t0);
        assert_eq!(gif.speed(), 2.0);
        assert_eq!(gif.until_next(), Some(ms(50)));
        assert!(gif.advance_to(t0 + ms(50)));
        assert_eq!(gif.frame(), 1);
        gif.cycle_speed(t0 + ms(50));
        assert_eq!(gif.speed(), 0.25, "wraps to the slowest");
        assert_eq!(gif.until_next(), Some(ms(400)));
    }

    #[test]
    fn a_long_stall_skips_whole_loops() {
        let (mut gif, t0) = player(&[100, 100, 100]);
        // 100 loops and 150 ms: lands half way through the second frame.
        assert!(gif.advance_to(t0 + ms(30_150)));
        assert_eq!(gif.frame(), 1);
        assert_eq!(gif.until_next(), Some(ms(50)));
    }

    #[test]
    fn one_frame_or_reduced_motion_never_plays() {
        let start = Instant::now();
        let mut still = GifPlayer::new([ms(100)], true, start);
        assert!(!still.is_playing());
        still.toggle(start);
        assert!(!still.is_playing());
        let mut calm = GifPlayer::new([ms(100), ms(100)], false, start);
        assert!(!calm.is_playing());
        assert!(!calm.advance_to(start + ms(500)));
        calm.toggle(start + ms(500));
        assert!(calm.is_playing());
    }

    #[test]
    fn segments_map_to_frames_both_ways() {
        assert_eq!(segment_count(12), 12);
        assert_eq!(segment_count(1000), MAX_SEGMENTS);
        for frame in 0..12 {
            assert_eq!(frame_for_segment(frame, 12), frame);
            assert_eq!(segment_for_frame(frame, 12), frame);
        }
        assert_eq!(frame_for_segment(0, 1000), 0);
        assert_eq!(frame_for_segment(MAX_SEGMENTS - 1, 1000), 996);
        assert_eq!(frame_for_segment(10_000, 1000), 996);
        assert_eq!(segment_for_frame(999, 1000), MAX_SEGMENTS - 1);
        // A seek lands on a frame its own segment shows as reached.
        for segment in 0..MAX_SEGMENTS {
            let frame = frame_for_segment(segment, 1000);
            assert_eq!(segment_for_frame(frame, 1000), segment);
        }
    }
}
