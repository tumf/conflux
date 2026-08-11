//! Mode-independent Start/F5 retry routing.
//!
//! `ProcessingError` is change-scoped: a failed change leaves the process in
//! `Running` or in persistent-idle `Select`, never in process-wide `Error`. The
//! Start boundary used to pick its route from that process mode alone, so the
//! configured recovery control was unreachable for the ordinary change-local
//! failure lifecycle. These tests pin the replacement: the class comes from
//! marked target evidence, with mode kept as a lifecycle guard.
//!
//! Everything runs over the parent module's in-memory doubles — a plain
//! `OrchestratorState`, the recording scheduler, and the fake queue — so no
//! process, repository, network, or timer is involved and scheduler liveness is
//! driven deterministically rather than by a real task.

use super::*;

/// The two changes most cases are arranged over: one retry-eligible, one
/// ordinary.
const ALPHA: &str = "alpha";
const BETA: &str = "beta";

/// Every mode in which an operator can press the configured Start key.
const NON_STOPPING_MODES: [OperatorMode; 4] = [
    OperatorMode::Select,
    OperatorMode::Running,
    OperatorMode::Stopped,
    OperatorMode::Error,
];

impl Harness {
    /// The exclusion recorded for `change_id`, if the outcome named one.
    fn exclusion(outcome: &RunControlOutcome, change_id: &str) -> ExcludedTarget {
        let RunControlOutcome::RunDispatched { excluded, .. } = outcome else {
            panic!("an accepted Start reports its exclusions: {outcome:?}");
        };
        excluded
            .iter()
            .find(|target| target.change_id == change_id)
            .unwrap_or_else(|| panic!("'{change_id}' must be named as excluded: {excluded:?}"))
            .clone()
    }
}

// ============================================================================
// Class selection per mode
// ============================================================================

/// Acceptance criterion 2: a live run keeps its lifecycle, and the marked
/// change-local error is still retryable through the shared Start transaction.
#[tokio::test]
async fn change_error_f5_retry_running_start_routes_only_marked_retry_targets() {
    let harness = Harness::new(&[ALPHA, BETA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA, BETA]);
    harness.scheduler.set_running(true);

    let outcome = harness
        .service
        .start(OperatorMode::Running)
        .await
        .expect("a marked retry-eligible target is startable under a live run");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Notified,
            excluded: vec![ExcludedTarget::new(BETA, "not queued")
                .with_detail(ORDINARY_DEFERRED_TO_MARK_SETTLEMENT)],
        },
        "only the retry route is admitted, and the ordinary mark is named as deferred"
    );
    assert_eq!(
        harness.effects().await.explicit_retries,
        vec![ALPHA.to_string()],
        "the retried target gets exactly one target-specific edge"
    );
    assert_eq!(
        harness.status(BETA).await,
        "not queued",
        "the ordinary mark keeps its status; Start did not queue it"
    );
    assert!(
        harness.scheduler.started_targets().is_empty(),
        "a live scheduler is woken, never joined by a second boundary"
    );
}

/// Acceptance criterion 1: persistent-idle `Select` is a live scheduler, so an
/// accepted retry wakes it instead of spawning a second boundary.
#[tokio::test]
async fn change_error_f5_retry_persistent_idle_select_retries_a_marked_error() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA]);
    harness.scheduler.set_running(true);

    let outcome = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect("a drained persistent scheduler still admits an explicit retry");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Notified,
            excluded: Vec::new(),
        }
    );
    assert_eq!(
        harness.status(ALPHA).await,
        "queued",
        "the terminal error was cleared through retry intent"
    );
    assert_eq!(
        harness.effects().await.explicit_retries,
        vec![ALPHA.to_string()]
    );
}

/// Acceptance criterion 3: with no boundary alive, the retry starts one, and it
/// starts with explicit-retry semantics.
#[tokio::test]
async fn change_error_f5_retry_stopped_starts_a_fresh_explicit_retry_boundary() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA]);

    let outcome = harness
        .service
        .start(OperatorMode::Stopped)
        .await
        .expect("a stopped run resumes through the marked retry route");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Started,
            excluded: Vec::new(),
        }
    );
    assert_eq!(
        harness.scheduler.calls(),
        vec![SchedulerCall::Started {
            targets: vec![ALPHA.to_string()],
            explicit_retry: true,
        }],
        "exactly one boundary starts, and it carries explicit-retry semantics"
    );
}

