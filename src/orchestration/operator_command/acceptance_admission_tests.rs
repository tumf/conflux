//! Shared-admission regressions for the unchanged-Acceptance-input refusal.
//!
//! The requirement these cover is that one classifier stands behind every retry
//! alias — `retry_change`, `retry_errors`, the Start/F5 retry route, and the
//! terminal-error queue alias — so an API snapshot and command admission cannot
//! disagree about whether a target is retryable. Each test therefore asserts two
//! things: the typed refusal, and that *nothing was dispatched*.
//!
//! Unit-scoped: the admission port is an in-memory double, so no worktree, no
//! Git, and no filesystem participates. The workspace-derived implementation is
//! covered by its own tests next to the manifest.

use std::sync::Arc;

use tokio::sync::RwLock;

use super::*;
use crate::events::{ExecutionEvent, StalledBlocker};
use crate::orchestration::acceptance::execution_manifest::{
    AcceptanceAdmission, AcceptanceHoldCategory, FINGERPRINT_COMPONENTS, UNCHANGED_ACCEPTANCE_INPUT,
};
use crate::orchestration::state::OrchestratorState;

const FINGERPRINT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// An in-memory admission port: refuses exactly the change IDs it was given.
struct FakeAdmission {
    refused: Vec<String>,
    /// Every change ID the service asked about, in order.
    queried: std::sync::Mutex<Vec<String>>,
}

