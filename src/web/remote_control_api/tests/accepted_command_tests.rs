//! Accepted-command serialization, exact revision capture, and two-phase liveness.
//!
//! Everything else in this directory verifies the API's own obligations against
//! a recording executor. These verify the obligations that only exist once the
//! *real* application transaction is behind the endpoint:
//!
//! - two new commands carrying one expected revision cannot both execute;
//! - a settled record carries the revision its own outcome dispatch produced,
//!   not whatever the process reached afterwards;
//! - a stop-and-dequeue waiting for confirmed termination monopolizes nothing.
//!
//! Integration-scoped: a real reducer, real services, a real coordinator, a real
//! router, and the recording scheduler. No process, repository, or network.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::json;

use crate::orchestration::operator_command::{
    ExecutionMarkStore, OperatorCommandService, ParallelRuntime, QueuePort, TerminationWaiter,
};
use crate::orchestration::operator_coordinator::{CoreMode, OperatorApplication};
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService};
use crate::orchestration::state::OrchestratorState;
use crate::web::remote_control_api::auth::RemoteControlAuth;
use crate::web::remote_control_api::executor::SharedServiceExecutor;
use crate::web::remote_control_api::projection::Projection;
use crate::web::remote_control_api::{router, RemoteControlState};
use crate::web::state::WebState;

use super::{post_json, send, status_and_json};

/// A minimal listing row for the monitoring snapshot the projection is built from.
fn change(id: &str) -> crate::openspec::Change {
    crate::openspec::Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: crate::openspec::ProposalMetadata::default(),
    }
}

/// How long a scripted termination is allowed to take before the command fails.
///
/// Two constraints pin this value. It must exceed the endpoint's synchronous
/// grace window, or the command would settle before the response is written and
/// the pending state these tests are about would never be observable. And it
/// must stay small enough that a default-suite test does not sit out a real
/// 30-second cancellation deadline.
const TERMINATION_TIMEOUT: Duration = Duration::from_millis(400);

/// How long an *unrelated* command may take while a termination is pending.
///
/// Comfortably below `TERMINATION_TIMEOUT`: the claim is that the second command
/// does not queue behind the first, and a bound that overlapped the wait could
/// not distinguish the two.
const UNRELATED_COMMAND_BUDGET: Duration = Duration::from_millis(150);

// ============================================================================
// Scripted runtime
// ============================================================================

/// A queue port whose cancellation outcome each change is scripted per test.
///
/// The interesting case is a waiter that never completes: it stands for a task
/// that has been asked to stop and has not confirmed, which is exactly the
/// window during which everything else must remain admissible.
#[derive(Default)]
struct ScriptedQueue {
    waiters: Mutex<HashMap<String, TerminationWaiter>>,
    cancellations: Mutex<Vec<String>>,
}

impl ScriptedQueue {
    /// Cancelling `change_id` returns a waiter that never completes.
    fn never_terminates(&self, change_id: &str) {
        self.waiters
            .lock()
            .unwrap()
            .insert(change_id.to_string(), TerminationWaiter::never());
    }

    /// Every cancellation request, in order.
    fn cancellations(&self) -> Vec<String> {
        self.cancellations.lock().unwrap().clone()
    }
}

#[async_trait]
impl QueuePort for ScriptedQueue {
    async fn add(&self, _change_id: &str) -> bool {
        true
    }

    async fn remove(&self, _change_id: &str) -> bool {
        true
    }

    async fn request_cancellation(
        &self,
        change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        self.cancellations
            .lock()
            .unwrap()
            .push(change_id.to_string());
        Ok(Some(
            self.waiters
                .lock()
                .unwrap()
                .get(change_id)
                .cloned()
                .unwrap_or_else(TerminationWaiter::already_terminated),
        ))
    }

    async fn notify_scheduler(&self) {}
}

// ============================================================================
// Harness
// ============================================================================

/// A router wired to the real application transaction.
struct Wired {
    router: axum::Router,
    projection: Arc<Projection>,
    web_state: Arc<WebState>,
    reducer: Arc<tokio::sync::RwLock<OrchestratorState>>,
    marks: Arc<ExecutionMarkStore>,
    core_mode: Arc<CoreMode>,
    queue: Arc<ScriptedQueue>,
    scheduler: Arc<RecordingScheduler>,
    application: Arc<OperatorApplication>,
}

