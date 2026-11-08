//! Optional per-frame timing (`OMOLA_FRAME_STATS=1`), printed to stderr once a second.
//!
//! The GUI issues G17-G22 (#60 "the GUI is slow") are compared with the numbers this
//! produces. Without the environment variable every method returns immediately, so the
//! normal build pays nothing but a branch per frame.

use std::time::{Duration, Instant};

/// One second of frames, as printed by [`FrameStats::record`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    pub frames: u32,
    pub avg_ms: f64,
    pub max_ms: f64,
}

/// Rolling window of frame durations.
#[derive(Debug)]
pub struct FrameStats {
    enabled: bool,
    window_start: Instant,
    frames: u32,
    total: Duration,
    max: Duration,
}

impl FrameStats {
    /// Enabled when `OMOLA_FRAME_STATS=1`; the window starts at the first frame.
    pub fn from_env() -> Self {
        let enabled = std::env::var_os("OMOLA_FRAME_STATS").is_some_and(|v| v == "1");
        Self::with_enabled(enabled)
    }

    /// Nothing is measured; every [`Self::record`] returns `None`.
    pub fn disabled() -> Self {
        Self::with_enabled(false)
    }

    fn with_enabled(enabled: bool) -> Self {
        Self {
            enabled,
            window_start: Instant::now(),
            frames: 0,
            total: Duration::ZERO,
            max: Duration::ZERO,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Record one frame. Returns a [`Summary`] and starts a new window once a second
    /// has passed since the last one.
    ///
    /// `now` is a parameter (instead of `Instant::now()` inside) so the window can be
    /// driven by a test without sleeping.
    pub fn record(&mut self, elapsed: Duration, now: Instant) -> Option<Summary> {
        if !self.enabled {
            return None;
        }
        self.frames += 1;
        self.total += elapsed;
        if elapsed > self.max {
            self.max = elapsed;
        }
        if now.saturating_duration_since(self.window_start) < Duration::from_secs(1) {
            return None;
        }
        let summary = Summary {
            frames: self.frames,
            avg_ms: self.total.as_secs_f64() * 1000.0 / f64::from(self.frames.max(1)),
            max_ms: self.max.as_secs_f64() * 1000.0,
        };
        self.window_start = now;
        self.frames = 0;
        self.total = Duration::ZERO;
        self.max = Duration::ZERO;
        Some(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_at(start: Instant) -> FrameStats {
        FrameStats {
            enabled: true,
            window_start: start,
            frames: 0,
            total: Duration::ZERO,
            max: Duration::ZERO,
        }
    }

    #[test]
    fn disabled_records_nothing() {
        let start = Instant::now();
        let mut stats = FrameStats::disabled();
        assert!(!stats.enabled());
        assert_eq!(
            stats.record(Duration::from_secs(5), start + Duration::from_secs(5)),
            None
        );
    }

    #[test]
    fn a_summary_arrives_after_one_second() {
        let start = Instant::now();
        let mut stats = enabled_at(start);
        assert_eq!(
            stats.record(Duration::from_millis(4), start + Duration::from_millis(500)),
            None,
            "half a second is not a full window"
        );
        let summary = stats
            .record(
                Duration::from_millis(10),
                start + Duration::from_millis(1000),
            )
            .expect("the window is full");
        assert_eq!(summary.frames, 2);
        assert!((summary.max_ms - 10.0).abs() < 1e-9, "{summary:?}");
        assert!((summary.avg_ms - 7.0).abs() < 1e-9, "{summary:?}");
    }

    #[test]
    fn the_counters_restart_after_a_summary() {
        let start = Instant::now();
        let mut stats = enabled_at(start);
        stats.record(Duration::from_millis(2), start + Duration::from_millis(600));
        let first = stats
            .record(Duration::from_millis(2), start + Duration::from_millis(1100))
            .expect("first window");
        assert_eq!(first.frames, 2);
        // The next window counts from the summary, not from the start.
        assert_eq!(
            stats.record(Duration::from_millis(3), start + Duration::from_millis(1500)),
            None
        );
        let second = stats
            .record(Duration::from_millis(3), start + Duration::from_millis(2200))
            .expect("second window");
        assert_eq!(second.frames, 2);
        assert!((second.max_ms - 3.0).abs() < 1e-9, "{second:?}");
    }
}