impl FakeAdmission {
    fn refusing(ids: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            refused: ids.iter().map(|id| id.to_string()).collect(),
            queried: std::sync::Mutex::new(Vec::new()),
        })
    }

    fn admitting_everything() -> Arc<Self> {
        Self::refusing(&[])
    }

    fn queried(&self) -> Vec<String> {
        self.queried.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl AcceptanceAdmissionPort for FakeAdmission {
    async fn classify(&self, change_id: &str) -> AcceptanceAdmission {
        self.queried.lock().unwrap().push(change_id.to_string());
        if self.refused.iter().any(|id| id == change_id) {
            AcceptanceAdmission::Refuse {
                outcome: UNCHANGED_ACCEPTANCE_INPUT,
                category: AcceptanceHoldCategory::ReviewDeadlineExhausted,
                fingerprint: FINGERPRINT.to_string(),
                components: FINGERPRINT_COMPONENTS.to_vec(),
            }
        } else {
            AcceptanceAdmission::Admit
        }
    }
}

struct Harness {
    service: OperatorCommandService,
    state: Arc<RwLock<OrchestratorState>>,
    queue: Arc<crate::tui::queue::DynamicQueue>,
    admission: Arc<FakeAdmission>,
}

fn harness(ids: &[&str], admission: Arc<FakeAdmission>) -> Harness {
    let state = Arc::new(RwLock::new(OrchestratorState::new(
        ids.iter().map(|id| id.to_string()).collect(),
        10,
    )));
    let queue = Arc::new(crate::tui::queue::DynamicQueue::new());
    let service = OperatorCommandService::new(
        state.clone(),
        queue.clone(),
        Arc::new(NoopQueueHooks),
        Arc::new(ExecutionMarkStore::new()),
    )
    .with_acceptance_admission(admission.clone());
    Harness {
        service,
        state,
        queue,
        admission,
    }
}

/// An Acceptance-owned execution hold, exactly as the boundary publishes one:
/// non-resumable, no unblock condition, acceptance phase.
fn execution_hold_blocker() -> StalledBlocker {
    StalledBlocker {
        category: AcceptanceHoldCategory::ReviewDeadlineExhausted
            .as_str()
            .to_string(),
        phase: "acceptance".to_string(),
        gate: "acceptance_execution_boundary".to_string(),
        error_summary: "review deadline exhausted".to_string(),
        evidence: vec![format!("input_fingerprint={FINGERPRINT}")],
        unblock_condition: None,
        prerequisite_owner: None,
        next_action: "change repository evidence and retry".to_string(),
        resumable: false,
        worktree_preserved: true,
    }
}

/// A resumable Acceptance stall, which is retryable on its own terms.
fn resumable_stall_blocker() -> StalledBlocker {
    StalledBlocker {
        resumable: true,
        ..execution_hold_blocker()
    }
}

async fn hold(harness: &Harness, change_id: &str, blocker: StalledBlocker) {
    let mut guard = harness.state.write().await;
    guard.apply_execution_event(&ExecutionEvent::ExecutionBlocked {
        change_id: change_id.to_string(),
        blocker,
    });
}

async fn mark_terminal_error(harness: &Harness, change_id: &str) {
    let mut guard = harness.state.write().await;
    guard.apply_execution_event(&ExecutionEvent::ApplyFailed {
        change_id: change_id.to_string(),
        error: "terminal error".to_string(),
    });
}

// ============================================================================
// retry_change
// ============================================================================

/// The core refusal: an unchanged fingerprint produces the typed
/// `unchanged_acceptance_input` error, names every component that would restore
/// eligibility, and dispatches nothing.
#[tokio::test]
async fn acceptance_execution_boundary_retry_change_refuses_unchanged_input() {
    let harness = harness(&["change-a"], FakeAdmission::refusing(&["change-a"]));
    hold(&harness, "change-a", resumable_stall_blocker()).await;

    let error = harness
        .service
        .retry_change("change-a")
        .await
        .expect_err("unchanged Acceptance input must be a typed refusal");

    match &error {
        OperatorCommandError::UnchangedAcceptanceInput {
            change_id,
            category,
            fingerprint,
            components,
        } => {
            assert_eq!(change_id, "change-a");
            assert_eq!(
                category,
                AcceptanceHoldCategory::ReviewDeadlineExhausted.as_str()
            );
            assert_eq!(fingerprint, FINGERPRINT);
            for expected in FINGERPRINT_COMPONENTS {
                assert!(
                    components.iter().any(|part| part == expected),
                    "the refusal must name '{expected}'"
                );
            }
        }
        other => panic!("expected an unchanged-input refusal, got {other:?}"),
    }
    let message = error.to_string();
    assert!(message.contains(UNCHANGED_ACCEPTANCE_INPUT), "{message}");
    assert!(
        message.contains("No analysis, Apply, gate, or Acceptance work was dispatched"),
        "{message}"
    );

    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "stalled",
        "a refused retry must leave the hold and its evidence in place"
    );
    assert!(
        harness.queue.is_empty().await,
        "no work may be dispatched for a refused retry"
    );
}

/// A changed fingerprint takes the ordinary route and publishes exactly one
/// retry edge — on the hold the boundary actually publishes.
///
/// The seed matters. Every runtime-owned execution hold carries
/// `resumable: false`, because an execution stop is not a wait on a named
/// prerequisite. Seeding a *resumable* stall here would prove only that the
/// resumability guard lets resumable things through, which was never in doubt;
/// what has to hold is that a changed fingerprint restores eligibility for the
/// non-resumable hold the boundary really emits.
#[tokio::test]
async fn acceptance_execution_boundary_retry_change_admits_changed_input() {
    let harness = harness(&["change-a"], FakeAdmission::admitting_everything());
    hold(&harness, "change-a", execution_hold_blocker()).await;

    let plan = harness
        .service
        .retry_change("change-a")
        .await
        .expect("a changed fingerprint must be admitted");

    assert_eq!(plan.change_ids, vec!["change-a".to_string()]);
    assert_eq!(plan.routes, vec![RetryRoute::AcceptanceStall]);
    assert!(plan.explicit_retry);
    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "queued"
    );
}

