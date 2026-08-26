//! TUI ↔ `/api/v2` parity for change-local Start/F5 retry.
//!
//! The defect this change fixes was a *routing* defect in one shared boundary,
//! so the risk it introduces is that one adapter reaches the new route and the
//! other does not. Every case below is therefore run twice over two
//! independently constructed but identically arranged processes — once through
//! [`handle_tui_command`], once through the `/api/v2` executor — and what is
//! compared is one value: the authoritative outcome each published, the reducer
//! and queue effects it committed, the scheduler calls it caused, the marks it
//! left, the process mode it settled in, and how many dispatches and projection
//! revisions it produced.
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
use crate::tui::types::AppExecutionMode;
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
/// Read from the dispatched `OperatorCommandEffect` rather than from an adapter
/// return value: that event *is* the shared decision, so two adapters agreeing
/// on it is the property under test rather than two similar-looking summaries.
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

/// Everything two adapters must agree on after one Start request.
#[derive(Debug, PartialEq, Eq)]
struct Settled {
    /// Accepted run dispatches, with their targets and retry semantics.
    runs: Vec<DispatchedRun>,
    /// Every scheduler interaction, in order.
    scheduler: Vec<SchedulerCall>,
    /// Reducer display status per change.
    statuses: Vec<(String, String)>,
    /// Target-specific explicit-retry edges the command published.
    explicit_retries: Vec<String>,
    /// The authoritative marked target set.
    marks: Vec<String>,
    /// The one process lifecycle mode both frontends project.
    mode: OperatorMode,
    /// The `app_mode` an `/api/v2` client would read.
    web_mode: String,
    /// What the TUI would render as its execution mode.
    tui_mode: AppExecutionMode,
    /// How many authoritative dispatches the command produced.
    dispatches: usize,
    /// How many projection revisions the command produced.
    revisions: u64,
}

/// How the `/api/v2` executor settled one Start: the outcome revision it
/// recorded, or the typed error code it refused with.
///
/// Spelled out rather than using the crate's `Result` alias, which binds the
/// orchestrator error type.
type V2Settlement = std::result::Result<Option<u64>, ErrorCode>;

/// One arranged process with both frontends on a single dispatch owner.
struct Wired {
    harness: AdapterHarness,
    app: AppState,
    web: Arc<WebState>,
    runs: Arc<RunRecorder>,
    baseline_revision: u64,
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
            runs: self.runs.runs(),
            scheduler: self.harness.scheduler.calls(),
            statuses,
            // Draining is the queue's only read and this is the process's last
            // observation, so nothing downstream can miss an edge.
            explicit_retries: self
                .harness
                .queue
                .drain_explicit_retries()
                .await
                .into_iter()
                .map(|edge| edge.change_id)
                .collect(),
            marks: self.harness.marks.marked_ids(),
            mode: self.harness.core_mode.get(),
            web_mode: web.app_mode,
            tui_mode: self.app.execution_mode,
            dispatches: self.harness.dispatch_count(),
            revisions: self.web.remote_control().projection().revision() - self.baseline_revision,
        }
    }
}

// ============================================================================
// Arrangement
// ============================================================================

/// The state a case is evaluated over.
///
/// Every variant marks something — a mark is the request's input, not a
/// distinguishing fact — so each is named for the *evidence* its marks carry.
#[derive(Debug, Clone, Copy)]
enum Setup {
    /// `alpha` carries retry-eligible terminal Error evidence and is marked.
    RetryableError,
    /// `alpha` is a marked retry-eligible error and `beta` a marked ordinary row.
    MixedMarks,
    /// A marked retry-eligible error whose worktree execution is refused.
    WorktreeIneligibleError,
    /// Only the ordinary row is marked; nothing carries retryable evidence.
    OrdinaryOnly,
}

async fn arrange(harness: &AdapterHarness, setup: Setup, scheduler_running: bool) {
    let to_error = |id: &'static str| async move {
        harness
            .state
            .write()
            .await
            .apply_execution_event(&ExecutionEvent::ProcessingError {
                id: id.to_string(),
                error: "apply reached its absolute runtime limit".to_string(),
            });
    };

    match setup {
        Setup::RetryableError => {
            to_error(ALPHA).await;
            harness.marks.replace([ALPHA.to_string()]);
        }
        Setup::MixedMarks => {
            to_error(ALPHA).await;
            harness
                .marks
                .replace(CHANGES.iter().map(|id| (*id).to_string()));
        }
        Setup::WorktreeIneligibleError => {
            to_error(ALPHA).await;
            harness.marks.replace([ALPHA.to_string()]);
            harness.parallel.set_parallel_ineligible([(
                ALPHA.to_string(),
                ParallelEligibility::UncommittedProposalFiles,
            )]);
        }
        Setup::OrdinaryOnly => {
            harness.marks.replace([BETA.to_string()]);
        }
    }
    harness.scheduler.set_running(scheduler_running);
}

