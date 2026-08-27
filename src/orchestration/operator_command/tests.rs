//! Tests for the shared operator command service.
//!
//! Evidence classes used here:
//! - unit: lifecycle matrix, mark store, and reducer routing with in-memory doubles
//! - integration: real `DynamicQueue`, real tokio tasks, real cancellation tokens

use super::*;
use crate::events::{ExecutionEvent, StalledBlocker};
use crate::orchestration::state::OrchestratorState;
use crate::tui::queue::DynamicQueue;
use std::sync::Mutex as StdMutex;

// ============================================================================
// Fixtures
// ============================================================================

fn shared_state(ids: &[&str]) -> Arc<RwLock<OrchestratorState>> {
    Arc::new(RwLock::new(OrchestratorState::new(
        ids.iter().map(|id| id.to_string()).collect(),
        10,
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
///
/// Every port call is recorded, not just the ones that change `contents`. A
/// mark-only assertion needs to prove a call never *happened*, and a double
/// that only tracked resulting state could not tell a refused call from an
/// absent one.
struct FakeQueue {
    contents: StdMutex<Vec<String>>,
    cancellation: CancellationBehavior,
    notified: StdMutex<usize>,
    explicit_retries: StdMutex<Vec<(String, RetryEdgeAuthority)>>,
    added: StdMutex<Vec<String>>,
    removed: StdMutex<Vec<String>>,
    cancellations: StdMutex<Vec<String>>,
}

impl FakeQueue {
    fn new(cancellation: CancellationBehavior) -> Self {
        Self {
            contents: StdMutex::new(Vec::new()),
            cancellation,
            notified: StdMutex::new(0),
            explicit_retries: StdMutex::new(Vec::new()),
            added: StdMutex::new(Vec::new()),
            removed: StdMutex::new(Vec::new()),
            cancellations: StdMutex::new(Vec::new()),
        }
    }

    /// Retried target IDs, in publication order.
    fn explicit_retries(&self) -> Vec<String> {
        self.explicit_retry_edges()
            .into_iter()
            .map(|(change_id, _)| change_id)
            .collect()
    }

    /// Retried targets with the authority their route granted, in publication order.
    fn explicit_retry_edges(&self) -> Vec<(String, RetryEdgeAuthority)> {
        self.explicit_retries
            .lock()
            .expect("explicit retries lock")
            .clone()
    }
}

#[async_trait]
impl QueuePort for FakeQueue {
    async fn add(&self, change_id: &str) -> bool {
        self.added
            .lock()
            .expect("added lock")
            .push(change_id.to_string());
        let mut contents = self.contents.lock().expect("contents lock");
        if contents.iter().any(|id| id == change_id) {
            return false;
        }
        contents.push(change_id.to_string());
        true
    }

    async fn remove(&self, change_id: &str) -> bool {
        self.removed
            .lock()
            .expect("removed lock")
            .push(change_id.to_string());
        let mut contents = self.contents.lock().expect("contents lock");
        let before = contents.len();
        contents.retain(|id| id != change_id);
        contents.len() != before
    }

    async fn request_cancellation(
        &self,
        _change_id: &str,
    ) -> std::result::Result<Option<TerminationWaiter>, String> {
        self.cancellations
            .lock()
            .expect("cancellations lock")
            .push(_change_id.to_string());
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

    async fn publish_explicit_retry(&self, change_id: &str, authority: RetryEdgeAuthority) {
        self.explicit_retries
            .lock()
            .expect("explicit retries lock")
            .push((change_id.to_string(), authority));
    }
}

/// Scheduler-task liveness double.
///
/// Liveness is driven explicitly rather than by spawning a task, which is what
/// keeps the boundary-ordering assertions deterministic and unit-scoped: no
/// process, no timer, no join handle.
#[derive(Debug, Default)]
struct FakeBoundary {
    running: std::sync::atomic::AtomicBool,
}

impl FakeBoundary {
    fn live() -> Self {
        let boundary = Self::default();
        boundary.set_running(true);
        boundary
    }

    fn set_running(&self, running: bool) {
        self.running
            .store(running, std::sync::atomic::Ordering::SeqCst);
    }
}

impl RunBoundaryLiveness for FakeBoundary {
    fn boundary_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
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

/// Every operator-facing execution mode.
const ALL_MODES: [OperatorMode; 5] = [
    OperatorMode::Select,
    OperatorMode::Running,
    OperatorMode::Stopping,
    OperatorMode::Stopped,
    OperatorMode::Error,
];

/// Non-terminal display statuses spanning idle, queued, active, error, and wait.
const NON_TERMINAL_STATUSES: [&str; 12] = [
    "not queued",
    "queued",
    "preparing",
    "applying",
    "accepting",
    "rejecting",
    "archiving",
    "resolving",
    "error",
    "stalled",
    "merge wait",
    "resolve pending",
];

/// The rows that are no longer run candidates.
const TERMINAL_STATUSES: [&str; 4] = ["archived", "merged", "pushed", "rejected"];

/// Unit: mark admission is decided by terminality alone.
///
/// Asserted over the whole mode axis so no mode can grow a private rule: the
/// classifier does not take a mode at all, and this proves the vocabulary it
/// exposes agrees with that for every status class.
#[test]
fn run_mark_intent_admission_depends_only_on_terminality() {
    for status in NON_TERMINAL_STATUSES {
        assert_eq!(
            classify_mark_admission(status, false),
            MarkAdmission::Allowed,
            "'{status}' is a visible non-terminal mark target"
        );
        assert!(is_markable_status(status, false));
    }
    for status in TERMINAL_STATUSES {
        assert_eq!(
            classify_mark_admission(status, false),
            MarkAdmission::TerminalTarget,
            "'{status}' is not a run candidate"
        );
        assert!(!is_markable_status(status, false));
    }
}

/// Unit: the explicit queue matrix still exists, and it is a *different*
/// function from mark admission. That separation is the whole change: a mode
/// that refuses queue mutation must still accept mark intent.
#[test]
fn run_mark_intent_queue_route_matrix_is_independent_of_mark_admission() {
    for status in ["not queued", "queued", "error"] {
        assert_eq!(
            classify_queue_intent_route(OperatorMode::Running, status),
            QueueIntentRoute::Mutable
        );
        assert!(
            is_markable_status(status, false),
            "'{status}' stays markable"
        );
    }
    for status in ["not queued", "queued", "error", "stalled", "merge wait"] {
        assert_eq!(
            classify_queue_intent_route(OperatorMode::Error, status),
            QueueIntentRoute::RetryRequired,
            "error-mode queue mutation is still retry-owned for '{status}'"
        );
        assert!(
            is_markable_status(status, false),
            "'{status}' is still markable in Error mode"
        );
    }
    for status in ["applying", "accepting", "archiving", "resolving"] {
        assert_eq!(
            classify_queue_intent_route(OperatorMode::Running, status),
            QueueIntentRoute::Immutable
        );
        assert!(
            is_markable_status(status, false),
            "an active row stays markable"
        );
    }
    assert_eq!(
        classify_queue_intent_route(OperatorMode::Stopping, "not queued"),
        QueueIntentRoute::Immutable
    );
    for mode in ALL_MODES {
        for status in TERMINAL_STATUSES {
            assert_eq!(
                classify_queue_intent_route(mode, status),
                QueueIntentRoute::Immutable,
                "final status '{status}' is immutable in {mode:?}"
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

/// Unit: a single mark in any mode writes the mark store and nothing else.
///
/// The recorders fail the test if a queue mutation, a hook, or a scheduler
/// notification happens, so "mark-only" is proved by absence of effects rather
/// than asserted in prose.
#[tokio::test]
async fn run_mark_intent_single_mark_is_lifecycle_independent_and_side_effect_free() {
    for mode in ALL_MODES {
        for status in ["not queued", "queued", "applying", "error", "merge wait"] {
            let fixture = fixture(&["change-a"]);
            reach_status(&fixture, "change-a", status).await;
            let before = fixture.state.read().await.display_status("change-a");
            assert_eq!(before, status, "fixture must really reach '{status}'");

            let outcome = fixture
                .service
                .execute(
                    mode,
                    OperatorCommand::SetExecutionMark {
                        change_id: "change-a".to_string(),
                        marked: true,
                    },
                )
                .await
                .expect("mark mutation is always admitted for a non-terminal row");

            assert_eq!(
                outcome,
                OperatorOutcome::MarkSet {
                    change_id: "change-a".to_string(),
                    marked: true,
                },
                "{mode:?}/{status} must report a plain mark mutation"
            );
            assert!(fixture.marks.is_marked("change-a"));
            assert_eq!(
                fixture.state.read().await.display_status("change-a"),
                before,
                "{mode:?}/{status}: a mark must not move reducer state"
            );
            assert!(
                fixture.hooks.adds().is_empty() && fixture.hooks.removes().is_empty(),
                "{mode:?}/{status}: a mark must run no queue hook"
            );
            assert!(
                fixture.queue.added.lock().expect("added").is_empty()
                    && fixture.queue.removed.lock().expect("removed").is_empty(),
                "{mode:?}/{status}: a mark must not mutate the dynamic queue"
            );
            assert_eq!(
                *fixture.queue.notified.lock().expect("notified"),
                0,
                "{mode:?}/{status}: a mark must not wake the scheduler"
            );
            assert!(
                fixture.queue.explicit_retries().is_empty(),
                "{mode:?}/{status}: a mark must not publish a retry edge"
            );
            assert!(
                fixture
                    .queue
                    .cancellations
                    .lock()
                    .expect("cancels")
                    .is_empty(),
                "{mode:?}/{status}: a mark must not request cancellation"
            );

            // Repeating the same mark is a no-op, not a second mutation.
            let repeated = fixture
                .service
                .set_execution_mark("change-a", true)
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
    }
}

/// Unit: unmarking an admitted row leaves the current run exactly as it was.
#[tokio::test]
async fn run_mark_intent_unmark_does_not_disturb_admitted_work() {
    for status in ["queued", "applying"] {
        let fixture = fixture(&["change-a"]);
        // Admit the change through the explicit queue service, the way a run
        // really admits work, then reach the status under test.
        fixture
            .service
            .add_to_queue("change-a")
            .await
            .expect("explicit queue add");
        reach_status(&fixture, "change-a", status).await;
        fixture.marks.set("change-a", true);
        let admitted_adds = fixture.queue.added.lock().expect("added").clone();
        let before = fixture.state.read().await.display_status("change-a");

        let outcome = fixture
            .service
            .set_execution_mark("change-a", false)
            .await
            .expect("unmark is always admitted for a non-terminal row");

        assert_eq!(
            outcome,
            OperatorOutcome::MarkSet {
                change_id: "change-a".to_string(),
                marked: false,
            }
        );
        assert!(!fixture.marks.is_marked("change-a"));
        assert_eq!(
            fixture.state.read().await.display_status("change-a"),
            before,
            "'{status}': unmarking must not change the current run's state"
        );
        assert_eq!(
            *fixture.queue.added.lock().expect("added"),
            admitted_adds,
            "'{status}': unmarking must not touch the dynamic queue"
        );
        assert!(
            fixture.queue.removed.lock().expect("removed").is_empty(),
            "'{status}': unmarking must not dequeue"
        );
        assert!(
            fixture
                .queue
                .cancellations
                .lock()
                .expect("cancels")
                .is_empty(),
            "'{status}': unmarking must not cancel"
        );
        assert_eq!(
            fixture.hooks.removes(),
            Vec::<String>::new(),
            "'{status}': unmarking must not run the queue-remove hook"
        );
    }
}

/// Unit: a terminal target settles as a reasoned unchanged no-op, not a failure.
#[tokio::test]
async fn run_mark_intent_terminal_target_is_a_reasoned_no_op() {
    for mode in ALL_MODES {
        let fixture = fixture(&["change-a"]);
        {
            let mut guard = fixture.state.write().await;
            guard.apply_execution_event(&ExecutionEvent::ChangeRejected {
                change_id: "change-a".to_string(),
                reason: "rejected".to_string(),
            });
        }
        assert_eq!(
            fixture.state.read().await.display_status("change-a"),
            "rejected"
        );

        let outcome = fixture
            .service
            .execute(
                mode,
                OperatorCommand::SetExecutionMark {
                    change_id: "change-a".to_string(),
                    marked: true,
                },
            )
            .await
            .expect("a terminal target settles successfully");

        assert_eq!(
            outcome,
            OperatorOutcome::NoOp {
                change_id: "change-a".to_string(),
                reason: NoOpReason::TerminalMarkTarget,
            },
            "{mode:?} must identify the terminal-target reason"
        );
        assert!(!fixture.marks.is_marked("change-a"));
        assert_eq!(
            fixture.state.read().await.display_status("change-a"),
            "rejected"
        );
        assert!(fixture.hooks.adds().is_empty() && fixture.hooks.removes().is_empty());
        assert_eq!(*fixture.queue.notified.lock().expect("notified"), 0);
    }
}

/// Drive `change_id` to `status` through the reducer's own event vocabulary.
async fn reach_status(fixture: &Fixture, change_id: &str, status: &str) {
    let mut guard = fixture.state.write().await;
    match status {
        "not queued" => {}
        "queued" => {
            guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
        }
        "applying" => {
            guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
                change_id: change_id.to_string(),
                command: "apply".to_string(),
            });
        }
        "error" => {
            guard.apply_execution_event(&ExecutionEvent::ProcessingError {
                id: change_id.to_string(),
                error: "boom".to_string(),
            });
        }
        "merge wait" => {
            guard.apply_execution_event(&ExecutionEvent::WorkspaceStatusUpdated {
                change_id: change_id.to_string(),
                workspace_name: format!("ws-{change_id}"),
                status: crate::vcs::WorkspaceStatus::MergeWait,
            });
        }
        other => panic!("unsupported fixture status '{other}'"),
    }
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
            change_id: "change-a".to_string(),
            // The reducer still carries the Apply activity when the dequeue is
            // committed, so the settlement names the phase that was actually
            // cancelled. There is no facts store and no evidence port on this
            // path, so the remaining fields are honestly unknown rather than
            // guessed.
            settlement: StopSettlement {
                cancelled_phase: ExecutionPhase::Apply,
                last_completed_phase: None,
                apply_commit_present: None,
                apply_commit_oid: None,
            },
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
            change_id: "change-a".to_string(),
            settlement: StopSettlement::none(),
        }
    );
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "not queued"
    );
}

/// The guard is on the *call* `.force_kill(`, not on the words "force kill".
///
/// `force_kill` returns a best-effort boolean that says a signal was sent, never
/// that the task ended, so neither the service nor the TUI adapter may invoke it
/// and treat the result as termination evidence. The operator-facing feature is
/// still named force-kill, and identifiers such as `confirm_force_kill` describe
/// the confirmation that produces a stop-and-dequeue command — they are not calls
/// into that API and must not trip this guard.
#[test]
fn operator_command_force_kill_result_is_not_treated_as_termination_proof() {
    let source = include_str!("../operator_command.rs");
    assert!(
        !source.contains(".force_kill("),
        "the service must use the cancellation handshake, not force_kill's boolean"
    );
    let handlers = include_str!("../../tui/command_handlers.rs");
    assert!(
        !handlers.contains(".force_kill("),
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

/// Only an accepted, state-changing `RetryError` may reach the live scheduler as
/// a target-ID-bearing one-shot retry edge. That edge is what releases a change's
/// ephemeral failed classification, so anything weaker must publish nothing.
#[tokio::test]
async fn failed_dependency_retry_edge_follows_accepted_state_changing_retry_only() {
    let fixture = fixture(&["change-a", "change-b"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "change-a".to_string(),
            error: "boom".to_string(),
        });
    }

    // Ordinary queue intent for a healthy change is not a retry.
    fixture
        .service
        .add_to_queue("change-b")
        .await
        .expect("ordinary queue add");
    assert!(
        fixture.queue.explicit_retries().is_empty(),
        "an ordinary AddToQueue must not look like an explicit retry"
    );

    // A generic scheduler wake is not a retry either.
    fixture.service.queue.notify_scheduler().await;
    assert!(
        fixture.queue.explicit_retries().is_empty(),
        "a generic queue notification must not look like an explicit retry"
    );

    let plan = fixture
        .service
        .retry_change("change-a")
        .await
        .expect("terminal error must be retryable");
    assert_eq!(plan.routes, vec![RetryRoute::TerminalError]);
    assert_eq!(
        fixture.queue.explicit_retries(),
        vec!["change-a".to_string()],
        "an accepted terminal-error retry publishes exactly one target-ID-bearing edge"
    );

    // The change is queued now, so a duplicate retry carries no retryable
    // evidence and must publish nothing further.
    assert!(
        fixture.service.retry_change("change-a").await.is_err(),
        "a retried change no longer carries retryable evidence"
    );
    assert_eq!(
        fixture.queue.explicit_retries(),
        vec!["change-a".to_string()],
        "a refused retry must not publish a second edge"
    );

    // A duplicate ordinary add is a reducer no-op and publishes nothing.
    fixture
        .service
        .add_to_queue("change-a")
        .await
        .expect("duplicate queue add");
    assert_eq!(
        fixture.queue.explicit_retries(),
        vec!["change-a".to_string()],
        "a no-op AddToQueue must not publish an edge"
    );
}

/// Every accepted retry route arms a target-specific edge, and the routes are
/// told apart by the authority the edge carries rather than by whether one
/// exists: an acceptance-stall retry restores queue intent for work the reducer
/// already lists as queued, so without an edge a live scheduler has nothing to
/// distinguish it from an ordinarily suppressible wake.
#[tokio::test]
async fn accepted_retry_publishes_explicit_retry_edge_for_every_route() {
    let fixture = fixture(&["stalled-change", "failed-change", "held-change"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "stalled-change".to_string(),
            blocker: acceptance_blocker(),
        });
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "failed-change".to_string(),
            error: "boom".to_string(),
        });
        // A non-resumable hold: classification refuses it, so it arms nothing.
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "held-change".to_string(),
            blocker: StalledBlocker {
                resumable: false,
                ..acceptance_blocker()
            },
        });
    }

    let stalled = fixture
        .service
        .retry_change("stalled-change")
        .await
        .expect("acceptance stall must be retryable");
    assert_eq!(stalled.routes, vec![RetryRoute::AcceptanceStall]);
    assert_eq!(
        fixture.queue.explicit_retry_edges(),
        vec![(
            "stalled-change".to_string(),
            RetryEdgeAuthority::AnalysisBypass
        )],
        "the stall route arms one edge for its own target, carrying analysis-bypass \
         authority only: no failed-classification release and no Apply-budget reset"
    );

    let failed = fixture
        .service
        .retry_change("failed-change")
        .await
        .expect("terminal error must be retryable");
    assert_eq!(failed.routes, vec![RetryRoute::TerminalError]);
    assert_eq!(
        fixture.queue.explicit_retry_edges(),
        vec![
            (
                "stalled-change".to_string(),
                RetryEdgeAuthority::AnalysisBypass
            ),
            (
                "failed-change".to_string(),
                RetryEdgeAuthority::TerminalError
            ),
        ],
        "the terminal-error route keeps its own wider authority, for its own target"
    );

    let refused = fixture
        .service
        .retry_change("held-change")
        .await
        .expect("refusal is not an error");
    assert!(
        refused.routes.is_empty(),
        "a non-resumable hold dispatches nothing"
    );

    // A retried change is `queued` now, so a second request carries no retryable
    // evidence, and a duplicate ordinary add is a reducer no-op.
    assert!(fixture.service.retry_change("failed-change").await.is_err());
    fixture
        .service
        .add_to_queue("stalled-change")
        .await
        .expect("duplicate queue add");
    fixture.service.queue.notify_scheduler().await;

    assert_eq!(
        fixture.queue.explicit_retry_edges().len(),
        2,
        "a refused retry, a reducer no-op, an ordinary add, and a generic wake arm nothing"
    );
}

/// The real `DynamicQueue` carries the edge, one-shot, with the retried ID.
#[tokio::test]
async fn failed_dependency_retry_edge_reaches_the_real_dynamic_queue_once() {
    let (service, state, _hooks, queue) = dynamic_queue_fixture(&["change-a"]);
    {
        let mut guard = state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ProcessingError {
            id: "change-a".to_string(),
            error: "boom".to_string(),
        });
    }

    service
        .retry_change("change-a")
        .await
        .expect("terminal error must be retryable");

    assert_eq!(
        queue.drain_explicit_retries().await,
        vec![crate::tui::queue::ExplicitRetryEdge {
            change_id: "change-a".to_string(),
            authority: RetryEdgeAuthority::TerminalError,
        }],
        "the scheduler receives the retried change ID with terminal-error authority"
    );
    assert!(
        queue.drain_explicit_retries().await.is_empty(),
        "the edge is one-shot and cannot be replayed by a later wake"
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
        .set_execution_mark("change-a", true)
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

/// Apply the compatibility workspace status the external-blocker dispatch
/// branch emits right after its structured event.
async fn observe_generic_blocked_workspace(fixture: &Fixture, change_id: &str) {
    fixture
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::WorkspaceStatusUpdated {
            change_id: change_id.to_string(),
            workspace_name: format!("ws-{change_id}"),
            status: crate::vcs::WorkspaceStatus::Blocked,
        });
}

/// Retry admission is decided after the *full* producer sequence, not after the
/// structured event alone. A preserved resumable external hold keeps routing to
/// the existing blocked-phase retry, and its projected evidence is unchanged.
#[tokio::test]
async fn external_blocker_hold_survives_dispatch_status_and_stays_explicitly_retryable() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "change-a".to_string(),
            blocker: external_prerequisite_blocker(),
        });
    }
    let before = fixture
        .state
        .read()
        .await
        .blocker_view("change-a")
        .expect("the structured event establishes the blocker view");

    observe_generic_blocked_workspace(&fixture, "change-a").await;

    {
        let guard = fixture.state.read().await;
        assert_eq!(
            guard.display_status("change-a"),
            "blocked",
            "the generic observation must not downgrade the wait"
        );
        let after = guard.blocker_view("change-a").expect("preserved hold");
        assert_eq!(
            after, before,
            "every projected blocker field stays reducer-derived and unchanged"
        );
        assert_eq!(
            after.kind,
            crate::orchestration::state::BlockerKind::External
        );
        assert_eq!(after.prerequisite_owner.as_deref(), Some("platform"));
        assert!(after.resumable);
        assert!(guard.externally_blocked_change_ids().contains("change-a"));
    }

    let plan = fixture
        .service
        .retry_change("change-a")
        .await
        .expect("a preserved external prerequisite wait is explicitly retryable");
    assert_eq!(plan.change_ids, ["change-a"]);
    assert_eq!(plan.routes, vec![RetryRoute::AcceptanceStall]);
    assert!(plan.explicit_retry, "the blocked phase must run again");

    let guard = fixture.state.read().await;
    assert_eq!(guard.display_status("change-a"), "queued");
    assert!(guard.externally_blocked_change_ids().is_empty());
}