/// The same admission through the bulk verb, and the unchanged case still
/// refused on the same seed. Together these are the whole contract: the
/// fingerprint — not the hold's `resumable` flag — decides whether a bounded
/// execution hold may be retried, and it decides identically for both aliases.
#[tokio::test]
async fn acceptance_execution_boundary_execution_hold_retry_follows_the_fingerprint() {
    for admitting in [true, false] {
        let admission = if admitting {
            FakeAdmission::admitting_everything()
        } else {
            FakeAdmission::refusing(&["change-a"])
        };
        let single_harness = harness(&["change-a"], admission);
        hold(&single_harness, "change-a", execution_hold_blocker()).await;

        let single = single_harness.service.retry_change("change-a").await;
        if admitting {
            let plan = single.expect("a moved fingerprint must admit the execution hold");
            assert_eq!(plan.change_ids, vec!["change-a".to_string()]);
            assert_eq!(plan.routes, vec![RetryRoute::AcceptanceStall]);
            assert!(plan.explicit_retry);
            assert_eq!(
                single_harness.state.read().await.display_status("change-a"),
                "queued"
            );
        } else {
            assert!(
                matches!(
                    single,
                    Err(OperatorCommandError::UnchangedAcceptanceInput { .. })
                ),
                "an unchanged execution hold must stay refused, got {single:?}"
            );
            assert_eq!(
                single_harness.state.read().await.display_status("change-a"),
                "stalled",
                "a refused retry keeps the hold and its evidence"
            );
        }

        // The bulk verb, against a fresh row in the same state.
        let bulk_harness = harness(
            &["change-a"],
            if admitting {
                FakeAdmission::admitting_everything()
            } else {
                FakeAdmission::refusing(&["change-a"])
            },
        );
        hold(&bulk_harness, "change-a", execution_hold_blocker()).await;
        let plan = bulk_harness
            .service
            .retry_errors(&["change-a".to_string()])
            .await;
        if admitting {
            assert_eq!(
                plan.change_ids,
                vec!["change-a".to_string()],
                "retry_errors must admit the same hold retry_change admits, exactly once"
            );
        } else {
            assert!(
                plan.change_ids.is_empty(),
                "retry_errors must refuse the same unchanged hold retry_change refuses"
            );
        }
    }
}

/// A non-resumable acceptance stall that is *not* fingerprint-guarded keeps the
/// old refusal. The boundary's exception is scoped to holds whose category the
/// admission classifier can actually answer for; an ordinary stall has no
/// fingerprint behind it, so retrying past it would still dispatch ambiguous
/// work.
#[tokio::test]
async fn acceptance_execution_boundary_unguarded_non_resumable_stall_is_still_refused() {
    let harness = harness(&["change-a"], FakeAdmission::admitting_everything());
    let unguarded = StalledBlocker {
        category: "permission_denied".to_string(),
        gate: "apply".to_string(),
        ..execution_hold_blocker()
    };
    hold(&harness, "change-a", unguarded).await;

    let plan = harness
        .service
        .retry_change("change-a")
        .await
        .expect("refusal of a non-resumable hold is not an error");

    assert!(
        plan.change_ids.is_empty() && plan.routes.is_empty() && !plan.explicit_retry,
        "a non-resumable stall with no fingerprint behind it must still dispatch nothing"
    );
    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "stalled"
    );
}

/// Unchanged input is a different refusal from an unsupported status, and the
/// unsupported one still wins when the status carries no retryable evidence at
/// all: there is nothing to refuse an Acceptance retry *for*.
#[tokio::test]
async fn acceptance_execution_boundary_unsupported_status_is_still_unsupported() {
    let harness = harness(&["change-a"], FakeAdmission::refusing(&["change-a"]));

    let error = harness
        .service
        .retry_change("change-a")
        .await
        .expect_err("a non-retryable status has no retry route");

    assert!(
        matches!(error, OperatorCommandError::RetryUnsupported { .. }),
        "got {error:?}"
    );
    assert!(
        harness.admission.queried().is_empty(),
        "admission must not be consulted for a target that has no retry route"
    );
}

