//! Immediate Running feedback for an accepted persistent-idle Start.
//!
//! The change these cover moves one decision — "is this woken dispatch the
//! operator's accepted Start?" — into a shared gate that Core, the TUI, and Web
//! all read. The risk that creates is divergence: one frontend opening the run
//! episode while another stays Ready, or a refusal being optimistically painted
//! as Running by whichever adapter submitted it.
//!
//! So every case here is arranged as one process with a real `CoreMode`, a real
//! `AppState`, and a real `WebState` on a single dispatch owner, and the
//! persistent-idle episode is produced by dispatching the *typed* idle event
//! rather than by writing the fact into three places. What is asserted is one
//! value per case: the mode all three settled in, the idle-episode fact, the
//! reducer-derived row status, the scheduler calls, and how many authoritative
//! dispatches and projection revisions the command produced.
//!
//! Integration-scoped by the shared `WebState` projection; the scheduler is the
//! recording double and the queue is the in-memory `DynamicQueue`, so no
//! repository, process, or network is touched.

use std::sync::Arc;

use super::tests::{create_test_change, AdapterHarness};
use super::*;

use crate::events::{ExecutionEvent, OperatorCommandEffect};
use crate::orchestration::operator_command::{OperatorMode, ParallelEligibility};
use crate::orchestration::run_control::testing::SchedulerCall;
use crate::tui::state::AppState;
use crate::tui::types::{AppExecutionMode, StopMode};
use crate::web::remote_control_api::dto::{CommandSpec, ErrorCode};
use crate::web::remote_control_api::executor::{RemoteControlExecutor, SharedServiceExecutor};
use crate::web::state::{WebEventSink, WebState};

/// The changes every case is arranged over: one retry-eligible, one ordinary.
const CHANGES: [&str; 2] = ["alpha", "beta"];
const ALPHA: &str = "alpha";
const BETA: &str = "beta";

// ============================================================================
// Observation
// ============================================================================

/// The authoritative run-dispatch facts one command published.
///
/// Read from the dispatched effect rather than an adapter's return value: that
/// event is the shared decision the three projections consume, so a case that
/// asserted on a summary instead would stop proving they consume the same one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DispatchedRun {
    change_ids: Vec<String>,
    explicit_retry: bool,
    scheduler_started: bool,
}

/// Records every accepted run dispatch the boundary fanned out.
#[derive(Default)]
struct RunRecorder {
    runs: std::sync::Mutex<Vec<DispatchedRun>>,
}