/// Build the arranged process one case is evaluated over.
async fn arranged(setup: Setup, mode: AppExecutionMode, scheduler_running: bool) -> Wired {
    let harness = AdapterHarness::new(&CHANGES);
    arrange(&harness, setup, scheduler_running).await;
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
    app.execution_mode = mode;
    app.warning_message = None;
    // Persistent-idle Ready is `Select` over a live scheduler; that qualifier is
    // part of the arranged process, not a presentation detail.
    app.persistent_scheduler_idle = mode == AppExecutionMode::Select && scheduler_running;

    let web = Arc::new(WebState::new(&[]));
    web.set_shared_state(harness.state.clone()).await;
    web.set_execution_marks(harness.marks.clone()).await;
    web.set_parallel_runtime(harness.parallel.clone()).await;
    web.set_repo_root(std::path::PathBuf::from("/repo")).await;
    let changes: Vec<_> = CHANGES.iter().map(|id| create_test_change(id)).collect();
    web.update_with_mode(&changes, mode.app_mode_token()).await;
    // All three halves of the arranged process carry the same idle-episode
    // qualifier: leaving this one behind would stage a split state the process
    // cannot be in, and the comparison would then report an arrangement gap as
    // an adapter divergence.
    web.set_persistent_scheduler_idle(app.persistent_scheduler_idle)
        .await;
    web.sync_remote_control_projection().await;

    let runs = Arc::new(RunRecorder::default());
    harness.attach(Arc::new(WebEventSink::new(web.clone())));
    harness.attach(runs.clone());
    harness.attach_revisions(web.clone());
    harness.core_mode.set(mode.operator_mode());
    harness
        .core_mode
        .set_persistent_idle(app.persistent_scheduler_idle);

    let baseline_revision = web.remote_control().projection().revision();
    Wired {
        harness,
        app,
        web,
        runs,
        baseline_revision,
    }
}

/// Run Start through the TUI adapter and report what it settled.
async fn through_tui(
    setup: Setup,
    mode: AppExecutionMode,
    scheduler_running: bool,
) -> (Settled, String) {
    let mut wired = arranged(setup, mode, scheduler_running).await;
    wired
        .harness
        .run(&mut wired.app, TuiCommand::StartProcessing(Vec::new()))
        .await;

    // Accepted settlements are logged; refusals also raise the warning line.
    let report = wired
        .app
        .warning_message
        .clone()
        .or_else(|| wired.app.logs.last().map(|entry| entry.message.clone()))
        .unwrap_or_default();
    (wired.settled().await, report)
}

/// Run Start through the `/api/v2` executor and report what it settled.
async fn through_v2(
    setup: Setup,
    mode: AppExecutionMode,
    scheduler_running: bool,
) -> (Settled, String, V2Settlement) {
    let mut wired = arranged(setup, mode, scheduler_running).await;
    let executor = SharedServiceExecutor::new(wired.harness.application.clone(), wired.web.clone());

    let (report, outcome) = match executor.execute(&CommandSpec::Start).await {
        Ok(summary) => (
            summary.detail.unwrap_or_default(),
            Ok(summary.result_revision),
        ),
        Err(failure) => (failure.message, Err(failure.error_code)),
    };
    // One TUI event-processing pass, exactly as the runner performs it.
    wired.harness.deliver(&mut wired.app).await;

    (wired.settled().await, report, outcome)
}

/// Compare both adapters over one arrangement and return what they agreed on.
async fn assert_adapters_agree(
    label: &str,
    setup: Setup,
    mode: AppExecutionMode,
    scheduler_running: bool,
) -> (Settled, String, String, V2Settlement) {
    let (tui, tui_report) = through_tui(setup, mode, scheduler_running).await;
    let (v2, v2_report, outcome) = through_v2(setup, mode, scheduler_running).await;

    assert_eq!(
        tui, v2,
        "{label}: TUI and /api/v2 must settle one Start identically"
    );
    (tui, tui_report, v2_report, outcome)
}

// ============================================================================
// Accepted retry classes
// ============================================================================