/// Process-wide `Error` keeps the behaviour it already had.
#[tokio::test]
async fn change_error_f5_retry_error_mode_retains_existing_retry_routing() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA]);

    let outcome = harness
        .service
        .start(OperatorMode::Error)
        .await
        .expect("Error mode routes marked retry-eligible rows as it always did");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Started,
            excluded: Vec::new(),
        }
    );
}

/// A resumable acceptance hold keeps its own route: the reducer restores queue
/// intent and no explicit-retry edge is published, because no failed
/// classification has to be released for it.
#[tokio::test]
async fn change_error_f5_retry_resumable_hold_keeps_its_existing_route() {
    let harness = Harness::new(&[ALPHA]);
    harness
        .apply(ExecutionEvent::ExecutionBlocked {
            change_id: ALPHA.to_string(),
            blocker: external_blocker(),
        })
        .await;
    harness.mark(&[ALPHA]);
    harness.scheduler.set_running(true);

    let outcome = harness
        .service
        .start(OperatorMode::Running)
        .await
        .expect("a resumable external hold is retry-eligible evidence");

    assert!(
        matches!(
            outcome,
            RunControlOutcome::RunDispatched {
                explicit_retry: true,
                scheduler: SchedulerEffect::Notified,
                ..
            }
        ),
        "unexpected outcome: {outcome:?}"
    );
    assert!(
        harness.effects().await.explicit_retries.is_empty(),
        "an acceptance hold releases no failed classification, so it publishes no edge"
    );
}

// ============================================================================
// Ordinary priority and the retry fallback
// ============================================================================

/// Acceptance criterion 6: ordinary Start wins, and the retry-only row is told
/// what would make it selectable rather than being retried implicitly.
#[tokio::test]
async fn change_error_f5_retry_ordinary_start_keeps_priority_over_retry_only_marks() {
    for mode in [OperatorMode::Select, OperatorMode::Stopped] {
        let harness = Harness::new(&[ALPHA, BETA]);
        harness.to_error(ALPHA).await;
        harness.mark(&[ALPHA, BETA]);

        let outcome = harness
            .service
            .start(mode)
            .await
            .expect("an ordinary startable mark is admitted");

        assert_eq!(
            outcome,
            RunControlOutcome::RunDispatched {
                change_ids: vec![BETA.to_string()],
                explicit_retry: false,
                scheduler: SchedulerEffect::Started,
                excluded: vec![ExcludedTarget::new(ALPHA, "error")
                    .with_detail(RETRY_DEFERRED_TO_ORDINARY_START)],
            },
            "{mode:?} must admit ordinary work and defer the retry-only row"
        );
        let effects = harness.effects().await;
        assert!(
            effects.explicit_retries.is_empty(),
            "{mode:?}: a deferred row must not be retried implicitly"
        );
        assert_eq!(
            harness.status(ALPHA).await,
            "error",
            "{mode:?}: the deferred row keeps its terminal error evidence"
        );
        assert!(
            Harness::exclusion(&outcome, ALPHA)
                .describe()
                .contains("remove the ordinary marks"),
            "{mode:?}: the diagnostic must be actionable"
        );
    }
}

/// The fallback half of the same rule: with no ordinary target startable, the
/// same mark set routes through explicit retry.
#[tokio::test]
async fn change_error_f5_retry_select_falls_back_to_retry_when_no_ordinary_target_is_startable() {
    let harness = Harness::new(&[ALPHA, BETA]);
    harness.to_error(ALPHA).await;
    harness.to_merge_wait(BETA).await;
    harness.mark(&[ALPHA, BETA]);

    let outcome = harness
        .service
        .start(OperatorMode::Select)
        .await
        .expect("the retry fallback runs once no ordinary target is startable");

    assert_eq!(
        outcome,
        RunControlOutcome::RunDispatched {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler: SchedulerEffect::Started,
            excluded: vec![ExcludedTarget::new(BETA, "merge wait")],
        }
    );
}

