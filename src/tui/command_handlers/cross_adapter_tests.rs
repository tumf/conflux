//! TUI ↔ `/api/v2` adapter parity.
//!
//! Every other test in this change checks one adapter. These check the property
//! the change exists for: for the same lifecycle intent, in the same mode, over
//! the same state, a keypress and a remote command produce the *same* reducer
//! transition, the *same* scheduler dispatch, and the *same* accepted /
//! no-op / refused settlement.
//!
//! Each row is run twice over two independently constructed but identically
//! arranged harnesses — once through [`handle_tui_command`], once through
//! [`SharedServiceExecutor`] — and the observable effects are compared as one
//! value. A divergence anywhere (an extra `start_run`, a queue intent applied on
//! only one side, a reservation taken twice) fails the comparison, so a future
//! frontend-local shortcut cannot pass silently.
//!
//! Unit-scoped: the scheduler is the recording double, the queue is the
//! in-memory `DynamicQueue`, and no repository, process, or network is touched.

use std::sync::Arc;

use super::tests::{create_test_change, AdapterHarness};
use super::*;

use crate::events::ExecutionEvent;
use crate::orchestration::operator_command::ParallelEligibility;
use crate::orchestration::run_control::testing::SchedulerCall;
use crate::tui::types::AppExecutionMode;
use crate::web::remote_control_api::dto::{CommandSpec, ErrorCode};
use crate::web::remote_control_api::executor::{RemoteControlExecutor, SharedServiceExecutor};
use crate::web::state::WebState;

/// The two changes every row is arranged over.
const CHANGES: [&str; 2] = ["c1", "c2"];

// ============================================================================
// Comparable observations
// ============================================================================

/// Everything both adapters must agree on after one command.
///
/// This is deliberately *effect* state, not presentation state: the TUI paints
/// logs and the API returns JSON, but the reducer transition, the scheduler
/// dispatch, the mark store, and the resolver ledger are the shared truth.
#[derive(Debug, PartialEq, Eq)]
struct Effects {
    /// Every scheduler interaction, in order.
    scheduler: Vec<SchedulerCall>,
    /// Reducer display status per change, in a stable order.
    statuses: Vec<(String, String)>,
    /// The change owning the single resolver slot, if any.
    active_resolver: Option<String>,
    /// Changes waiting behind the active resolver, in FIFO order.
    queued_resolves: Vec<String>,
    /// The authoritative marked target set.
    marks: Vec<String>,
}

async fn effects(harness: &AdapterHarness) -> Effects {
    let statuses = {
        let guard = harness.state.read().await;
        CHANGES
            .iter()
            .map(|id| ((*id).to_string(), guard.display_status(id).to_string()))
            .collect()
    };
    Effects {
        scheduler: harness.scheduler.calls(),
        statuses,
        active_resolver: harness.resolves.active(),
        queued_resolves: harness.resolves.waiting(),
        marks: harness.marks.marked_ids(),
    }
}

/// How a command settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Settlement {
    /// A real effect happened.
    Changed,
    /// Valid, but nothing changed.
    NoOp,
    /// Refused with this code, without the effect the command asked for.
    Failed(ErrorCode),
}

impl Settlement {
    /// True when the operator must be told the command did not do what it asked.
    fn is_reported_to_the_operator(self) -> bool {
        !matches!(self, Self::Changed)
    }
}

// ============================================================================
// Arrangement
// ============================================================================

/// The starting state a row is evaluated over.
///
/// Kept as plain data so the table below stays readable; [`arrange`] is the one
/// place that knows how each is built, and it is applied identically to the TUI
/// harness and the v2 harness.
#[derive(Debug, Clone, Copy)]
enum Setup {
    /// Two idle, unmarked, not-queued changes.
    Bare,
    /// Both changes carry an execution mark.
    Marked,
    /// Both marked, and a scheduler run is already alive.
    MarkedWithLiveScheduler,
    /// Both marked, and the next launch is refused by the runtime.
    MarkedWithFailingLaunch,
    /// `c1` is marked and terminally errored.
    MarkedError,
    /// A scheduler run is alive; nothing is marked.
    LiveScheduler,
    /// `c1` is in a reducer-visible merge wait.
    MergeWait,
    /// `c1` is in a merge wait and already holds a resolve reservation.
    MergeWaitAlreadyReserved,
    /// Both changes are marked and queued; `c2` is not parallel-eligible.
    MarkedWithIneligible,
}

