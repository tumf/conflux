//! Run-level elapsed clock shared by the TUI `Elapsed` line and `/api/v2`.
//!
//! This is the TUI's existing rule extracted, not a new policy: an opening
//! Start sets the start boundary and clears any frozen value; a freeze records
//! `now - started` whenever a start exists and overwrites any earlier frozen
//! value, which is exactly what `started.elapsed()` does today on a repeated
//! `Stopped` or a delayed `AllCompleted`. There is no first-close-wins rule and
//! no reset on freeze. *Which* events open or freeze is decided by each
//! frontend's own handlers; this type only owns the transition they apply.
//!
//! Time is injected so both frontends, fed the same instants, publish the same
//! values. Process-local presentation state: never persisted, never a
//! scheduling, resume, acceptance, or archive input.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

/// Apply an opening Start to the TUI-shaped fields.
pub(crate) fn open_parts(
    started: &mut Option<Instant>,
    frozen: &mut Option<Duration>,
    now: Instant,
) {
    *started = Some(now);
    *frozen = None;
}

/// Apply a freeze to the TUI-shaped fields; a no-op before any Start.
pub(crate) fn freeze_parts(
    started: Option<Instant>,
    frozen: &mut Option<Duration>,
    now: Instant,
) -> bool {
    let Some(started) = started else {
        return false;
    };
    *frozen = Some(now.saturating_duration_since(started));
    true
}

/// One owner process's run clock with the wall samples a client needs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunClock {
    started: Option<(Instant, DateTime<Utc>)>,
    frozen: Option<(Duration, DateTime<Utc>)>,
}

impl RunClock {
    /// Open a run: always replaces the start and clears the frozen value.
    pub fn open_at(&mut self, now: Instant, wall: DateTime<Utc>) {
        // The `open_parts` transition, carrying the wall sample with the start.
        self.started = Some((now, wall));
        self.frozen = None;
    }

    /// Freeze at `now`, overwriting any earlier freeze; a no-op with no start.
    pub fn freeze_at(&mut self, now: Instant, wall: DateTime<Utc>) {
        let mut frozen = None;
        if freeze_parts(self.started.map(|(instant, _)| instant), &mut frozen, now) {
            self.frozen = frozen.map(|elapsed| (elapsed, wall));
        }
    }

    /// Monotonic and wall start samples of the current run.
    pub fn started(&self) -> Option<(Instant, DateTime<Utc>)> {
        self.started
    }

    /// Frozen elapsed duration and the wall sample of the freeze.
    pub fn frozen(&self) -> Option<(Duration, DateTime<Utc>)> {
        self.frozen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000 + seconds, 0).unwrap()
    }

    #[test]
    fn remote_progress_timing_run_clock_freeze_overwrites_and_open_clears() {
        let t0 = Instant::now();
        let mut clock = RunClock::default();
        clock.freeze_at(t0, wall(0));
        assert_eq!(
            clock,
            RunClock::default(),
            "a freeze before any Start is a no-op"
        );

        clock.open_at(t0, wall(0));
        assert_eq!(clock.started(), Some((t0, wall(0))));
        assert_eq!(clock.frozen(), None);

        clock.freeze_at(t0 + Duration::from_secs(4), wall(4));
        assert_eq!(clock.frozen(), Some((Duration::from_secs(4), wall(4))));
        clock.freeze_at(t0 + Duration::from_secs(9), wall(9));
        assert_eq!(
            clock.frozen(),
            Some((Duration::from_secs(9), wall(9))),
            "a repeated terminal event re-freezes at the later instant"
        );

        clock.open_at(t0 + Duration::from_secs(20), wall(20));
        assert_eq!(
            clock.started(),
            Some((t0 + Duration::from_secs(20), wall(20)))
        );
        assert_eq!(clock.frozen(), None);
    }
}
