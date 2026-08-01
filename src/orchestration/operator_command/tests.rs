//! Tests for the shared operator command service.
//!
//! Evidence classes used here:
//! - unit: lifecycle matrix, mark store, and reducer routing with in-memory doubles
//! - integration: real `DynamicQueue`, real tokio tasks, real cancellation tokens

use super::*;
use crate::events::{ExecutionEvent, StalledBlocker};
use crate::orchestration::state::{ExecutionMode, OrchestratorState};
use crate::tui::queue::DynamicQueue;
use std::sync::Mutex as StdMutex;

// ============================================================================
// Fixtures
// ============================================================================

fn shared_state(ids: &[&str]) -> Arc<RwLock<OrchestratorState>> {
    Arc::new(RwLock::new(OrchestratorState::with_mode(
        ids.iter().map(|id| id.to_string()).collect(),
        10,
        ExecutionMode::Parallel,
    )))
}

fn acceptance_blocker() -> StalledBlocker {
    StalledBlocker {
        category: "acceptance_finding".to_string(),
        phase: "acceptance".to_string(),
        gate: "acceptance".to_string(),
        error_summary: "unresolved acceptance finding".to_string(),
        evidence: vec!["tests/acceptance.rs:1".to_string()],
        // Repository-fixable facts with no unblock condition: the classifier
        // keeps this an execution stall rather than an external wait.
        unblock_condition: None,
        prerequisite_owner: None,
        next_action: "resolve finding and retry".to_string(),
        resumable: true,
        worktree_preserved: true,
    }
}

/// Reported facts that validate as a non-repository prerequisite, which the
/// orchestrator classifies as an external `blocked` wait.
fn external_prerequisite_blocker() -> StalledBlocker {
    StalledBlocker {
        category: "credential".to_string(),
        phase: "acceptance".to_string(),
        gate: "acceptance".to_string(),
        error_summary: "STAGING_API_KEY is unset".to_string(),
        evidence: vec!["verification run reported STAGING_API_KEY is unset".to_string()],
        unblock_condition: Some(
            "STAGING_API_KEY is present in the verification environment".to_string(),
        ),
        prerequisite_owner: Some("platform".to_string()),
        next_action: "provision STAGING_API_KEY then retry acceptance".to_string(),
        resumable: true,
        worktree_preserved: true,
    }
}

/// Hook port double that records dispatch cardinality.
#[derive(Default)]
struct RecordingHooks {
    adds: StdMutex<Vec<String>>,
    removes: StdMutex<Vec<String>>,
}

impl RecordingHooks {
    fn adds(&self) -> Vec<String> {
        self.adds.lock().expect("adds lock").clone()
    }

    fn removes(&self) -> Vec<String> {
        self.removes.lock().expect("removes lock").clone()
    }
}

#[async_trait]
impl QueueHookPort for RecordingHooks {
    async fn on_queue_add(&self, change_id: &str) {
        self.adds
            .lock()
            .expect("adds lock")
            .push(change_id.to_string());
    }

    async fn on_queue_remove(&self, change_id: &str) {
        self.removes
            .lock()
            .expect("removes lock")
            .push(change_id.to_string());
    }
}

/// How a queue double answers cancellation requests.
#[derive(Clone)]
enum CancellationBehavior {
    /// A handle exists and termination is confirmed immediately.
    ConfirmedImmediately,
    /// A handle exists but termination is never confirmed.
    NeverTerminates,
    /// No handle is registered.
    NoHandle,
    /// Issuing cancellation itself failed.
    Fails(String),
}

/// In-memory queue double for unit-scoped service tests.
struct FakeQueue {
    contents: StdMutex<Vec<String>>,
    cancellation: CancellationBehavior,
    notified: StdMutex<usize>,
}

impl FakeQueue {
    fn new(cancellation: CancellationBehavior) -> Self {
        Self {
            contents: StdMutex::new(Vec::new()),
            cancellation,
            notified: StdMutex::new(0),
        }
    }
}

#[async_trait]
impl QueuePort for FakeQueue {
    async fn add(&self, change_id: &str) -> bool {
        let mut contents = self.contents.lock().expect("contents lock");
        if contents.iter().any(|id| id == change_id) {
            return false;
        }
        contents.push(change_id.to_string());
        true
    }