impl Wired {
    async fn new(change_ids: &[&str]) -> Self {
        let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            change_ids.iter().map(|id| id.to_string()).collect(),
            10,
        )));
        let marks = Arc::new(ExecutionMarkStore::new());
        let parallel = Arc::new(ParallelRuntime::new());
        let queue = Arc::new(ScriptedQueue::default());
        let scheduler = Arc::new(RecordingScheduler::new());
        let service = Arc::new(
            OperatorCommandService::new(
                reducer.clone(),
                queue.clone(),
                Arc::new(crate::orchestration::operator_command::NoopQueueHooks),
                marks.clone(),
            )
            .with_parallel(parallel.clone())
            .with_cancellation_timeout(TERMINATION_TIMEOUT),
        );
        let run_control = Arc::new(RunControlService::new(
            reducer.clone(),
            service,
            scheduler.clone(),
            Arc::new(ResolveReservations::new()),
            parallel,
        ));

        let web_state = Arc::new(WebState::new(&[]));
        web_state.set_shared_state(reducer.clone()).await;
        web_state.set_execution_marks(marks.clone()).await;
        let changes: Vec<_> = change_ids.iter().map(|id| change(id)).collect();
        web_state.update_with_mode(&changes, "select").await;
        web_state.sync_remote_control_projection().await;

        let core_mode = Arc::new(CoreMode::new());
        let (executor, application) = crate::web::remote_control_api::executor::wired_for_test(
            reducer.clone(),
            run_control,
            web_state.clone(),
            core_mode.clone(),
        );

        let runtime = web_state.remote_control();
        runtime.bind(Arc::new(executor)).await;
        runtime.bind_gate(application.gate()).await;
        let projection = runtime.projection();
        let auth = RemoteControlAuth::new(None, &[]).expect("no-token auth is valid");
        let router = router(
            RemoteControlState::new(projection.clone(), Arc::new(auth), runtime.clone())
                .with_gate(runtime.gate()),
        );

        Self {
            router,
            projection,
            web_state,
            reducer,
            marks,
            core_mode,
            queue,
            scheduler,
            application,
        }
    }

    fn revision(&self) -> u64 {
        self.projection.revision()
    }

    async fn submit(&self, command: serde_json::Value, revision: u64, key: &str) -> ResponseFacts {
        let mut object = command.as_object().unwrap().clone();
        object.insert("expected_revision".to_string(), json!(revision));
        object.insert("idempotency_key".to_string(), json!(key));
        let body = serde_json::Value::Object(object).to_string();
        let (status, json) =
            status_and_json(send(&self.router, post_json("/api/v2/commands", None, &body)).await)
                .await;
        ResponseFacts { status, json }
    }
}

/// The parts of a command response every test here asserts on.
struct ResponseFacts {
    status: StatusCode,
    json: serde_json::Value,
}

impl ResponseFacts {
    fn state(&self) -> &str {
        self.json["state"].as_str().unwrap_or_default()
    }

    fn result_revision(&self) -> Option<u64> {
        self.json["result_revision"].as_u64()
    }

    fn error_code(&self) -> &str {
        self.json["error_code"].as_str().unwrap_or_default()
    }