// ============================================================================
// Refusals: mutation-free by construction
// ============================================================================

/// Everything a refusal must leave untouched, compared as one value.
async fn assert_refusal_is_mutation_free(
    harness: &Harness,
    mode: OperatorMode,
    label: &str,
) -> RunControlError {
    let before = harness.effects().await;
    let marks_before = harness.marks.marked_ids();

    let error = harness
        .service
        .start(mode)
        .await
        .expect_err(&format!("{label}: Start must be refused"));

    assert_eq!(
        harness.effects().await,
        before,
        "{label}: a refusal leaves no reducer, queue, retry-edge, or scheduler effect"
    );
    assert_eq!(
        harness.marks.marked_ids(),
        marks_before,
        "{label}: a refusal leaves marks alone"
    );
    error
}

/// Acceptance criterion 5: `Stopping` refuses Start in every arrangement.
#[tokio::test]
async fn change_error_f5_retry_stopping_refuses_start_without_mutation() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA]);
    harness.scheduler.set_running(true);

    let error = assert_refusal_is_mutation_free(&harness, OperatorMode::Stopping, "stopping").await;

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
}

/// A live run with nothing retryable marked refuses, and says which marks it
/// left to the settlement path instead of claiming they are unrunnable.
#[tokio::test]
async fn change_error_f5_retry_running_without_retry_evidence_refuses_with_target_detail() {
    let harness = Harness::new(&[BETA]);
    harness.mark(&[BETA]);
    harness.scheduler.set_running(true);

    let error = assert_refusal_is_mutation_free(&harness, OperatorMode::Running, "running").await;

    let RunControlError::NoEligibleTarget { detail, .. } = error else {
        panic!("a live run with no retryable mark is a target refusal: {error:?}");
    };
    assert!(detail.contains(BETA), "the excluded target must be named");
    assert!(
        detail.contains("mark settlement"),
        "the operator must be told what does own the ordinary mark: {detail}"
    );
}

/// Acceptance criterion 5: the complete-request worktree fence runs before class
/// selection, so a worktree-ineligible retry mark refuses the whole request in
/// every mode.
#[tokio::test]
async fn change_error_f5_retry_worktree_ineligible_retry_mark_refuses_before_class_selection() {
    for mode in NON_STOPPING_MODES {
        let harness = Harness::new(&[ALPHA, BETA]);
        harness.to_error(ALPHA).await;
        harness.mark(&[ALPHA, BETA]);
        harness.scheduler.set_running(mode == OperatorMode::Running);
        harness.eligibility.set_parallel_ineligible([(
            ALPHA.to_string(),
            ParallelEligibility::UncommittedProposalFiles,
        )]);

        let error =
            assert_refusal_is_mutation_free(&harness, mode, &format!("{mode:?} worktree fence"))
                .await;

        let RunControlError::NoEligibleTarget { detail, .. } = error else {
            panic!("{mode:?}: the fence rejects the complete request: {error:?}");
        };
        assert!(
            detail.contains(ALPHA) && detail.contains("uncommitted"),
            "{mode:?}: the fence must name the ineligible target: {detail}"
        );
    }
}

/// Acceptance criterion 5: active-run Apply iteration-limit evidence is
/// mutation-free in every mode a Start can arrive in.
#[tokio::test]
async fn change_error_f5_retry_active_iteration_limit_refuses_in_every_mode() {
    for mode in NON_STOPPING_MODES {
        let harness = Harness::new(&[ALPHA]);
        harness.to_iteration_limit(ALPHA, 50, 50).await;
        harness.mark(&[ALPHA]);
        // The live boundary is what owns the gate; every mode is evaluated
        // against the same owning task.
        harness.scheduler.set_running(true);

        let error =
            assert_refusal_is_mutation_free(&harness, mode, &format!("{mode:?} active limit"))
                .await;

        let RunControlError::NoEligibleTarget { detail, .. } = error else {
            panic!("{mode:?}: an all-limited request is refused at admission: {error:?}");
        };
        assert!(
            detail.contains(ALPHA),
            "{mode:?}: the refused target must be named: {detail}"
        );
        assert_eq!(
            harness.status(ALPHA).await,
            "error",
            "{mode:?}: the typed evidence survives the refusal"
        );
    }
}