async fn arrange(harness: &AdapterHarness, setup: Setup) {
    let mark_all = || {
        harness
            .marks
            .replace(CHANGES.iter().map(|id| (*id).to_string()))
    };

    match setup {
        Setup::Bare => {}
        Setup::Marked => mark_all(),
        Setup::MarkedWithLiveScheduler => {
            mark_all();
            harness.scheduler.set_running(true);
        }
        Setup::MarkedWithFailingLaunch => {
            mark_all();
            harness.scheduler.fail_launch("runtime refused the launch");
        }
        Setup::MarkedError => {
            harness
                .state
                .write()
                .await
                .apply_execution_event(&ExecutionEvent::ProcessingError {
                    id: "c1".to_string(),
                    error: "boom".to_string(),
                });
            harness.marks.replace(["c1".to_string()]);
        }
        Setup::LiveScheduler => harness.scheduler.set_running(true),
        Setup::MergeWait => merge_wait(harness).await,
        Setup::MergeWaitAlreadyReserved => {
            merge_wait(harness).await;
            harness
                .run_control
                .resolve_merge("c1")
                .await
                .expect("the first resolve of a merge-wait change is accepted");
        }
        Setup::MarkedWithIneligible => {
            mark_all();
            {
                let mut guard = harness.state.write().await;
                for id in CHANGES {
                    guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                        id.to_string(),
                    ));
                }
            }
            harness.parallel.set_parallel_ineligible([(
                "c2".to_string(),
                ParallelEligibility::UncommittedProposalFiles,
            )]);
        }
    }
}

async fn merge_wait(harness: &AdapterHarness) {
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "c1".to_string(),
            reason: "manual resolution required".to_string(),
            auto_resumable: false,
        });
}

/// The `app_mode` string the v2 projection publishes for a TUI execution mode.
///
/// Both adapters must be given the *same* operator mode or the comparison would
/// be meaningless, and this is the only place the two vocabularies meet. The
/// token comes from the shared execution vocabulary rather than a table local to
/// this test, so canonical `app_mode` stays execution-only for both sides.
fn app_mode_string(mode: &AppExecutionMode) -> &'static str {
    mode.app_mode_token()
}

// ============================================================================
// Adapter drivers
// ============================================================================

/// Run one intent through the TUI adapter and report what it settled as.
///
/// The second element is the operator-facing message the TUI surfaced, which is
/// its counterpart to the v2 summary detail.
async fn through_tui(
    setup: Setup,
    mode: AppExecutionMode,
    command: TuiCommand,
) -> (Effects, Option<String>) {
    let harness = AdapterHarness::new(&CHANGES);
    arrange(&harness, setup).await;
    // Captured before the app is built: binding an `AppState` republishes the
    // eligibility set from its own rows, which is exactly what production does.
    let ineligible = harness.parallel.ineligible_ids();

    let mut app = harness.app(&CHANGES);
    app.execution_mode = mode;
    for change in &mut app.changes {
        change.parallel_eligibility = if ineligible.contains(&change.id) {
            ParallelEligibility::UncommittedProposalFiles
        } else {
            ParallelEligibility::Eligible
        };
    }
    app.publish_parallel_runtime();
    app.apply_display_statuses_from_reducer(&harness.state.read().await.all_display_statuses());
    app.warning_message = None;

    harness.run(&mut app, command).await;

    (effects(&harness).await, app.warning_message.clone())
}

