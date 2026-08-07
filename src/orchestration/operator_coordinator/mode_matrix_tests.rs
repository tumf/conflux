//! The process-lifetime dispatch boundary, end to end.
//!
//! One event, one reducer transition, one Core mode transition, one delivery per
//! frontend — regardless of which producer raised it. These tests drive the
//! *real* boundary with a real Web projection attached, so a regression that
//! moved a projection back into an adapter, or that let a command outcome reach
//! `/api/v2` without reaching Core mode, fails here rather than in production.
//!
//! Integration-scoped: a real reducer, a real dispatcher, and a real `WebState`
//! with its `/api/v2` projection. No process, repository, or network.

use std::sync::Arc;

use super::tests::{merge_wait, Harness};
use super::*;
use crate::events::{EventSink, ExecutionEvent};
use crate::orchestration::run_control::SchedulerEffect;
use crate::web::state::{WebEventSink, WebState};

/// The full production wiring: coordinator, dispatch boundary, and Web frontend.
struct Bound {
    harness: Harness,
    web: Arc<WebState>,
}

impl Bound {
    async fn new(change_ids: &[&str]) -> Self {
        let harness = Harness::new(change_ids);
        let web = Arc::new(WebState::new(&[]));
        web.set_shared_state(harness.state.clone()).await;
        web.set_execution_marks(harness.marks.clone()).await;
        web.set_parallel_runtime(harness.parallel.clone()).await;
        let changes: Vec<_> = change_ids.iter().map(|id| listing_row(id)).collect();
        web.update_with_mode(&changes, "select").await;
        web.sync_remote_control_projection().await;

        // The same coordinator, now publishing through a boundary the Web
        // frontend is attached to. Rebuilding the application rather than the
        // dispatcher would give the two halves different owners, which is the
        // arrangement this whole change removes.
        let dispatcher = Arc::new(
            crate::events::EventDispatcher::new(
                harness.state.clone(),
                vec![
                    Arc::new(TimelineSink {
                        timeline: harness.timeline.clone(),
                    }),
                    Arc::new(WebEventSink::new(web.clone())),
                ],
            )
            .with_core_mode(Some(harness.core.clone())),
        );
        let revisions: Arc<dyn crate::events::OutcomeRevisions> = web.clone();
        let application = Arc::new(
            OperatorApplication::new(
                harness.core.clone(),
                harness.application.run_control(),
                dispatcher.clone(),
            )
            .with_revisions(Some(revisions)),
        );

        Self {
            harness: Harness {
                dispatcher,
                application,
                ..harness
            },
            web,
        }
    }

    async fn app_mode(&self) -> String {
        self.web.get_state().await.app_mode
    }

    async fn is_resolving(&self) -> bool {
        self.web.get_state().await.is_resolving
    }

    fn revision(&self) -> u64 {
        self.web.remote_control().projection().revision()
    }

    /// The `/api/v2` event types published so far, in order.
    fn published(&self) -> Vec<String> {
        let projection = self.web.remote_control().projection();
        match projection.events_after(0) {
            crate::web::remote_control_api::projection::EventsSince::Replay(events) => events
                .into_iter()
                .map(|envelope| envelope.event_type)
                .collect(),
            crate::web::remote_control_api::projection::EventsSince::Gap => {
                panic!("the event ring must retain a short test's events")
            }
        }
    }
}

/// A sink that mirrors dispatched event names onto the harness timeline.
struct TimelineSink {
    timeline: Arc<super::tests::Timeline>,
}

#[async_trait::async_trait]
impl EventSink for TimelineSink {
    async fn on_event(&self, event: &ExecutionEvent) {
        self.timeline
            .push_public(crate::events::classify_event(event).0);
    }

    async fn on_state_changed(&self, _state: &crate::orchestration::state::OrchestratorState) {}
}

fn listing_row(id: &str) -> crate::openspec::Change {
    crate::openspec::Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: crate::openspec::ProposalMetadata::default(),
    }
}

/// Every accepted command outcome reaches Core mode and the Web projection at
/// the same revision, through one dispatch.
#[tokio::test]
async fn accepted_operator_command_mode_matrix_projects_every_accepted_effect() {
    let bound = Bound::new(&["c1"]).await;
    let harness = &bound.harness;
    harness.marks.set("c1", true);

    // ── start ───────────────────────────────────────────────────────────────
    let before = bound.revision();
    let start = harness.apply(OperatorIntent::Start).await;
    assert!(matches!(
        start.outcome,
        Ok(ApplicationOutcome::Run(
            RunControlOutcome::RunDispatched { .. }
        ))
    ));
    assert_eq!(harness.core.get(), OperatorMode::Running);
    assert_eq!(bound.app_mode().await, "running");
    let dispatched = start
        .revision
        .expect("a changed command records a revision");
    assert!(
        dispatched > before,
        "the accepted outcome advances revision"
    );
    assert_eq!(
        dispatched,
        bound.revision(),
        "Core mode, the snapshot, and the recorded revision describe one instant"
    );

    // ── graceful stop, then cancel ──────────────────────────────────────────
    let stop = harness.apply(OperatorIntent::Stop).await;
    assert!(matches!(
        stop.outcome,
        Ok(ApplicationOutcome::Run(RunControlOutcome::StopRequested))
    ));
    assert_eq!(harness.core.get(), OperatorMode::Stopping);
    assert_eq!(bound.app_mode().await, "stopping");

    let cancel = harness.apply(OperatorIntent::CancelStop).await;
    assert!(matches!(
        cancel.outcome,
        Ok(ApplicationOutcome::Run(RunControlOutcome::StopCancelled))
    ));
    assert_eq!(
        harness.core.get(),
        OperatorMode::Running,
        "cancel-stop returns the process to Running for every frontend"
    );
    assert_eq!(bound.app_mode().await, "running");

    // The vocabulary is exact where an existing event already means the thing.
    let published = bound.published();
    assert!(
        published.contains(&"stopping".to_string()),
        "graceful stop reuses the existing Stopping event: {published:?}"
    );
    assert!(
        published
            .iter()
            .filter(|event| *event == "operator_command_applied")
            .count()
            >= 2,
        "run dispatch and stop cancellation both publish accepted decision facts: {published:?}"
    );
}