/// The terminal-error route is guarded too: the same classifier stands behind
/// every alias, so a change that reached `error` after an Acceptance hold cannot
/// slip past by way of a different verb.
#[tokio::test]
async fn acceptance_execution_boundary_terminal_error_route_is_guarded() {
    let harness = harness(&["change-a"], FakeAdmission::refusing(&["change-a"]));
    mark_terminal_error(&harness, "change-a").await;
    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "error"
    );

    let error = harness
        .service
        .retry_change("change-a")
        .await
        .expect_err("the terminal-error alias must consult the same classifier");

    assert!(
        matches!(error, OperatorCommandError::UnchangedAcceptanceInput { .. }),
        "got {error:?}"
    );
    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "error",
        "a refused retry must not move the row"
    );
}

// ============================================================================
// Bulk retry
// ============================================================================

/// Mixed eligibility: one unchanged-input target is refused with its typed
/// reason while an eligible sibling is admitted exactly once. Neither target's
/// evidence is discarded.
#[tokio::test]
async fn acceptance_execution_boundary_bulk_retry_reports_mixed_eligibility() {
    let harness = harness(&["alpha", "beta"], FakeAdmission::refusing(&["alpha"]));
    hold(&harness, "alpha", resumable_stall_blocker()).await;
    hold(&harness, "beta", resumable_stall_blocker()).await;

    let (routes, refusals) = harness
        .service
        .plan_retry_errors_with_refusals(&["alpha".to_string(), "beta".to_string()])
        .await;

    assert_eq!(
        routes,
        vec![("beta".to_string(), RetryRoute::AcceptanceStall)],
        "only the eligible sibling may be admitted"
    );
    assert_eq!(refusals.len(), 1);
    assert!(matches!(
        refusals[0],
        OperatorCommandError::UnchangedAcceptanceInput { ref change_id, .. } if change_id == "alpha"
    ));

    let plan = harness.service.commit_retry_routes(&routes).await;
    assert_eq!(plan.change_ids, vec!["beta".to_string()]);

    let guard = harness.state.read().await;
    assert_eq!(
        guard.display_status("alpha"),
        "stalled",
        "the refused target keeps its hold and evidence"
    );
    assert_eq!(guard.display_status("beta"), "queued");
}

/// `retry_errors` itself skips the refused target rather than failing the whole
/// request — one unchanged-input target is not a reason to refuse every
/// unrelated candidate.
#[tokio::test]
async fn acceptance_execution_boundary_bulk_retry_admits_the_eligible_sibling() {
    let harness = harness(&["alpha", "beta"], FakeAdmission::refusing(&["alpha"]));
    hold(&harness, "alpha", resumable_stall_blocker()).await;
    hold(&harness, "beta", resumable_stall_blocker()).await;

    let plan = harness
        .service
        .retry_errors(&["alpha".to_string(), "beta".to_string()])
        .await;

    assert_eq!(plan.change_ids, vec!["beta".to_string()]);
    assert!(plan.explicit_retry);
    assert_eq!(
        harness.admission.queried(),
        vec!["alpha".to_string(), "beta".to_string()],
        "the same classifier must be consulted for every bulk target"
    );
}

/// The bulk verb reports the refused target rather than silently shortening the
/// accepted list, and admits the eligible sibling exactly once.
#[tokio::test]
async fn acceptance_execution_boundary_bulk_retry_carries_the_refusal_per_target() {
    let harness = harness(&["alpha", "beta"], FakeAdmission::refusing(&["alpha"]));
    hold(&harness, "alpha", execution_hold_blocker()).await;
    hold(&harness, "beta", execution_hold_blocker()).await;

    let (plan, refusals) = harness
        .service
        .retry_errors_with_refusals(&["alpha".to_string(), "beta".to_string()])
        .await;

    assert_eq!(
        plan.change_ids,
        vec!["beta".to_string()],
        "only the admitted sibling may be dispatched, and exactly once"
    );
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    assert_eq!(refusals[0].change_id(), "alpha");
    assert_eq!(
        refusals[0].outcome_token(),
        Some(UNCHANGED_ACCEPTANCE_INPUT),
        "the refusal must carry its own stable token so a caller reports it per target"
    );

    let guard = harness.state.read().await;
    assert_eq!(guard.display_status("alpha"), "stalled");
    assert_eq!(guard.display_status("beta"), "queued");
}