    fn command_id(&self) -> String {
        self.json["command_id"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
}

// ============================================================================
// Revision control
// ============================================================================

/// A changed command records the revision its own outcome dispatch produced,
/// and later progress advances the projection without rewriting it.
#[tokio::test]
async fn accepted_command_revision_records_the_outcome_dispatch() {
    let wired = Wired::new(&["c1"]).await;
    let admitted = wired.revision();

    let response = wired
        .submit(
            json!({"type": "set_execution_mark", "change_id": "c1", "marked": true}),
            admitted,
            "mark-c1",
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.state(), "succeeded");
    let recorded = response
        .result_revision()
        .expect("a changed command records a revision");
    assert!(
        recorded > admitted,
        "an accepted mark must advance the revision it is recorded at ({recorded} vs {admitted})"
    );
    assert_eq!(
        recorded,
        wired.revision(),
        "the recorded revision is the one the outcome produced"
    );

    // Unrelated later progress moves the projection on. The settled record must
    // not follow it: `result_revision` identifies the snapshot containing this
    // command's decision fields, not the newest one.
    wired
        .web_state
        .apply_execution_event(&crate::events::ExecutionEvent::ProcessingStarted(
            "c1".to_string(),
        ))
        .await;
    assert!(wired.revision() > recorded, "later progress must advance");

    let replay = wired
        .submit(
            json!({"type": "set_execution_mark", "change_id": "c1", "marked": true}),
            admitted,
            "mark-c1",
        )
        .await;
    assert_eq!(
        replay.result_revision(),
        Some(recorded),
        "an exact replay returns the originally stored revision"
    );
    assert_eq!(replay.command_id(), response.command_id());
}

/// A no-op and an ordinary failure both settle at the unchanged admitted revision.
#[tokio::test]
async fn accepted_command_revision_no_op_and_failure_keep_the_admitted_revision() {
    let wired = Wired::new(&["c1"]).await;
    let admitted = wired.revision();

    // The mark already has the requested value, so the command is valid and
    // changes nothing.
    let no_op = wired
        .submit(
            json!({"type": "set_execution_mark", "change_id": "c1", "marked": false}),
            admitted,
            "noop-c1",
        )
        .await;
    assert_eq!(no_op.state(), "no_op");
    assert_eq!(no_op.result_revision(), Some(admitted));
    assert_eq!(
        wired.revision(),
        admitted,
        "a no-op must not advance the revision"
    );

    // Stop is refused for the mode, before any effect exists.
    let failure = wired
        .submit(json!({"type": "stop"}), admitted, "stop-invalid")
        .await;
    assert_eq!(failure.state(), "failed");
    assert_eq!(failure.error_code(), "lifecycle_conflict");
    assert_eq!(
        failure.result_revision(),
        Some(admitted),
        "an ordinary failure has no effect and settles at the unchanged revision"
    );
    assert_eq!(wired.revision(), admitted);
}

/// Two new identities at one expected revision cannot both execute.
#[tokio::test]
async fn accepted_command_revision_rejects_a_second_command_at_one_revision() {
    let wired = Wired::new(&["c1", "c2"]).await;
    let admitted = wired.revision();

    let first = wired
        .submit(
            json!({"type": "set_execution_mark", "change_id": "c1", "marked": true}),
            admitted,
            "first",
        )
        .await;
    assert_eq!(first.state(), "succeeded");

    let second = wired
        .submit(
            json!({"type": "set_execution_mark", "change_id": "c2", "marked": true}),
            admitted,
            "second",
        )
        .await;
    assert_eq!(second.status, StatusCode::CONFLICT);
    assert_eq!(second.json["error_code"], "stale_revision");
    assert!(
        !wired.marks.is_marked("c2"),
        "the stale command must be refused before any service execution"
    );
}

/// A reused key bound to a different typed identity conflicts without effect.
#[tokio::test]
async fn accepted_command_revision_mismatched_identity_conflicts() {
    let wired = Wired::new(&["c1", "c2"]).await;
    let admitted = wired.revision();

    let first = wired
        .submit(
            json!({"type": "set_execution_mark", "change_id": "c1", "marked": true}),
            admitted,
            "shared-key",
        )
        .await;
    assert_eq!(first.state(), "succeeded");

    let mismatch = wired
        .submit(
            json!({"type": "set_execution_mark", "change_id": "c2", "marked": true}),
            admitted,
            "shared-key",
        )
        .await;
    assert_eq!(mismatch.status, StatusCode::CONFLICT);
    assert_eq!(mismatch.json["error_code"], "idempotency_mismatch");
    assert!(!wired.marks.is_marked("c2"));
}

/// A two-phase timeout commits no dequeue state and settles at the revision
/// observed under the reacquired boundary.
#[tokio::test]
async fn accepted_command_revision_two_phase_timeout_certifies_no_dequeue() {
    let wired = Wired::new(&["c1"]).await;
    wired.queue.never_terminates("c1");
    let admitted = wired.revision();

    let response = wired
        .submit(
            json!({"type": "stop_and_dequeue", "change_id": "c1"}),
            admitted,
            "dequeue-c1",
        )
        .await;

    // The endpoint answers before the wait finishes; the record settles later.
    let command_id = response.command_id();
    let settled = await_settlement(&wired, &command_id).await;

    assert_eq!(settled["state"], "failed");
    assert_eq!(settled["error_code"], "root_busy");
    assert_eq!(
        settled["result_revision"].as_u64(),
        Some(wired.revision()),
        "a post-wait refusal stores the explicit unchanged settlement revision"
    );
    assert_eq!(
        wired.queue.cancellations(),
        vec!["c1".to_string()],
        "cancellation is issued exactly once and never repeated"
    );
    assert_eq!(
        wired.reducer.read().await.display_status("c1"),
        "not queued",
        "a timed-out dequeue commits no reducer mutation"
    );
}

// ============================================================================
// Two-phase liveness
// ============================================================================

/// A pending stop-and-dequeue monopolizes neither admission nor event fan-out.
///
/// The waiter never completes, so if the confirmation wait were held inside the
/// application gate — or inside the endpoint's critical section — the unrelated
/// command below would sit behind it until the timeout expired.
#[tokio::test]
async fn accepted_operator_command_dequeue_liveness_keeps_the_process_admissible() {
    let wired = Wired::new(&["c1", "c2"]).await;
    wired.queue.never_terminates("c1");
    let admitted = wired.revision();

    let pending = wired
        .submit(
            json!({"type": "stop_and_dequeue", "change_id": "c1"}),
            admitted,
            "dequeue-c1",
        )
        .await;
    assert_eq!(
        pending.status,
        StatusCode::ACCEPTED,
        "a command awaiting confirmed termination stays Running: {:?}",
        pending.json
    );
    assert_eq!(pending.state(), "running");

    // An unrelated command must execute and settle while the wait is pending.
    let started = std::time::Instant::now();
    let unrelated = wired
        .submit(
            json!({"type": "set_execution_mark", "change_id": "c2", "marked": true}),
            wired.revision(),
            "mark-c2",
        )
        .await;
    let elapsed = started.elapsed();
    assert_eq!(
        unrelated.state(),
        "succeeded",
        "an unrelated command must not wait for the dequeue timeout: {:?}",
        unrelated.json
    );
    assert!(
        elapsed < UNRELATED_COMMAND_BUDGET,
        "an unrelated command queued behind the pending termination ({elapsed:?})"
    );
    assert!(wired.marks.is_marked("c2"));

    // Force stop, specifically: the operator's escape hatch cannot be the thing
    // a stuck termination blocks.
    wired.scheduler.set_running(true);
    wired.core_mode.set(
        crate::orchestration::operator_command::OperatorMode::Running,
    );
    let force_stop = wired
        .submit(json!({"type": "force_stop"}), wired.revision(), "force")
        .await;
    assert_eq!(
        force_stop.state(),
        "succeeded",
        "force stop must remain admissible: {:?}",
        force_stop.json
    );

    // Event fan-out is live too: an unrelated dispatch still reaches the
    // projection while the waiter is stuck.
    let before = wired.revision();
    wired
        .application
        .run_control()
        .operator()
        .apply_execution_mark("c1", true)
        .await;
    wired.web_state.sync_remote_control_projection().await;
    assert!(
        wired.revision() > before,
        "the projection must keep advancing while a termination is pending"
    );

    // And the pending record eventually settles on its own timeout.
    let settled = await_settlement(&wired, &pending.command_id()).await;
    assert_eq!(settled["state"], "failed");
    assert_eq!(
        wired.queue.cancellations(),
        vec!["c1".to_string()],
        "the waiter is issued one cancellation, not one per poll"
    );
}

/// An exact replay of a pending two-phase command joins the original record.
#[tokio::test]
async fn accepted_operator_command_dequeue_liveness_replay_issues_no_second_cancellation() {
    let wired = Wired::new(&["c1"]).await;
    wired.queue.never_terminates("c1");
    let admitted = wired.revision();

    let first = wired
        .submit(
            json!({"type": "stop_and_dequeue", "change_id": "c1"}),
            admitted,
            "dequeue-c1",
        )
        .await;
    assert_eq!(first.state(), "running");

    let replay = wired
        .submit(
            json!({"type": "stop_and_dequeue", "change_id": "c1"}),
            admitted,
            "dequeue-c1",
        )
        .await;

    assert_eq!(
        replay.command_id(),
        first.command_id(),
        "replay must join the in-progress record"
    );
    assert_eq!(
        wired.queue.cancellations(),
        vec!["c1".to_string()],
        "replay must not issue a second cancellation or start a second waiter"
    );

    await_settlement(&wired, &first.command_id()).await;
}

/// Poll a command record until it leaves the Running state.
///
/// Bounded well above the scripted termination timeout so a slow machine cannot
/// turn a real settlement into a flake, and returns as soon as the record moves.
async fn await_settlement(wired: &Wired, command_id: &str) -> serde_json::Value {
    let deadline = TERMINATION_TIMEOUT * 40;
    let started = std::time::Instant::now();
    loop {
        let record = wired
            .projection
            .command(command_id)
            .expect("the record was reserved");
        if !matches!(
            record.state,
            crate::web::remote_control_api::dto::CommandState::Running
        ) {
            return serde_json::to_value(record).expect("a command record serializes");
        }
        assert!(
            started.elapsed() < deadline,
            "the command never settled within {deadline:?}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