/// An accepted active resolve projects the resolver, `is_resolving`, and Running
/// in the same outcome dispatch; a queued one moves neither.
#[tokio::test]
async fn accepted_operator_command_mode_matrix_projects_resolve_reservations() {
    let bound = Bound::new(&["c1", "c2"]).await;
    let harness = &bound.harness;
    merge_wait(harness, "c1").await;
    merge_wait(harness, "c2").await;

    let active = harness
        .apply(OperatorIntent::ResolveMerge {
            change_id: "c1".to_string(),
        })
        .await;
    assert!(matches!(
        &active.outcome,
        Ok(ApplicationOutcome::Run(
            RunControlOutcome::ResolveReserved {
                reservation: ResolveReservation::Active,
                scheduler: SchedulerEffect::Started,
                ..
            }
        ))
    ));
    assert!(
        bound.is_resolving().await,
        "an active reservation publishes is_resolving at the accepted revision"
    );
    assert_eq!(bound.app_mode().await, "running");
    assert_eq!(
        active.revision,
        Some(bound.revision()),
        "the reservation and the mode are one snapshot"
    );

    let mode_before = bound.app_mode().await;
    let queued = harness
        .apply(OperatorIntent::ResolveMerge {
            change_id: "c2".to_string(),
        })
        .await;
    assert!(matches!(
        &queued.outcome,
        Ok(ApplicationOutcome::Run(
            RunControlOutcome::ResolveReserved {
                reservation: ResolveReservation::Queued { .. },
                scheduler: SchedulerEffect::None,
                ..
            }
        ))
    ));
    assert_eq!(
        bound.app_mode().await,
        mode_before,
        "a queued reservation changes no mode"
    );
}

/// Authoritative lifecycle events move Core mode and every frontend together.
#[tokio::test]
async fn accepted_operator_command_mode_matrix_lifecycle_events_move_every_frontend() {
    let bound = Bound::new(&["c1"]).await;
    let harness = &bound.harness;

    for (event, core, app_mode) in [
        (
            ExecutionEvent::ProcessingStarted("c1".to_string()),
            OperatorMode::Running,
            "running",
        ),
        (ExecutionEvent::Stopping, OperatorMode::Stopping, "stopping"),
        (ExecutionEvent::Stopped, OperatorMode::Stopped, "stopped"),
    ] {
        harness.dispatcher.dispatch(event.clone()).await;
        assert_eq!(
            harness.core.get(),
            core,
            "{event:?} must move the one Core mode"
        );
        assert_eq!(
            bound.app_mode().await,
            app_mode,
            "{event:?} must move the Web projection to the same value"
        );
    }

    // A late completion overwrites neither.
    harness
        .dispatcher
        .dispatch(ExecutionEvent::AllCompleted)
        .await;
    assert_eq!(harness.core.get(), OperatorMode::Stopped);
    assert_eq!(bound.app_mode().await, "stopped");
}

/// A duplicate delivery of one dispatch changes nothing.
///
/// The dispatch owner delivers once, but a frontend boundary is exactly where an
/// accidental second delivery would show up as a doubled sequence and a doubled
/// revision, so the guard is asserted rather than assumed.
#[tokio::test]
async fn accepted_operator_command_mode_matrix_duplicate_delivery_is_inert() {
    let bound = Bound::new(&["c1"]).await;

    let dispatch = crate::events::EventDispatch {
        id: crate::events::next_dispatch_id(),
        event: &ExecutionEvent::Stopping,
        ownership: crate::events::event_ownership(&ExecutionEvent::Stopping),
        state: None,
    };
    bound.web.apply_dispatch(&dispatch).await;
    let after_first = bound.revision();
    let events_after_first = bound.published().len();

    bound.web.apply_dispatch(&dispatch).await;

    assert_eq!(
        bound.revision(),
        after_first,
        "a repeated dispatch identity must not advance the revision"
    );
    assert_eq!(
        bound.published().len(),
        events_after_first,
        "a repeated dispatch identity must not publish a second event"
    );
}

/// The accepted-outcome event carries decision facts, not a second reducer.
///
/// `OperatorCommandApplied` is applied to the reducer like every other event.
/// If it re-applied the reducer commands the services already committed, a
/// queue intent would be added twice and an accepted start would be
/// indistinguishable from two.
#[tokio::test]
async fn accepted_operator_command_mode_matrix_outcome_event_reapplies_no_reducer_command() {
    let bound = Bound::new(&["c1"]).await;
    let harness = &bound.harness;
    harness.marks.set("c1", true);
    harness.apply(OperatorIntent::Start).await;
    let after_start = harness.status("c1").await;

    // Deliver the same decision facts a second time, directly.
    harness
        .dispatcher
        .dispatch(ExecutionEvent::OperatorCommandApplied {
            effect: crate::events::OperatorCommandEffect::RunDispatched {
                change_ids: vec!["c1".to_string()],
                explicit_retry: false,
                scheduler_started: true,
            },
        })
        .await;

    assert_eq!(
        harness.status("c1").await,
        after_start,
        "the outcome event must not reapply the reducer command that produced it"
    );
}