/// Run one intent through the `/api/v2` executor and report what it settled as.
///
/// The third element is the summary detail, the only place v2 names a
/// consequence the command did not ask for.
async fn through_v2(
    setup: Setup,
    mode: AppExecutionMode,
    command: CommandSpec,
) -> (Effects, Settlement, Option<String>) {
    let harness = AdapterHarness::new(&CHANGES);
    arrange(&harness, setup).await;

    let web_state = Arc::new(WebState::new(&[]));
    web_state.set_shared_state(harness.state.clone()).await;
    web_state.set_execution_marks(harness.marks.clone()).await;
    web_state
        .set_repo_root(std::path::PathBuf::from("/repo"))
        .await;
    let changes: Vec<_> = CHANGES.iter().map(|id| create_test_change(id)).collect();
    web_state
        .update_with_mode(&changes, app_mode_string(&mode))
        .await;
    web_state.sync_remote_control_projection().await;

    let executor = SharedServiceExecutor::new(
        harness.run_control.operator(),
        harness.run_control.clone(),
        web_state.clone(),
        web_state.remote_control().projection(),
    );

    let (settlement, detail) = match executor.execute(&command).await {
        Ok(summary) if summary.changed => (Settlement::Changed, summary.detail),
        Ok(summary) => (Settlement::NoOp, summary.detail),
        Err(failure) => (
            Settlement::Failed(failure.error_code),
            Some(failure.message),
        ),
    };

    (effects(&harness).await, settlement, detail)
}

// ============================================================================
// The table
// ============================================================================

/// One lifecycle intent, expressed in both adapter vocabularies.
struct Row {
    /// What the row demonstrates; used as the assertion label.
    name: &'static str,
    setup: Setup,
    mode: AppExecutionMode,
    tui: TuiCommand,
    v2: CommandSpec,
    expect: Settlement,
    /// A change the command did not name but whose operator intent it
    /// clears; both adapters must say so. `None` when the command has no
    /// consequence beyond what it was asked to do.
    notice: Option<&'static str>,
}

fn rows() -> Vec<Row> {
    vec![
        // ── start ───────────────────────────────────────────────────────────
        Row {
            name: "start with an idle scheduler spawns one run over the marked set",
            setup: Setup::Marked,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "start with a live scheduler wakes it instead of spawning a second run",
            setup: Setup::MarkedWithLiveScheduler,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "start from a stopped run resumes the marked set",
            setup: Setup::Marked,
            mode: AppExecutionMode::Stopped,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "start is refused while a run owns the lifecycle",
            setup: Setup::MarkedWithLiveScheduler,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Failed(ErrorCode::LifecycleConflict),
            notice: None,
        },
        Row {
            name: "start with an empty target set is not a success",
            setup: Setup::Bare,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Failed(ErrorCode::TargetIneligible),
            notice: None,
        },
        Row {
            name: "a runtime launch failure is reported, not claimed as started",
            setup: Setup::MarkedWithFailingLaunch,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Failed(ErrorCode::InternalError),
            notice: None,
        },
        // ── retry ───────────────────────────────────────────────────────────
        // Retry has no command variant of its own on either side: `Error` mode is
        // what turns the shared start intent into an explicit retry.
        Row {
            name: "retry routes a marked error row and dispatches the scheduler",
            setup: Setup::MarkedError,
            mode: AppExecutionMode::Error,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "retry without retryable evidence changes nothing",
            setup: Setup::Marked,
            mode: AppExecutionMode::Error,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::NoOp,
            notice: None,
        },
        // ── stop family ─────────────────────────────────────────────────────
        Row {
            name: "graceful stop while running sets the stop request",
            setup: Setup::LiveScheduler,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::Stop,
            v2: CommandSpec::Stop,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "graceful stop outside running is refused",
            setup: Setup::Bare,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::Stop,
            v2: CommandSpec::Stop,
            expect: Settlement::Failed(ErrorCode::LifecycleConflict),
            notice: None,
        },
        // Persistent-idle Ready is `select` over a scheduler that is still
        // alive, so the stop family stays admitted there while pre-run Select
        // (the two rows above and below, over `Setup::Bare`) stays refused.
        Row {
            name: "graceful stop from persistent-idle Ready addresses the live scheduler",
            setup: Setup::LiveScheduler,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::Stop,
            v2: CommandSpec::Stop,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "force stop from persistent-idle Ready cancels the live scheduler",
            setup: Setup::LiveScheduler,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::ForceStop,
            v2: CommandSpec::ForceStop,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "cancel stop while stopping withdraws the request",
            setup: Setup::LiveScheduler,
            mode: AppExecutionMode::Stopping,
            tui: TuiCommand::CancelStop,
            v2: CommandSpec::CancelStop,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "cancel stop outside stopping is refused",
            setup: Setup::LiveScheduler,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::CancelStop,
            v2: CommandSpec::CancelStop,
            expect: Settlement::Failed(ErrorCode::LifecycleConflict),
            notice: None,
        },
        Row {
            name: "force stop while running cancels the live run",
            setup: Setup::LiveScheduler,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::ForceStop,
            v2: CommandSpec::ForceStop,
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "force stop outside running and stopping is refused",
            setup: Setup::Bare,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::ForceStop,
            v2: CommandSpec::ForceStop,
            expect: Settlement::Failed(ErrorCode::LifecycleConflict),
            notice: None,
        },
        // ── resolve ─────────────────────────────────────────────────────────
        Row {
            name: "resolve of a merge-wait change takes the single resolver slot",
            setup: Setup::MergeWait,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::ResolveMerge("c1".to_string()),
            v2: CommandSpec::ResolveMerge {
                change_id: "c1".to_string(),
            },
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "a duplicate resolve submission does not reserve twice",
            setup: Setup::MergeWaitAlreadyReserved,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::ResolveMerge("c1".to_string()),
            v2: CommandSpec::ResolveMerge {
                change_id: "c1".to_string(),
            },
            expect: Settlement::NoOp,
            notice: None,
        },
        Row {
            name: "resolve of a stale target is refused without a reservation",
            setup: Setup::Bare,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::ResolveMerge("c1".to_string()),
            v2: CommandSpec::ResolveMerge {
                change_id: "c1".to_string(),
            },
            expect: Settlement::Failed(ErrorCode::TargetIneligible),
            notice: None,
        },
    ]
}

