//! `/api/v2` coherence between a revoking event and the mark it revokes.
//!
//! The property under test is a *revision* property, not a value property: a
//! client that reads the snapshot named by a failure event's envelope must never
//! see the failure alongside the stale `execution_marked: true`. That can only
//! hold if mark reconciliation runs inside the authoritative dispatch, before
//! this sink builds its candidate snapshot — so every test here drives the real
//! `dispatch_event_with_marks` boundary rather than poking the projection.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::events::{dispatch_event_with_marks, EventSink, ExecutionEvent};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::mark_reconciliation::ExecutionMarkReconciler;
use crate::orchestration::operator_command::{ExecutionMarkStore, ParallelRuntime};
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::dto::InstanceSnapshot;
use crate::web::remote_control_api::projection::EventsSince;
use crate::web::state::{WebEventSink, WebState};

/// The three authorities the runner binds in production, wired the same way.
struct Wired {
    web_state: Arc<WebState>,
    reducer: Arc<tokio::sync::RwLock<OrchestratorState>>,
    marks: Arc<ExecutionMarkStore>,
    reconciler: ExecutionMarkReconciler,
    sinks: Vec<Arc<dyn EventSink>>,
}

impl Wired {
    async fn new(change_ids: &[&str]) -> Self {
        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        let web_state = Arc::new(WebState::new(
            &change_ids.iter().map(|id| change(id)).collect::<Vec<_>>(),
        ));
        web_state.set_shared_state(reducer.clone()).await;
        web_state.set_execution_marks(marks.clone()).await;
        web_state.set_parallel_runtime(parallel.clone()).await;

        Self {
            sinks: vec![Arc::new(WebEventSink::new(web_state.clone()))],
            reconciler: ExecutionMarkReconciler::new(marks.clone(), parallel),
            web_state,
            reducer,
            marks,
        }
    }

    async fn dispatch(&self, event: ExecutionEvent) {
        dispatch_event_with_marks(&self.reducer, &self.sinks, event, Some(&self.reconciler)).await;
    }

    /// The published snapshot and its revision.
    fn published(&self) -> (InstanceSnapshot, u64) {
        let (snapshot, revision, _) = self.web_state.remote_control().projection().snapshot();
        (snapshot, revision)
    }

    /// The revision the envelope for `event_type` carried, and its sequence.
    fn envelope_revision(&self, event_type: &str) -> u64 {
        let EventsSince::Replay(events) =
            self.web_state.remote_control().projection().events_after(0)
        else {
            panic!("the replay window must still hold every event of this test");
        };
        let envelope = events
            .iter()
            .rev()
            .find(|event| event.event_type == event_type)
            .unwrap_or_else(|| panic!("no `{event_type}` event reached the v2 stream"));
        envelope.state_revision
    }
}

fn change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

fn marked(snapshot: &InstanceSnapshot, change_id: &str) -> bool {
    snapshot
        .changes
        .iter()
        .find(|change| change.id == change_id)
        .unwrap_or_else(|| panic!("change '{change_id}' must be projected"))
        .execution_marked
}

fn refresh(
    active: &[&str],
    rejected: &[&str],
    committed: &[&str],
    dirty: &[&str],
) -> ExecutionEvent {
    ExecutionEvent::ChangesRefreshed {
        changes: active.iter().map(|id| change(id)).collect(),
        rejected_changes: rejected.iter().map(|id| change(id)).collect(),
        committed_change_ids: committed.iter().map(|id| id.to_string()).collect(),
        uncommitted_file_change_ids: dirty.iter().map(|id| id.to_string()).collect(),
        worktree_change_ids: HashSet::new(),
        worktree_paths: HashMap::new(),
        worktree_not_ahead_ids: HashSet::new(),
        merge_wait_ids: HashSet::new(),
    }
}

