//! Explicit resume of a preserved mark whose only terminal evidence is a stop.
//!
//! The regression is the one an operator hit on a live v0.6.310 owner. Two
//! changes were force-stopped with their execution marks preserved, so the
//! coherent snapshot reported `execution_marked=true`, `queue_intent=not_queued`,
//! and `display_status=stopped`. Re-marking settled as `unchanged` because the
//! marks already held the requested value, delayed mark settlement excluded both
//! rows as terminal, and a remote Start refused them as `target_ineligible`.
//! Nothing but restarting the owner — which erases the process-local stop —
//! could satisfy the canonical Interrupted Change Handling contract.
//!
//! These tests pin the replacement transition: in process mode `Stopped`, and
//! only there, an explicit Start treats that row shape as ordinary resumable
//! work, clears the stop's own terminal and dequeue residue, keeps the mark, and
//! starts one ordinary boundary. Mark mutation still resumes nothing, and every
//! other terminal or waiting evidence keeps its existing explicit route.
//!
//! Everything runs over the parent module's in-memory doubles — a plain
//! `OrchestratorState`, the recording scheduler, and the fake queue — so no
//! process, repository, network, or timer is involved.

use super::*;

use crate::orchestration::mark_settlement::{
    classify_mark_settlement_row, MarkSettlementExclusion, MarkSettlementRow,
};
use crate::orchestration::operator_command::ParallelEligibility;

/// The force-stopped change every case is arranged around.
const ALPHA: &str = "alpha";
/// A second marked change, used to prove the resume is target-scoped.
const BETA: &str = "beta";

/// Every mode an operator can press the configured Start key in, except the one
/// that owns the resume.
const NON_RESUMING_MODES: [OperatorMode; 3] = [
    OperatorMode::Select,
    OperatorMode::Running,
    OperatorMode::Error,
];

impl Harness {
    /// Settle a change into the terminal `stopped` outcome a targeted force-stop
    /// produces, then restore the operator's preserved mark.
    ///
    /// The mark is set *after* the settlement because the production force-stop
    /// commit clears it; what the operator is left holding — and what they
    /// re-assert with `cflx client mark` — is the preserved selection this whole
    /// change is about.
    async fn to_stopped(&self, change_id: &str) {
        self.state
            .write()
            .await
            .apply_command(ReducerCommand::StopChange(change_id.to_string()));
        self.marks.set(change_id, true);
        assert_eq!(
            self.status(change_id).await,
            "stopped",
            "the arrangement must reproduce the observed row"
        );
    }

    /// The observed row shape, exactly as the owner snapshot published it.
    async fn observed_row(&self, change_id: &str) -> (bool, bool, String) {
        let guard = self.state.read().await;
        let queued = guard.queued_change_ids().contains(&change_id.to_string());
        (
            self.marks.is_marked(change_id),
            queued,
            guard.display_status(change_id).to_string(),
        )
    }
}

// ============================================================================
// The regression
// ============================================================================

/// Acceptance criteria 1 and 2, in the order they happen to an operator.
///
/// The arrangement is the observed snapshot; the assertion is that one explicit
/// Start turns it into ordinary queued work over exactly one fresh boundary,
/// with the mark preserved and with ordinary — not explicit-retry — semantics.
#[tokio::test]
async fn stopped_marked_resume_explicit_start_resumes_a_preserved_stopped_mark() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_stopped(ALPHA).await;

    assert_eq!(
        harness.observed_row(ALPHA).await,
        (true, false, "stopped".to_string()),
        "marked, not queued, stopped: the row the owner refused to start"
    );

    let outcome = harness
        .service
        .start(OperatorMode::Stopped)
        .await
        .expect("an explicit Start in Stopped resumes a preserved stopped mark");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: false,
            scheduler: SchedulerEffect::Started,
            excluded: Vec::new(),
        },
        "the resumed target is admitted as ordinary work, not as a retry"
    );
    assert_eq!(
        harness.observed_row(ALPHA).await,
        (true, true, "queued".to_string()),
        "the stop's terminal classification is cleared, the mark is preserved, \
         and ordinary queue intent is established"
    );
    assert_eq!(
        harness.scheduler.calls(),
        vec![SchedulerCall::Started {
            targets: vec![ALPHA.to_string()],
            explicit_retry: false,
        }],
        "exactly one fresh scheduler boundary starts, with ordinary semantics"
    );
    assert!(
        harness.effects().await.explicit_retries.is_empty(),
        "a stop is not a failure: no explicit-retry edge may be published for it"
    );
}

