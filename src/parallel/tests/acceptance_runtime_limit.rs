//! Parallel routing for an Acceptance invocation stopped by its absolute
//! runtime limit.
//!
//! Expiry is terminal for the run. It must bypass command-failure recovery, the
//! missing-verdict continuation, another Acceptance invocation, and Apply
//! re-entry — otherwise the per-invocation deadline multiplies through whichever
//! retry counter it lands in, which is exactly the failure this routing exists
//! to prevent.
//!
//! Every test here injects the limit and the typed termination instead of
//! waiting for one: the configured floor is 300 seconds and no test may spend
//! it.

use crate::agent::AgentRunner;
use crate::config::OrchestratorConfig;
use crate::orchestration::acceptance::{
    classify_acceptance_runtime_limit, observe_acceptance_invocation_result,
    AcceptanceCommandDiagnostic, AcceptanceCommandRetryCounter, AcceptanceRuntimeLimit,
};
use crate::orchestration::AcceptanceResult;
use crate::process_manager::{
    CommandTermination, ProcessGroupCleanupReport, ProcessGroupQuiescence,
};

const CHANGE: &str = "change-a";

/// The typed limit as the executor builds it: from the runner's own termination
/// reason and its process-group cleanup evidence, never from an exit status.
fn expired_limit(limit_secs: u64) -> AcceptanceRuntimeLimit {
    classify_acceptance_runtime_limit(
        CommandTermination::RuntimeLimit,
        limit_secs,
        &ProcessGroupCleanupReport::for_test(
            ProcessGroupQuiescence::Confirmed,
            Some(4242),
            "group empty after SIGTERM",
        ),
    )
    .expect("runtime-limit termination classifies as a runtime limit")
}

fn runtime_limit_result(limit_secs: u64) -> AcceptanceResult {
    AcceptanceResult::RuntimeLimit {
        limit: expired_limit(limit_secs),
    }
}

fn diagnostic(tag: &str) -> AcceptanceCommandDiagnostic {
    AcceptanceCommandDiagnostic {
        error: format!("Acceptance command failed with exit code: Some(1) [{tag}]"),
        exit_code: Some(1),
        stdout_tail: None,
        stderr_tail: Some(format!("stderr {tag}")),
    }
}

fn agent() -> AgentRunner {
    AgentRunner::new(OrchestratorConfig::default())
}

/// The consecutive command-failure count is what turns one 1,800-second
/// deadline into 5,400 seconds if a terminated invocation is mistaken for a
/// failed one. Expiry must not touch that budget in either direction.
#[test]
fn runtime_limit_expiry_leaves_the_command_failure_count_untouched() {
    let mut counter = AcceptanceCommandRetryCounter::default();
    let mut agent = agent();

    // The ordinary case: nothing failed before the limit expired.
    observe_acceptance_invocation_result(
        &mut counter,
        &mut agent,
        CHANGE,
        &runtime_limit_result(1_800),
    );
    assert_eq!(
        counter.consecutive_failures(),
        0,
        "expiry must not increment the consecutive command-failure count"
    );
    assert!(
        agent.acceptance_command_recovery(CHANGE).is_none(),
        "expiry must not create corrective command-recovery prompt context"
    );

    // And it is not a completed invocation either, so it does not *reset* a
    // sequence it never participated in.
    counter.record_command_failure(diagnostic("a"));
    counter.record_command_failure(diagnostic("b"));
    agent.set_acceptance_command_recovery(CHANGE, diagnostic("b"));
    assert_eq!(counter.consecutive_failures(), 2);

    observe_acceptance_invocation_result(
        &mut counter,
        &mut agent,
        CHANGE,
        &runtime_limit_result(1_800),
    );
    assert_eq!(
        counter.consecutive_failures(),
        2,
        "expiry never completed an invocation, so it must not reset the budget"
    );
    assert!(
        agent.acceptance_command_recovery(CHANGE).is_some(),
        "expiry must not clear another failure's latest-only evidence"
    );
}