    async fn remove(&self, change_id: &str) -> bool {
        let mut contents = self.contents.lock().expect("contents lock");
        let before = contents.len();
        contents.retain(|id| id != change_id);
        contents.len() != before
    }

    async fn request_cancellation(
        &self,
        _change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        match &self.cancellation {
            CancellationBehavior::ConfirmedImmediately => {
                Ok(Some(TerminationWaiter::already_terminated()))
            }
            CancellationBehavior::NeverTerminates => Ok(Some(TerminationWaiter::never())),
            CancellationBehavior::NoHandle => Ok(None),
            CancellationBehavior::Fails(message) => Err(message.clone()),
        }
    }

    async fn notify_scheduler(&self) {
        *self.notified.lock().expect("notified lock") += 1;
    }
}

struct Fixture {
    service: OperatorCommandService,
    state: Arc<RwLock<OrchestratorState>>,
    hooks: Arc<RecordingHooks>,
    queue: Arc<FakeQueue>,
    marks: Arc<ExecutionMarkStore>,
}

fn fixture(ids: &[&str]) -> Fixture {
    fixture_with_cancellation(ids, CancellationBehavior::ConfirmedImmediately)
}

fn fixture_with_cancellation(ids: &[&str], cancellation: CancellationBehavior) -> Fixture {
    let state = shared_state(ids);
    let hooks = Arc::new(RecordingHooks::default());
    let queue = Arc::new(FakeQueue::new(cancellation));
    let marks = Arc::new(ExecutionMarkStore::new());
    let service =
        OperatorCommandService::new(state.clone(), queue.clone(), hooks.clone(), marks.clone())
            .with_cancellation_timeout(Duration::from_millis(50));
    Fixture {
        service,
        state,
        hooks,
        queue,
        marks,
    }
}

/// Fixture backed by the real `DynamicQueue` (integration evidence).
fn dynamic_queue_fixture(
    ids: &[&str],
) -> (
    OperatorCommandService,
    Arc<RwLock<OrchestratorState>>,
    Arc<RecordingHooks>,
    DynamicQueue,
) {
    let state = shared_state(ids);
    let hooks = Arc::new(RecordingHooks::default());
    let queue = DynamicQueue::new();
    let service = OperatorCommandService::new(
        state.clone(),
        Arc::new(queue.clone()),
        hooks.clone(),
        Arc::new(ExecutionMarkStore::new()),
    )
    .with_cancellation_timeout(Duration::from_millis(500));
    (service, state, hooks, queue)
}

// ============================================================================
// Task 1 / Task 5: lifecycle matrix and state-axis separation (unit)
// ============================================================================

#[test]
fn operator_command_mark_route_matrix_matches_mode_and_status() {
    // Select mode: marks are pure operator intent.
    assert_eq!(
        classify_mark_route(OperatorMode::Select, "not queued"),
        MarkRoute::MarkOnly
    );
    assert_eq!(
        classify_mark_route(OperatorMode::Select, "applying"),
        MarkRoute::MarkOnly
    );

    // Stopped mode: mark-only for resumable rows.
    for status in ["not queued", "error", "merge wait", "resolve pending"] {
        assert_eq!(
            classify_mark_route(OperatorMode::Stopped, status),
            MarkRoute::MarkOnly,
            "stopped mode must mutate marks for '{status}'"
        );
    }
    assert_eq!(
        classify_mark_route(OperatorMode::Stopped, "applying"),
        MarkRoute::Immutable
    );

    // Running mode: ordinary rows use queue intent.
    for status in ["not queued", "queued", "error"] {
        assert_eq!(
            classify_mark_route(OperatorMode::Running, status),
            MarkRoute::QueueIntent,
            "running mode must use queue intent for '{status}'"
        );
    }
    // Running mode: base-lane waits are mark-only.
    for status in ["merge wait", "resolve pending"] {
        assert_eq!(
            classify_mark_route(OperatorMode::Running, status),
            MarkRoute::MarkOnly,
            "running mode must allow mark-only mutation for '{status}'"
        );
    }
    // Running mode: active rows are stopped through stop-and-dequeue.
    for status in [
        "applying",
        "accepting",
        "rejecting",
        "archiving",
        "resolving",
    ] {
        assert_eq!(
            classify_mark_route(OperatorMode::Running, status),
            MarkRoute::Immutable,
            "active row '{status}' must not be mutated by marks"
        );
    }

    // Error mode always requires an explicit retry command.
    for status in ["not queued", "queued", "error", "stalled", "merge wait"] {
        assert_eq!(
            classify_mark_route(OperatorMode::Error, status),
            MarkRoute::RetryRequired,
            "error mode must require retry for '{status}'"
        );
    }

    // Final outcomes are immutable in every mode.
    for mode in [
        OperatorMode::Select,
        OperatorMode::Running,
        OperatorMode::Stopping,
        OperatorMode::Stopped,
        OperatorMode::Error,
    ] {
        for status in ["archived", "merged", "pushed", "rejected"] {
            assert_eq!(
                classify_mark_route(mode, status),
                MarkRoute::Immutable,
                "final status '{status}' must be immutable in {mode:?}"
            );
        }
    }
}

