//! The one place that decides what "finished" means.
//!
//! Two very different callers need that answer and must never give different
//! ones:
//!
//! * `cflx client wait`, an external observer holding a bounded deadline;
//! * the owner's own execution-scoped completion sinks, which push a callback.
//!
//! If the second had its own classifier, an owner could dispatch `completed`
//! for a change a concurrent `wait` was still refusing to call finished — and
//! the push is the one an agent would act on. So the execution contract, the
//! repository oracle, and the "is this claim even worth verifying" predicate all
//! live here, and both callers are thin.
//!
//! # The rule
//!
//! A typed owner state is a *claim*. Current repository evidence for the owner's
//! declared terminal mode is the *proof*. Nothing else counts: not a
//! `display_status`, not a settled command record, not the change disappearing
//! from the snapshot, and not a callback exiting zero.

pub use crate::client::repo::{verify as certify, Verdict};

/// Display statuses that claim the change reached a terminal success.
///
/// They trigger *verification*, never a success claim of their own. `rejected`
/// is deliberately absent: it is terminal without being a success, so sending it
/// to the repository oracle would only waste a Git round trip on a change that
/// already has its answer.
pub const CLAIMED_SUCCESS_STATUSES: [&str; 3] = ["archived", "merged", "pushed"];

/// Whether an observed change is claiming an outcome worth verifying.
///
/// `None` means the owner stopped tracking the change. That is the case this
/// predicate exists for: disappearance proves nothing on its own, so it must
/// lead to repository verification rather than to either conclusion. Reading it
/// as success is the exact bug the completion contract exists to prevent;
/// reading it as failure would abandon work that really did finish.
pub fn claims_terminal_success(display_status: Option<&str>) -> bool {
    match display_status {
        Some(status) => CLAIMED_SUCCESS_STATUSES.contains(&status),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_post_archive_statuses_claim_a_success_worth_verifying() {
        assert!(claims_terminal_success(Some("archived")));
        assert!(claims_terminal_success(Some("merged")));
        assert!(claims_terminal_success(Some("pushed")));
        assert!(!claims_terminal_success(Some("applying")));
        assert!(!claims_terminal_success(Some("accepted")));
        // Terminal, but not a success: it never reaches the oracle.
        assert!(!claims_terminal_success(Some("rejected")));
    }

    #[test]
    fn a_change_that_stopped_being_tracked_must_still_be_verified() {
        assert!(claims_terminal_success(None));
    }
}