/// The same observer resets for every result that genuinely completed an
/// invocation, so the assertion above is about the runtime limit rather than
/// about an observer that never resets anything.
#[test]
fn a_completed_invocation_still_ends_the_command_failure_sequence() {
    let mut counter = AcceptanceCommandRetryCounter::default();
    let mut agent = agent();

    counter.record_command_failure(diagnostic("a"));
    agent.set_acceptance_command_recovery(CHANGE, diagnostic("a"));
    assert_eq!(counter.consecutive_failures(), 1);

    observe_acceptance_invocation_result(&mut counter, &mut agent, CHANGE, &AcceptanceResult::Pass);
    assert_eq!(counter.consecutive_failures(), 0);
    assert!(agent.acceptance_command_recovery(CHANGE).is_none());

    // Cancellation is the other deliberate termination and behaves like expiry.
    counter.record_command_failure(diagnostic("c"));
    observe_acceptance_invocation_result(
        &mut counter,
        &mut agent,
        CHANGE,
        &AcceptanceResult::Cancelled,
    );
    assert_eq!(
        counter.consecutive_failures(),
        1,
        "an operator stop did not complete an invocation either"
    );
}

/// Expiry admits no continuation at all: not another Acceptance invocation, not
/// a verdict-driven route, and not the missing-verdict protocol. Each of these
/// predicates is the one dispatch reads before it schedules more work.
#[test]
fn runtime_limit_admits_no_continuation() {
    let expired = runtime_limit_result(1_800);

    assert!(expired.is_runtime_limit());
    assert!(
        !expired.permits_acceptance_retry(),
        "another Acceptance invocation would re-run exactly what the limit stopped"
    );
    assert!(
        !expired.is_canonical_verdict(),
        "a terminated invocation produced no verdict, so verdict routing must not see it"
    );
    assert!(
        !expired.is_pass(),
        "expiry is never a PASS, so it can never reach archive"
    );

    // It is its own variant, so it can reach none of the arms that re-enter
    // Apply, retry the command, or continue the missing-verdict protocol.
    assert!(!matches!(
        expired,
        AcceptanceResult::Fail { .. }
            | AcceptanceResult::Continue
            | AcceptanceResult::MissingVerdict { .. }
            | AcceptanceResult::MalformedFinding { .. }
            | AcceptanceResult::BareBlocker { .. }
            | AcceptanceResult::Stalled { .. }
            | AcceptanceResult::PermissionStalled { .. }
            | AcceptanceResult::CommandFailed { .. }
            | AcceptanceResult::Cancelled
    ));
}

/// The operator-facing error dispatch returns names the limit that expired, the
/// knob to change, and the fact that nothing is retried for them.
#[test]
fn the_terminal_error_is_actionable_and_names_the_effective_limit() {
    let summary = expired_limit(30).summary(CHANGE);

    assert!(summary.contains("30s"), "the effective limit: {summary}");
    assert!(
        summary.contains("acceptance_max_runtime_secs"),
        "the knob an operator changes: {summary}"
    );
    assert!(
        summary.contains("not retried automatically"),
        "the retry decision has to be visible: {summary}"
    );
    assert!(
        summary.contains("not a PASS"),
        "expiry must not read as a passing review: {summary}"
    );
    assert!(summary.contains(CHANGE), "the affected proposal: {summary}");
}

/// Cleanup is a separate fact from termination: an unprovable process group is
/// reported in the same terminal error rather than acknowledged as quiescent.
#[test]
fn an_unprovable_process_group_is_reported_not_acknowledged() {
    let limit = classify_acceptance_runtime_limit(
        CommandTermination::RuntimeLimit,
        1_800,
        &ProcessGroupCleanupReport::for_test(
            ProcessGroupQuiescence::Unverifiable,
            Some(4242),
            "a SIGTERM-immune descendant outlived the verification budget",
        ),
    )
    .expect("classified");

    assert!(!limit.cleanup_confirmed);
    let summary = limit.summary(CHANGE);
    assert!(
        summary.contains("NOT confirmed quiescent"),
        "an unproven group must be reported: {summary}"
    );
    assert!(
        summary.contains("SIGTERM-immune descendant"),
        "the cleanup diagnostics travel with the outcome: {summary}"
    );
}