#[tokio::test]
async fn tui_and_v2_settle_every_lifecycle_intent_identically() {
    for row in rows() {
        let (tui_effects, tui_message) = through_tui(row.setup, row.mode, row.tui.clone()).await;
        let (v2_effects, v2_settlement, v2_detail) = through_v2(row.setup, row.mode, row.v2).await;

        assert_eq!(
            v2_settlement, row.expect,
            "{}: /api/v2 settlement must match the declared outcome",
            row.name
        );
        assert_eq!(
            tui_effects, v2_effects,
            "{}: the TUI and /api/v2 must produce the same reducer, scheduler, mark, and resolver effects",
            row.name
        );
        // A refusal or a no-op is reported by both adapters; so is a consequence
        // the command did not ask for. Anything else is a plain success and must
        // be silent on both sides, or one frontend is telling an operator
        // something the other is hiding.
        let must_report = row.expect.is_reported_to_the_operator() || row.notice.is_some();
        assert_eq!(
            tui_message.is_some(),
            must_report,
            "{}: the TUI must surface exactly what /api/v2 reports, got {tui_message:?}",
            row.name
        );

        if let Some(cleared) = row.notice {
            // Naming the change is the point: an operator whose intent vanished
            // must be able to tell that from a lost command, on both adapters.
            let tui_message = tui_message.expect("a consequence row surfaces a TUI message");
            assert!(
                tui_message.contains(cleared),
                "{}: the TUI must name '{cleared}', got {tui_message:?}",
                row.name
            );
            let v2_detail = v2_detail.expect("a consequence row carries a v2 detail");
            assert!(
                v2_detail.contains(cleared),
                "{}: /api/v2 must name '{cleared}', got {v2_detail:?}",
                row.name
            );
        }
    }
}

// ============================================================================
// Persistent-idle Ready as a live run-control target
// ============================================================================