/// The same sequence with a non-resumable hold: retry is still refused, and the
/// refusal must not consume the evidence the operator owns. Losing resumability
/// to the generic observation would have made this hold silently retryable.
#[tokio::test]
async fn external_blocker_hold_survives_dispatch_status_and_refuses_a_non_resumable_retry() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "change-a".to_string(),
            blocker: StalledBlocker {
                resumable: false,
                ..external_prerequisite_blocker()
            },
        });
    }
    let before = fixture
        .state
        .read()
        .await
        .blocker_view("change-a")
        .expect("the structured event establishes the blocker view");

    observe_generic_blocked_workspace(&fixture, "change-a").await;

    let plan = fixture
        .service
        .retry_change("change-a")
        .await
        .expect("refusal is not an error");
    assert!(
        plan.change_ids.is_empty() && plan.routes.is_empty() && !plan.explicit_retry,
        "a non-resumable hold must dispatch nothing"
    );

    let guard = fixture.state.read().await;
    assert_eq!(guard.display_status("change-a"), "blocked");
    let after = guard.blocker_view("change-a").expect("retained evidence");
    assert_eq!(
        after, before,
        "a refused retry discards no blocker evidence"
    );
    assert!(!after.resumable);
    assert!(guard.externally_blocked_change_ids().contains("change-a"));
    assert!(
        fixture.queue.explicit_retries().is_empty(),
        "a refused retry publishes no edge"
    );
}