/// A revoking event and `execution_marked: false` share one state revision.
///
/// The whole test is the ordering claim: the sink builds its candidate snapshot
/// from the reconciled store, so the revision the failure envelope names already
/// reports the mark as gone. An unrelated target keeps its mark in the same
/// snapshot, and a duplicate delivery advances no further revision.
#[tokio::test]
async fn execution_event_clears_authoritative_mark_before_projection() {
    let wired = Wired::new(&["alpha", "beta"]).await;
    wired.marks.set("alpha", true);
    wired.marks.set("beta", true);

    // Establish a baseline revision that still reports both marks.
    wired
        .dispatch(ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "apply".to_string(),
        })
        .await;
    let (before, before_revision) = wired.published();
    assert!(marked(&before, "alpha") && marked(&before, "beta"));

    wired
        .dispatch(ExecutionEvent::ApplyFailed {
            change_id: "alpha".to_string(),
            error: "boom".to_string(),
        })
        .await;

    let (after, after_revision) = wired.published();
    assert!(
        after_revision > before_revision,
        "the failure must advance the published revision"
    );
    assert_eq!(
        wired.envelope_revision("apply_failed"),
        after_revision,
        "the failure event must name the revision that already reports the cleared mark"
    );
    assert!(
        !marked(&after, "alpha"),
        "the failure revision still exposes the stale mark"
    );
    assert!(
        marked(&after, "beta"),
        "an unrelated target lost its mark in the same snapshot"
    );
    assert_eq!(
        after
            .changes
            .iter()
            .find(|change| change.id == "alpha")
            .map(|change| change.display_status.as_str()),
        Some("error"),
        "the same revision must carry the reducer transition"
    );

    // Duplicate delivery changes neither the reducer nor the marks, so it must
    // not advance another revision.
    wired
        .dispatch(ExecutionEvent::ApplyFailed {
            change_id: "alpha".to_string(),
            error: "boom".to_string(),
        })
        .await;
    let (_, duplicate_revision) = wired.published();
    assert_eq!(
        duplicate_revision, after_revision,
        "a duplicate revocation must be revision-idempotent"
    );
    assert_eq!(wired.marks.marked_ids(), vec!["beta".to_string()]);
}

/// One refresh publishes the rejected and parallel-ineligible revocations it
/// decided, in its own revision.
///
/// The ineligible target is asserted through the published snapshot; the
/// rejected marker row leaves the active listing entirely, so its revocation is
/// asserted where it is authoritative — the shared store the next snapshot and
/// every Start target resolution read.
#[tokio::test]
async fn refresh_publishes_rejected_and_ineligible_revocations_in_one_revision() {
    let wired = Wired::new(&["alpha", "beta", "gamma"]).await;
    for id in ["alpha", "beta", "gamma"] {
        wired.marks.set(id, true);
    }

    // `alpha` becomes a rejected marker row, `gamma` has uncommitted proposal
    // files, `beta` stays committed and clean.
    wired
        .dispatch(refresh(
            &["beta", "gamma"],
            &["alpha"],
            &["beta", "gamma"],
            &["gamma"],
        ))
        .await;

    let (snapshot, revision) = wired.published();
    assert_eq!(
        wired.envelope_revision("changes_refreshed"),
        revision,
        "the refresh event must name the revision its own decision produced"
    );
    assert!(
        !marked(&snapshot, "gamma"),
        "a parallel-ineligible target stayed marked"
    );
    assert!(
        marked(&snapshot, "beta"),
        "an eligible target lost its mark to another row's cleanup"
    );
    assert_eq!(
        wired.marks.marked_ids(),
        vec!["beta".to_string()],
        "the rejected marker row kept its shared mark"
    );

    // Repeating the same observation decides the same thing and publishes no
    // further revision.
    wired
        .dispatch(refresh(
            &["beta", "gamma"],
            &["alpha"],
            &["beta", "gamma"],
            &["gamma"],
        ))
        .await;
    let (_, repeated) = wired.published();
    assert_eq!(
        repeated, revision,
        "a repeated refresh cleanup must be idempotent"
    );
    assert_eq!(wired.marks.marked_ids(), vec!["beta".to_string()]);
}