/// Verification `persistent-idle-ready-regressions`: Ready produced by a
/// persistent-scheduler idle transition stays a live run-control target.
///
/// The parity table above already proves both adapters settle the stop family
/// identically from this mode. What is left is the behaviour that is specific to
/// it: Start notifies instead of spawning and does not claim Running, marks stay
/// mark-only, graceful stop wakes the parked waiter, the controls stay
/// discoverable, and an idle-origin cancel-stop returns to Ready.
#[tokio::test]
async fn persistent_idle_commands_use_live_scheduler() {
    use crate::events::persistent_idle_may_project_ready;
    use crate::tui::key_handlers::{esc_stop_action, EscStopAction};
    use crate::tui::types::StopMode;

    // ── Start wakes the same scheduler and does not claim Running ──────────
    let harness = AdapterHarness::new(&CHANGES);
    harness
        .marks
        .replace(CHANGES.iter().map(|id| id.to_string()));
    harness.scheduler.set_running(true);

    let mut app = harness.app(&CHANGES);
    app.execution_mode = AppExecutionMode::Select;
    app.persistent_scheduler_idle = true;

    // A mark made while idle-Ready is a mark, not a queue mutation.
    let statuses_before = harness.state.read().await.all_display_statuses();
    harness
        .run_control
        .operator()
        .set_execution_mark(AppExecutionMode::Select.operator_mode(), "c1", true)
        .await
        .expect("marking in Select is accepted");
    assert_eq!(
        harness.state.read().await.all_display_statuses(),
        statuses_before,
        "a mark made in idle Ready must not synthesize queue intent"
    );

    harness
        .run(&mut app, TuiCommand::StartProcessing(Vec::new()))
        .await;

    assert!(
        harness.scheduler.started_targets().is_empty(),
        "Start against a live idle scheduler must not spawn a second run"
    );
    assert!(
        harness.scheduler.calls().contains(&SchedulerCall::Notified),
        "Start must wake the scheduler that is already alive"
    );
    for id in CHANGES {
        assert_eq!(
            harness.state.read().await.display_status(id),
            "queued",
            "Start applies the existing reducer queue intent for '{id}'"
        );
    }
    assert_eq!(
        app.execution_mode,
        AppExecutionMode::Select,
        "a notified Start has started nothing yet, so Ready stands"
    );
    assert!(
        app.persistent_scheduler_idle,
        "a Start notification does not close the idle episode"
    );

    // ── The controls are discoverable, and pre-run Select's are not ────────
    assert_eq!(
        esc_stop_action(&AppExecutionMode::Select, &StopMode::None, true),
        EscStopAction::RequestGracefulStop,
        "idle Ready keeps the first-Esc graceful stop"
    );
    assert_eq!(
        esc_stop_action(&AppExecutionMode::Select, &StopMode::None, false),
        EscStopAction::None,
        "pre-run Select has no scheduler to stop"
    );

    // ── Graceful stop records the request, then wakes the parked waiter ────
    let harness = AdapterHarness::new(&CHANGES);
    harness.scheduler.set_running(true);
    let mut app = harness.app(&CHANGES);
    app.execution_mode = AppExecutionMode::Select;
    app.persistent_scheduler_idle = true;

    harness.run(&mut app, TuiCommand::Stop).await;

    assert_eq!(
        harness.scheduler.calls(),
        vec![SchedulerCall::GracefulStop(true), SchedulerCall::Notified],
        "the stop request is recorded before the idle waiter is woken"
    );
    assert_eq!(app.execution_mode, AppExecutionMode::Stopping);
    assert_eq!(app.stop_mode, StopMode::GracefulPending);
    assert!(
        app.persistent_scheduler_idle,
        "an idle-origin stop keeps its episode identity"
    );

    // The second Esc still escalates to the shared force stop.
    assert_eq!(
        esc_stop_action(&app.execution_mode, &app.stop_mode, true),
        EscStopAction::RequestImmediateStop
    );

    // ── Cancel-stop returns to Ready, not to Running ───────────────────────
    harness.run(&mut app, TuiCommand::CancelStop).await;
    assert_eq!(
        app.execution_mode,
        AppExecutionMode::Select,
        "withdrawing an idle-origin stop restores Ready"
    );
    assert_eq!(app.stop_mode, StopMode::None);
    assert!(app.persistent_scheduler_idle);

    // Once admitted work has closed the episode, cancel-stop restores Running.
    app.handle_orchestrator_event(ExecutionEvent::WorkspacePreparationStarted {
        change_id: "c1".to_string(),
    });
    assert_eq!(app.execution_mode, AppExecutionMode::Running);
    assert!(!app.persistent_scheduler_idle);
    harness.run(&mut app, TuiCommand::Stop).await;
    harness.run(&mut app, TuiCommand::CancelStop).await;
    assert_eq!(app.execution_mode, AppExecutionMode::Running);

    // ── Force stop cancels the same scheduler ──────────────────────────────
    let harness = AdapterHarness::new(&CHANGES);
    harness.scheduler.set_running(true);
    let mut app = harness.app(&CHANGES);
    app.execution_mode = AppExecutionMode::Select;
    app.persistent_scheduler_idle = true;

    harness.run(&mut app, TuiCommand::ForceStop).await;
    assert!(
        harness
            .scheduler
            .calls()
            .contains(&SchedulerCall::Cancelled),
        "force stop must cancel the live scheduler behind idle Ready"
    );
    assert!(
        harness.scheduler.started_targets().is_empty(),
        "force stop never spawns anything"
    );

    // ── Liveness, not the presentation fact, is the admission authority ────
    let harness = AdapterHarness::new(&CHANGES);
    let mut app = harness.app(&CHANGES);
    app.execution_mode = AppExecutionMode::Select;
    // A stale client fact over a scheduler that already exited.
    app.persistent_scheduler_idle = true;

    harness.run(&mut app, TuiCommand::Stop).await;
    assert!(
        harness.scheduler.calls().is_empty(),
        "a stale idle fact must not authorize a stop against an exited scheduler"
    );
    assert_eq!(app.execution_mode, AppExecutionMode::Select);
    assert!(app
        .warning_message
        .as_deref()
        .is_some_and(|message| message.contains("stop is not available")));

    // Both frontends read the same guard for the same mode token.
    assert!(persistent_idle_may_project_ready(
        AppExecutionMode::Running.app_mode_token()
    ));
    for retained in [
        AppExecutionMode::Select,
        AppExecutionMode::Stopping,
        AppExecutionMode::Stopped,
        AppExecutionMode::Error,
    ] {
        assert!(
            !persistent_idle_may_project_ready(retained.app_mode_token()),
            "{retained:?} must not be turned into persistent-idle Ready"
        );
    }
}