/// An Acceptance-owned non-external stall keeps the same guarantees: it remains
/// `stalled`, keeps its Acceptance ownership, and stays retryable.
#[tokio::test]
async fn external_blocker_hold_survives_dispatch_status_for_an_acceptance_owned_stall() {
    let fixture = fixture(&["change-a"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "change-a".to_string(),
            blocker: acceptance_blocker(),
        });
    }
    let before = fixture
        .state
        .read()
        .await
        .blocker_view("change-a")
        .expect("the structured event establishes the blocker view");

    observe_generic_blocked_workspace(&fixture, "change-a").await;

    {
        let guard = fixture.state.read().await;
        assert_eq!(guard.display_status("change-a"), "stalled");
        assert_eq!(
            guard.blocker_view("change-a").as_ref(),
            Some(&before),
            "the Acceptance-owned hold keeps its guidance and resumability"
        );
        assert!(guard.acceptance_stalled_change_ids().contains("change-a"));
    }

    let plan = fixture
        .service
        .retry_change("change-a")
        .await
        .expect("a preserved resumable acceptance hold is retryable");
    assert_eq!(plan.routes, vec![RetryRoute::AcceptanceStall]);
    assert!(plan.explicit_retry);
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

// ============================================================================
// Parallel control: classification (unit — pure functions, no boundaries)
// ============================================================================

/// Build a bulk-mark row without repeating the struct literal per case.
///
/// No reducer archive record: `archived_row` is the counterpart for that case.
fn row<'a>(change_id: &'a str, display_status: &'a str, marked: bool) -> MarkTargetRow<'a> {
    MarkTargetRow {
        change_id,
        display_status,
        archive_complete: false,
        marked,
    }
}

/// Build a bulk-mark row the reducer has already recorded as archived.
fn archived_row<'a>(
    change_id: &'a str,
    display_status: &'a str,
    marked: bool,
) -> MarkTargetRow<'a> {
    MarkTargetRow {
        change_id,
        display_status,
        archive_complete: true,
        marked,
    }
}

/// Bool-to-reason shorthand for cases where only observation is under test.
fn eligibility(parallel_eligible: bool) -> ParallelEligibility {
    if parallel_eligible {
        ParallelEligibility::Eligible
    } else {
        ParallelEligibility::UncommittedProposalFiles
    }
}

#[test]
fn bulk_mark_target_state_is_derived_from_eligible_rows_only() {
    // One unmarked eligible row means "mark all", even though the excluded row
    // is unmarked too and the other eligible row is already marked.
    let rows = [
        row("marked", "not queued", true),
        row("unmarked", "not queued", false),
        row("rejected", "rejected", false),
    ];
    let plan = plan_bulk_marks(&rows);
    assert!(plan.target_state, "a partially marked set marks all");
    assert_eq!(plan.eligible, vec!["marked", "unmarked"]);
    assert_eq!(
        plan.excluded,
        vec![("rejected".to_string(), MarkExclusion::FinalStatus)]
    );

    // Every eligible row marked means "unmark all"; the unmarked excluded row
    // must not drag the target state back to `true`.
    let rows = [
        row("a", "not queued", true),
        row("b", "not queued", true),
        row("rejected", "rejected", false),
    ];
    let plan = plan_bulk_marks(&rows);
    assert!(!plan.target_state, "a fully marked set unmarks all");
    assert_eq!(plan.eligible, vec!["a", "b"]);
}

/// Unit: without archive evidence the bulk classifier excludes terminality only.
///
/// Mode, activity, wait state, and worktree eligibility all used to appear here.
/// Their absence is the contract: a mark is next-run intent, and only a row with
/// no next run is refused. Reducer-recorded archive completion is the *other* way
/// a row can have no next run, and it is asserted separately below.
#[test]
fn run_mark_intent_bulk_classification_excludes_terminal_rows_only() {
    for status in NON_TERMINAL_STATUSES {
        assert_eq!(
            classify_bulk_mark_row(status, false),
            None,
            "'{status}' is a bulk mark target"
        );
    }
    for status in TERMINAL_STATUSES {
        assert_eq!(
            classify_bulk_mark_row(status, false),
            Some(MarkExclusion::FinalStatus),
            "'{status}' is excluded as terminal"
        );
    }
}

/// Unit: every mode produces the same plan, because the plan does not see one.
#[test]
fn run_mark_intent_bulk_plan_is_identical_in_every_mode() {
    let rows = [
        row("idle", "not queued", false),
        row("active", "applying", false),
        row("error", "error", false),
        row("wait", "merge wait", false),
        row("archived", "archived", false),
    ];

    let plan = plan_bulk_marks(&rows);

    assert!(plan.target_state);
    assert_eq!(plan.eligible, vec!["idle", "active", "error", "wait"]);
    assert_eq!(
        plan.excluded,
        vec![("archived".to_string(), MarkExclusion::FinalStatus)]
    );
}

#[test]
fn bulk_mark_plan_over_zero_rows_is_an_empty_unmark() {
    let plan = plan_bulk_marks(&[]);
    assert!(plan.is_empty());
    assert!(plan.excluded.is_empty());
    assert!(!plan.target_state);
    assert_eq!(plan.exclusion_summary(), "");
}

#[test]
fn bulk_mark_exclusion_summary_groups_reasons_with_counts() {
    let rows = [
        row("archived-a", "archived", false),
        row("merged-b", "merged", false),
        row("eligible", "not queued", false),
    ];
    let plan = plan_bulk_marks(&rows);

    assert_eq!(plan.eligible, vec!["eligible"]);
    assert_eq!(
        plan.exclusion_summary(),
        format!("2 {}", MarkExclusion::FinalStatus.reason())
    );
    // Machine-readable tokens are distinct so a client can branch on them.
    let tokens: std::collections::HashSet<&str> =
        MarkExclusion::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(tokens.len(), MarkExclusion::ALL.len());
}

/// The two parallel refusals are still reported apart wherever they apply.
///
/// A row absent from `HEAD` has no uncommitted content, so telling the operator
/// to commit it names work that does not exist. They no longer touch marks, but
/// the vocabulary the queue and start paths report is unchanged.
#[test]
fn parallel_refusals_keep_naming_the_reason_they_actually_observed() {
    assert_eq!(
        ParallelEligibility::UncommittedProposalFiles.queue_exclusion(),
        Some(MarkExclusion::ParallelIneligible)
    );
    assert_eq!(
        ParallelEligibility::ProposalAbsentFromHead.queue_exclusion(),
        Some(MarkExclusion::ParallelProposalAbsent)
    );
    assert_eq!(ParallelEligibility::Eligible.queue_exclusion(), None);
    assert!(
        !MarkExclusion::ParallelProposalAbsent
            .reason()
            .to_lowercase()
            .contains("uncommit"),
        "an absent proposal must never be described as uncommitted"
    );
}

