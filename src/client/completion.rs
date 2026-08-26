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
use crate::web::remote_control_api::dto::BlockerKind;

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

/// Display statuses the current owner will not advance on its own.
///
/// Each one is a row the owner has already settled or parked: a failed
/// execution, a merge the owner refuses to retry unattended, an operator stop,
/// and an execution hold with no prerequisite left to clear. None of them
/// becomes anything else without a new operator command, so an observer holding
/// out for a later event is holding out for the operator, not for the owner.
///
/// `rejected` is deliberately absent: it is equally final, but it already has
/// its own outcome and reporting it as "needs an action" would lose the verdict.
pub const MANUAL_ACTION_STATUSES: [&str; 4] = ["error", "merge wait", "stopped", "stalled"];

/// What a `wait` observer should do about one observed display status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// The owner can still advance this row by itself, so keep observing.
    ///
    /// This is the default for anything unrecognized, on purpose: an owner that
    /// grows a new intermediate status must not make existing waits give up on
    /// work that is still moving.
    KeepObserving,
    /// A success is being claimed; the repository decides whether it is one.
    Certify,
    /// The change reached its terminal rejection.
    Rejected,
    /// Nothing will move without a new operator action.
    RequiresAction,
}

/// Classify one observed display status for an observer that never mutates.
///
/// The distinction it draws is not "terminal versus not" but "can this owner
/// still advance it": a dependency wait is a hold that clears when the owner
/// archives the proposal it is waiting on, while `merge wait` is a hold that
/// clears when a human resolves a conflict. Only the second is worth releasing
/// a waiter for.
///
/// `blocked` is the one status where the display string alone cannot answer
/// that, which is why the structured blocker is a parameter rather than
/// something a caller folds in beforehand. A `dependency` hold — or a hold the
/// owner published no structured blocker for — is owner-progressing work. An
/// `external` hold is the opposite: the owner validated a non-repository
/// prerequisite, parked the change, and will not look again until an operator
/// retries it, so an observer sitting there is waiting for a human who does not
/// know they are being waited for.
///
/// `None` — the owner stopped tracking the change — stays [`Disposition::Certify`]
/// for the same reason [`claims_terminal_success`] does: disappearance proves
/// nothing either way, so the repository has to answer.
pub fn classify(display_status: Option<&str>, blocker: Option<BlockerKind>) -> Disposition {
    match display_status {
        Some("rejected") => Disposition::Rejected,
        Some(status) if MANUAL_ACTION_STATUSES.contains(&status) => Disposition::RequiresAction,
        // Gated on the status rather than on the kind alone: the projection
        // publishes a blocker only for `blocked` and `stalled`, and reading a
        // stale or unexpected one on any other row would release a waiter from
        // work that is still moving.
        Some("blocked") if blocker == Some(BlockerKind::External) => Disposition::RequiresAction,
        other if claims_terminal_success(other) => Disposition::Certify,
        _ => Disposition::KeepObserving,
    }
}