/// The resumed row is admissible to the scheduler's own ordinary queue gate.
///
/// `queued` is a display string; `is_ordinary_queue_eligible` is what a
/// scheduler pass actually consults before it will analyze or dispatch a
/// candidate. A resume that cleared the terminal state but left the stop's
/// dequeue guard behind would satisfy the first and fail the second, and the
/// change would sit in the queue forever.
#[tokio::test]
async fn stopped_marked_resume_clears_the_dequeue_guard_that_blocks_admission() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_stopped(ALPHA).await;
    assert!(
        !harness.state.read().await.is_ordinary_queue_eligible(ALPHA),
        "the control: a stopped row is not admissible before the resume"
    );

    harness
        .service
        .start(OperatorMode::Stopped)
        .await
        .expect("the preserved stopped mark resumes");

    let guard = harness.state.read().await;
    assert!(
        guard.is_ordinary_queue_eligible(ALPHA),
        "the resumed row must pass the scheduler's own ordinary admission gate"
    );
    assert!(
        guard.ordinary_queue_eligible_change_ids().contains(ALPHA),
        "and it must be in the set one coherent scheduler evaluation reads"
    );
}

// ============================================================================
// Mark mutation is not resume
// ============================================================================

/// Acceptance criterion 3: marking, re-marking, and bulk marking a stopped
/// change change nothing about it, and wake no scheduler.
#[tokio::test]
async fn stopped_marked_resume_mark_mutation_never_resumes_stopped_work() {
    let harness = Harness::new(&[ALPHA, BETA]);
    harness.to_stopped(ALPHA).await;
    let before = harness.effects().await;

    // Re-marking an already-marked row, unmarking it, marking it again, and the
    // bulk write are four different operator gestures; none of them is a
    // lifecycle control.
    harness
        .operator
        .set_execution_mark(ALPHA, true)
        .await
        .expect("re-marking a stopped row is accepted as a mark");
    harness
        .operator
        .set_execution_mark(ALPHA, false)
        .await
        .expect("unmarking a stopped row is accepted as a mark");
    harness
        .operator
        .set_execution_mark(ALPHA, true)
        .await
        .expect("marking a stopped row again is accepted as a mark");
    harness.mark(&[ALPHA, BETA]);

    assert_eq!(
        harness.effects().await,
        before,
        "no mark gesture may change terminal evidence, queue intent, or the scheduler"
    );
    assert_eq!(harness.status(ALPHA).await, "stopped");
}

/// The same criterion at the settlement boundary: an expired stability deadline
/// classifies a stopped row as terminal and plans no mutation for it.
///
/// Classification is the whole decision — a row excluded here never reaches the
/// queue command path at all — so pinning it is what keeps the timer from
/// becoming an implicit resume.
#[tokio::test]
async fn stopped_marked_resume_settlement_classifies_a_stopped_row_as_terminal() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_stopped(ALPHA).await;

    let status = harness.status(ALPHA).await;
    let classification = classify_mark_settlement_row(&MarkSettlementRow {
        change_id: ALPHA,
        display_status: &status,
        tracked: true,
        parallel_eligible: true,
        marked: true,
    });

    assert_eq!(
        classification.err(),
        Some(MarkSettlementExclusion::Terminal),
        "a settled stop is terminal to settlement; only explicit Start moves it"
    );
}

// ============================================================================
// Mode scoping
// ============================================================================