/// Observation precedence and the store round-trip, in one place.
#[test]
fn parallel_eligibility_observation_prefers_the_actionable_reason() {
    let committed = HashSet::from(["clean".to_string(), "dirty".to_string()]);
    let uncommitted = HashSet::from(["dirty".to_string(), "brand-new".to_string()]);

    assert_eq!(
        ParallelEligibility::observe("clean", &committed, &uncommitted),
        ParallelEligibility::Eligible
    );
    assert_eq!(
        ParallelEligibility::observe("absent", &committed, &uncommitted),
        ParallelEligibility::ProposalAbsentFromHead
    );
    assert_eq!(
        ParallelEligibility::observe("dirty", &committed, &uncommitted),
        ParallelEligibility::UncommittedProposalFiles
    );
    // Untracked and absent at once: it is reported as the condition a commit fixes.
    assert_eq!(
        ParallelEligibility::observe("brand-new", &committed, &uncommitted),
        ParallelEligibility::UncommittedProposalFiles
    );

    let runtime = ParallelRuntime::new();
    runtime.set_parallel_ineligible([
        ("clean".to_string(), ParallelEligibility::Eligible),
        (
            "absent".to_string(),
            ParallelEligibility::ProposalAbsentFromHead,
        ),
    ]);

    // An "ineligible" entry that claims eligibility is dropped rather than stored.
    assert_eq!(runtime.ineligible_ids(), vec!["absent".to_string()]);
    assert!(runtime.is_eligible("clean"));
    assert!(!runtime.is_eligible("absent"));
    assert_eq!(runtime.eligibility("clean"), ParallelEligibility::Eligible);
    assert_eq!(
        runtime.eligibility("absent"),
        ParallelEligibility::ProposalAbsentFromHead
    );
}

#[test]
fn parallel_cleanup_targets_only_names_ineligible_rows_carrying_intent() {
    let rows = [
        ParallelCleanupRow {
            change_id: "eligible-marked",
            parallel_eligible: true,
            marked: true,
            queued: true,
        },
        ParallelCleanupRow {
            change_id: "ineligible-idle",
            parallel_eligible: false,
            marked: false,
            queued: false,
        },
        ParallelCleanupRow {
            change_id: "ineligible-marked",
            parallel_eligible: false,
            marked: true,
            queued: false,
        },
        ParallelCleanupRow {
            change_id: "ineligible-queued",
            parallel_eligible: false,
            marked: false,
            queued: true,
        },
    ];

    assert_eq!(
        parallel_cleanup_targets(&rows),
        vec!["ineligible-marked", "ineligible-queued"],
        "only ineligible rows that actually carry intent need clearing"
    );
}

// ============================================================================
// Worktree eligibility: service behavior (unit — in-memory queue and reducer)
// ============================================================================

/// A fixture whose worktree runtime carries the given exclusions.
fn parallel_fixture(ids: &[&str], ineligible: &[&str]) -> Fixture {
    let fixture = fixture(ids);
    let parallel = fixture.service.parallel();
    parallel.set_max_concurrent(4);
    parallel.set_vcs_backend("git");
    parallel.set_parallel_ineligible(ineligible.iter().map(|id| {
        (
            id.to_string(),
            ParallelEligibility::UncommittedProposalFiles,
        )
    }));
    fixture
}