#[test]
fn operator_command_service_never_exposes_a_gated_display_status() {
    let source = include_str!("../operator_command.rs");
    assert!(
        !source.contains("\"gated\""),
        "the operator command service must not introduce a 'gated' display status"
    );
}

#[tokio::test]
async fn operator_command_error_mode_rejects_mark_mutation_without_side_effects() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "change-a".to_string(),
            error: "boom".to_string(),
        });
    }

    let error = fixture
        .service
        .set_execution_mark(OperatorMode::Error, "change-a", true)
        .await
        .expect_err("error mode must reject mark mutation");

    assert!(matches!(
        error,
        OperatorCommandError::MarkNotAllowed {
            route: MarkRoute::RetryRequired,
            ..
        }
    ));
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "error",
        "rejected mark mutation must not change reducer state"
    );
    assert!(
        !fixture.marks.is_marked("change-a"),
        "rejected mark mutation must not change the mark store"
    );
    assert!(
        fixture.hooks.adds().is_empty() && fixture.hooks.removes().is_empty(),
        "rejected requests must not run queue hooks"
    );
}

#[tokio::test]
async fn operator_command_active_rows_reject_mark_mutation_in_running_mode() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "change-a".to_string(),
            command: "apply".to_string(),
        });
    }

    let error = fixture
        .service
        .set_execution_mark(OperatorMode::Running, "change-a", false)
        .await
        .expect_err("active rows must reject mark mutation");

    assert!(matches!(
        error,
        OperatorCommandError::MarkNotAllowed {
            route: MarkRoute::Immutable,
            ..
        }
    ));
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "applying"
    );
}

#[tokio::test]
async fn operator_command_mark_only_route_leaves_queue_intent_untouched() {
    let fixture = fixture(&["change-a"]);
    let before = fixture.state.read().await.display_status("change-a");

    let outcome = fixture
        .service
        .set_execution_mark(OperatorMode::Select, "change-a", true)
        .await
        .expect("select mode must mutate marks");

    assert_eq!(
        outcome,
        OperatorOutcome::MarkSet {
            change_id: "change-a".to_string(),
            marked: true,
        }
    );
    assert!(fixture.marks.is_marked("change-a"));
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        before,
        "an execution mark must not change queue intent or display status"
    );

    // Repeating the same mark is a no-op, not a second mutation.
    let repeated = fixture
        .service
        .set_execution_mark(OperatorMode::Select, "change-a", true)
        .await
        .expect("repeat mark must succeed");
    assert_eq!(
        repeated,
        OperatorOutcome::NoOp {
            change_id: "change-a".to_string(),
            reason: NoOpReason::MarkUnchanged,
        }
    );
}

#[tokio::test]
async fn operator_command_running_mark_routes_to_queue_intent() {
    let fixture = fixture(&["change-a"]);

    let outcome = fixture
        .service
        .set_execution_mark(OperatorMode::Running, "change-a", true)
        .await
        .expect("running mode add must succeed");

    let OperatorOutcome::Queue(queue_outcome) = outcome else {
        panic!("running-mode mark must route to queue intent");
    };
    assert_eq!(queue_outcome.mutation, QueueMutation::Added);
    assert!(queue_outcome.reducer_changed);
    assert!(queue_outcome.dynamic_queue_mutated);
    assert_eq!(queue_outcome.display_status, "queued");
    assert!(fixture.marks.is_marked("change-a"));
    assert_eq!(fixture.hooks.adds(), vec!["change-a".to_string()]);
    assert_eq!(
        *fixture.queue.notified.lock().expect("notified"),
        1,
        "a real addition must wake the scheduler exactly once"
    );
}