/// Acceptance criterion 4: Select, Running, and process-wide Error keep the
/// behaviour they had. A stopped mark is never resumed there — it is either
/// named as an exclusion beside admitted work or named in the refusal — and its
/// evidence survives either way.
///
/// Each mode settles differently on purpose: `Select` admits the ordinary mark
/// beside it, while `Running` and `Error` admit only retry routes and this mark
/// set carries none, so both refuse. What is compared is the property they all
/// share, read out of whichever settlement the mode produces.
#[tokio::test]
async fn stopped_marked_resume_other_modes_keep_the_stopped_evidence() {
    for mode in NON_RESUMING_MODES {
        let harness = Harness::new(&[ALPHA, BETA]);
        harness.to_stopped(ALPHA).await;
        harness.marks.set(BETA, true);
        harness.scheduler.set_running(mode == OperatorMode::Running);

        match harness.service.start(mode).await {
            Ok(RunControlOutcome::RunDispatched {
                change_ids,
                excluded,
                ..
            }) => {
                assert!(
                    !change_ids.contains(&ALPHA.to_string()),
                    "{mode:?}: a stopped row must not be resumed outside Stopped: {change_ids:?}"
                );
                assert!(
                    excluded
                        .iter()
                        .any(|target| target.change_id == ALPHA && target.status == "stopped"),
                    "{mode:?}: it must be named as excluded with its own status: {excluded:?}"
                );
            }
            Ok(other) => panic!("{mode:?}: unexpected outcome: {other:?}"),
            Err(RunControlError::NoEligibleTarget { detail, .. }) => assert!(
                detail.contains(&format!("{ALPHA} (stopped)")),
                "{mode:?}: the refusal must name the stopped mark and its status: {detail}"
            ),
            Err(other) => panic!("{mode:?}: unexpected error: {other:?}"),
        }

        assert_eq!(
            harness.status(ALPHA).await,
            "stopped",
            "{mode:?}: the stop evidence survives the request"
        );
    }
}

/// `Stopping` still refuses Start outright: a run walking to its own termination
/// must not have work admitted into it, resumable or otherwise.
#[tokio::test]
async fn stopped_marked_resume_stopping_still_refuses_start() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_stopped(ALPHA).await;
    harness.scheduler.set_running(true);
    let before = harness.effects().await;

    let error = harness
        .service
        .start(OperatorMode::Stopping)
        .await
        .expect_err("Stopping refuses Start");

    assert!(
        matches!(
            error,
            RunControlError::InvalidMode {
                command: RunCommandKind::Start,
                mode: OperatorMode::Stopping,
            }
        ),
        "unexpected error: {error:?}"
    );
    assert_eq!(harness.effects().await, before);
}

// ============================================================================
// Mixed evidence and the worktree fence
// ============================================================================

/// Acceptance criterion 5, second half: with the whole marked set worktree
/// eligible, only the ordinary stopped and `not queued` rows are admitted, and
/// every other evidence keeps its own route and its own exclusion.
#[tokio::test]
async fn stopped_marked_resume_preserves_non_ordinary_terminal_evidence() {
    let harness = Harness::new(&["stopped", "ordinary", "failed", "waiting", "held", "done"]);
    harness.to_stopped("stopped").await;
    harness.to_error("failed").await;
    harness.to_merge_wait("waiting").await;
    harness
        .apply(ExecutionEvent::ExecutionBlocked {
            change_id: "held".to_string(),
            blocker: external_blocker(),
        })
        .await;
    harness
        .apply(ExecutionEvent::MergeCompleted {
            change_id: "done".to_string(),
            revision: "abc123".to_string(),
        })
        .await;
    harness.mark(&["stopped", "ordinary", "failed", "waiting", "held", "done"]);
    // `to_stopped` set the mark on its own; the bulk write above is the
    // authoritative set, so restore it.
    assert!(harness.marks.is_marked("stopped"));

    let outcome = harness
        .service
        .start(OperatorMode::Stopped)
        .await
        .expect("the ordinary and resumable rows are startable");

    let RunControlOutcome::RunDispatched {
        change_ids,
        explicit_retry,
        excluded,
        ..
    } = &outcome
    else {
        panic!("unexpected outcome: {outcome:?}");
    };
    // Request order is the coherent mark snapshot's own order.
    assert_eq!(
        change_ids,
        &vec!["ordinary".to_string(), "stopped".to_string()],
        "only the ordinary stopped and not-queued rows are admitted"
    );
    assert!(!explicit_retry, "the launch keeps ordinary semantics");

    let mut named: Vec<(String, String)> = excluded
        .iter()
        .map(|target| (target.change_id.clone(), target.status.clone()))
        .collect();
    named.sort();
    assert_eq!(
        named,
        vec![
            ("done".to_string(), "merged".to_string()),
            ("failed".to_string(), "error".to_string()),
            ("held".to_string(), "blocked".to_string()),
            ("waiting".to_string(), "merge wait".to_string()),
        ],
        "every non-ordinary target is named with the evidence that excluded it"
    );
    for (id, status) in named {
        assert_eq!(
            harness.status(&id).await,
            status,
            "'{id}' keeps its evidence; the resume converted nothing"
        );
    }
    assert!(
        harness.effects().await.explicit_retries.is_empty(),
        "no marked row is implicitly retried by an ordinary resume"
    );
}