#[tokio::test]
async fn bulk_mark_marks_every_eligible_change_and_reports_exclusions() {
    let fixture = parallel_fixture(&["a", "b", "uncommitted"], &["uncommitted"]);
    fixture.marks.replace(["a".to_string()]);

    let outcome = fixture
        .service
        .set_all_execution_marks()
        .await
        .expect("a bulk mutation is always admitted");

    match outcome {
        OperatorOutcome::BulkMarks {
            marked,
            changed,
            excluded,
        } => {
            assert!(marked, "one unmarked eligible row marks the whole set");
            assert_eq!(
                changed,
                vec!["b".to_string(), "uncommitted".to_string()],
                "'a' was already marked; worktree eligibility excludes nothing"
            );
            assert!(excluded.is_empty(), "no row is terminal");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert_eq!(
        fixture.marks.marked_ids(),
        vec!["a".to_string(), "b".to_string(), "uncommitted".to_string()]
    );
}

#[tokio::test]
async fn bulk_mark_unmarks_every_eligible_change_when_all_are_marked() {
    let fixture = parallel_fixture(&["a", "b"], &[]);
    fixture.marks.replace(["a".to_string(), "b".to_string()]);

    let outcome = fixture
        .service
        .set_all_execution_marks()
        .await
        .expect("a bulk mutation is always admitted");

    match outcome {
        OperatorOutcome::BulkMarks {
            marked, changed, ..
        } => {
            assert!(!marked);
            assert_eq!(changed, vec!["a".to_string(), "b".to_string()]);
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    assert!(fixture.marks.marked_ids().is_empty());
}

/// Unit: a bulk mark writes marks only, in every mode, including for an
/// in-flight row. The queue recorders prove no membership moved with it.
#[tokio::test]
async fn run_mark_intent_bulk_mark_never_moves_queue_intent() {
    for mode in ALL_MODES {
        let fixture = parallel_fixture(&["idle", "active"], &[]);
        {
            let mut guard = fixture.state.write().await;
            guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
                change_id: "active".to_string(),
                command: "apply".to_string(),
            });
        }

        let outcome = fixture
            .service
            .execute(mode, OperatorCommand::SetAllExecutionMarks)
            .await
            .expect("a bulk mutation is always admitted");

        match outcome {
            OperatorOutcome::BulkMarks {
                marked,
                changed,
                excluded,
            } => {
                assert!(marked, "{mode:?} must mark the whole non-terminal set");
                assert_eq!(
                    changed,
                    vec!["active".to_string(), "idle".to_string()],
                    "{mode:?}: an in-flight row still accepts next-run intent"
                );
                assert!(excluded.is_empty());
            }
            other => panic!("{mode:?} unexpected outcome: {other:?}"),
        }

        assert_eq!(
            fixture.state.read().await.display_status("idle"),
            "not queued",
            "{mode:?}: a bulk mark must not create queue intent"
        );
        assert_eq!(
            fixture.state.read().await.display_status("active"),
            "applying",
            "{mode:?}: a bulk mark must not disturb active execution"
        );
        assert!(fixture.queue.contents.lock().unwrap().is_empty());
        assert!(fixture.queue.added.lock().unwrap().is_empty());
        assert!(fixture.queue.removed.lock().unwrap().is_empty());
        assert!(fixture.queue.cancellations.lock().unwrap().is_empty());
        assert_eq!(*fixture.queue.notified.lock().unwrap(), 0);
        assert!(fixture.queue.explicit_retries().is_empty());
        assert!(fixture.hooks.adds().is_empty() && fixture.hooks.removes().is_empty());

        // The reverse mutation is equally inert.
        fixture
            .service
            .set_all_execution_marks()
            .await
            .expect("the reverse mutation is admitted too");
        assert!(fixture.marks.marked_ids().is_empty());
        assert!(fixture.hooks.removes().is_empty());
        assert!(fixture.queue.removed.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn bulk_mark_with_zero_eligible_rows_changes_nothing() {
    let fixture = parallel_fixture(&["rejected"], &[]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ChangeRejected {
            change_id: "rejected".to_string(),
            reason: "acceptance refused the proposal".to_string(),
        });
    }

    let outcome = fixture
        .service
        .set_all_execution_marks()
        .await
        .expect("a zero-eligible bulk mutation is valid, not an error");

    assert!(matches!(
        outcome,
        OperatorOutcome::NoOp {
            reason: NoOpReason::NoEligibleMarkTarget,
            ..
        }
    ));
    assert!(fixture.marks.marked_ids().is_empty());
    assert!(fixture.queue.contents.lock().unwrap().is_empty());
}

// ============================================================================
// Workspace preparation counts as active execution
// ============================================================================

/// Put a change into the reducer's ephemeral preparation activity.
async fn enter_preparation(state: &Arc<RwLock<OrchestratorState>>, change_id: &str) {
    let mut guard = state.write().await;
    guard.apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
    guard.apply_execution_event(&ExecutionEvent::WorkspacePreparationStarted {
        change_id: change_id.to_string(),
    });
    assert_eq!(guard.display_status(change_id), "preparing");
}

/// Unit: the shared lifecycle matrix classifies `preparing` as active, so no
/// frontend can offer a queue-intent mutation on a change whose worktree Conflux
/// is currently building. Marking it is a different question and stays allowed:
/// preparation is not a terminal outcome.
#[test]
fn preparing_is_active_in_the_shared_lifecycle_matrix() {
    assert!(is_active_status("preparing"));
    assert!(!is_final_status("preparing"));

    assert_eq!(
        classify_queue_intent_route(OperatorMode::Running, "preparing"),
        QueueIntentRoute::Immutable
    );
    assert_eq!(classify_bulk_mark_row("preparing", false), None);
    assert!(is_markable_status("preparing", false));
}

/// Unit: inline preparation registers no termination handle, so an immediate
/// dequeue cannot be proven and must be refused with the active state intact.
#[tokio::test]
async fn preparing_is_active_so_dequeue_without_a_handle_is_refused() {
    let fixture = fixture_with_cancellation(&["change-a"], CancellationBehavior::NoHandle);
    enter_preparation(&fixture.state, "change-a").await;

    let error = fixture
        .service
        .stop_and_dequeue("change-a")
        .await
        .expect_err("preparation has no handle, so termination cannot be confirmed");

    assert_eq!(
        error,
        OperatorCommandError::MissingCancellationHandle {
            change_id: "change-a".to_string()
        }
    );
    assert_eq!(
        fixture.state.read().await.display_status("change-a"),
        "preparing",
        "a refused stop must not fabricate a dequeued row"
    );
}

/// Unit: the refusal still records the stop mark, which is what later stops
/// execution before an operation agent starts. `DynamicQueue` is in-memory
/// state with no external boundary, so this stays unit-scoped.
#[tokio::test]
async fn preparing_is_active_and_a_refused_stop_retains_the_stop_mark() {
    let (service, state, _hooks, queue) = dynamic_queue_fixture(&["change-a"]);
    enter_preparation(&state, "change-a").await;

    let error = service
        .stop_and_dequeue("change-a")
        .await
        .expect_err("preparation has no handle");
    assert_eq!(
        error,
        OperatorCommandError::MissingCancellationHandle {
            change_id: "change-a".to_string()
        }
    );

    assert!(
        queue.is_stopped("change-a").await,
        "the stop mark must survive the refusal so preparation cannot hand off to an agent"
    );
}

// ============================================================================
// Settled Apply iteration limit: explicit retry admission (unit)
// ============================================================================
//
// Every test here drives an in-memory reducer, an in-memory queue double, a
// recording hook port, and an explicitly driven liveness flag. Nothing touches a
// process, repository, clock, or network, so the whole family is unit evidence.
//
// The condition under test is the one the operator hit: an Apply invocation
// exhausted its dispatch ceiling, the change settled into terminal `error`, and
// the persistent scheduler that owned the invocation is *still alive*. The
// retained record is diagnostic evidence about that invocation; it is explicit
// operator intent, and nothing else, that decides whether a later invocation may
// open.

/// Every mutation the retry paths can perform, captured at one instant.
///
/// Comparing the whole record before and after a command is what makes "no side
/// effects" checkable rather than asserted one field at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MutationSnapshot {
    display_status: String,
    error_detail: Option<String>,
    queue_intent_queued: bool,
    marks: Vec<String>,
    queue_contents: Vec<String>,
    explicit_retries: Vec<String>,
    hook_adds: Vec<String>,
    hook_removes: Vec<String>,
    notifications: usize,
}

/// A settled-limit fixture: terminal-error changes retaining an Apply ceiling.
///
/// The returned [`FakeBoundary`] is live for the whole test and is deliberately
/// *not* bound to the service: scheduler-task liveness is the observability
/// authority behind `scheduler_running`, and the point of this family is that it
/// no longer reaches operator-command admission at all. Every test asserts it is
/// still running at the moment the service admits the command.
async fn limited_fixture(ids: &[&str], limited: &[&str]) -> (Fixture, Arc<FakeBoundary>) {
    let state = shared_state(ids);
    let hooks = Arc::new(RecordingHooks::default());
    let queue = Arc::new(FakeQueue::new(CancellationBehavior::ConfirmedImmediately));
    let marks = Arc::new(ExecutionMarkStore::new());
    let boundary = Arc::new(FakeBoundary::live());
    let service =
        OperatorCommandService::new(state.clone(), queue.clone(), hooks.clone(), marks.clone())
            .with_cancellation_timeout(Duration::from_millis(50));
    {
        let mut guard = state.write().await;
        for change_id in limited {
            guard.apply_execution_event(&ExecutionEvent::ProcessingError {
                id: (*change_id).to_string(),
                error: "max iterations reached".to_string(),
            });
            guard.record_apply_iteration_limit(change_id, 50, 50);
        }
    }
    (
        Fixture {
            service,
            state,
            hooks,
            queue,
            marks,
        },
        boundary,
    )
}

impl Fixture {
    async fn snapshot(&self, change_id: &str) -> MutationSnapshot {
        let guard = self.state.read().await;
        MutationSnapshot {
            display_status: guard.display_status(change_id).to_string(),
            error_detail: guard
                .change_runtime(change_id)
                .and_then(crate::orchestration::state::ChangeRuntimeState::error_message)
                .map(str::to_string),
            queue_intent_queued: matches!(
                guard.change_runtime(change_id).map(|rt| &rt.queue_intent),
                Some(crate::orchestration::state::QueueIntent::Queued)
            ),
            marks: self.marks.marked_ids(),
            queue_contents: self.queue.contents.lock().expect("contents lock").clone(),
            explicit_retries: self.queue.explicit_retries(),
            hook_adds: self.hooks.adds(),
            hook_removes: self.hooks.removes(),
            notifications: *self.queue.notified.lock().expect("notified lock"),
        }
    }

    async fn to_error(&self, change_id: &str) {
        self.state
            .write()
            .await
            .apply_execution_event(&ExecutionEvent::ProcessingError {
                id: change_id.to_string(),
                error: "boom".to_string(),
            });
    }

    /// The retained Apply-dispatch ceiling for `change_id`, as attempts/max.
    async fn retained_limit(&self, change_id: &str) -> Option<(u32, u32)> {
        self.state
            .read()
            .await
            .apply_iteration_limit(change_id)
            .map(|record| (record.attempts, record.max))
    }
}

/// The regression itself: a live scheduler no longer refuses the retry.
#[tokio::test]
async fn settled_apply_limit_admits_individual_retry_while_the_scheduler_is_live() {
    let (fixture, boundary) = limited_fixture(&["limited"], &["limited"]).await;
    assert_eq!(
        fixture.retained_limit("limited").await,
        Some((50, 50)),
        "the settled invocation's diagnostic is present before any command"
    );
    assert!(
        boundary.boundary_running(),
        "the persistent scheduler that owned the exhausted invocation is alive"
    );

    let plan = fixture
        .service
        .retry_change("limited")
        .await
        .expect("a settled terminal error is retryable on its own evidence");

    assert!(
        boundary.boundary_running(),
        "and it was still alive when the service admitted the command"
    );
    assert_eq!(plan.change_ids, vec!["limited".to_string()]);
    assert_eq!(
        plan.routes,
        vec![RetryRoute::TerminalError],
        "the ordinary terminal-error route, with no limit exception of its own"
    );
    assert!(plan.explicit_retry);

    let after = fixture.snapshot("limited").await;
    assert_ne!(
        after.display_status, "error",
        "the terminal error was consumed by the explicit intent"
    );
    assert_eq!(
        after.explicit_retries,
        vec!["limited".to_string()],
        "exactly one target-specific explicit-retry edge is published"
    );
    assert_eq!(
        fixture.retained_limit("limited").await,
        None,
        "and the diagnostic is consumed by the same explicit intent, so the \
         next invocation starts from a fresh ceiling"
    );
}

/// The explicit per-target queue alias is retry intent, so it routes identically.
#[tokio::test]
async fn settled_apply_limit_terminal_error_queue_alias_retries_explicitly() {
    let (fixture, boundary) = limited_fixture(&["limited"], &["limited"]).await;

    let outcome = fixture
        .service
        .add_to_queue("limited")
        .await
        .expect("the terminal-error alias is explicit retry intent");
    assert!(boundary.boundary_running());
    assert_eq!(outcome.change_id, "limited");
    assert_eq!(outcome.mutation, QueueMutation::Added);
    assert!(
        outcome.reducer_changed,
        "the alias applied `RetryError` rather than settling as a no-op"
    );
    assert_ne!(
        outcome.display_status, "error",
        "the terminal error was consumed by the alias"
    );

    let after = fixture.snapshot("limited").await;
    assert_eq!(
        after.explicit_retries,
        vec!["limited".to_string()],
        "the alias publishes the same single edge individual retry does"
    );
    assert_eq!(fixture.retained_limit("limited").await, None);
}

/// A mark is next-run intent, not retry authorization: nothing is consumed.
#[tokio::test]
async fn settled_apply_limit_marking_alone_retries_nothing() {
    let (fixture, _boundary) = limited_fixture(&["limited"], &["limited"]).await;
    let before = fixture.snapshot("limited").await;

    let outcome = fixture
        .service
        .set_execution_mark("limited", true)
        .await
        .expect("a settled limited row still accepts next-run intent");
    assert_eq!(
        outcome,
        OperatorOutcome::MarkSet {
            change_id: "limited".to_string(),
            marked: true,
        }
    );

    let after = fixture.snapshot("limited").await;
    assert_eq!(
        after.display_status, before.display_status,
        "the terminal error survives a mark"
    );
    assert_eq!(after.error_detail, before.error_detail);
    assert_eq!(after.queue_intent_queued, before.queue_intent_queued);
    assert_eq!(
        after.explicit_retries, before.explicit_retries,
        "a mark must not publish an explicit-retry edge"
    );
    assert_eq!(
        fixture.retained_limit("limited").await,
        Some((50, 50)),
        "and the diagnostic stays observable until explicit intent consumes it"
    );
}

#[tokio::test]
async fn settled_apply_limit_leaves_ordinary_retry_unchanged() {
    let (fixture, _boundary) = limited_fixture(&["limited", "ordinary"], &["limited"]).await;
    fixture.to_error("ordinary").await;

    let plan = fixture
        .service
        .retry_change("ordinary")
        .await
        .expect("an unrelated terminal error is still retryable");

    assert_eq!(plan.change_ids, vec!["ordinary".to_string()]);
    assert_eq!(plan.routes, vec![RetryRoute::TerminalError]);
    assert_eq!(
        fixture.queue.explicit_retries(),
        vec!["ordinary".to_string()],
        "only the requested target releases its failed classification"
    );
    assert_eq!(
        fixture.retained_limit("limited").await,
        Some((50, 50)),
        "an unrelated retry never touches another row's diagnostic"
    );
}

/// Unit: a settled limited row classifies exactly as an ordinary error row, and
/// a bulk mark still publishes no retry edge and no queue mutation for it.
#[tokio::test]
async fn settled_apply_limit_bulk_mark_adds_no_exclusion_of_its_own() {
    let (fixture, _boundary) = limited_fixture(&["limited", "ordinary"], &["limited"]).await;
    fixture.to_error("ordinary").await;
    let before = fixture.snapshot("limited").await;

    let outcome = fixture
        .service
        .set_all_execution_marks()
        .await
        .expect("a bulk mutation is always admitted");

    match outcome {
        OperatorOutcome::BulkMarks {
            marked,
            changed,
            excluded,
        } => {
            assert!(marked, "an unmarked eligible row means mark-all");
            assert!(
                changed.contains(&"limited".to_string())
                    && changed.contains(&"ordinary".to_string()),
                "both terminal-error rows are mark targets: {changed:?}"
            );
            assert!(
                excluded.is_empty(),
                "the retained diagnostic adds no exclusion of its own: {excluded:?}"
            );
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    let after = fixture.snapshot("limited").await;
    assert_eq!(after.display_status, before.display_status);
    assert_eq!(after.error_detail, before.error_detail);
    assert_eq!(after.queue_intent_queued, before.queue_intent_queued);
    assert_eq!(
        after.explicit_retries, before.explicit_retries,
        "a bulk mark must not publish an explicit-retry edge for a limited row"
    );
    assert_eq!(fixture.retained_limit("limited").await, Some((50, 50)));
}

/// The retired exclusion has no token, no reason string, and no membership.
#[test]
fn settled_apply_limit_is_not_a_mark_exclusion_reason() {
    for exclusion in MarkExclusion::ALL {
        assert_ne!(
            exclusion.as_str(),
            "apply_iteration_limit_active",
            "no published exclusion token may reintroduce the retired gate"
        );
    }
    assert_eq!(
        MarkExclusion::ALL.len(),
        8,
        "the retired exclusion is gone from the display grouping order"
    );
}

/// The diagnostic is retained until explicit intent consumes it, and the finish
/// hook's cumulative evidence reads the same record while it is retained.
#[tokio::test]
async fn settled_apply_limit_diagnostic_stays_observable_until_explicit_retry() {
    let (fixture, _boundary) = limited_fixture(&["limited"], &["limited"]).await;

    assert_eq!(
        fixture.state.read().await.parallel_finish_report(),
        ("iteration_limit", 50),
        "the finish-hook owner observes the exact cumulative count"
    );
    let before = fixture.snapshot("limited").await;
    assert_eq!(
        before.error_detail.as_deref(),
        Some("max iterations reached"),
        "the operator-visible error detail is retained too"
    );

    // Read-only classification is not intent, so it consumes nothing.
    assert_eq!(
        fixture
            .service
            .plan_retry_change("limited")
            .await
            .expect("classification is accepted"),
        Some(RetryRoute::TerminalError)
    );
    assert_eq!(fixture.snapshot("limited").await, before);
    assert_eq!(fixture.retained_limit("limited").await, Some((50, 50)));

    fixture
        .service
        .retry_change("limited")
        .await
        .expect("explicit retry is admitted");
    assert_eq!(
        fixture.state.read().await.parallel_finish_report(),
        ("completed", 0),
        "the consumed diagnostic no longer describes a live invocation"
    );
}

#[tokio::test]
async fn settled_apply_limit_bulk_retry_includes_the_limited_target() {
    let (fixture, boundary) = limited_fixture(&["limited", "ordinary", "held"], &["limited"]).await;
    fixture.to_error("ordinary").await;
    fixture
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::AcceptanceGated {
            change_id: "held".to_string(),
            blocker: acceptance_blocker(),
        });

    let plan = fixture
        .service
        .retry_errors(&[
            "limited".to_string(),
            "ordinary".to_string(),
            "held".to_string(),
        ])
        .await;

    assert!(boundary.boundary_running());
    assert_eq!(
        plan.change_ids,
        vec![
            "limited".to_string(),
            "ordinary".to_string(),
            "held".to_string()
        ],
        "the settled limited target is accepted alongside unrelated ones"
    );
    assert_eq!(
        plan.routes,
        vec![
            RetryRoute::TerminalError,
            RetryRoute::TerminalError,
            RetryRoute::AcceptanceStall
        ],
        "each target keeps its own independent route"
    );
    assert_eq!(
        fixture.queue.explicit_retry_edges(),
        vec![
            ("limited".to_string(), RetryEdgeAuthority::TerminalError),
            ("ordinary".to_string(), RetryEdgeAuthority::TerminalError),
            ("held".to_string(), RetryEdgeAuthority::AnalysisBypass),
        ],
        "each admitted target arms exactly one edge, carrying its own route's authority"
    );
    assert_eq!(fixture.retained_limit("limited").await, None);
}

/// A genuinely unsupported target in the same request still keeps its evidence.
#[tokio::test]
async fn settled_apply_limit_bulk_retry_still_excludes_unsupported_targets() {
    let (fixture, _boundary) = limited_fixture(&["limited", "idle"], &["limited"]).await;
    let before_idle = fixture.snapshot("idle").await;

    let plan = fixture
        .service
        .retry_errors(&["limited".to_string(), "idle".to_string()])
        .await;

    assert_eq!(
        plan.change_ids,
        vec!["limited".to_string()],
        "an idle row carries no retryable evidence"
    );
    assert_eq!(
        fixture.snapshot("idle").await.display_status,
        before_idle.display_status
    );
    assert_eq!(
        fixture.queue.explicit_retries(),
        vec!["limited".to_string()]
    );
}

// ============================================================================
// Changes row layout: reducer-recorded archive completion retires mark intent
// ============================================================================
//
// The classifier cases are unit evidence over pure functions. The service cases
// drive a real `OrchestratorState` through real `ChangeArchived` /
// `ResolveStarted` events and then submit real commands, so they are integration
// evidence for the caller-supplied-evidence contract: orchestration reads
// `OrchestratorState::archived_changes` from the same snapshot as the display
// status, never a frontend cache.

/// Live post-archive display statuses an archive-complete row can hold.
const POST_ARCHIVE_LIVE_STATUSES: [&str; 3] = ["resolving", "resolve pending", "merge wait"];

/// Unit: archive evidence excludes a row whose display status is still live, and
/// it reports its own stable reason rather than borrowing terminality's.
#[test]
fn tui_change_row_layout_mark_contract_archive_evidence_excludes_live_post_archive_rows() {
    for status in POST_ARCHIVE_LIVE_STATUSES {
        assert_eq!(
            classify_mark_admission(status, true),
            MarkAdmission::ArchiveComplete,
            "'{status}' with an archive record is not a run candidate"
        );
        assert!(!is_markable_status(status, true));
        assert_eq!(
            classify_bulk_mark_row(status, true),
            Some(MarkExclusion::ArchiveComplete),
            "'{status}' reports the archive-complete reason"
        );

        // The control: the identical status without the record stays markable.
        assert_eq!(
            classify_mark_admission(status, false),
            MarkAdmission::Allowed,
            "'{status}' with no archive record is still a run candidate"
        );
        assert!(is_markable_status(status, false));
        assert_eq!(classify_bulk_mark_row(status, false), None);
    }

    // Terminality wins where both hold: that is the reason an operator reads.
    for status in TERMINAL_STATUSES {
        assert_eq!(
            classify_mark_admission(status, true),
            MarkAdmission::TerminalTarget
        );
        assert_eq!(
            classify_bulk_mark_row(status, true),
            Some(MarkExclusion::FinalStatus)
        );
    }

    assert_eq!(
        MarkExclusion::ArchiveComplete.as_str(),
        "archive_complete",
        "the token is stable: clients branch on it rather than on prose"
    );
    assert!(MarkExclusion::ALL.contains(&MarkExclusion::ArchiveComplete));
}

/// Unit: a bulk plan derives its target state from the remaining rows only.
#[test]
fn tui_change_row_layout_mark_contract_bulk_plan_excludes_archive_complete_rows() {
    let rows = [
        row("idle", "not queued", true),
        // Same status, opposite evidence: only the record separates them.
        row("fresh-resolving", "resolving", true),
        archived_row("archived-resolving", "resolving", false),
        archived_row("archived-wait", "merge wait", true),
    ];

    let plan = plan_bulk_marks(&rows);

    assert_eq!(plan.eligible, vec!["idle", "fresh-resolving"]);
    assert_eq!(
        plan.excluded,
        vec![
            (
                "archived-resolving".to_string(),
                MarkExclusion::ArchiveComplete
            ),
            ("archived-wait".to_string(), MarkExclusion::ArchiveComplete),
        ]
    );
    assert!(
        !plan.target_state,
        "every *eligible* row was marked, so the unmarked archive-complete row \
         must not flip the derived state"
    );
    assert!(plan.exclusion_summary().contains("archive complete"));
}

/// Integration: the service reads the reducer's archive record in the same
/// snapshot as the display status and settles a single mark as an unchanged
/// reasoned no-op.
#[tokio::test]
async fn tui_change_row_layout_mark_contract_single_mark_on_an_archive_complete_row_is_unchanged() {
    let fixture = fixture(&["archived-change", "fresh-change"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ChangeArchived(
            "archived-change".to_string(),
        ));
        // The control: a resolve retry that archived nothing in this process.
        guard.apply_execution_event(&ExecutionEvent::ResolveStarted {
            change_id: "fresh-change".to_string(),
            command: "resolve".to_string(),
        });
    }
    {
        let guard = fixture.state.read().await;
        assert!(guard.is_archived("archived-change"));
        assert!(!guard.is_archived("fresh-change"));
        // Both rows display the same live status; only the record differs.
        assert_eq!(guard.display_status("archived-change"), "resolving");
        assert_eq!(guard.display_status("fresh-change"), "resolving");
    }

    let before = fixture.snapshot("archived-change").await;

    let outcome = fixture
        .service
        .set_execution_mark("archived-change", true)
        .await
        .expect("an archive-complete target settles rather than failing");

    assert_eq!(
        outcome,
        OperatorOutcome::NoOp {
            change_id: "archived-change".to_string(),
            reason: NoOpReason::ArchiveCompleteMarkTarget,
        }
    );
    assert_eq!(
        fixture.snapshot("archived-change").await,
        before,
        "no mark, queue, runtime, retry, hook, or scheduler effect may occur"
    );

    // The control row accepts the identical request.
    assert_eq!(
        fixture
            .service
            .set_execution_mark("fresh-change", true)
            .await
            .expect("a resolve retry with no archive record is markable"),
        OperatorOutcome::MarkSet {
            change_id: "fresh-change".to_string(),
            marked: true,
        }
    );
    assert_eq!(fixture.marks.marked_ids(), vec!["fresh-change".to_string()]);
}

/// Integration: bulk marks exclude the archive-complete row before mutating, and
/// apply one coherent state to the rest.
#[tokio::test]
async fn tui_change_row_layout_mark_contract_bulk_mark_excludes_archive_complete_rows() {
    let fixture = fixture(&["archived-change", "fresh-change", "idle-change"]);
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ChangeArchived(
            "archived-change".to_string(),
        ));
        guard.apply_execution_event(&ExecutionEvent::ResolveStarted {
            change_id: "fresh-change".to_string(),
            command: "resolve".to_string(),
        });
    }

    let before_archived = fixture.snapshot("archived-change").await;

    let outcome = fixture
        .service
        .set_all_execution_marks()
        .await
        .expect("a bulk mark with eligible rows is accepted");

    let OperatorOutcome::BulkMarks {
        marked,
        changed,
        excluded,
    } = outcome
    else {
        panic!("expected a bulk-mark outcome, got {outcome:?}");
    };
    assert!(marked);
    let mut changed = changed;
    changed.sort();
    assert_eq!(
        changed,
        vec!["fresh-change".to_string(), "idle-change".to_string()]
    );
    assert_eq!(
        excluded,
        vec![(
            "archived-change".to_string(),
            MarkExclusion::ArchiveComplete
        )]
    );
    assert!(
        !fixture
            .marks
            .marked_ids()
            .contains(&"archived-change".to_string()),
        "the excluded row must not receive the derived mark"
    );
    assert_eq!(
        fixture
            .snapshot("archived-change")
            .await
            .queue_intent_queued,
        before_archived.queue_intent_queued,
        "and nothing else about it moves either"
    );
}

// ============================================================================
// Targeted force-stop: eligibility, target-only cancellation, settlement
// ============================================================================

/// Managed-ownership double for the targeted force-stop path.
///
/// It models exactly what the port can answer — which changes own a live
/// managed process group, and what killing one proves — and records every
/// change it was asked to kill. Recording the *calls* is what lets a test prove
/// an unrelated change was never signalled, which a double that only tracked
/// resulting state could not distinguish from a signal that happened to fail.
struct FakeManagedProcesses {
    live: StdMutex<std::collections::HashSet<String>>,
    /// When set, every kill reports this unconfirmed evidence instead.
    unconfirmed: Option<String>,
    killed: StdMutex<Vec<String>>,
}

impl FakeManagedProcesses {
    fn with_live(ids: &[&str]) -> Self {
        Self {
            live: StdMutex::new(ids.iter().map(|id| (*id).to_string()).collect()),
            unconfirmed: None,
            killed: StdMutex::new(Vec::new()),
        }
    }

    fn none() -> Self {
        Self::with_live(&[])
    }

    fn unconfirmed(ids: &[&str], detail: &str) -> Self {
        let mut port = Self::with_live(ids);
        port.unconfirmed = Some(detail.to_string());
        port
    }

    fn killed(&self) -> Vec<String> {
        self.killed.lock().expect("killed lock").clone()
    }
}

#[async_trait]
impl ManagedProcessTermination for FakeManagedProcesses {
    async fn owns_managed_process(&self, change_id: &str) -> bool {
        self.live.lock().expect("live lock").contains(change_id)
    }

    async fn kill_managed_process(&self, change_id: &str) -> ImmediateKillEvidence {
        self.killed
            .lock()
            .expect("killed lock")
            .push(change_id.to_string());
        if let Some(detail) = &self.unconfirmed {
            return ImmediateKillEvidence::unconfirmed(1, detail.clone());
        }
        let signalled = self.live.lock().expect("live lock").remove(change_id);
        if signalled {
            ImmediateKillEvidence::confirmed(1)
        } else {
            ImmediateKillEvidence::nothing_to_kill()
        }
    }
}

/// A fixture whose managed-ownership answers the test controls.
fn force_stop_fixture(
    ids: &[&str],
    cancellation: CancellationBehavior,
    managed: Arc<FakeManagedProcesses>,
) -> Fixture {
    let state = shared_state(ids);
    let hooks = Arc::new(RecordingHooks::default());
    let queue = Arc::new(FakeQueue::new(cancellation));
    let marks = Arc::new(ExecutionMarkStore::new());
    let service =
        OperatorCommandService::new(state.clone(), queue.clone(), hooks.clone(), marks.clone())
            .with_cancellation_timeout(Duration::from_millis(50))
            .with_managed_termination(managed);
    Fixture {
        service,
        state,
        hooks,
        queue,
        marks,
    }
}

async fn start_apply(fixture: &Fixture, change_id: &str) {
    fixture
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: change_id.to_string(),
            command: "apply".to_string(),
        });
}