/// Verification `persistent-idle-ready-regressions`: after Ready, the first
/// typed admitted-work start — and nothing before it — restores Running.
///
/// The whole sequence is driven into a TUI `AppState` and a `WebState` from the
/// *same* ordered events, and the two modes are compared at every step, so a
/// frontend-local shortcut on either side fails the comparison.
#[tokio::test]
async fn admitted_work_restores_running_after_idle() {
    use crate::events::EventSink;
    use crate::orchestration::state::OrchestratorState;
    use crate::web::state::WebEventSink;

    /// One step: the event to deliver, and the mode both frontends must report.
    struct Step {
        what: &'static str,
        event: ExecutionEvent,
        mode: &'static str,
        idle: bool,
    }

    let prepare = |id: &str| ExecutionEvent::WorkspacePreparationStarted {
        change_id: id.to_string(),
    };

    let steps = vec![
        Step {
            what: "ordinary workspace preparation starts the run",
            event: prepare("change-a"),
            mode: "running",
            idle: false,
        },
        Step {
            what: "the scheduler parks with nothing to execute",
            event: ExecutionEvent::PersistentSchedulerIdle,
            mode: "select",
            idle: true,
        },
        Step {
            what: "a no-op wake analyses and admits nothing",
            event: ExecutionEvent::AnalysisStarted {
                remaining_changes: 1,
                attempt_id: "attempt-1".to_string(),
            },
            mode: "select",
            idle: true,
        },
        Step {
            what: "a catalog refresh is not execution evidence either",
            event: ExecutionEvent::WorktreesRefreshed {
                worktrees: Vec::new(),
            },
            mode: "select",
            idle: true,
        },
        Step {
            what: "actual admitted work resumes Running and closes the episode",
            event: prepare("change-a"),
            mode: "running",
            idle: false,
        },
        Step {
            what: "the scheduler parks again: a second idle edge",
            event: ExecutionEvent::PersistentSchedulerIdle,
            mode: "select",
            idle: true,
        },
        Step {
            what: "scheduler-owned resolve work resumes Running",
            event: ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "resolve".to_string(),
            },
            mode: "running",
            idle: false,
        },
        Step {
            what: "and parks once more",
            event: ExecutionEvent::PersistentSchedulerIdle,
            mode: "select",
            idle: true,
        },
        Step {
            what: "scheduler-owned base-lane rejection review resumes Running",
            event: ExecutionEvent::WorkspaceStatusUpdated {
                change_id: "change-a".to_string(),
                workspace_name: "ws-a".to_string(),
                status: crate::vcs::WorkspaceStatus::Rejecting,
            },
            mode: "running",
            idle: false,
        },
    ];

    let reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["change-a".to_string()],
        10,
    )));
    let web_state = Arc::new(WebState::new(&[]));
    web_state.set_shared_state(reducer.clone()).await;
    let sinks: Vec<Arc<dyn EventSink>> = vec![Arc::new(WebEventSink::new(web_state.clone()))];
    let mut app = AppState::new(Vec::new());

    for step in steps {
        crate::events::dispatch_event(reducer.as_ref(), &sinks, step.event.clone()).await;
        app.handle_orchestrator_event(step.event);

        let web = web_state.get_state().await;
        assert_eq!(web.app_mode, step.mode, "web: {}", step.what);
        assert_eq!(
            web.persistent_scheduler_idle, step.idle,
            "web idle episode: {}",
            step.what
        );
        assert_eq!(
            app.execution_mode.app_mode_token(),
            step.mode,
            "tui: {}",
            step.what
        );
        assert_eq!(
            app.persistent_scheduler_idle, step.idle,
            "tui idle episode: {}",
            step.what
        );
        // The comparison itself: neither frontend may reach a conclusion the
        // other does not.
        assert_eq!(
            (app.execution_mode == AppExecutionMode::Running),
            (web.app_mode == "running"),
            "tui/web divergence: {}",
            step.what
        );
    }
}