// ============================================================================
// The terminal-error queue alias
// ============================================================================

/// `add_to_queue` with `set_queue_intent = true` on a terminal-error row *is* a
/// retry — it applies `RetryError` and publishes an explicit-retry edge — so it
/// consults the same classifier, and a refusal mutates nothing.
#[tokio::test]
async fn acceptance_execution_boundary_queue_intent_alias_refuses_a_terminal_error_row() {
    let harness = harness(&["change-a"], FakeAdmission::refusing(&["change-a"]));
    mark_terminal_error(&harness, "change-a").await;

    let error = harness
        .service
        .add_to_queue("change-a")
        .await
        .expect_err("the queue-intent alias must consult the same classifier");

    assert!(
        matches!(error, OperatorCommandError::UnchangedAcceptanceInput { .. }),
        "got {error:?}"
    );
    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "error",
        "a refused alias must not release the terminal classification"
    );
    assert!(
        harness.queue.is_empty().await,
        "no work may be queued for a refused alias"
    );
}

/// The same alias on a *stalled* acceptance hold, which `AddToQueue` would
/// otherwise release and queue.
#[tokio::test]
async fn acceptance_execution_boundary_queue_intent_alias_refuses_a_stalled_hold() {
    let harness = harness(&["change-a"], FakeAdmission::refusing(&["change-a"]));
    hold(&harness, "change-a", execution_hold_blocker()).await;

    let error = harness
        .service
        .add_to_queue("change-a")
        .await
        .expect_err("releasing a stalled acceptance hold is retry intent");

    assert!(
        matches!(error, OperatorCommandError::UnchangedAcceptanceInput { .. }),
        "got {error:?}"
    );
    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "stalled",
        "the hold and its blocker evidence must survive"
    );
    assert!(harness.queue.is_empty().await);
}

/// An ordinary addition is not a retry: it arms no edge and releases nothing, so
/// it keeps its unconditional path and never pays for a classifier call.
#[tokio::test]
async fn acceptance_execution_boundary_ordinary_queue_addition_skips_the_classifier() {
    let harness = harness(&["change-a"], FakeAdmission::refusing(&["change-a"]));

    let outcome = harness
        .service
        .add_to_queue("change-a")
        .await
        .expect("an ordinary addition is not retry intent");

    assert!(outcome.reducer_changed);
    assert!(
        harness.admission.queried().is_empty(),
        "admission must not be consulted for an addition that dispatches no retry"
    );
}

// ============================================================================
// Automatic mark settlement
// ============================================================================