#[tokio::test]
async fn operator_command_typed_dispatch_matches_direct_calls() {
    let direct = fixture(&["change-a"]);
    let dispatched = fixture(&["change-a"]);

    let direct_outcome = direct
        .service
        .add_to_queue("change-a")
        .await
        .expect("direct add");
    let dispatched_outcome = dispatched
        .service
        .execute(
            OperatorMode::Running,
            OperatorCommand::AddToQueue {
                change_id: "change-a".to_string(),
            },
        )
        .await
        .expect("dispatched add");

    assert_eq!(
        OperatorOutcome::Queue(direct_outcome),
        dispatched_outcome,
        "typed command dispatch must equal the direct service call"
    );
    assert_eq!(direct.hooks.adds(), dispatched.hooks.adds());
    assert_eq!(
        direct.state.read().await.display_status("change-a"),
        dispatched.state.read().await.display_status("change-a")
    );
}

// ============================================================================
// Task 5: dependency-blocked queue intent (unit)
// ============================================================================

#[tokio::test]
async fn operator_command_dependency_blocked_addition_keeps_queue_intent() {
    let fixture = fixture(&["change-a"]);

    let outcome = fixture
        .service
        .add_to_queue("change-a")
        .await
        .expect("dependency-ineligible addition must not be rejected");
    assert!(outcome.reducer_changed, "queue intent must be accepted");
    assert_eq!(outcome.display_status, "queued");

    // The scheduler later reports the dependency blocker.
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::DependencyBlocked {
            change_id: "change-a".to_string(),
            dependency_ids: vec!["dep-a".to_string()],
        });
    }

    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "blocked",
        "a dependency-ineligible change must display 'blocked', never 'gated'"
    );

    // Queue intent survives the blocker: resolving it restores 'queued'.
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::DependencyResolved {
            change_id: "change-a".to_string(),
        });
    }
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "queued",
        "queue intent must be retained across a dependency blocker"
    );
}

#[tokio::test]
async fn operator_command_stalled_hold_keeps_stalled_display_status() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "change-a".to_string(),
            blocker: acceptance_blocker(),
        });
    }

    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "stalled",
        "resumable non-dependency holds must remain 'stalled'"
    );
}

// ============================================================================
// Task 2: queue mutation and hook cardinality (integration: real DynamicQueue)
// ============================================================================

#[tokio::test]
async fn operator_command_queue_add_runs_hook_exactly_once() {
    let (service, _state, hooks, queue) = dynamic_queue_fixture(&["change-a"]);

    let outcome = service
        .add_to_queue("change-a")
        .await
        .expect("add succeeds");
    assert!(outcome.dynamic_queue_mutated);
    assert_eq!(hooks.adds(), vec!["change-a".to_string()]);
    assert!(queue.contains("change-a").await);
}

#[tokio::test]
async fn operator_command_duplicate_queue_add_is_a_noop_without_hook() {
    let (service, _state, hooks, _queue) = dynamic_queue_fixture(&["change-a"]);

    service.add_to_queue("change-a").await.expect("first add");
    let second = service.add_to_queue("change-a").await.expect("second add");

    assert!(
        !second.reducer_changed,
        "a duplicate addition must be a reducer no-op"
    );
    assert!(!second.dynamic_queue_mutated);
    assert_eq!(
        hooks.adds(),
        vec!["change-a".to_string()],
        "on_queue_add must run exactly once for one real mutation"
    );
}

#[tokio::test]
async fn operator_command_add_of_already_present_queue_entry_runs_no_hook() {
    let (service, _state, hooks, queue) = dynamic_queue_fixture(&["change-a"]);
    // Pre-populate the runtime queue so the push itself is a no-op even though
    // the reducer still has to record queue intent.
    queue.push("change-a".to_string()).await;

    let outcome = service
        .add_to_queue("change-a")
        .await
        .expect("add succeeds");

    assert!(
        outcome.reducer_changed,
        "queue intent must still be recorded"
    );
    assert!(
        !outcome.dynamic_queue_mutated,
        "an already-queued change is not a dynamic mutation"
    );
    assert!(
        hooks.adds().is_empty(),
        "no dynamic mutation means no on_queue_add"
    );
}