/// The eligibility table, as a pure function.
///
/// Unit evidence: no reducer, no queue, no process. Every row of the published
/// contract is asserted here so a later change to the table cannot silently
/// widen what an immediate kill is offered for.
#[test]
fn force_stop_change_eligibility_covers_every_row() {
    use ForceStopAdmission::{DequeueOnly, KillAndDequeue, Refused};
    use ForceStopExclusion as X;

    // An untracked target is refused before any status is even consulted.
    assert_eq!(
        classify_force_stop_change("not queued", false, false),
        Refused(X::UnknownTarget)
    );
    assert_eq!(
        classify_force_stop_change("applying", false, true),
        Refused(X::UnknownTarget),
        "a live process is not evidence that the owner tracks the change"
    );

    for terminal in ["archived", "merged", "pushed", "rejected"] {
        assert_eq!(
            classify_force_stop_change(terminal, true, false),
            Refused(X::TerminalTarget),
            "{terminal}"
        );
    }

    // Every active row: eligible exactly while a managed process is owned.
    for active in ACTIVE_STATUSES {
        assert_eq!(
            classify_force_stop_change(active, true, true),
            KillAndDequeue,
            "{active} with a live process"
        );
        assert_eq!(
            classify_force_stop_change(active, true, false),
            Refused(X::NoLiveProcess),
            "{active} without one"
        );
    }

    // Admitted with nothing running: dequeue-only, signalling nothing.
    for admitted in ["queued", "blocked"] {
        assert_eq!(
            classify_force_stop_change(admitted, true, false),
            DequeueOnly,
            "{admitted}"
        );
        assert_eq!(
            classify_force_stop_change(admitted, true, true),
            KillAndDequeue,
            "{admitted} that started a phase between the read and the request"
        );
    }

    assert_eq!(
        classify_force_stop_change("merge wait", true, false),
        Refused(X::MergeWait)
    );
    assert_eq!(
        classify_force_stop_change("resolve pending", true, false),
        Refused(X::ResolveWait)
    );

    // Unadmitted rows, including the parked ones an operator has to act on.
    for unadmitted in ["not queued", "error", "stopped", "stalled"] {
        assert_eq!(
            classify_force_stop_change(unadmitted, true, false),
            Refused(X::NotAdmitted),
            "{unadmitted}"
        );
    }
}