/// Acceptance criterion 5, first half: the complete-request worktree fence runs
/// before class selection, so one ineligible mark refuses the whole resume
/// without touching the stopped row it was mixed with.
#[tokio::test]
async fn stopped_marked_resume_worktree_ineligible_mark_refuses_before_class_selection() {
    let harness = Harness::new(&[ALPHA, BETA]);
    harness.to_stopped(ALPHA).await;
    harness.mark(&[ALPHA, BETA]);
    harness.eligibility.set_parallel_ineligible([(
        BETA.to_string(),
        ParallelEligibility::UncommittedProposalFiles,
    )]);
    let before = harness.effects().await;
    let marks_before = harness.marks.marked_ids();

    let error = harness
        .service
        .start(OperatorMode::Stopped)
        .await
        .expect_err("the fence rejects the complete request");

    let RunControlError::NoEligibleTarget { detail, .. } = &error else {
        panic!("unexpected error: {error:?}");
    };
    assert!(
        detail.contains(BETA) && detail.contains("uncommitted"),
        "the fence must name the ineligible target: {detail}"
    );
    assert_eq!(
        harness.effects().await,
        before,
        "no reducer, queue, retry-edge, or scheduler effect may survive the refusal"
    );
    assert_eq!(harness.marks.marked_ids(), marks_before);
}

/// A `Stopped` request whose marks contain nothing resumable is refused, and the
/// refusal names the resume route so the operator can tell it exists.
#[tokio::test]
async fn stopped_marked_resume_exhausted_request_names_the_resume_route() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_merge_wait(ALPHA).await;
    harness.mark(&[ALPHA]);
    let before = harness.effects().await;

    let error = harness
        .service
        .start(OperatorMode::Stopped)
        .await
        .expect_err("a merge-wait mark is neither ordinary nor resumable");

    let RunControlError::NoEligibleTarget { detail, .. } = &error else {
        panic!("unexpected error: {error:?}");
    };
    assert!(
        detail.contains("resumable") && detail.contains(ALPHA),
        "the refusal must name the route and the excluded target: {detail}"
    );
    assert_eq!(harness.effects().await, before);
}

// ============================================================================
// Fail-atomicity
// ============================================================================

/// Acceptance criterion 6: a scheduler preparation failure leaves the stopped
/// row, its mark, its queue intent, and the scheduler exactly as they were.
#[tokio::test]
async fn stopped_marked_resume_preparation_failure_is_fail_atomic() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_stopped(ALPHA).await;
    harness.scheduler.fail_launch("runtime refused the launch");
    let before = harness.effects().await;
    let marks_before = harness.marks.marked_ids();

    let error = harness
        .service
        .start(OperatorMode::Stopped)
        .await
        .expect_err("a refused launch is reported, never claimed as resumed");

    assert!(
        matches!(
            error,
            RunControlError::DispatchFailed {
                command: RunCommandKind::Start,
                ..
            }
        ),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        harness.effects().await,
        before,
        "terminal state, queue intent, and scheduler counts equal their pre-command values"
    );
    assert_eq!(harness.marks.marked_ids(), marks_before);
    assert_eq!(harness.status(ALPHA).await, "stopped");
}