#[tokio::test]
async fn operator_command_queue_remove_runs_hook_exactly_once() {
    let (service, _state, hooks, queue) = dynamic_queue_fixture(&["change-a"]);
    service.add_to_queue("change-a").await.expect("add");

    let outcome = service
        .remove_from_queue("change-a")
        .await
        .expect("remove succeeds");

    assert!(outcome.dynamic_queue_mutated);
    assert_eq!(hooks.removes(), vec!["change-a".to_string()]);
    assert!(!queue.contains("change-a").await);
    assert_eq!(
        queue.drain_removed().await,
        vec!["change-a".to_string()],
        "removal must reach the scheduler pending set"
    );
}

#[tokio::test]
async fn operator_command_duplicate_queue_remove_runs_no_hook() {
    let (service, _state, hooks, _queue) = dynamic_queue_fixture(&["change-a"]);
    service.add_to_queue("change-a").await.expect("add");
    service.remove_from_queue("change-a").await.expect("remove");

    let second = service
        .remove_from_queue("change-a")
        .await
        .expect("second remove");

    assert!(!second.reducer_changed);
    assert!(!second.dynamic_queue_mutated);
    assert_eq!(
        hooks.removes(),
        vec!["change-a".to_string()],
        "on_queue_remove must not run for a no-op removal"
    );
}

#[tokio::test]
async fn operator_command_removal_of_never_queued_change_runs_no_hook() {
    let (service, _state, hooks, _queue) = dynamic_queue_fixture(&["change-a"]);

    let outcome = service
        .remove_from_queue("change-a")
        .await
        .expect("remove succeeds");

    assert!(!outcome.dynamic_queue_mutated);
    assert!(hooks.removes().is_empty());
}

#[test]
fn operator_command_service_is_the_only_queue_hook_dispatcher() {
    // Initial queue construction must not run queue hooks. The orchestrator and
    // scheduler sources therefore never reference the queue hook types at all:
    // only the shared service dispatches them.
    for (name, source) in [
        (
            "tui/orchestrator.rs",
            include_str!("../../tui/orchestrator.rs"),
        ),
        ("tui/runner.rs", include_str!("../../tui/runner.rs")),
        (
            "parallel/dispatch.rs",
            include_str!("../../parallel/dispatch.rs"),
        ),
        (
            "parallel/queue_state.rs",
            include_str!("../../parallel/queue_state.rs"),
        ),
        (
            "tui/command_handlers.rs",
            include_str!("../../tui/command_handlers.rs"),
        ),
    ] {
        assert!(
            !source.contains("OnQueueAdd") && !source.contains("OnQueueRemove"),
            "{name} must not dispatch queue hooks directly; the shared service owns them"
        );
    }

    let service_source = include_str!("../operator_command.rs");
    assert!(
        service_source.contains("HookType::OnQueueAdd")
            && service_source.contains("HookType::OnQueueRemove"),
        "the shared service must be the queue hook dispatcher"
    );
}

// ============================================================================
// Task 4: cancellation-first stop-and-dequeue
// ============================================================================

#[tokio::test]
async fn operator_command_stop_and_dequeue_waits_for_confirmed_termination() {
    // Integration: real DynamicQueue, real cancellation token, real tokio task.
    let (service, state, _hooks, queue) = dynamic_queue_fixture(&["change-a"]);
    {
        let mut guard = state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "change-a".to_string(),
            command: "apply".to_string(),
        });
    }

    let token = CancellationToken::new();
    queue
        .register_kill_token("change-a".to_string(), token.clone())
        .await;

    let task_queue = queue.clone();
    let terminated = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let task_terminated = terminated.clone();
    let worker = tokio::spawn(async move {
        token.cancelled().await;
        // Simulate executor teardown finishing before completion is reported.
        tokio::time::sleep(Duration::from_millis(20)).await;
        task_terminated.store(true, std::sync::atomic::Ordering::SeqCst);
        task_queue.unregister_kill_token("change-a").await;
    });

    let outcome = service
        .stop_and_dequeue("change-a")
        .await
        .expect("stop-and-dequeue must succeed once termination is confirmed");

    assert_eq!(
        outcome,
        OperatorOutcome::Dequeued {
            change_id: "change-a".to_string()
        }
    );
    assert!(
        terminated.load(std::sync::atomic::Ordering::SeqCst),
        "dequeue must be applied only after the task actually terminated"
    );
    assert_eq!(
        state.read().await.display_status("change-a"),
        "not queued",
        "confirmed termination must apply DequeueChange"
    );
    worker.await.expect("worker task");
}

