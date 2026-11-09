//! Optional per-frame timing (`OMOLA_FRAME_STATS=1`), printed to stderr once a second.

use std::time::{Duration, Instant};

pub struct FrameStats {
    enabled: bool,
    window_start: Instant,
    frames: u32,
    total: Duration,
    max: Duration,
}

impl FrameStats {
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var_os("OMOLA_FRAME_STATS").is_some_and(|value| value == "1"),
            window_start: Instant::now(),
            frames: 0,
            total: Duration::ZERO,
            max: Duration::ZERO,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            window_start: Instant::now(),
            frames: 0,
            total: Duration::ZERO,
            max: Duration::ZERO,
        }
    }

    /// Record one frame and return a summary after the one-second window expires.
    pub fn record(&mut self, elapsed: Duration, now: Instant) -> Option<Summary> {
        if !self.enabled {
            return None;
        }

        self.frames += 1;
        self.total += elapsed;
        self.max = self.max.max(elapsed);

        if now.duration_since(self.window_start) < Duration::from_secs(1) {
            return None;
        }

        let summary = Summary {
            frames: self.frames,
            avg_ms: self.total.as_secs_f64() * 1000.0 / f64::from(self.frames),
            max_ms: self.max.as_secs_f64() * 1000.0,
        };
        self.window_start = now;
        self.frames = 0;
        self.total = Duration::ZERO;
        self.max = Duration::ZERO;
        Some(summary)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub frames: u32,
    pub avg_ms: f64,
    pub max_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled(start: Instant) -> FrameStats {
        FrameStats {
            enabled: true,
            window_start: start,
            frames: 0,
            total: Duration::ZERO,
            max: Duration::ZERO,
        }
    }

    #[test]
    fn disabled_stats_never_return_a_summary() {
        let start = Instant::now();
        let mut stats = FrameStats::disabled();

        assert_eq!(stats.record(Duration::from_millis(8), start), None);
        assert_eq!(stats.record(Duration::from_millis(12), start), None);
    }

    #[test]
    fn enabled_stats_wait_for_one_second_and_report_average_and_maximum() {
        let start = Instant::now();
        let mut stats = enabled(start);

        assert_eq!(
            stats.record(Duration::from_millis(4), start + Duration::from_millis(500)),
            None
        );
        let summary = stats
            .record(Duration::from_millis(10), start + Duration::from_secs(1))
            .unwrap();

        assert_eq!(summary.frames, 2);
        assert_eq!(summary.avg_ms, 7.0);
        assert_eq!(summary.max_ms, 10.0);
    }

    #[test]
    fn summary_resets_the_window_counters() {
        let start = Instant::now();
        let mut stats = enabled(start);

        assert!(stats
            .record(Duration::from_millis(8), start + Duration::from_secs(1))
            .is_some());
        assert_eq!(
            stats.record(
                Duration::from_millis(2),
                start + Duration::from_millis(1500)
            ),
            None
        );
        let summary = stats
            .record(Duration::from_millis(6), start + Duration::from_secs(2))
            .unwrap();

        assert_eq!(summary.frames, 2);
        assert_eq!(summary.avg_ms, 4.0);
        assert_eq!(summary.max_ms, 6.0);
    }
}