/// The first `on_merged` hook failure publishes the recovery row and the cleared
/// mark together; a replay after an explicit re-mark keeps the fresh intent.
#[tokio::test]
async fn on_merged_recovery_and_cleared_mark_are_coherent() {
    let wired = Wired::new(&["alpha"]).await;
    wired.marks.set("alpha", true);

    let hook_failure = ExecutionEvent::HookFailed {
        change_id: "alpha".to_string(),
        hook_type: crate::hooks::HookType::OnMerged.config_key().to_string(),
        error: "publish script exited 1".to_string(),
    };

    wired.dispatch(hook_failure.clone()).await;
    let (snapshot, revision) = wired.published();
    assert_eq!(wired.envelope_revision("hook_failed"), revision);
    assert!(!marked(&snapshot, "alpha"));
    assert_eq!(
        snapshot
            .changes
            .iter()
            .find(|change| change.id == "alpha")
            .map(|change| change.display_status.as_str()),
        Some("merge wait"),
        "the recovery row and the cleared mark must land together"
    );

    // An operator re-marks the steady recovery row; the replayed hook failure
    // creates no new reducer edge, so the fresh intent survives.
    wired.marks.set("alpha", true);
    wired.dispatch(hook_failure).await;
    let (after, _) = wired.published();
    assert!(
        marked(&after, "alpha"),
        "a duplicate hook failure discarded a fresh re-mark"
    );
}

/// The archive edge publishes the cleared mark in the revision its own envelope
/// names, and no post-archive transition can bring the mark back.
///
/// Archive is the terminal revocation edge, and it is the one edge classified
/// from a run-level aggregate (`is_archived`) rather than per-change runtime, so
/// the ordering claim is asserted here at the real dispatch boundary instead of
/// being inferred from the reducer-level edge tests.
#[tokio::test]
async fn run_mark_intent_archive_revision_publishes_the_cleared_mark() {
    let wired = Wired::new(&["alpha", "beta"]).await;
    wired.marks.set("alpha", true);
    wired.marks.set("beta", true);

    // Establish a baseline revision that still reports both marks.
    wired
        .dispatch(ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "apply".to_string(),
        })
        .await;
    let (before, before_revision) = wired.published();
    assert!(marked(&before, "alpha") && marked(&before, "beta"));

    wired
        .dispatch(ExecutionEvent::ChangeArchived("alpha".to_string()))
        .await;

    let (after, after_revision) = wired.published();
    assert!(
        after_revision > before_revision,
        "the archive must advance the published revision"
    );
    assert_eq!(
        wired.envelope_revision("change_archived"),
        after_revision,
        "the archive event must name the revision that already reports the cleared mark"
    );
    assert!(
        !marked(&after, "alpha"),
        "the archive revision still exposes the mark it revoked"
    );
    assert!(
        marked(&after, "beta"),
        "an unrelated target lost its mark to another row's archive"
    );
    assert_eq!(
        after
            .changes
            .iter()
            .find(|change| change.id == "alpha")
            .and_then(|change| change.latest_activity.as_ref())
            .map(|activity| activity.event_type.as_str()),
        Some("change_archived"),
        "the same revision must carry the reducer transition"
    );

    // Post-archive success transitions carry no revoking edge of their own, and
    // must not resurrect the mark in any later published revision either.
    for event in [
        ExecutionEvent::MergeCompleted {
            change_id: "alpha".to_string(),
            revision: "rev".to_string(),
        },
        ExecutionEvent::PushCompleted {
            change_id: "alpha".to_string(),
            remote: "origin".to_string(),
            branch: "cflx/alpha".to_string(),
        },
    ] {
        wired.dispatch(event.clone()).await;
        let (snapshot, _) = wired.published();
        assert!(
            !marked(&snapshot, "alpha"),
            "{event:?} recreated the mark the archive edge revoked"
        );
        assert!(
            marked(&snapshot, "beta"),
            "{event:?} disturbed an unrelated mark"
        );
    }
    assert_eq!(wired.marks.marked_ids(), vec!["beta".to_string()]);
}

/// Process-level `Stopped` publishes a revision that still names every resume
/// target.
#[tokio::test]
async fn process_stop_revision_retains_marked_resume_targets() {
    let wired = Wired::new(&["alpha", "beta"]).await;
    wired.marks.set("alpha", true);
    wired.marks.set("beta", true);

    wired
        .dispatch(ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "apply".to_string(),
        })
        .await;
    wired.dispatch(ExecutionEvent::Stopped).await;

    let (snapshot, _) = wired.published();
    assert!(
        marked(&snapshot, "alpha") && marked(&snapshot, "beta"),
        "a process stop must publish the same marked resume target set"
    );
}