#[tokio::test]
async fn operator_command_stop_and_dequeue_fails_without_cancellation_handle() {
    // Integration: real DynamicQueue with no registered handle for an active change.
    let (service, state, _hooks, _queue) = dynamic_queue_fixture(&["change-a"]);
    {
        let mut guard = state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "change-a".to_string(),
            command: "apply".to_string(),
        });
    }

    let error = service
        .stop_and_dequeue("change-a")
        .await
        .expect_err("an active change without a handle cannot be dequeued");

    assert_eq!(
        error,
        OperatorCommandError::MissingCancellationHandle {
            change_id: "change-a".to_string()
        }
    );
    assert_eq!(
        state.read().await.display_status("change-a"),
        "applying",
        "a failed stop must preserve active state"
    );
}

#[tokio::test]
async fn operator_command_stop_and_dequeue_fails_when_cancellation_errors() {
    let fixture = fixture_with_cancellation(
        &["change-a"],
        CancellationBehavior::Fails("cancel channel closed".to_string()),
    );
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "change-a".to_string(),
            command: "apply".to_string(),
        });
    }

    let error = fixture
        .service
        .stop_and_dequeue("change-a")
        .await
        .expect_err("a failed cancellation must not dequeue");

    assert_eq!(
        error,
        OperatorCommandError::CancellationFailed {
            change_id: "change-a".to_string(),
            message: "cancel channel closed".to_string(),
        }
    );
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "applying"
    );
}

#[tokio::test]
async fn operator_command_stop_and_dequeue_times_out_without_termination() {
    let fixture = fixture_with_cancellation(&["change-a"], CancellationBehavior::NeverTerminates);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "change-a".to_string(),
            command: "apply".to_string(),
        });
    }

    let error = fixture
        .service
        .stop_and_dequeue("change-a")
        .await
        .expect_err("unconfirmed termination must fail");

    assert!(matches!(
        error,
        OperatorCommandError::TerminationTimeout { .. }
    ));
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "applying",
        "a timeout must preserve active state"
    );
}

#[tokio::test]
async fn operator_command_stop_and_dequeue_of_idle_row_needs_no_handle() {
    let fixture = fixture_with_cancellation(&["change-a"], CancellationBehavior::NoHandle);
    fixture
        .service
        .add_to_queue("change-a")
        .await
        .expect("queue the change");

    let outcome = fixture
        .service
        .stop_and_dequeue("change-a")
        .await
        .expect("a queued but idle row can be dequeued directly");

    assert_eq!(
        outcome,
        OperatorOutcome::Dequeued {
            change_id: "change-a".to_string()
        }
    );
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "not queued"
    );
}

#[test]
fn operator_command_force_kill_result_is_not_treated_as_termination_proof() {
    let source = include_str!("../operator_command.rs");
    assert!(
        !source.contains("force_kill"),
        "the service must use the cancellation handshake, not force_kill's boolean"
    );
    let handlers = include_str!("../../tui/command_handlers.rs");
    assert!(
        !handlers.contains("force_kill"),
        "the TUI adapter must not treat force_kill's result as proof of termination"
    );
}

// ============================================================================
// Task 6: retry routing
// ============================================================================

#[tokio::test]
async fn operator_command_terminal_error_retry_uses_retry_error() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "change-a".to_string(),
            error: "boom".to_string(),
        });
    }
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "error"
    );

    let plan = fixture
        .service
        .retry_change("change-a")
        .await
        .expect("terminal error must be retryable");

    assert_eq!(plan.change_ids, vec!["change-a".to_string()]);
    assert_eq!(plan.routes, vec![RetryRoute::TerminalError]);
    assert!(plan.explicit_retry);
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "queued",
        "RetryError must restore ordinary queued dispatch eligibility"
    );
}