/// Acceptance criterion 1 and 7: persistent-idle `Select` retries the marked
/// change-local error through both adapters, waking the live scheduler once.
#[tokio::test]
async fn change_error_f5_retry_persistent_idle_select_is_identical_on_both_adapters() {
    let (settled, _, _, outcome) = assert_adapters_agree(
        "persistent-idle select",
        Setup::RetryableError,
        AppExecutionMode::Select,
        true,
    )
    .await;

    assert_eq!(
        settled.runs,
        vec![DispatchedRun {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler_started: false,
        }]
    );
    assert_eq!(settled.scheduler, vec![SchedulerCall::Notified]);
    assert_eq!(settled.explicit_retries, vec![ALPHA.to_string()]);
    assert_eq!(settled.dispatches, 1, "one authoritative outcome");
    assert_eq!(settled.revisions, 1, "one projection revision");
    assert!(
        matches!(outcome, Ok(Some(_))),
        "the accepted command records its own outcome revision: {outcome:?}"
    );
    // Retry routing is unchanged above; what the accepted outcome *projects* is
    // not. An accepted Start against a persistent-idle episode opens the run
    // episode immediately, and an explicit retry is one of those Starts — see
    // `idle_start_running_tests` for the episode contract itself.
    assert_eq!(
        settled.mode,
        OperatorMode::Running,
        "an accepted retry from persistent-idle Ready opens the run episode"
    );
    assert_eq!(settled.tui_mode, AppExecutionMode::Running);
    assert_eq!(settled.web_mode, "running");
}

/// Acceptance criterion 2 and 7: a live run retries the marked change-local
/// error without converting the process to global Error, and the ordinary mark
/// is left to the settlement path on both adapters.
#[tokio::test]
async fn change_error_f5_retry_running_retry_is_identical_on_both_adapters() {
    let (settled, tui_report, v2_report, _) = assert_adapters_agree(
        "running",
        Setup::MixedMarks,
        AppExecutionMode::Running,
        true,
    )
    .await;

    assert_eq!(
        settled.runs,
        vec![DispatchedRun {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler_started: false,
        }]
    );
    assert_eq!(settled.scheduler, vec![SchedulerCall::Notified]);
    assert_eq!(settled.explicit_retries, vec![ALPHA.to_string()]);
    assert_eq!(
        settled.statuses,
        vec![
            (ALPHA.to_string(), "queued".to_string()),
            (BETA.to_string(), "not queued".to_string()),
        ],
        "the retry clears terminal error; the ordinary mark is untouched"
    );
    assert_eq!(
        settled.mode,
        OperatorMode::Running,
        "a change-local retry must not move the process into global Error"
    );
    assert_eq!(settled.tui_mode, AppExecutionMode::Running);
    assert_eq!(settled.web_mode, "running");
    for (adapter, report) in [("tui", &tui_report), ("v2", &v2_report)] {
        assert!(
            report.contains(BETA),
            "{adapter} must name the excluded ordinary mark: {report}"
        );
        assert!(
            report.contains("mark settlement"),
            "{adapter} must say what does own it: {report}"
        );
    }
}

/// Acceptance criterion 2: an accepted retry *joins* the run in progress rather
/// than reopening it.
///
/// The run-scoped presentation reset clears the current change and restarts the
/// run's elapsed timer, which is right for a Start that opens a run and wrong
/// for one admitted into a live one: `beta` is work this request never named,
/// and it is what would lose them.
#[tokio::test]
async fn change_error_f5_retry_running_retry_preserves_the_live_run_presentation() {
    let mut wired = arranged(Setup::RetryableError, AppExecutionMode::Running, true).await;
    wired.app.current_change = Some(BETA.to_string());
    let started_at = std::time::Instant::now() - std::time::Duration::from_secs(90);
    wired.app.orchestration_started_at = Some(started_at);

    wired
        .harness
        .run(&mut wired.app, TuiCommand::StartProcessing(Vec::new()))
        .await;

    assert_eq!(
        wired.settled().await.runs,
        vec![DispatchedRun {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler_started: false,
        }],
        "the retry has to be accepted for the rest of this to mean anything"
    );
    assert_eq!(
        wired.app.current_change.as_deref(),
        Some(BETA),
        "unrelated live work keeps the change the run is showing"
    );
    assert_eq!(
        wired.app.orchestration_started_at,
        Some(started_at),
        "joining a live run must not restart its elapsed timer"
    );
}

/// Acceptance criterion 3 and 7: with no boundary alive, both adapters start one
/// fresh boundary with explicit-retry semantics.
#[tokio::test]
async fn change_error_f5_retry_stopped_starts_one_explicit_retry_boundary_on_both_adapters() {
    let (settled, _, _, _) = assert_adapters_agree(
        "stopped",
        Setup::RetryableError,
        AppExecutionMode::Stopped,
        false,
    )
    .await;

    assert_eq!(
        settled.runs,
        vec![DispatchedRun {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler_started: true,
        }]
    );
    assert_eq!(
        settled.scheduler,
        vec![SchedulerCall::Started {
            targets: vec![ALPHA.to_string()],
            explicit_retry: true,
        }]
    );
    assert_eq!(settled.explicit_retries, vec![ALPHA.to_string()]);
}

