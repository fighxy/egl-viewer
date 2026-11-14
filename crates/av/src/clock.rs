//! The playback clock: media seconds from wall time, speed and pause, pulled toward the audio
//! playhead when there is sound (the sound card's clock leads, video follows it).

use std::time::Instant;

/// A difference to the audio playhead past this is a jump (a seek landed): take it at once.
const JUMP: f64 = 0.15;
/// Smaller differences close by this share on every reading, so frames do not stutter.
const NUDGE: f64 = 0.05;

#[derive(Debug, Clone)]
pub struct Clock {
    /// Media seconds at `at`.
    media: f64,
    at: Instant,
    speed: f64,
    paused: bool,
}

impl Clock {
    pub fn new(now: Instant, paused: bool, speed: f64) -> Self {
        Self {
            media: 0.0,
            at: now,
            speed,
            paused,
        }
    }

    pub fn position(&self, now: Instant) -> f64 {
        if self.paused {
            self.media
        } else {
            self.media + now.saturating_duration_since(self.at).as_secs_f64() * self.speed
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn speed(&self) -> f64 {
        self.speed
    }

    fn anchor(&mut self, now: Instant) {
        self.media = self.position(now);
        self.at = now;
    }

    pub fn play(&mut self, now: Instant) {
        if self.paused {
            self.at = now;
            self.paused = false;
        }
    }

    pub fn pause(&mut self, now: Instant) {
        if !self.paused {
            self.anchor(now);
            self.paused = true;
        }
    }

    pub fn seek(&mut self, media: f64, now: Instant) {
        self.media = media.max(0.0);
        self.at = now;
    }

    pub fn set_speed(&mut self, speed: f64, now: Instant) {
        self.anchor(now);
        self.speed = speed;
    }

    /// Follows the audio playhead: jumps when far off, else closes a little of the gap.
    pub fn follow(&mut self, audio: f64, now: Instant) {
        if self.paused {
            return;
        }
        let here = self.position(now);
        let gap = audio - here;
        self.media = if gap.abs() > JUMP {
            audio
        } else {
            here + gap * NUDGE
        };
        self.at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn later(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn runs_at_speed_and_stops_when_paused() {
        let t0 = Instant::now();
        let mut clock = Clock::new(t0, false, 1.0);
        assert!((clock.position(later(t0, 500)) - 0.5).abs() < 1e-9);
        clock.set_speed(2.0, later(t0, 500));
        assert!((clock.position(later(t0, 1000)) - 1.5).abs() < 1e-9);
        clock.pause(later(t0, 1000));
        assert!((clock.position(later(t0, 5000)) - 1.5).abs() < 1e-9);
        clock.play(later(t0, 5000));
        assert!((clock.position(later(t0, 5250)) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn seeks_while_paused_stay_put() {
        let t0 = Instant::now();
        let mut clock = Clock::new(t0, true, 1.0);
        clock.seek(42.0, later(t0, 10));
        assert_eq!(clock.position(later(t0, 900)), 42.0);
        clock.seek(-3.0, later(t0, 10));
        assert_eq!(clock.position(later(t0, 900)), 0.0);
    }

    #[test]
    fn follows_the_audio_playhead() {
        let t0 = Instant::now();
        let mut clock = Clock::new(t0, false, 1.0);
        // Far off: jump.
        clock.follow(10.0, t0);
        assert_eq!(clock.position(t0), 10.0);
        // Near: a small step toward it.
        clock.follow(10.1, t0);
        let p = clock.position(t0);
        assert!(p > 10.0 && p < 10.1, "{p}");
        // Paused: the audio does not move it.
        clock.pause(t0);
        clock.follow(20.0, t0);
        assert!(clock.position(t0) < 10.1);
    }
}