#[tokio::test]
async fn operator_command_acceptance_stalled_retry_requests_explicit_retry() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "change-a".to_string(),
            blocker: acceptance_blocker(),
        });
    }
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "stalled"
    );

    let plan = fixture
        .service
        .retry_change("change-a")
        .await
        .expect("acceptance stall must be retryable");

    assert_eq!(plan.routes, vec![RetryRoute::AcceptanceStall]);
    assert!(
        plan.explicit_retry,
        "acceptance-stalled retry must run with explicit-retry semantics so the \
         reconciled hold is consumed and acceptance resumes without rerunning apply"
    );
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "queued"
    );
}

/// Explicit retry is refused for a non-resumable Acceptance hold: the blocker
/// evidence the operator still owns must survive, and no ambiguous work may be
/// dispatched past it.
#[tokio::test]
async fn operator_command_retry_refuses_a_non_resumable_acceptance_stall() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "change-a".to_string(),
            blocker: StalledBlocker {
                resumable: false,
                ..acceptance_blocker()
            },
        });
    }

    let plan = fixture
        .service
        .retry_change("change-a")
        .await
        .expect("refusal is not an error");
    assert!(
        plan.change_ids.is_empty() && plan.routes.is_empty() && !plan.explicit_retry,
        "a non-resumable acceptance hold must dispatch nothing"
    );

    let guard = fixture.state.read().await;
    assert_eq!(
        guard.display_status("change-a"),
        "stalled",
        "a refused retry must keep the change stalled with its blocker evidence"
    );
    assert!(guard.acceptance_stalled_change_ids().contains("change-a"));
}

#[tokio::test]
async fn operator_command_retry_rejects_changes_without_retryable_evidence() {
    let fixture = fixture(&["change-a"]);

    let error = fixture
        .service
        .retry_change("change-a")
        .await
        .expect_err("a not-queued change carries no retryable evidence");

    assert_eq!(
        error,
        OperatorCommandError::RetryUnsupported {
            change_id: "change-a".to_string(),
            display_status: "not queued".to_string(),
        }
    );
}

#[tokio::test]
async fn operator_command_bulk_retry_skips_unsupported_holds() {
    let fixture = fixture(&["error-change", "merged-change", "idle-change"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "error-change".to_string(),
            error: "boom".to_string(),
        });
        guard.apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "merged-change".to_string(),
            revision: "rev-a".to_string(),
        });
    }

    let plan = fixture
        .service
        .retry_errors(&[
            "error-change".to_string(),
            "merged-change".to_string(),
            "idle-change".to_string(),
        ])
        .await;

    assert_eq!(
        plan.change_ids,
        vec!["error-change".to_string()],
        "bulk retry must not consume unsupported or mismatched holds"
    );
    assert_eq!(
        fixture.state.read().await.display_status("merged-change"),
        "merged",
        "a final outcome must keep its evidence"
    );
}

#[tokio::test]
async fn operator_command_retry_of_empty_selection_is_a_noop() {
    let fixture = fixture(&["change-a"]);
    let plan = fixture.service.retry_errors(&[]).await;
    assert!(plan.is_empty());
    assert!(!plan.explicit_retry);
    assert!(fixture.queue.contents.lock().expect("contents").is_empty());
}

// ============================================================================
// Task 7: restart and routing regression coverage
// ============================================================================

#[test]
fn operator_command_execution_marks_reset_after_process_restart() {
    let store = ExecutionMarkStore::new();
    store.set("change-a", true);
    store.set("change-b", true);
    assert_eq!(
        store.marked_ids(),
        vec!["change-a".to_string(), "change-b".to_string()]
    );

    // A restart drops process-local state and rebuilds the store from nothing.
    drop(store);
    let restarted = ExecutionMarkStore::new();

    assert!(!restarted.is_marked("change-a"));
    assert!(!restarted.is_marked("change-b"));
    assert!(
        restarted.marked_ids().is_empty(),
        "every execution mark must be false after restart"
    );
}

#[test]
fn operator_command_execution_marks_are_never_persisted() {
    let source = include_str!("../operator_command.rs");
    for forbidden in [
        "std::fs",
        "tokio::fs",
        "write_all",
        "File::create",
        "serde_json::to_writer",
    ] {
        assert!(
            !source.contains(forbidden),
            "execution marks must stay process-local: found '{forbidden}'"
        );
    }
}