// ============================================================================
// Bulk execution marks
// ============================================================================
//
// The bulk toggle has no `TuiCommand` of its own: `x` classifies the rows the
// TUI is painting and then emits ordinary queue commands. That makes it exactly
// the place a frontend-local target set could creep back in, so it is compared
// the same way the table above compares everything else — same arrangement, two
// adapters, one value.

/// Rows every bulk-mark comparison is arranged over: `(id, status, eligible)`.
type BulkRow = (&'static str, &'static str, bool);

/// Arrange one bulk-mark case on a fresh harness.
async fn arrange_bulk(harness: &AdapterHarness, rows: &[BulkRow], marked: &[&str]) {
    harness
        .parallel
        .set_parallel_ineligible(rows.iter().filter(|(_, _, ok)| !ok).map(|(id, ..)| {
            (
                id.to_string(),
                ParallelEligibility::UncommittedProposalFiles,
            )
        }));
    harness
        .marks
        .replace(marked.iter().map(|id| id.to_string()));

    let mut guard = harness.state.write().await;
    for (id, status, _) in rows {
        match *status {
            "queued" => {
                guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                    (*id).to_string(),
                ));
            }
            "applying" => guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
                change_id: (*id).to_string(),
                command: "apply".to_string(),
            }),
            "rejected" => guard.apply_execution_event(&ExecutionEvent::ChangeRejected {
                change_id: (*id).to_string(),
                reason: "acceptance refused the proposal".to_string(),
            }),
            "not queued" => {}
            other => panic!("unsupported bulk-mark arrangement status '{other}'"),
        }
    }
}

async fn bulk_through_tui(rows: &[BulkRow], marked: &[&str], mode: AppExecutionMode) -> Effects {
    let ids: Vec<&str> = rows.iter().map(|(id, ..)| *id).collect();
    let harness = AdapterHarness::new(&ids);
    arrange_bulk(&harness, rows, marked).await;

    let mut app = harness.app(&ids);
    app.execution_mode = mode;
    app.apply_display_statuses_from_reducer(&harness.state.read().await.all_display_statuses());
    for (index, (_, _, eligible)) in rows.iter().enumerate() {
        app.changes[index].parallel_eligibility = if *eligible {
            ParallelEligibility::Eligible
        } else {
            ParallelEligibility::UncommittedProposalFiles
        };
        app.changes[index].selected = marked.contains(&app.changes[index].id.as_str());
    }
    app.publish_parallel_runtime();

    // The key handler classifies and marks; the emitted queue commands are what
    // the runner loop feeds back through the shared service.
    for command in app.toggle_all_marks() {
        harness.run(&mut app, command).await;
    }

    effects(&harness).await
}