/// Preparation reads state; it never writes it. A prepared-but-dropped resume is
/// indistinguishable from a resume that was never requested.
#[tokio::test]
async fn stopped_marked_resume_planning_commits_nothing_until_commit() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_stopped(ALPHA).await;
    let before = harness.effects().await;

    let prepared = harness
        .service
        .prepare_start(OperatorMode::Stopped)
        .await
        .expect("classification accepted the preserved stopped mark");
    assert_eq!(
        harness.effects().await,
        before,
        "preparation must not mutate reducer, queue, retry edges, or scheduler"
    );

    drop(prepared);
    assert_eq!(
        harness.effects().await,
        before,
        "a dropped preparation leaves nothing behind"
    );
}

// ============================================================================
// The reducer transition on its own
// ============================================================================

/// The resume command clears an operator stop and nothing else.
///
/// Every other terminal outcome is a fact this transition must refuse, or an
/// ordinary Start could erase a merged, pushed, rejected, or failed result by
/// naming it in a mark set. Each outcome is produced by the event that really
/// produces it, so the arrangement cannot claim a terminal shape the reducer
/// never builds.
#[tokio::test]
async fn stopped_marked_resume_reducer_refuses_every_non_stop_terminal_outcome() {
    let refused: [(ExecutionEvent, &str); 4] = [
        (
            ExecutionEvent::MergeCompleted {
                change_id: ALPHA.to_string(),
                revision: "abc123".to_string(),
            },
            "merged",
        ),
        (
            ExecutionEvent::PushCompleted {
                change_id: ALPHA.to_string(),
                remote: "origin".to_string(),
                branch: "cflx/alpha".to_string(),
            },
            "pushed",
        ),
        (
            ExecutionEvent::ChangeRejected {
                change_id: ALPHA.to_string(),
                reason: "acceptance rejected the change".to_string(),
            },
            "rejected",
        ),
        (
            ExecutionEvent::ProcessingError {
                id: ALPHA.to_string(),
                error: "boom".to_string(),
            },
            "error",
        ),
    ];

    for (event, status) in refused {
        let mut state = OrchestratorState::new(vec![ALPHA.to_string()], 10);
        state.apply_execution_event(&event);
        assert_eq!(state.display_status(ALPHA), status);

        let outcome = state.apply_command(ReducerCommand::ResumeStopped(ALPHA.to_string()));

        assert!(
            matches!(outcome, ReduceOutcome::NoOp),
            "{status}: resume must refuse a terminal outcome it did not produce"
        );
        assert_eq!(
            state.display_status(ALPHA),
            status,
            "{status}: the evidence survives the refused resume"
        );
    }
}

/// An untracked change and an already-ordinary row are both no-ops: the command
/// resumes a stop, and where there is no stop there is nothing to resume.
#[test]
fn stopped_marked_resume_reducer_is_a_no_op_without_a_stop_to_clear() {
    let mut state = OrchestratorState::new(vec![ALPHA.to_string()], 10);

    assert!(matches!(
        state.apply_command(ReducerCommand::ResumeStopped(ALPHA.to_string())),
        ReduceOutcome::NoOp
    ));
    assert_eq!(state.display_status(ALPHA), "not queued");

    assert!(matches!(
        state.apply_command(ReducerCommand::ResumeStopped("unknown".to_string())),
        ReduceOutcome::NoOp
    ));
}

/// Resuming twice is not two resumes: the second request finds no stop.
#[test]
fn stopped_marked_resume_reducer_resume_is_idempotent() {
    let mut state = OrchestratorState::new(vec![ALPHA.to_string()], 10);
    state.apply_command(ReducerCommand::StopChange(ALPHA.to_string()));

    assert!(matches!(
        state.apply_command(ReducerCommand::ResumeStopped(ALPHA.to_string())),
        ReduceOutcome::Changed(_)
    ));
    assert_eq!(state.display_status(ALPHA), "queued");

    assert!(
        matches!(
            state.apply_command(ReducerCommand::ResumeStopped(ALPHA.to_string())),
            ReduceOutcome::NoOp
        ),
        "a resumed row carries no stop for a second resume to clear"
    );
    assert_eq!(state.display_status(ALPHA), "queued");
}