/// A non-resumable acceptance stall keeps its blocker evidence: Start refuses
/// rather than consuming a hold nothing can resume.
#[tokio::test]
async fn change_error_f5_retry_non_resumable_hold_is_refused_with_evidence_intact() {
    let harness = Harness::new(&[ALPHA]);
    harness
        .apply(ExecutionEvent::AcceptanceGated {
            change_id: ALPHA.to_string(),
            blocker: StalledBlocker {
                resumable: false,
                ..StalledBlocker::acceptance_external("human_decision", "owner must decide")
            },
        })
        .await;
    // Same display status as the resumable case above; the *only* difference is
    // resumability, which is what makes this the contrast that matters.
    assert_eq!(harness.status(ALPHA).await, "blocked");
    harness.mark(&[ALPHA]);
    harness.scheduler.set_running(true);

    let error =
        assert_refusal_is_mutation_free(&harness, OperatorMode::Running, "non-resumable").await;

    assert!(
        matches!(error, RunControlError::NoEligibleTarget { .. }),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        harness.status(ALPHA).await,
        "blocked",
        "the hold keeps its blocker evidence rather than being consumed"
    );
}

// ============================================================================
// Planning is read-only
// ============================================================================

/// Preparation reads state; it never writes it. This is what makes a failed
/// preparation the *absence* of an effect rather than a rollback to be trusted.
#[tokio::test]
async fn change_error_f5_retry_planning_commits_nothing_until_the_command_is_committed() {
    for mode in NON_STOPPING_MODES {
        let harness = Harness::new(&[ALPHA, BETA]);
        harness.to_error(ALPHA).await;
        harness.mark(&[ALPHA]);
        harness.scheduler.set_running(mode == OperatorMode::Running);
        let before = harness.effects().await;

        let prepared = harness
            .service
            .prepare_start(mode)
            .await
            .unwrap_or_else(|error| panic!("{mode:?}: preparation must succeed: {error:?}"));

        assert_eq!(
            harness.effects().await,
            before,
            "{mode:?}: preparation must not mutate reducer, queue, retry edges, or scheduler"
        );

        // Dropping the prepared command rolls the launch back by never having
        // issued it.
        drop(prepared);
        assert_eq!(
            harness.effects().await,
            before,
            "{mode:?}: a dropped preparation leaves nothing behind"
        );
    }
}

/// A runtime that refuses the launch is reported, and nothing is committed —
/// including in the modes that only became reachable with retry-class Start.
#[tokio::test]
async fn change_error_f5_retry_launch_failure_leaves_no_retry_effect() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA]);
    harness.scheduler.fail_launch("runtime refused the launch");

    let error = assert_refusal_is_mutation_free(&harness, OperatorMode::Stopped, "launch").await;

    assert!(
        matches!(
            error,
            RunControlError::DispatchFailed {
                command: RunCommandKind::Start,
                ..
            }
        ),
        "a Start-selected retry reports a refused *start*: {error:?}"
    );
}

// ============================================================================
// Commit: one route, one edge, one scheduler effect
// ============================================================================

/// Acceptance criterion 8 and the retry-edge ownership rule: the accepted route
/// is `RetryError`, it publishes exactly one target-specific edge, it restores
/// the mark, and it never substitutes an ordinary queue addition.
#[tokio::test]
async fn change_error_f5_retry_terminal_error_publishes_exactly_one_target_specific_edge() {
    let harness = Harness::new(&[ALPHA, BETA]);
    harness.to_error(ALPHA).await;
    harness.to_error(BETA).await;
    harness.mark(&[ALPHA]);
    harness.scheduler.set_running(true);

    harness
        .service
        .start(OperatorMode::Running)
        .await
        .expect("the marked error is retryable");

    let effects = harness.effects().await;
    assert_eq!(
        effects.explicit_retries,
        vec![ALPHA.to_string()],
        "only the retried target's edge is published"
    );
    assert!(
        effects.queue.is_empty(),
        "terminal error retry never routes through an ordinary queue addition: {:?}",
        effects.queue
    );
    assert_eq!(
        effects.notifications, 0,
        "the retry uses the run-control dispatch, not a generic queue notification"
    );
    assert!(
        harness.marks.is_marked(ALPHA),
        "an accepted retry restores the execution mark"
    );
    assert_eq!(
        harness.status(BETA).await,
        "error",
        "the unmarked failed change keeps its terminal evidence"
    );
}

