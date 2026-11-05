//! Per-change net-active execution clock shared by the TUI and `/api/v2`.
//!
//! One rule, two frontends: an interval opens on a transition into an active
//! display status, stays open across repeated or successive active statuses,
//! closes exactly once on a transition out or on process `Stopped`, and the
//! closed intervals accumulate for the life of the owner process. There is no
//! reset: a restarted process begins from zero because nothing here is ever
//! persisted.
//!
//! Time is always injected. The clock never samples a clock itself, so the same
//! event sequence fed the same instants produces the same totals in both
//! frontends, and nothing changes merely because time passed.
//!
//! The arithmetic lives in the `*_parts` functions so the TUI row, which keeps
//! its open boundary and accumulated total as its own public fields, applies the
//! identical rule rather than a copy of it.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::orchestration::operator_command::is_active_status;

/// Open an interval at `now` unless one is already open.
pub(crate) fn open_parts(open: &mut Option<Instant>, now: Instant) -> bool {
    if open.is_some() {
        return false;
    }
    *open = Some(now);
    true
}

/// Close the open interval at `now`, adding it to `accumulated`.
///
/// A no-op with no open interval, which is what makes a process `Stopped` and
/// the reducer's own `not queued` synchronization close the interval exactly
/// once between them, in either order.
pub(crate) fn close_parts(
    open: &mut Option<Instant>,
    accumulated: &mut Option<Duration>,
    now: Instant,
) -> bool {
    let Some(started) = open.take() else {
        return false;
    };
    *accumulated = Some(accumulated.unwrap_or_default() + now.saturating_duration_since(started));
    true
}

/// Closed intervals plus the open one; `None` only with no history at all.
pub(crate) fn total_parts(
    open: Option<Instant>,
    accumulated: Option<Duration>,
    now: Instant,
) -> Option<Duration> {
    match (accumulated, open) {
        (None, None) => None,
        (accumulated, open) => Some(
            accumulated.unwrap_or_default()
                + open
                    .map(|started| now.saturating_duration_since(started))
                    .unwrap_or_default(),
        ),
    }
}

/// Net-active execution time of one change for one owner process.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetActiveClock {
    accumulated: Option<Duration>,
    open: Option<Instant>,
    /// Wall instant sampled with `open`, published as `active_since`.
    open_wall: Option<DateTime<Utc>>,
}

impl NetActiveClock {
    /// Open an interval; idempotent while one is open (the first boundary wins).
    pub fn open_at(&mut self, now: Instant, wall: DateTime<Utc>) {
        if open_parts(&mut self.open, now) {
            self.open_wall = Some(wall);
        }
    }

    /// Close the open interval once; idempotent while closed.
    pub fn close_at(&mut self, now: Instant) {
        if close_parts(&mut self.open, &mut self.accumulated, now) {
            self.open_wall = None;
        }
    }

    /// Apply one observed display status through the shared active vocabulary.
    pub fn observe_status_at(&mut self, status: &str, now: Instant, wall: DateTime<Utc>) {
        if is_active_status(status) {
            self.open_at(now, wall);
        } else {
            self.close_at(now);
        }
    }

    /// Closed intervals plus the open part; `None` only with no history.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn total_at(&self, now: Instant) -> Option<Duration> {
        total_parts(self.open, self.accumulated, now)
    }

    /// Whether any interval was ever opened in this process.
    pub fn has_history(&self) -> bool {
        self.open.is_some() || self.accumulated.is_some()
    }

    /// Sum of closed intervals only.
    pub fn accumulated(&self) -> Duration {
        self.accumulated.unwrap_or_default()
    }

    /// Wall instant the open interval started, or `None` while closed.
    pub fn active_since(&self) -> Option<DateTime<Utc>> {
        self.open_wall
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(seconds: i64) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(1_800_000_000 + seconds, 0).unwrap()
    }

    #[test]
    fn remote_progress_timing_clock_accumulates_closed_intervals_and_open_boundary() {
        let t0 = Instant::now();
        let mut clock = NetActiveClock::default();
        assert!(!clock.has_history());
        assert_eq!(clock.total_at(t0), None);

        clock.open_at(t0, wall(0));
        clock.close_at(t0 + Duration::from_secs(10));
        clock.open_at(t0 + Duration::from_secs(30), wall(30));

        assert_eq!(clock.accumulated(), Duration::from_secs(10));
        assert_eq!(clock.active_since(), Some(wall(30)));
        assert_eq!(
            clock.total_at(t0 + Duration::from_secs(35)),
            Some(Duration::from_secs(15))
        );
        assert!(clock.has_history());
    }

    #[test]
    fn remote_progress_timing_clock_open_and_close_are_idempotent() {
        let t0 = Instant::now();
        let mut clock = NetActiveClock::default();
        clock.open_at(t0, wall(0));
        clock.open_at(t0 + Duration::from_secs(5), wall(5));
        assert_eq!(clock.active_since(), Some(wall(0)), "first boundary wins");

        clock.close_at(t0 + Duration::from_secs(8));
        clock.close_at(t0 + Duration::from_secs(20));
        assert_eq!(clock.accumulated(), Duration::from_secs(8));
        assert_eq!(clock.active_since(), None);
        assert_eq!(
            clock.total_at(t0 + Duration::from_secs(100)),
            Some(Duration::from_secs(8)),
            "a closed clock does not grow with time"
        );
    }

    #[test]
    fn remote_progress_timing_clock_observe_status_uses_the_shared_vocabulary() {
        let t0 = Instant::now();
        for active in crate::orchestration::operator_command::ACTIVE_STATUSES {
            let mut clock = NetActiveClock::default();
            clock.observe_status_at(active, t0, wall(0));
            assert_eq!(clock.active_since(), Some(wall(0)), "'{active}' opens");
        }
        for inactive in [
            "queued",
            "blocked",
            "not queued",
            "error",
            "merge wait",
            "archived",
            "merged",
            "pushed",
            "rejected",
        ] {
            let mut clock = NetActiveClock::default();
            clock.observe_status_at("applying", t0, wall(0));
            clock.observe_status_at(inactive, t0 + Duration::from_secs(3), wall(3));
            assert_eq!(clock.active_since(), None, "'{inactive}' closes");
            assert_eq!(clock.accumulated(), Duration::from_secs(3));
        }
    }
}