/// Settlement is the one admission path with no operator in front of it, and a
/// restart discards the in-memory hold — so an already-refused change presents
/// as an ordinary `not queued` row. The classifier reads the worktree, which is
/// where the refusal actually lives.
#[tokio::test]
async fn acceptance_execution_boundary_mark_settlement_refuses_unchanged_input() {
    use crate::orchestration::mark_settlement::{MarkSettlementAction, MarkSettlementExclusion};

    let harness = harness(&["change-a"], FakeAdmission::refusing(&["change-a"]));
    harness.service.marks().set("change-a", true);

    let plan = harness
        .service
        .plan_mark_settlement(&["change-a".to_string()])
        .await;

    assert!(
        plan.additions.is_empty(),
        "an unchanged-input target must not be planned as a queue addition"
    );
    assert_eq!(
        plan.excluded,
        vec![(
            "change-a".to_string(),
            MarkSettlementExclusion::UnchangedAcceptanceInput
        )],
        "the settlement detail must name the refusal, not a lifecycle reason"
    );
    assert_eq!(
        MarkSettlementExclusion::UnchangedAcceptanceInput.as_str(),
        UNCHANGED_ACCEPTANCE_INPUT
    );

    // And the application half refuses too, so a plan derived a moment earlier
    // cannot slip a mutation past a hold that landed since.
    let applied = harness
        .service
        .apply_settlement_queue_intent("change-a", MarkSettlementAction::Add)
        .await;

    assert_eq!(
        applied.skipped,
        Some(MarkSettlementExclusion::UnchangedAcceptanceInput)
    );
    assert!(!applied.outcome.reducer_changed);
    assert!(!applied.outcome.dynamic_queue_mutated);
    assert!(
        harness.queue.is_empty().await,
        "settlement must dispatch no work for a refused target"
    );
    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "not queued",
        "a refused settlement must not move the row"
    );
}

/// The mirror: an admitting port leaves settlement exactly as it was.
#[tokio::test]
async fn acceptance_execution_boundary_mark_settlement_admits_a_changed_target() {
    let harness = harness(&["change-a"], FakeAdmission::admitting_everything());
    harness.service.marks().set("change-a", true);

    let plan = harness
        .service
        .plan_mark_settlement(&["change-a".to_string()])
        .await;

    assert_eq!(plan.additions, vec!["change-a".to_string()]);
    assert!(plan.excluded.is_empty(), "{:?}", plan.excluded);
}

// ============================================================================
// Non-resumable holds and the default port
// ============================================================================

/// The operator-facing projection of a bounded hold: `stalled`, blocker kind
/// `none`, non-resumable — and therefore a row `cflx client wait` releases with
/// `change_requires_action` instead of holding out for an owner that will never
/// advance it again.
#[tokio::test]
async fn acceptance_execution_boundary_hold_projects_as_a_releasing_stalled_row() {
    use crate::client::completion::{classify, Disposition};
    use crate::orchestration::state::BlockerKind;

    let harness = harness(&["change-a"], FakeAdmission::refusing(&["change-a"]));
    let published = crate::orchestration::acceptance::execution_manifest::AcceptanceExecutionHold {
        category: AcceptanceHoldCategory::ReviewDeadlineExhausted,
        budget_secs: 3600,
        cleanup_confirmed: true,
        cleanup_diagnostics: "owned process group confirmed quiescent".to_string(),
        fingerprint: FINGERPRINT.to_string(),
        evidence: vec!["focused-gate: executed and passed".to_string()],
    }
    .to_stalled_blocker("change-a");
    hold(&harness, "change-a", published).await;

    let guard = harness.state.read().await;
    let runtime = guard
        .change_runtime("change-a")
        .expect("the hold must produce a runtime row");
    assert_eq!(guard.display_status("change-a"), "stalled");
    assert_eq!(runtime.blocker_kind(), BlockerKind::None);
    assert!(runtime.is_acceptance_stalled());
    assert!(
        !runtime.is_resumable_acceptance_stall(),
        "a bounded execution hold is never resumable on unchanged input"
    );
    drop(guard);

    assert_eq!(
        classify(
            Some("stalled"),
            Some(crate::web::remote_control_api::dto::BlockerKind::None),
        ),
        Disposition::RequiresAction,
        "`cflx client wait` must release rather than wait for an owner that will not advance"
    );
}

/// The default port admits everything, so an assembly with no managed worktree
/// behaves exactly as it did before this guard existed.
#[tokio::test]
async fn acceptance_execution_boundary_default_port_admits() {
    let admission = AlwaysAdmitAcceptance;

    assert_eq!(
        AcceptanceAdmissionPort::classify(&admission, "change-a").await,
        AcceptanceAdmission::Admit
    );
}