#[tokio::test]
async fn operator_command_marks_do_not_change_workspace_derived_routing() {
    let fixture = fixture(&["change-a"]);
    let before = fixture.state.read().await.all_display_statuses();

    fixture
        .service
        .set_execution_mark(OperatorMode::Select, "change-a", true)
        .await
        .expect("mark");
    let after_mark = fixture.state.read().await.all_display_statuses();
    assert_eq!(
        before, after_mark,
        "marks must not participate in reducer-derived routing"
    );

    fixture.marks.clear();
    let after_clear = fixture.state.read().await.all_display_statuses();
    assert_eq!(
        before, after_clear,
        "clearing marks must not change workspace-derived routing"
    );
}

/// Explicit operator retry always permits the blocked phase to run again: the
/// hold is released so the next execution result — not the preserved metadata —
/// decides whether the change is blocked again or progresses.
#[tokio::test]
async fn operator_command_retry_reruns_an_externally_blocked_change() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "change-a".to_string(),
            blocker: external_prerequisite_blocker(),
        });
    }

    {
        let guard = fixture.state.read().await;
        assert_eq!(guard.display_status("change-a"), "blocked");
        assert_eq!(
            guard.change_runtime("change-a").unwrap().blocker_kind(),
            crate::orchestration::state::BlockerKind::External
        );
        assert!(guard.externally_blocked_change_ids().contains("change-a"));
    }

    let plan = fixture
        .service
        .retry_change("change-a")
        .await
        .expect("an external prerequisite wait is explicitly retryable");
    assert_eq!(plan.change_ids, ["change-a"]);
    assert!(plan.explicit_retry, "the blocked phase must run again");

    let guard = fixture.state.read().await;
    assert_eq!(guard.display_status("change-a"), "queued");
    assert!(
        guard.externally_blocked_change_ids().is_empty(),
        "the hold must be released so fresh evidence can reclassify"
    );
}

/// A dependency wait is not explicitly retryable: it clears when the dependency
/// completes, so a retry request must not be routed as an acceptance hold even
/// though both waits display as `blocked`.
#[tokio::test]
async fn operator_command_retry_is_unsupported_for_a_dependency_wait() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::DependencyBlocked {
            change_id: "change-a".to_string(),
            dependency_ids: vec!["alpha".to_string()],
        });
    }

    let error = fixture
        .service
        .retry_change("change-a")
        .await
        .expect_err("a dependency wait carries no retryable evidence");
    assert!(matches!(
        error,
        OperatorCommandError::RetryUnsupported { ref display_status, .. }
            if display_status == "blocked"
    ));
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "blocked",
        "a refused retry must leave the dependency wait untouched"
    );
}

/// An externally blocked change must not stop unrelated ready work, and a
/// proposal that depends on it keeps its own dependency blocker kind rather than
/// inheriting the external one.
#[tokio::test]
async fn externally_blocked_change_does_not_block_unrelated_or_dependent_kinds() {
    let fixture = fixture(&["alpha", "beta", "gamma"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "alpha".to_string(),
            blocker: external_prerequisite_blocker(),
        });
        guard.apply_execution_event(&ExecutionEvent::DependencyBlocked {
            change_id: "beta".to_string(),
            dependency_ids: vec!["alpha".to_string()],
        });
        guard.apply_command(ReducerCommand::AddToQueue("gamma".to_string()));
    }

    let guard = fixture.state.read().await;
    assert_eq!(guard.display_status("alpha"), "blocked");
    assert_eq!(guard.display_status("beta"), "blocked");
    assert_eq!(guard.display_status("gamma"), "queued");

    assert_eq!(
        guard.change_runtime("beta").unwrap().blocker_kind(),
        crate::orchestration::state::BlockerKind::Dependency,
        "a dependent must not inherit the external blocker kind"
    );
    assert_eq!(
        guard.externally_blocked_change_ids(),
        std::collections::HashSet::from(["alpha".to_string()]),
        "only the change with the validated prerequisite is externally blocked"
    );
    assert_eq!(
        guard.queued_change_ids(),
        vec!["gamma".to_string()],
        "unrelated ready work continues"
    );
}