/// Process-wide `Error` keeps the routing it already had, on both adapters.
#[tokio::test]
async fn change_error_f5_retry_global_error_mode_is_identical_on_both_adapters() {
    let (settled, _, _, _) = assert_adapters_agree(
        "global error",
        Setup::RetryableError,
        AppExecutionMode::Error,
        false,
    )
    .await;

    assert_eq!(
        settled.runs,
        vec![DispatchedRun {
            change_ids: vec![ALPHA.to_string()],
            explicit_retry: true,
            scheduler_started: true,
        }]
    );
}

/// Acceptance criterion 6 and 7: ordinary Start keeps priority, and both
/// adapters explain what would make the deferred retry row selectable.
#[tokio::test]
async fn change_error_f5_retry_mixed_marks_admit_ordinary_work_on_both_adapters() {
    let (settled, tui_report, v2_report, _) = assert_adapters_agree(
        "mixed marks",
        Setup::MixedMarks,
        AppExecutionMode::Select,
        false,
    )
    .await;

    assert_eq!(
        settled.runs,
        vec![DispatchedRun {
            change_ids: vec![BETA.to_string()],
            explicit_retry: false,
            scheduler_started: true,
        }],
        "the ordinary mark is admitted with ordinary Start semantics"
    );
    assert!(
        settled.explicit_retries.is_empty(),
        "a deferred retry row must not be retried implicitly"
    );
    assert_eq!(
        settled.statuses,
        vec![
            (ALPHA.to_string(), "error".to_string()),
            (BETA.to_string(), "queued".to_string()),
        ]
    );
    for (adapter, report) in [("tui", &tui_report), ("v2", &v2_report)] {
        assert!(
            report.contains(ALPHA) && report.contains("remove the ordinary marks"),
            "{adapter} must explain how to reach the deferred retry row: {report}"
        );
    }
}

// ============================================================================
// Refusals
// ============================================================================

/// Acceptance criterion 5 and 7: `Stopping` refuses Start with no effect on
/// either adapter.
#[tokio::test]
async fn change_error_f5_retry_stopping_refuses_on_both_adapters() {
    let (settled, _, _, outcome) = assert_adapters_agree(
        "stopping",
        Setup::RetryableError,
        AppExecutionMode::Stopping,
        true,
    )
    .await;

    assert_eq!(outcome, Err(ErrorCode::LifecycleConflict));
    assert!(settled.runs.is_empty());
    assert!(settled.scheduler.is_empty());
    assert!(settled.explicit_retries.is_empty());
    assert_eq!(settled.dispatches, 0);
    assert_eq!(settled.revisions, 0);
    assert_eq!(
        settled.statuses,
        vec![
            (ALPHA.to_string(), "error".to_string()),
            (BETA.to_string(), "not queued".to_string()),
        ]
    );
}

/// Acceptance criterion 5 and 7: the complete-request worktree fence refuses
/// before class selection on both adapters.
#[tokio::test]
async fn change_error_f5_retry_worktree_fence_refuses_on_both_adapters() {
    let (settled, tui_report, v2_report, outcome) = assert_adapters_agree(
        "worktree fence",
        Setup::WorktreeIneligibleError,
        AppExecutionMode::Running,
        true,
    )
    .await;

    assert_eq!(outcome, Err(ErrorCode::TargetIneligible));
    assert!(settled.runs.is_empty());
    assert!(settled.scheduler.is_empty());
    assert!(settled.explicit_retries.is_empty());
    assert_eq!(settled.dispatches, 0);
    for (adapter, report) in [("tui", &tui_report), ("v2", &v2_report)] {
        assert!(
            report.contains(ALPHA),
            "{adapter} must name the ineligible target: {report}"
        );
    }
}

/// A live run with nothing retryable marked refuses identically on both
/// adapters, and neither converts the ordinary mark into queue intent.
#[tokio::test]
async fn change_error_f5_retry_running_without_retryable_marks_refuses_on_both_adapters() {
    let (settled, _, _, outcome) = assert_adapters_agree(
        "running without retryable marks",
        Setup::OrdinaryOnly,
        AppExecutionMode::Running,
        true,
    )
    .await;

    assert_eq!(outcome, Err(ErrorCode::TargetIneligible));
    assert!(settled.runs.is_empty());
    assert!(settled.scheduler.is_empty());
    assert_eq!(
        settled.statuses,
        vec![
            (ALPHA.to_string(), "not queued".to_string()),
            (BETA.to_string(), "not queued".to_string()),
        ],
        "a refused Start creates no queue intent"
    );
    assert_eq!(settled.marks, vec![BETA.to_string()], "marks survive");
}