/// Exclusion tokens are stable, distinct, and machine-readable.
#[test]
fn force_stop_change_exclusion_tokens_are_a_closed_stable_set() {
    let mut seen = std::collections::BTreeSet::new();
    for exclusion in ForceStopExclusion::ALL {
        let token = exclusion.as_str();
        assert!(seen.insert(token), "{token} appears twice");
        assert!(
            token.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            "{token} is not snake_case"
        );
        assert!(!exclusion.reason().is_empty());
    }
    assert_eq!(seen.len(), ForceStopExclusion::ALL.len());
}

/// The whole point: one concurrent change is killed, the other is untouched.
#[tokio::test]
async fn force_stop_change_kills_only_the_named_target() {
    let managed = Arc::new(FakeManagedProcesses::with_live(&["alpha", "beta"]));
    let fixture = force_stop_fixture(
        &["alpha", "beta"],
        CancellationBehavior::ConfirmedImmediately,
        managed.clone(),
    );
    start_apply(&fixture, "alpha").await;
    start_apply(&fixture, "beta").await;
    fixture.marks.set("alpha", true);
    fixture.marks.set("beta", true);

    let outcome = fixture
        .service
        .force_stop_change("alpha")
        .await
        .expect("an applying change with a live process can be force-stopped");

    assert_eq!(
        outcome,
        OperatorOutcome::ForceStopped {
            change_id: "alpha".to_string(),
            // No facts store on this fixture, so the episode is honestly
            // unknown rather than invented.
            execution_id: None,
            terminated: true,
            settlement: StopSettlement {
                cancelled_phase: ExecutionPhase::Apply,
                last_completed_phase: None,
                apply_commit_present: None,
                apply_commit_oid: None,
            },
        }
    );

    // Exactly one change was ever signalled, and one cancellation issued.
    assert_eq!(managed.killed(), vec!["alpha".to_string()]);
    assert_eq!(
        fixture
            .queue
            .cancellations
            .lock()
            .expect("cancellations lock")
            .clone(),
        vec!["alpha".to_string()]
    );

    let after_alpha = fixture.snapshot("alpha").await;
    assert_eq!(after_alpha.display_status, "not queued");
    assert_eq!(
        after_alpha.marks,
        vec!["beta".to_string()],
        "the target's mark is revoked and only the target's"
    );

    let after_beta = fixture.snapshot("beta").await;
    assert_eq!(
        after_beta.display_status, "applying",
        "the unrelated change keeps running"
    );
    assert!(
        managed.owns_managed_process("beta").await,
        "and keeps its managed process group"
    );
}

/// Later mark settlement cannot re-admit a force-stopped change.
#[tokio::test]
async fn force_stop_change_revokes_the_mark_so_settlement_cannot_redispatch() {
    let managed = Arc::new(FakeManagedProcesses::with_live(&["alpha"]));
    let fixture = force_stop_fixture(
        &["alpha"],
        CancellationBehavior::ConfirmedImmediately,
        managed,
    );
    start_apply(&fixture, "alpha").await;
    fixture.marks.set("alpha", true);

    fixture
        .service
        .force_stop_change("alpha")
        .await
        .expect("force-stop settles");

    assert!(!fixture.marks.is_marked("alpha"));

    // The batch a settlement would run over is derived from current marks, so
    // an unmarked target contributes no addition at all.
    let plan = fixture
        .service
        .plan_mark_settlement(&["alpha".to_string()])
        .await;
    assert!(
        plan.additions.is_empty(),
        "a revoked mark must produce no settlement addition: {plan:?}"
    );
}

/// A dequeue-only target signals nothing at all.
#[tokio::test]
async fn force_stop_change_dequeues_an_admitted_target_without_signalling() {
    let managed = Arc::new(FakeManagedProcesses::none());
    let fixture = force_stop_fixture(
        &["alpha", "beta"],
        CancellationBehavior::NoHandle,
        managed.clone(),
    );
    fixture
        .service
        .add_to_queue("alpha")
        .await
        .expect("queue alpha");
    fixture
        .service
        .add_to_queue("beta")
        .await
        .expect("queue beta");

    let outcome = fixture
        .service
        .force_stop_change("alpha")
        .await
        .expect("an admitted target with no process is dequeued");

    assert_eq!(
        outcome,
        OperatorOutcome::ForceStopped {
            change_id: "alpha".to_string(),
            execution_id: None,
            terminated: false,
            settlement: StopSettlement::none(),
        }
    );
    assert!(
        managed.killed().is_empty(),
        "no process belonging to any change may be signalled"
    );
    assert_eq!(fixture.snapshot("alpha").await.display_status, "not queued");
    assert_eq!(
        fixture.snapshot("beta").await.display_status,
        "queued",
        "the unrelated queued change keeps its admission"
    );
}