/// Whether a success-claiming status is a row the owner has already settled.
///
/// It matters only when the repository refuses to certify the claim. A settled
/// row that evidence does not back will never be re-settled, so continuing to
/// observe it is waiting for an event the owner has no reason to publish; a
/// change that merely vanished from the snapshot may still come back, and an
/// observer that gave up on it would abandon live work.
pub fn is_settled_success_claim(display_status: Option<&str>) -> bool {
    display_status.is_some_and(|status| CLAIMED_SUCCESS_STATUSES.contains(&status))
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

    /// Every status the owner's own projection can produce, classified once.
    ///
    /// Written as the full list rather than as samples because the bug this
    /// exists to prevent is an *omission*: a status nobody classified falls into
    /// the `KeepObserving` default, and a hold that needs an operator would then
    /// look exactly like work still in flight.
    #[test]
    fn every_owner_status_is_classified_by_whether_the_owner_can_still_advance_it() {
        for status in [
            "not queued",
            "queued",
            "blocked",
            "preparing",
            "applying",
            "accepting",
            "rejecting",
            "archiving",
            "resolving",
            "resolve pending",
            "reject pending",
        ] {
            assert_eq!(
                classify(Some(status), None),
                Disposition::KeepObserving,
                "'{status}' can still advance without an operator"
            );
        }
        for status in ["error", "merge wait", "stopped", "stalled"] {
            assert_eq!(
                classify(Some(status), None),
                Disposition::RequiresAction,
                "'{status}' needs a new operator action"
            );
        }
        for status in ["merged", "pushed", "archived"] {
            assert_eq!(
                classify(Some(status), None),
                Disposition::Certify,
                "{status}"
            );
        }
        assert_eq!(classify(Some("rejected"), None), Disposition::Rejected);
        // Disappearance is the one case with no status to read; the repository
        // answers it, exactly as it did before this classification existed.
        assert_eq!(classify(None, None), Disposition::Certify);
    }

    /// The one status whose disposition the display string cannot decide alone.
    ///
    /// Both halves are asserted together because they are the same rule: an
    /// external prerequisite is a hold only an operator clears, and every other
    /// blocked row is the owner's own work queue. Splitting them would let one
    /// half regress while the other kept passing.
    #[test]
    fn a_blocked_row_is_classified_by_its_structured_blocker() {
        assert_eq!(
            classify(Some("blocked"), Some(BlockerKind::External)),
            Disposition::RequiresAction,
            "an external prerequisite will not clear without an operator"
        );
        for kind in [None, Some(BlockerKind::None), Some(BlockerKind::Dependency)] {
            assert_eq!(
                classify(Some("blocked"), kind),
                Disposition::KeepObserving,
                "{kind:?} is a hold the owner clears by itself"
            );
        }
    }

    /// A blocker kind read off any other row changes nothing.
    ///
    /// The projection publishes one only for `blocked` and `stalled`, so a kind
    /// arriving with a live phase is either stale or a future projection this
    /// build does not understand. Neither is a reason to abandon moving work,
    /// and neither may downgrade a success claim away from verification.
    #[test]
    fn an_external_kind_on_another_status_does_not_release_the_waiter() {
        assert_eq!(
            classify(Some("applying"), Some(BlockerKind::External)),
            Disposition::KeepObserving
        );
        assert_eq!(
            classify(Some("merged"), Some(BlockerKind::External)),
            Disposition::Certify
        );
        // `stalled` already needs an operator; the kind neither adds to that nor
        // takes anything away.
        assert_eq!(
            classify(Some("stalled"), Some(BlockerKind::External)),
            Disposition::RequiresAction
        );
    }

    /// An unknown status is live work, not settled work.
    ///
    /// A future owner that adds an intermediate phase must not silently release
    /// every waiter observing it; the cost of guessing wrong the other way is a
    /// wait that keeps observing, which is what a waiter is for.
    #[test]
    fn an_unrecognized_status_keeps_observing() {
        assert_eq!(
            classify(Some("polishing"), None),
            Disposition::KeepObserving
        );
        assert_eq!(classify(Some(""), None), Disposition::KeepObserving);
    }

    /// The two success claims part company only when the repository says no.
    #[test]
    fn only_a_present_success_row_is_a_settled_claim() {
        assert!(is_settled_success_claim(Some("merged")));
        assert!(is_settled_success_claim(Some("pushed")));
        assert!(is_settled_success_claim(Some("archived")));
        // Still verified, never released: it may reappear in the next snapshot.
        assert!(!is_settled_success_claim(None));
        assert!(!is_settled_success_claim(Some("applying")));
    }

    /// The two status tables must stay disjoint: a status that both claimed
    /// success and demanded an operator would be classified by whichever branch
    /// happened to run first.
    #[test]
    fn no_status_both_claims_success_and_demands_an_operator() {
        for status in MANUAL_ACTION_STATUSES {
            assert!(
                !CLAIMED_SUCCESS_STATUSES.contains(&status),
                "'{status}' cannot be both"
            );
        }
    }
}
