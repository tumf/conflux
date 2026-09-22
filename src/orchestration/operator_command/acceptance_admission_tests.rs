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
/// retry edge.
#[tokio::test]
async fn acceptance_execution_boundary_retry_change_admits_changed_input() {
    let harness = harness(&["change-a"], FakeAdmission::admitting_everything());
    hold(&harness, "change-a", resumable_stall_blocker()).await;

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

// ============================================================================
// Non-resumable holds and the default port
// ============================================================================

/// A non-resumable hold whose fingerprint *did* move is still refused by the
/// existing resumability guard: the fingerprint check does not widen retry
/// admission, it only narrows it.
#[tokio::test]
async fn acceptance_execution_boundary_changed_input_does_not_widen_admission() {
    let harness = harness(&["change-a"], FakeAdmission::admitting_everything());
    hold(&harness, "change-a", execution_hold_blocker()).await;

    let plan = harness
        .service
        .retry_change("change-a")
        .await
        .expect("refusal of a non-resumable hold is not an error");

    assert!(
        plan.change_ids.is_empty() && plan.routes.is_empty() && !plan.explicit_retry,
        "a non-resumable hold must still dispatch nothing"
    );
    assert_eq!(
        harness.state.read().await.display_status("change-a"),
        "stalled"
    );
}

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
