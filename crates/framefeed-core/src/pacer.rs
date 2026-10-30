//! Frame pacing.
//!
//! The pacer owns the *presentation* schedule, never the decode rate. It answers
//! one question per frame: show it now, or wait? If the pipeline falls behind, it
//! resynchronises instead of trying to catch up, and counts what it skipped so
//! `--stats` can report honest numbers.

use std::time::{Duration, Instant};

/// What the caller should do with the frame it just produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pace {
    /// Write the frame immediately.
    Present,
    /// Sleep for this long before writing it.
    Wait(Duration),
}

/// Deadline-based presentation clock.
#[derive(Clone, Debug)]
pub struct Pacer {
    interval: Duration,
    next: Instant,
    started: bool,
    dropped: u64,
}

impl Pacer {
    /// Creates a pacer presenting `fps` frames per second.
    ///
    /// A non-positive or non-finite rate disables pacing (frames present as fast
    /// as they are produced).
    #[must_use]
    pub fn new(fps: f64, start: Instant) -> Self {
        let interval = if fps.is_finite() && fps > 0.0 {
            Duration::from_secs_f64(1.0 / fps)
        } else {
            Duration::ZERO
        };
        Self {
            interval,
            next: start,
            started: false,
            dropped: 0,
        }
    }

    /// Interval between two presentations.
    #[must_use]
    pub const fn interval(&self) -> Duration {
        self.interval
    }

    /// Number of frames skipped because the pipeline was too far behind.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Next scheduled presentation instant.
    #[must_use]
    pub const fn next_deadline(&self) -> Instant {
        self.next
    }

    /// Decides what to do with the frame available at `now`, and advances the
    /// schedule.
    ///
    /// More than two frames late triggers a resynchronisation: the schedule is
    /// rebased on `now` and the skipped frames are counted as dropped.
    pub fn tick(&mut self, now: Instant) -> Pace {
        if !self.started {
            self.started = true;
            self.next = now;
        }
        if self.interval.is_zero() {
            return Pace::Present;
        }
        if now < self.next {
            let wait = self.next - now;
            // Reserve this frame's slot *before* sleeping: the caller presents it
            // when the wait ends, so the next frame must get the following slot.
            // Without this, every other frame would present immediately and the
            // stream would run at twice the requested rate.
            self.next += self.interval;
            return Pace::Wait(wait);
        }

        let behind = now.saturating_duration_since(self.next);
        if behind >= self.interval {
            // `skipped` deadlines elapsed before this frame could be presented.
            let skipped =
                u64::try_from(behind.as_nanos() / self.interval.as_nanos()).unwrap_or(u64::MAX);
            self.dropped = self.dropped.saturating_add(skipped);
            if skipped >= 2 {
                // Too far behind to catch up: rebase instead of bursting frames.
                self.next = now + self.interval;
            } else {
                self.next += self.interval.saturating_mul(2);
            }
            return Pace::Present;
        }

        self.next += self.interval;
        Pace::Present
    }
}

#[cfg(test)]
mod tests {
    use super::{Pace, Pacer};
    use std::time::{Duration, Instant};

    fn pacer() -> (Pacer, Instant) {
        let start = Instant::now();
        (Pacer::new(100.0, start), start) // 10 ms per frame
    }

    #[test]
    fn first_frame_presents_immediately() {
        let (mut pacer, start) = pacer();
        assert_eq!(pacer.tick(start), Pace::Present);
        assert_eq!(pacer.interval(), Duration::from_millis(10));
    }
    #[test]
    fn waiting_reserves_the_slot_for_that_frame() {
        let (mut pacer, start) = pacer();
        assert_eq!(pacer.tick(start), Pace::Present);
        // Frame 2 waits for the +10 ms slot.
        assert_eq!(
            pacer.tick(start + Duration::from_millis(2)),
            Pace::Wait(Duration::from_millis(8))
        );
        // Frame 3 therefore targets +20 ms, not +10 ms.
        assert_eq!(
            pacer.tick(start + Duration::from_millis(10)),
            Pace::Wait(Duration::from_millis(10))
        );
        assert_eq!(
            pacer.tick(start + Duration::from_millis(20)),
            Pace::Wait(Duration::from_millis(10))
        );
    }

    #[test]
    fn paced_loop_yields_one_frame_per_interval() {
        let start = Instant::now();
        let mut pacer = Pacer::new(10.0, start);
        let mut clock = start;

        for index in 0..10 {
            match pacer.tick(clock) {
                Pace::Present => assert_eq!(index, 0, "only the first frame is immediate"),
                Pace::Wait(delay) => {
                    assert_eq!(delay, Duration::from_millis(100));
                    clock += delay;
                }
            }
        }

        assert_eq!(
            clock,
            start + Duration::from_millis(900),
            "ten frames span nine intervals"
        );
        assert_eq!(pacer.dropped(), 0);
    }

    #[test]
    fn schedule_does_not_drift() {
        let (mut pacer, start) = pacer();
        pacer.tick(start);
        pacer.tick(start + Duration::from_millis(10));
        pacer.tick(start + Duration::from_millis(20));
        assert_eq!(
            pacer.next_deadline(),
            start + Duration::from_millis(30),
            "deadlines accumulate from the schedule, not from wake-up times"
        );
        assert_eq!(pacer.dropped(), 0);
    }

    #[test]
    fn falling_far_behind_resyncs_and_counts_drops() {
        let (mut pacer, start) = pacer();
        pacer.tick(start); // deadline 10 ms
        let pace = pacer.tick(start + Duration::from_millis(55));
        assert_eq!(pace, Pace::Present);
        assert_eq!(pacer.dropped(), 4);
        assert_eq!(
            pacer.next_deadline(),
            start + Duration::from_millis(65),
            "the schedule rebases on the late frame"
        );
    }

    #[test]
    fn two_frames_late_resyncs_too() {
        let (mut pacer, start) = pacer();
        pacer.tick(start);
        assert_eq!(pacer.tick(start + Duration::from_millis(20)), Pace::Present);
        assert_eq!(pacer.dropped(), 1);
    }

    #[test]
    fn zero_fps_disables_pacing() {
        let (mut pacer, start) = pacer();
        let mut unpaced = Pacer::new(0.0, start);
        assert_eq!(unpaced.tick(start), Pace::Present);
        assert_eq!(unpaced.tick(start), Pace::Present);
        assert_eq!(pacer.tick(start), Pace::Present);
    }
}