/// An ineligible target changes nothing, and never reaches a signal.
#[tokio::test]
async fn force_stop_change_refuses_an_ineligible_target_without_side_effects() {
    let managed = Arc::new(FakeManagedProcesses::none());
    let fixture = force_stop_fixture(
        &["alpha", "beta"],
        CancellationBehavior::ConfirmedImmediately,
        managed.clone(),
    );
    fixture
        .service
        .add_to_queue("beta")
        .await
        .expect("queue beta");
    fixture.marks.set("alpha", true);
    let before_beta = fixture.snapshot("beta").await;

    // `alpha` is tracked but carries no admission at all.
    let error = fixture
        .service
        .force_stop_change("alpha")
        .await
        .expect_err("an unadmitted target is refused");
    assert_eq!(
        error,
        OperatorCommandError::ForceStopIneligible {
            change_id: "alpha".to_string(),
            display_status: "not queued".to_string(),
            reason: ForceStopExclusion::NotAdmitted,
        }
    );

    // An unknown target is refused the same way, and neither refusal signals.
    let unknown = fixture
        .service
        .force_stop_change("never-heard-of-it")
        .await
        .expect_err("an unknown target is refused");
    assert!(matches!(
        unknown,
        OperatorCommandError::ForceStopIneligible {
            reason: ForceStopExclusion::UnknownTarget,
            ..
        }
    ));

    assert!(managed.killed().is_empty());
    assert!(fixture
        .queue
        .cancellations
        .lock()
        .expect("cancellations lock")
        .is_empty());
    assert!(
        fixture.marks.is_marked("alpha"),
        "a refused force-stop must not revoke a mark"
    );
    assert_eq!(fixture.snapshot("beta").await, before_beta);
}

/// An active row without a managed process is refused rather than half-killed.
#[tokio::test]
async fn force_stop_change_refuses_an_active_row_that_owns_no_process() {
    let managed = Arc::new(FakeManagedProcesses::none());
    let fixture = force_stop_fixture(
        &["alpha"],
        CancellationBehavior::ConfirmedImmediately,
        managed.clone(),
    );
    start_apply(&fixture, "alpha").await;

    let error = fixture
        .service
        .force_stop_change("alpha")
        .await
        .expect_err("there is nothing to kill");

    assert_eq!(
        error,
        OperatorCommandError::ForceStopIneligible {
            change_id: "alpha".to_string(),
            display_status: "applying".to_string(),
            reason: ForceStopExclusion::NoLiveProcess,
        }
    );
    assert!(managed.killed().is_empty());
    assert_eq!(
        fixture.snapshot("alpha").await.display_status,
        "applying",
        "a refused force-stop preserves active state"
    );
}

/// A group that will not empty refuses settlement rather than dequeuing on top
/// of a process that may still be writing to the worktree.
#[tokio::test]
async fn force_stop_change_refuses_to_settle_an_unproven_process_group() {
    let managed = Arc::new(FakeManagedProcesses::unconfirmed(
        &["alpha"],
        "pgid=4242: members were still alive after SIGKILL",
    ));
    let fixture = force_stop_fixture(
        &["alpha"],
        CancellationBehavior::ConfirmedImmediately,
        managed.clone(),
    );
    start_apply(&fixture, "alpha").await;
    fixture.marks.set("alpha", true);

    let error = fixture
        .service
        .force_stop_change("alpha")
        .await
        .expect_err("an unproven group must not settle");

    assert!(matches!(
        error,
        OperatorCommandError::ForceStopUnconfirmed { .. }
    ));
    assert!(
        error.to_string().contains("pgid=4242"),
        "the refusal must carry the identity that survived: {error}"
    );
    assert_eq!(managed.killed(), vec!["alpha".to_string()]);
    assert!(
        fixture
            .queue
            .cancellations
            .lock()
            .expect("cancellations lock")
            .is_empty(),
        "cancellation is not issued after an unproven kill"
    );
    assert_eq!(
        fixture.snapshot("alpha").await.display_status,
        "applying",
        "no dequeue is committed on an unproven group"
    );
    assert!(fixture.marks.is_marked("alpha"));
}

/// The settlement reports the phase that was really interrupted, read at commit.
#[tokio::test]
async fn force_stop_change_reports_the_phase_it_actually_cancelled() {
    let managed = Arc::new(FakeManagedProcesses::with_live(&["alpha"]));
    let fixture = force_stop_fixture(
        &["alpha"],
        CancellationBehavior::ConfirmedImmediately,
        managed,
    );
    {
        let mut guard = fixture.state.write().await;
        guard.apply_execution_event(&ExecutionEvent::ArchiveStarted {
            change_id: "alpha".to_string(),
            command: "archive".to_string(),
        });
    }

    let outcome = fixture
        .service
        .force_stop_change("alpha")
        .await
        .expect("archiving is an eligible phase");

    let OperatorOutcome::ForceStopped { settlement, .. } = outcome else {
        panic!("a targeted force-stop settles as ForceStopped");
    };
    assert_eq!(settlement.cancelled_phase, ExecutionPhase::Archive);
}

/// A never-terminating task fails the settlement; the kill is not rolled back.
#[tokio::test]
async fn force_stop_change_fails_when_the_task_never_reports_completion() {
    let managed = Arc::new(FakeManagedProcesses::with_live(&["alpha"]));
    let fixture = force_stop_fixture(
        &["alpha"],
        CancellationBehavior::NeverTerminates,
        managed.clone(),
    );
    start_apply(&fixture, "alpha").await;

    let error = fixture
        .service
        .force_stop_change("alpha")
        .await
        .expect_err("an unconfirmed task handshake must not dequeue");

    assert!(matches!(
        error,
        OperatorCommandError::TerminationTimeout { .. }
    ));
    assert_eq!(managed.killed(), vec!["alpha".to_string()]);
    assert_eq!(
        fixture.snapshot("alpha").await.display_status,
        "applying",
        "no dequeue is committed without the task's own completion evidence"
    );
}

/// Published eligibility and admission read one table.
#[tokio::test]
async fn force_stop_change_admission_matches_published_eligibility() {
    let managed = Arc::new(FakeManagedProcesses::with_live(&["alpha"]));
    let fixture = force_stop_fixture(
        &["alpha", "beta"],
        CancellationBehavior::ConfirmedImmediately,
        managed,
    );
    start_apply(&fixture, "alpha").await;
    start_apply(&fixture, "beta").await;

    assert_eq!(
        fixture.service.force_stop_admission("alpha").await,
        ForceStopAdmission::KillAndDequeue
    );
    assert_eq!(
        fixture.service.force_stop_admission("beta").await,
        ForceStopAdmission::Refused(ForceStopExclusion::NoLiveProcess),
        "beta presents as active but owns nothing this owner can signal"
    );
    assert_eq!(
        fixture.service.force_stop_admission("unknown").await,
        ForceStopAdmission::Refused(ForceStopExclusion::UnknownTarget)
    );
}

/// The episode identity is fixed before cancellation, not re-read after it.
#[tokio::test]
async fn force_stop_change_reports_the_execution_id_it_cancelled() {
    let managed = Arc::new(FakeManagedProcesses::with_live(&["alpha"]));
    let state = shared_state(&["alpha"]);
    let hooks = Arc::new(RecordingHooks::default());
    let queue = Arc::new(FakeQueue::new(CancellationBehavior::ConfirmedImmediately));
    let marks = Arc::new(ExecutionMarkStore::new());
    let facts = Arc::new(ExecutionFactsStore::new());
    let started = ExecutionEvent::ApplyStarted {
        change_id: "alpha".to_string(),
        command: "apply".to_string(),
    };
    {
        let mut guard = state.write().await;
        guard.apply_execution_event(&started);
        facts.observe(1, &started, Some(&guard), chrono::Utc::now());
    }
    let expected = facts
        .execution_id("alpha")
        .expect("an admitted phase opens an episode");
    let service =
        OperatorCommandService::new(state.clone(), queue.clone(), hooks.clone(), marks.clone())
            .with_cancellation_timeout(Duration::from_millis(50))
            .with_managed_termination(managed)
            .with_execution_facts(facts);

    let outcome = service
        .force_stop_change("alpha")
        .await
        .expect("force-stop settles");

    let OperatorOutcome::ForceStopped { execution_id, .. } = outcome else {
        panic!("a targeted force-stop settles as ForceStopped");
    };
    assert_eq!(execution_id, Some(expected));
}

/// The targeted path never constructs a process-wide control.
///
/// A source guard rather than a behavioral one, because the property is an
/// absence: no runtime observation can prove the code *cannot* reach process-wide
/// ForceStop, but reading the module can.
#[test]
fn force_stop_change_never_reaches_process_wide_force_stop() {
    let source = include_str!("../operator_command.rs");
    let targeted = source
        .split("Targeted force-stop\n    // ----")
        .nth(1)
        .expect("the targeted force-stop section exists");
    let section = targeted
        .split("/// Whether a typed push episode is open")
        .next()
        .expect("the section ends at the next helper");
    for forbidden in [
        // Process-wide lifecycle: the intent, the mode, and the run boundary.
        "OperatorIntent",
        "OperatorCommand::ForceStop",
        "OperatorMode",
        "RunControl",
        "boundary_running",
        // Whole-set mark writes, which would move marks nobody named.
        "marks.replace(",
        "marks.clear()",
        "marked_ids()",
        // Unscoped process addressing.
        "kill_process_group_immediately",
        "libc::",
        "nix::",
    ] {
        assert!(
            !section.contains(forbidden),
            "the targeted force-stop path must not reach `{forbidden}`"
        );
    }
    // The single kill route, named positively so removing the port would fail
    // here rather than quietly leaving the guard above vacuously true.
    assert!(
        section.contains(".kill_managed_process("),
        "the only kill route is the managed ownership port"
    );
}