/// Acceptance criterion 3, cardinality half: a live scheduler is notified
/// exactly once and no second boundary is started.
#[tokio::test]
async fn change_error_f5_retry_live_scheduler_is_woken_exactly_once() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA]);
    harness.scheduler.set_running(true);

    harness
        .service
        .start(OperatorMode::Running)
        .await
        .expect("the marked error is retryable");

    assert_eq!(
        harness.scheduler.calls(),
        vec![SchedulerCall::Notified],
        "one wake, no start"
    );
}

/// The reducer is the final authority: a route it refuses settles as a no-op
/// whose reserved dispatch is dropped rather than issued.
#[tokio::test]
async fn change_error_f5_retry_reducer_refusal_settles_as_a_no_op_without_dispatch() {
    let harness = Harness::new(&[ALPHA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA]);

    let prepared = harness
        .service
        .prepare_start(OperatorMode::Stopped)
        .await
        .expect("classification accepted the marked error");

    // The route is consumed between preparation and commit, so the reducer
    // transition the prepared command still intends becomes a no-op.
    harness
        .service
        .retry_change(ALPHA)
        .await
        .expect("the first retry consumes the terminal error");
    let after_first = harness.effects().await;

    let committed = harness
        .service
        .commit(prepared)
        .await
        .expect("a consumed route settles rather than failing");

    assert_eq!(
        committed.outcome,
        RunControlOutcome::NoOp {
            reason: RunNoOpReason::NoRetryableTarget,
        }
    );
    committed.activate(harness.scheduler.as_ref()).await;
    assert_eq!(
        harness.effects().await,
        after_first,
        "a no-op commit issues no second edge, dispatch, or reducer transition"
    );
}

// ============================================================================
// Explicit retry stays separate from ordinary mark settlement
// ============================================================================

/// A settlement runtime that admits dynamic queue work and settles nothing.
///
/// Bound only so the *arming* decision is observable: what matters here is
/// whether an accepted retry starts the ordinary stability deadline, not what a
/// settlement pass would later plan.
struct AdmittingSettlementRuntime;

#[async_trait]
impl crate::orchestration::mark_settlement::MarkSettlementRuntime for AdmittingSettlementRuntime {
    fn admits_dynamic_queue(&self) -> bool {
        true
    }

    async fn settle_marks(&self) -> crate::orchestration::mark_settlement::MarkSettlementPlan {
        crate::orchestration::mark_settlement::MarkSettlementPlan::default()
    }

    async fn report_abandoned_settlement(&self, _pending: Vec<String>) {}
}

/// Acceptance criterion 8: an accepted F5 retry does not arm the ordinary
/// ten-second mark-settlement deadline.
///
/// The ordinary operator mark at the end is the positive control: it proves this
/// process really can arm, so the negative assertion above it is a property of
/// the retry path rather than of an unbound coordinator.
#[tokio::test]
async fn change_error_f5_retry_accepted_retry_arms_no_mark_settlement_deadline() {
    let harness = Harness::new(&[ALPHA, BETA]);
    harness.to_error(ALPHA).await;
    harness.mark(&[ALPHA]);
    harness.scheduler.set_running(true);

    let settlement_runtime: Arc<dyn crate::orchestration::mark_settlement::MarkSettlementRuntime> =
        Arc::new(AdmittingSettlementRuntime);
    let settlement = harness.marks.settlement();
    settlement.bind_runtime(Arc::downgrade(&settlement_runtime));

    harness
        .service
        .start(OperatorMode::Running)
        .await
        .expect("the marked error is retryable");

    assert!(
        !settlement.is_armed(),
        "an accepted explicit retry must not arm the ordinary stability deadline"
    );

    harness
        .operator
        .set_execution_mark(BETA, true)
        .await
        .expect("an ordinary mark is accepted");
    assert!(
        settlement.is_armed(),
        "the control: a standalone operator mark does arm it in this process"
    );
}