async fn bulk_through_v2(
    rows: &[BulkRow],
    marked: &[&str],
    mode: AppExecutionMode,
) -> (Effects, Settlement) {
    let ids: Vec<&str> = rows.iter().map(|(id, ..)| *id).collect();
    let harness = AdapterHarness::new(&ids);
    arrange_bulk(&harness, rows, marked).await;

    let web_state = Arc::new(WebState::new(&[]));
    web_state.set_shared_state(harness.state.clone()).await;
    web_state.set_execution_marks(harness.marks.clone()).await;
    web_state
        .set_parallel_runtime(harness.parallel.clone())
        .await;
    let changes: Vec<_> = ids.iter().map(|id| create_test_change(id)).collect();
    web_state
        .update_with_mode(&changes, app_mode_string(&mode))
        .await;
    web_state.sync_remote_control_projection().await;

    let executor = SharedServiceExecutor::new(
        harness.run_control.operator(),
        harness.run_control.clone(),
        web_state.clone(),
        web_state.remote_control().projection(),
    );
    let settlement = match executor
        .execute(&CommandSpec::SetAllExecutionMarks {})
        .await
    {
        Ok(summary) if summary.changed => Settlement::Changed,
        Ok(_) => Settlement::NoOp,
        Err(failure) => Settlement::Failed(failure.error_code),
    };

    (effects(&harness).await, settlement)
}

#[tokio::test]
async fn tui_and_v2_derive_the_same_bulk_mark_target_set_and_exclusions() {
    struct Case {
        name: &'static str,
        rows: Vec<BulkRow>,
        marked: Vec<&'static str>,
        mode: AppExecutionMode,
        expect: Settlement,
    }

    let cases = vec![
        Case {
            name: "select mode marks every eligible row and skips a final one",
            rows: vec![("c1", "not queued", true), ("c2", "rejected", true)],
            marked: vec![],
            mode: AppExecutionMode::Select,
            expect: Settlement::Changed,
        },
        Case {
            name: "a fully marked eligible set unmarks, ignoring the excluded row's mark state",
            rows: vec![("c1", "not queued", true), ("c2", "rejected", true)],
            marked: vec!["c1"],
            mode: AppExecutionMode::Select,
            expect: Settlement::Changed,
        },
        Case {
            name: "an uncommitted row is excluded from the target set",
            rows: vec![("c1", "not queued", true), ("c2", "not queued", false)],
            marked: vec![],
            mode: AppExecutionMode::Select,
            expect: Settlement::Changed,
        },
        Case {
            name: "running mode turns the mark into queue intent and skips the active row",
            rows: vec![("c1", "not queued", true), ("c2", "applying", true)],
            marked: vec![],
            mode: AppExecutionMode::Running,
            expect: Settlement::Changed,
        },
        Case {
            name: "running mode takes queue intent back out when unmarking",
            rows: vec![("c1", "queued", true), ("c2", "applying", true)],
            marked: vec!["c1"],
            mode: AppExecutionMode::Running,
            expect: Settlement::Changed,
        },
        Case {
            name: "a target set with no eligible row changes nothing on either side",
            rows: vec![("c1", "rejected", true), ("c2", "applying", true)],
            marked: vec![],
            mode: AppExecutionMode::Running,
            expect: Settlement::NoOp,
        },
    ];

    for case in cases {
        let tui = bulk_through_tui(&case.rows, &case.marked, case.mode).await;
        let (v2, settlement) = bulk_through_v2(&case.rows, &case.marked, case.mode).await;

        assert_eq!(
            settlement, case.expect,
            "{}: /api/v2 settlement must match the declared outcome",
            case.name
        );
        assert_eq!(
            tui, v2,
            "{}: the TUI and /api/v2 must produce the same marks, queue intent, and scheduler effects",
            case.name
        );
    }
}