impl RunRecorder {
    fn runs(&self) -> Vec<DispatchedRun> {
        self.runs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

#[async_trait::async_trait]
impl crate::events::EventSink for RunRecorder {
    async fn on_event(&self, event: &ExecutionEvent) {
        if let ExecutionEvent::OperatorCommandApplied {
            effect:
                OperatorCommandEffect::RunDispatched {
                    change_ids,
                    explicit_retry,
                    scheduler_started,
                },
        } = event
        {
            self.runs
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(DispatchedRun {
                    change_ids: change_ids.clone(),
                    explicit_retry: *explicit_retry,
                    scheduler_started: *scheduler_started,
                });
        }
    }

    async fn on_state_changed(&self, _state: &crate::orchestration::state::OrchestratorState) {}
}

/// One convergent observation of the whole process.
///
/// Core, TUI, and Web each contribute their mode *and* their idle-episode fact,
/// so the two halves of the projection are compared together — a frontend that
/// reached Running while leaving the episode open would describe a state the
/// process cannot be in.
#[derive(Debug, PartialEq, Eq)]
struct Settled {
    /// The one process lifecycle mode Core admits commands against.
    core_mode: OperatorMode,
    /// What the TUI would render as its execution mode.
    tui_mode: AppExecutionMode,
    /// Whether the TUI still holds an open idle episode.
    tui_idle: bool,
    /// The `app_mode` an `/api/v2` client would read.
    web_mode: String,
    /// Whether an `/api/v2` client would still read an open idle episode.
    web_idle: bool,
    /// Accepted run dispatches, with their targets and retry semantics.
    runs: Vec<DispatchedRun>,
    /// Every scheduler interaction, in order.
    scheduler: Vec<SchedulerCall>,
    /// Reducer display status per change: where the admitted rows really are.
    statuses: Vec<(String, String)>,
    /// Target-specific explicit-retry edges the command published.
    explicit_retries: Vec<String>,
    /// How many authoritative dispatches the command produced.
    dispatches: usize,
    /// How many projection revisions the command produced.
    revisions: u64,
}

/// One arranged process with Core and both frontends on a single dispatch owner.
struct Wired {
    harness: AdapterHarness,
    app: AppState,
    web: Arc<WebState>,
    runs: Arc<RunRecorder>,
    baseline_revision: u64,
    baseline_dispatches: usize,
}

impl Wired {
    async fn settled(&self) -> Settled {
        let statuses = {
            let guard = self.harness.state.read().await;
            CHANGES
                .iter()
                .map(|id| ((*id).to_string(), guard.display_status(id).to_string()))
                .collect()
        };
        let web = self.web.get_state().await;
        Settled {
            core_mode: self.harness.core_mode.get(),
            tui_mode: self.app.execution_mode,
            tui_idle: self.app.persistent_scheduler_idle,
            web_mode: web.app_mode,
            web_idle: web.persistent_scheduler_idle,
            runs: self.runs.runs(),
            scheduler: self.harness.scheduler.calls(),
            statuses,
            // Draining is the queue's only read and this is the process's last
            // observation, so nothing downstream can miss an edge.
            explicit_retries: self.harness.queue.drain_explicit_retries().await,
            dispatches: self.harness.dispatch_count() - self.baseline_dispatches,
            revisions: self.web.remote_control().projection().revision() - self.baseline_revision,
        }
    }

    /// Deliver one authoritative event through the owner both frontends read.
    async fn dispatch(&mut self, event: ExecutionEvent) {
        self.harness.dispatcher.dispatch(event).await;
        self.harness.deliver(&mut self.app).await;
    }
}

// ============================================================================
// Arrangement
// ============================================================================

/// The evidence a case's marks carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setup {
    /// Both rows are marked and ordinary: the plain accepted-Start case.
    MarkedOrdinary,
    /// `alpha` carries retry-eligible terminal Error evidence and is marked.
    MarkedRetryableError,
    /// Nothing is marked, so Start resolves an empty target set.
    Unmarked,
    /// Both rows are marked but neither may execute in parallel.
    MarkedIneligible,
}

async fn arrange(harness: &AdapterHarness, setup: Setup) {
    let mark_all = || {
        harness
            .marks
            .replace(CHANGES.iter().map(|id| (*id).to_string()))
    };

    match setup {
        Setup::MarkedOrdinary => mark_all(),
        Setup::MarkedRetryableError => {
            harness
                .state
                .write()
                .await
                .apply_execution_event(&ExecutionEvent::ProcessingError {
                    id: ALPHA.to_string(),
                    error: "apply reached its absolute runtime limit".to_string(),
                });
            harness.marks.replace([ALPHA.to_string()]);
        }
        Setup::Unmarked => {}
        Setup::MarkedIneligible => {
            mark_all();
            harness
                .parallel
                .set_parallel_ineligible(CHANGES.iter().map(|id| {
                    (
                        (*id).to_string(),
                        ParallelEligibility::UncommittedProposalFiles,
                    )
                }));
        }
    }
}

/// Build the arranged process, parked in a persistent-idle episode.
///
/// `scheduler_live` false is the stale-presentation case: the frontends hold an
/// idle fact for a scheduler that has since exited, which is precisely the state
/// the projection must not treat as scheduler-wake evidence.
async fn arranged(setup: Setup, scheduler_live: bool) -> Wired {
    let harness = AdapterHarness::new(&CHANGES);
    arrange(&harness, setup).await;
    harness.scheduler.set_running(scheduler_live);
    let ineligible = harness.parallel.ineligible_ids();

    let mut app = harness.app(&CHANGES);
    for change in &mut app.changes {
        change.parallel_eligibility = if ineligible.contains(&change.id) {
            ParallelEligibility::UncommittedProposalFiles
        } else {
            ParallelEligibility::Eligible
        };
    }
    app.publish_parallel_runtime();
    app.apply_display_statuses_from_reducer(&harness.state.read().await.all_display_statuses());
    app.sync_execution_marks_from_store();
    // The episode is *entered* below rather than written here: a run that was
    // really executing is the only thing the typed idle transition may project
    // Ready from, and arranging the destination directly would skip the guard.
    app.execution_mode = AppExecutionMode::Running;
    app.warning_message = None;

    let web = Arc::new(WebState::new(&[]));
    web.set_shared_state(harness.state.clone()).await;
    web.set_execution_marks(harness.marks.clone()).await;
    web.set_parallel_runtime(harness.parallel.clone()).await;
    web.set_repo_root(std::path::PathBuf::from("/repo")).await;
    let changes: Vec<_> = CHANGES.iter().map(|id| create_test_change(id)).collect();
    web.update_with_mode(&changes, AppExecutionMode::Running.app_mode_token())
        .await;
    web.sync_remote_control_projection().await;

    let runs = Arc::new(RunRecorder::default());
    harness.attach(Arc::new(WebEventSink::new(web.clone())));
    harness.attach(runs.clone());
    harness.attach_revisions(web.clone());
    harness.core_mode.set(OperatorMode::Running);

    let mut wired = Wired {
        harness,
        app,
        web,
        runs,
        baseline_revision: 0,
        baseline_dispatches: 0,
    };

    // The one typed transition that opens the episode, delivered to Core and
    // both frontends from the single owner they share.
    wired
        .dispatch(ExecutionEvent::PersistentSchedulerIdle)
        .await;
    let parked = wired.settled().await;
    assert_eq!(
        (
            parked.core_mode,
            parked.tui_mode,
            parked.tui_idle,
            parked.web_mode.as_str(),
            parked.web_idle
        ),
        (
            OperatorMode::Select,
            AppExecutionMode::Select,
            true,
            "select",
            true
        ),
        "the arrangement is persistent-idle Ready in Core and both frontends"
    );

    wired.baseline_revision = wired.web.remote_control().projection().revision();
    wired.baseline_dispatches = wired.harness.dispatch_count();
    wired
}

/// Run Start through the TUI adapter and report what the process settled in.
async fn start_through_tui(setup: Setup, scheduler_live: bool) -> (Settled, String) {
    let mut wired = arranged(setup, scheduler_live).await;
    wired
        .harness
        .run(&mut wired.app, TuiCommand::StartProcessing(Vec::new()))
        .await;

    let report = wired
        .app
        .warning_message
        .clone()
        .or_else(|| wired.app.logs.last().map(|entry| entry.message.clone()))
        .unwrap_or_default();
    (wired.settled().await, report)
}

/// Run Start through the `/api/v2` executor and report the same.
async fn start_through_v2(
    setup: Setup,
    scheduler_live: bool,
) -> (Settled, std::result::Result<Option<u64>, ErrorCode>) {
    let mut wired = arranged(setup, scheduler_live).await;
    let executor = SharedServiceExecutor::new(wired.harness.application.clone(), wired.web.clone());

    let outcome = match executor.execute(&CommandSpec::Start).await {
        Ok(summary) => Ok(summary.result_revision),
        Err(failure) => Err(failure.error_code),
    };
    // One TUI event-processing pass, exactly as the runner performs it.
    wired.harness.deliver(&mut wired.app).await;

    (wired.settled().await, outcome)
}

// ============================================================================
// The accepted Start
// ============================================================================

/// Verification `idle-start-running-regressions`: an accepted Start against a
/// persistent-idle episode opens the run for Core, the TUI, and Web at once,
/// through the same authoritative dispatch, without spawning a second scheduler.
///
/// Both adapters are compared as one value. The whole defect class this guards
/// against is a projection that reaches the adapter which submitted the command
/// and no further, which a single-adapter assertion cannot see.
#[tokio::test]
async fn idle_start_running_accepted_start_projects_running_on_every_adapter() {
    let (tui, _) = start_through_tui(Setup::MarkedOrdinary, true).await;
    let (v2, outcome) = start_through_v2(Setup::MarkedOrdinary, true).await;

    assert_eq!(
        tui, v2,
        "a keypress and a remote Start must settle the same idle episode identically"
    );
    assert!(
        matches!(outcome, Ok(Some(_))),
        "the accepted Start names the revision its projection landed in, got {outcome:?}"
    );

    assert_eq!(
        (tui.core_mode, tui.tui_mode, tui.web_mode.as_str()),
        (OperatorMode::Running, AppExecutionMode::Running, "running"),
        "the accepted Start opens the run episode everywhere at once"
    );
    assert!(
        !tui.tui_idle && !tui.web_idle,
        "the accepted Start closes the idle presentation episode"
    );

    // Presentation moved; the rows did not. They are queued because the reducer
    // says so, and admitted work has not begun.
    assert_eq!(
        tui.statuses,
        vec![
            (ALPHA.to_string(), "queued".to_string()),
            (BETA.to_string(), "queued".to_string()),
        ],
        "row status comes from reducer queue intent, not from the mode"
    );

    assert_eq!(
        tui.runs,
        vec![DispatchedRun {
            change_ids: vec![ALPHA.to_string(), BETA.to_string()],
            explicit_retry: false,
            scheduler_started: false,
        }],
        "exactly one accepted dispatch, and it woke rather than spawned"
    );
    assert_eq!(
        tui.scheduler,
        vec![SchedulerCall::Notified],
        "the live scheduler is notified and no second scheduler task is spawned"
    );
    assert_eq!(
        (tui.dispatches, tui.revisions),
        (1, 1),
        "one command publishes one authoritative dispatch and one revision"
    );
}

/// Verification `idle-start-running-regressions`: an accepted explicit retry
/// from persistent-idle Ready gets the same immediate projection, and keeps its
/// own routing.
#[tokio::test]
async fn idle_start_running_accepted_retry_projects_running_without_changing_routing() {
    let (tui, _) = start_through_tui(Setup::MarkedRetryableError, true).await;
    let (v2, outcome) = start_through_v2(Setup::MarkedRetryableError, true).await;

    assert_eq!(tui, v2, "retry routing is one shared decision");
    assert!(matches!(outcome, Ok(Some(_))));

    assert_eq!(
        (tui.core_mode, tui.tui_mode, tui.web_mode.as_str()),
        (OperatorMode::Running, AppExecutionMode::Running, "running"),
        "an accepted retry opens the run episode like any other accepted Start"
    );
    assert!(!tui.tui_idle && !tui.web_idle);

    assert_eq!(
        tui.runs,
        vec![DispatchedRun {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler_started: false,
        }],
        "the retry keeps its explicit-retry semantics and its woken scheduler"
    );
    assert_eq!(
        tui.explicit_retries,
        vec![ALPHA.to_string()],
        "the one-shot retry edge is armed exactly once, as before"
    );
    assert_eq!(tui.scheduler, vec![SchedulerCall::Notified]);
}

/// Verification `idle-start-running-regressions`: everything that is not an
/// accepted Start against a live parked scheduler leaves Ready exactly as it was.
///
/// The refusal cases are the whole reason the gate is a conjunction rather than
/// "a woken dispatch means Running", so they are asserted over the same three
/// projections as the acceptance above.
#[tokio::test]
async fn idle_start_running_refused_and_stale_start_stay_ready() {
    // ── No marked target: the command resolves an empty set ────────────────
    let (targetless, report) = start_through_tui(Setup::Unmarked, true).await;
    assert_eq!(
        (
            targetless.core_mode,
            targetless.tui_mode,
            targetless.web_mode.as_str()
        ),
        (OperatorMode::Select, AppExecutionMode::Select, "select"),
        "a Start with nothing to start leaves Ready alone"
    );
    assert!(
        targetless.tui_idle && targetless.web_idle,
        "a refused Start does not close the idle episode"
    );
    assert!(
        targetless.runs.is_empty() && targetless.scheduler.is_empty(),
        "nothing was dispatched, so nothing may be projected"
    );
    assert_eq!(
        (targetless.dispatches, targetless.revisions),
        (0, 0),
        "a refusal publishes no authoritative outcome at all"
    );
    assert!(
        !report.is_empty(),
        "the operator is told the Start did nothing"
    );

    // ── Marked but ineligible: the same refusal from a different cause ─────
    let (ineligible, _) = start_through_tui(Setup::MarkedIneligible, true).await;
    assert_eq!(
        (
            ineligible.core_mode,
            ineligible.tui_mode,
            ineligible.web_mode.as_str()
        ),
        (OperatorMode::Select, AppExecutionMode::Select, "select"),
        "an ineligible target set is not an accepted Start either"
    );
    assert!(ineligible.tui_idle && ineligible.web_idle);
    assert!(ineligible.runs.is_empty());

    // ── A stale idle fact over a scheduler that already exited ─────────────
    //
    // Presentation says "parked"; liveness says otherwise, and liveness is the
    // authority. Start is admitted through the *ordinary* spawn route, so
    // Running here is proven by a scheduler that really started — never by the
    // woken-dispatch gate, which has no scheduler to have woken.
    let (stale, _) = start_through_tui(Setup::MarkedOrdinary, false).await;
    assert_eq!(
        stale.runs,
        vec![DispatchedRun {
            change_ids: vec![ALPHA.to_string(), BETA.to_string()],
            explicit_retry: false,
            scheduler_started: true,
        }],
        "a stale presentation fact cannot make a dead scheduler a woken one"
    );
    assert_eq!(
        stale.scheduler,
        vec![SchedulerCall::Started {
            targets: vec![ALPHA.to_string(), BETA.to_string()],
            explicit_retry: false,
        }],
        "the ordinary route spawns the scheduler the presentation claimed existed"
    );
    assert_eq!(
        (stale.core_mode, stale.tui_mode, stale.web_mode.as_str()),
        (OperatorMode::Running, AppExecutionMode::Running, "running"),
        "a really-started scheduler is Running by the pre-existing rule"
    );
    assert!(!stale.tui_idle && !stale.web_idle);
}

/// Verification `idle-start-running-regressions`: raw key input is intent only.
///
/// The gate lives behind preparation and the reducer commit, so pressing F5
/// cannot move the mode by itself. Asserted by handling the key and settling
/// nothing: the submission is still queued, and every projection is untouched.
#[tokio::test]
async fn idle_start_running_raw_key_input_projects_nothing() {
    let mut wired = arranged(Setup::MarkedOrdinary, true).await;

    let mut ctx = wired.harness.context(&mut wired.app);
    handle_tui_command(
        TuiCommand::StartProcessing(Vec::new()),
        &mut ctx,
        &wired.harness.state,
    )
    .await
    .expect("the key handler accepts the intent");

    let pressed = wired.settled().await;
    assert_eq!(
        (
            pressed.core_mode,
            pressed.tui_mode,
            pressed.web_mode.as_str()
        ),
        (OperatorMode::Select, AppExecutionMode::Select, "select"),
        "the key handler records intent; only an accepted outcome projects Running"
    );
    assert!(pressed.tui_idle && pressed.web_idle);
    assert!(pressed.runs.is_empty());
    assert_eq!((pressed.dispatches, pressed.revisions), (0, 0));

    // And the very same intent, once the transaction actually settles it, does.
    wired.harness.settle(&mut wired.app).await;
    let settled = wired.settled().await;
    assert_eq!(
        (
            settled.core_mode,
            settled.tui_mode,
            settled.web_mode.as_str()
        ),
        (OperatorMode::Running, AppExecutionMode::Running, "running"),
        "acceptance, not the keypress, is what opens the run episode"
    );
}

/// Verification `idle-start-running-regressions`: a non-Start client enqueue
/// admits work without claiming a run episode.
///
/// It commits the same reducer queue intent an accepted Start does, which is
/// exactly why it is the interesting case: if queue addition were the trigger,
/// this would project Running, and the spec says typed admitted-work evidence
/// remains its trigger. The follow-on no-work idle edge is then delivered to a
/// projection that is already Select, and must be a no-op on both frontends.
#[tokio::test]
async fn idle_start_running_non_start_enqueue_does_not_project_running() {
    let mut wired = arranged(Setup::Unmarked, true).await;
    let executor = SharedServiceExecutor::new(wired.harness.application.clone(), wired.web.clone());

    executor
        .execute(&CommandSpec::SetQueueIntent {
            change_id: ALPHA.to_string(),
            queued: true,
        })
        .await
        .expect("a client may add queue intent to a live idle scheduler");
    wired.harness.deliver(&mut wired.app).await;

    let enqueued = wired.settled().await;
    assert_eq!(
        (
            enqueued.core_mode,
            enqueued.tui_mode,
            enqueued.web_mode.as_str()
        ),
        (OperatorMode::Select, AppExecutionMode::Select, "select"),
        "queue admission without an accepted Start outcome is not a run episode"
    );
    assert!(
        enqueued.tui_idle && enqueued.web_idle,
        "the idle episode survives a non-Start queue delta"
    );
    assert!(
        enqueued.runs.is_empty(),
        "no run dispatch exists to be read as an accepted Start"
    );
    assert_eq!(
        enqueued.statuses,
        vec![
            (ALPHA.to_string(), "queued".to_string()),
            (BETA.to_string(), "not queued".to_string()),
        ],
        "the queue intent really was committed"
    );

    // The rearmed no-work idle edge reaches frontends that never left Select.
    let before = wired.settled().await;
    wired
        .dispatch(ExecutionEvent::PersistentSchedulerIdle)
        .await;
    let after = wired.settled().await;
    assert_eq!(
        (
            after.core_mode,
            after.tui_mode,
            after.tui_idle,
            after.web_mode.clone(),
            after.web_idle
        ),
        (
            before.core_mode,
            before.tui_mode,
            before.tui_idle,
            before.web_mode.clone(),
            before.web_idle
        ),
        "a no-work idle edge observed while Select changes nothing"
    );

    // Typed admitted-work evidence is what still opens the run for this path.
    wired
        .dispatch(ExecutionEvent::WorkspacePreparationStarted {
            change_id: ALPHA.to_string(),
        })
        .await;
    let working = wired.settled().await;
    assert_eq!(
        (
            working.core_mode,
            working.tui_mode,
            working.web_mode.as_str()
        ),
        (OperatorMode::Running, AppExecutionMode::Running, "running"),
        "typed admitted work remains the Running trigger for non-Start admission"
    );
    assert!(!working.tui_idle && !working.web_idle);
}

/// Verification `idle-start-running-regressions`: the no-work park closes the
/// episode the accepted Start opened.
///
/// Without this, immediate Running would be a one-way door: an analysis that
/// admits nothing would leave every frontend claiming a run forever.
#[tokio::test]
async fn idle_start_running_no_work_park_returns_every_adapter_to_ready() {
    let mut wired = arranged(Setup::MarkedOrdinary, true).await;
    wired
        .harness
        .run(&mut wired.app, TuiCommand::StartProcessing(Vec::new()))
        .await;
    assert_eq!(wired.settled().await.tui_mode, AppExecutionMode::Running);

    // Analysis runs and admits nothing; it is not work, so it moves nothing.
    wired
        .dispatch(ExecutionEvent::AnalysisStarted {
            remaining_changes: 2,
            attempt_id: "attempt-1".to_string(),
        })
        .await;
    let analysing = wired.settled().await;
    assert_eq!(
        (
            analysing.core_mode,
            analysing.tui_mode,
            analysing.web_mode.as_str()
        ),
        (OperatorMode::Running, AppExecutionMode::Running, "running"),
        "dependency analysis neither opens nor closes the episode"
    );

    // The scheduler rearmed its edge on the committed intent, so its next park
    // is a fresh transition — and that is what returns everyone to Ready.
    wired
        .dispatch(ExecutionEvent::PersistentSchedulerIdle)
        .await;
    let parked = wired.settled().await;
    assert_eq!(
        (
            parked.core_mode,
            parked.tui_mode,
            parked.tui_idle,
            parked.web_mode.as_str(),
            parked.web_idle
        ),
        (
            OperatorMode::Select,
            AppExecutionMode::Select,
            true,
            "select",
            true
        ),
        "one fresh idle transition returns Core and both frontends to Ready"
    );
}

// ============================================================================
// Stop and cancellation orderings
// ============================================================================

/// Verification `idle-start-running-regressions`: cancel-stop restores the state
/// the stop interrupted, for all three orderings the episode can be in.
///
/// Clearing the idle fact at accepted Start is what makes these differ, so all
/// three are driven through the real command path over the same arrangement
/// rather than asserted from the projection rule in isolation.
#[tokio::test]
async fn idle_start_running_cancel_stop_restores_the_interrupted_episode() {
    // ── 1. Idle-origin stop with no accepted Start restores Ready ──────────
    let mut wired = arranged(Setup::MarkedOrdinary, true).await;
    wired.harness.run(&mut wired.app, TuiCommand::Stop).await;
    let stopping = wired.settled().await;
    assert_eq!(
        (
            stopping.core_mode,
            stopping.tui_mode,
            stopping.web_mode.as_str()
        ),
        (
            OperatorMode::Stopping,
            AppExecutionMode::Stopping,
            "stopping"
        )
    );
    assert!(
        stopping.tui_idle && stopping.web_idle,
        "an idle-origin stop keeps its episode identity"
    );
    assert_eq!(wired.app.stop_mode, StopMode::GracefulPending);

    wired
        .harness
        .run(&mut wired.app, TuiCommand::CancelStop)
        .await;
    let cancelled = wired.settled().await;
    assert_eq!(
        (
            cancelled.core_mode,
            cancelled.tui_mode,
            cancelled.tui_idle,
            cancelled.web_mode.as_str(),
            cancelled.web_idle
        ),
        (
            OperatorMode::Select,
            AppExecutionMode::Select,
            true,
            "select",
            true
        ),
        "withdrawing an idle-origin stop restores Ready, not Running"
    );

    // ── 2. Accepted Start, then stop, then cancel restores Running ─────────
    let mut wired = arranged(Setup::MarkedOrdinary, true).await;
    wired
        .harness
        .run(&mut wired.app, TuiCommand::StartProcessing(Vec::new()))
        .await;
    wired.harness.run(&mut wired.app, TuiCommand::Stop).await;
    assert_eq!(wired.settled().await.tui_mode, AppExecutionMode::Stopping);
    wired
        .harness
        .run(&mut wired.app, TuiCommand::CancelStop)
        .await;
    let after_start = wired.settled().await;
    assert_eq!(
        (
            after_start.core_mode,
            after_start.tui_mode,
            after_start.tui_idle,
            after_start.web_mode.as_str(),
            after_start.web_idle
        ),
        (
            OperatorMode::Running,
            AppExecutionMode::Running,
            false,
            "running",
            false
        ),
        "the accepted Start already closed the episode, so cancel-stop returns to it"
    );

    // ── 3. Work start wins during Stopping, and cancel-stop follows it ─────
    let mut wired = arranged(Setup::MarkedOrdinary, true).await;
    wired.harness.run(&mut wired.app, TuiCommand::Stop).await;
    wired
        .dispatch(ExecutionEvent::WorkspacePreparationStarted {
            change_id: ALPHA.to_string(),
        })
        .await;
    let raced = wired.settled().await;
    assert_eq!(
        (raced.core_mode, raced.tui_mode, raced.web_mode.as_str()),
        (
            OperatorMode::Stopping,
            AppExecutionMode::Stopping,
            "stopping"
        ),
        "a pending graceful stop outranks work that started after it was requested"
    );
    assert!(
        !raced.tui_idle && !raced.web_idle,
        "work really started, so the idle episode is over even under a stop"
    );

    wired
        .harness
        .run(&mut wired.app, TuiCommand::CancelStop)
        .await;
    let resumed = wired.settled().await;
    assert_eq!(
        (
            resumed.core_mode,
            resumed.tui_mode,
            resumed.tui_idle,
            resumed.web_mode.as_str(),
            resumed.web_idle
        ),
        (
            OperatorMode::Running,
            AppExecutionMode::Running,
            false,
            "running",
            false
        ),
        "cancel-stop returns to Running once work has closed the episode"
    );
}

/// Verification `idle-start-running-regressions`: force stop and terminal
/// retention are unchanged by the new projection.
#[tokio::test]
async fn idle_start_running_force_stop_and_terminal_modes_are_preserved() {
    let mut wired = arranged(Setup::MarkedOrdinary, true).await;
    wired
        .harness
        .run(&mut wired.app, TuiCommand::StartProcessing(Vec::new()))
        .await;
    wired
        .harness
        .run(&mut wired.app, TuiCommand::ForceStop)
        .await;

    assert!(
        wired
            .harness
            .scheduler
            .calls()
            .contains(&SchedulerCall::Cancelled),
        "force stop cancels the same scheduler the Start woke"
    );

    wired.dispatch(ExecutionEvent::Stopped).await;
    let stopped = wired.settled().await;
    assert_eq!(
        (
            stopped.core_mode,
            stopped.tui_mode,
            stopped.web_mode.as_str()
        ),
        (OperatorMode::Stopped, AppExecutionMode::Stopped, "stopped")
    );
    assert!(
        !stopped.tui_idle && !stopped.web_idle,
        "a terminal stop ends the idle episode"
    );

    // A late idle transition may not resurrect Ready out of a terminal mode.
    wired
        .dispatch(ExecutionEvent::PersistentSchedulerIdle)
        .await;
    let late = wired.settled().await;
    assert_eq!(
        (late.core_mode, late.tui_mode, late.web_mode.as_str()),
        (OperatorMode::Stopped, AppExecutionMode::Stopped, "stopped"),
        "the retained terminal mode is unchanged by a late idle event"
    );
    assert!(!late.tui_idle && !late.web_idle);
}
