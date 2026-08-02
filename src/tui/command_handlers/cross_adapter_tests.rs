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
use crate::orchestration::run_control::testing::SchedulerCall;
use crate::tui::types::AppMode;
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

/// The `app_mode` string the v2 projection publishes for a TUI mode.
///
/// Both adapters must be given the *same* operator mode or the comparison would
/// be meaningless, and this is the only place the two vocabularies meet.
fn app_mode_string(mode: &AppMode) -> &'static str {
    match mode {
        AppMode::Running => "running",
        AppMode::Stopping => "stopping",
        AppMode::Stopped => "stopped",
        AppMode::Error => "error",
        _ => "select",
    }
}

// ============================================================================
// Adapter drivers
// ============================================================================

/// Run one intent through the TUI adapter and report what it settled as.
async fn through_tui(setup: Setup, mode: AppMode, command: TuiCommand) -> (Effects, bool) {
    let harness = AdapterHarness::new(&CHANGES);
    arrange(&harness, setup).await;

    let mut app = harness.app(&CHANGES);
    app.mode = mode;
    app.apply_display_statuses_from_reducer(&harness.state.read().await.all_display_statuses());
    app.warning_message = None;

    harness.run(&mut app, command).await;

    (effects(&harness).await, app.warning_message.is_some())
}

/// Run one intent through the `/api/v2` executor and report what it settled as.
async fn through_v2(setup: Setup, mode: AppMode, command: CommandSpec) -> (Effects, Settlement) {
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

    let settlement = match executor.execute(&command).await {
        Ok(summary) if summary.changed => Settlement::Changed,
        Ok(_) => Settlement::NoOp,
        Err(failure) => Settlement::Failed(failure.error_code),
    };

    (effects(&harness).await, settlement)
}

// ============================================================================
// The table
// ============================================================================

/// One lifecycle intent, expressed in both adapter vocabularies.
struct Row {
    /// What the row demonstrates; used as the assertion label.
    name: &'static str,
    setup: Setup,
    mode: AppMode,
    tui: TuiCommand,
    v2: CommandSpec,
    expect: Settlement,
}

fn rows() -> Vec<Row> {
    vec![
        // ── start ───────────────────────────────────────────────────────────
        Row {
            name: "start with an idle scheduler spawns one run over the marked set",
            setup: Setup::Marked,
            mode: AppMode::Select,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Changed,
        },
        Row {
            name: "start with a live scheduler wakes it instead of spawning a second run",
            setup: Setup::MarkedWithLiveScheduler,
            mode: AppMode::Select,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Changed,
        },
        Row {
            name: "start from a stopped run resumes the marked set",
            setup: Setup::Marked,
            mode: AppMode::Stopped,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Changed,
        },
        Row {
            name: "start is refused while a run owns the lifecycle",
            setup: Setup::MarkedWithLiveScheduler,
            mode: AppMode::Running,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Failed(ErrorCode::LifecycleConflict),
        },
        Row {
            name: "start with an empty target set is not a success",
            setup: Setup::Bare,
            mode: AppMode::Select,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Failed(ErrorCode::TargetIneligible),
        },
        Row {
            name: "a runtime launch failure is reported, not claimed as started",
            setup: Setup::MarkedWithFailingLaunch,
            mode: AppMode::Select,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Failed(ErrorCode::InternalError),
        },
        // ── retry ───────────────────────────────────────────────────────────
        Row {
            name: "retry routes a marked error row and dispatches the scheduler",
            setup: Setup::MarkedError,
            mode: AppMode::Error,
            tui: TuiCommand::Retry,
            v2: CommandSpec::Start,
            expect: Settlement::Changed,
        },
        Row {
            name: "retry without retryable evidence changes nothing",
            setup: Setup::Marked,
            mode: AppMode::Error,
            tui: TuiCommand::Retry,
            v2: CommandSpec::Start,
            expect: Settlement::NoOp,
        },
        // ── stop family ─────────────────────────────────────────────────────
        Row {
            name: "graceful stop while running sets the stop request",
            setup: Setup::LiveScheduler,
            mode: AppMode::Running,
            tui: TuiCommand::Stop,
            v2: CommandSpec::Stop,
            expect: Settlement::Changed,
        },
        Row {
            name: "graceful stop outside running is refused",
            setup: Setup::Bare,
            mode: AppMode::Select,
            tui: TuiCommand::Stop,
            v2: CommandSpec::Stop,
            expect: Settlement::Failed(ErrorCode::LifecycleConflict),
        },
        Row {
            name: "cancel stop while stopping withdraws the request",
            setup: Setup::LiveScheduler,
            mode: AppMode::Stopping,
            tui: TuiCommand::CancelStop,
            v2: CommandSpec::CancelStop,
            expect: Settlement::Changed,
        },
        Row {
            name: "cancel stop outside stopping is refused",
            setup: Setup::LiveScheduler,
            mode: AppMode::Running,
            tui: TuiCommand::CancelStop,
            v2: CommandSpec::CancelStop,
            expect: Settlement::Failed(ErrorCode::LifecycleConflict),
        },
        Row {
            name: "force stop while running cancels the live run",
            setup: Setup::LiveScheduler,
            mode: AppMode::Running,
            tui: TuiCommand::ForceStop,
            v2: CommandSpec::ForceStop,
            expect: Settlement::Changed,
        },
        Row {
            name: "force stop outside running and stopping is refused",
            setup: Setup::Bare,
            mode: AppMode::Select,
            tui: TuiCommand::ForceStop,
            v2: CommandSpec::ForceStop,
            expect: Settlement::Failed(ErrorCode::LifecycleConflict),
        },
        // ── resolve ─────────────────────────────────────────────────────────
        Row {
            name: "resolve of a merge-wait change takes the single resolver slot",
            setup: Setup::MergeWait,
            mode: AppMode::Select,
            tui: TuiCommand::ResolveMerge("c1".to_string()),
            v2: CommandSpec::ResolveMerge {
                change_id: "c1".to_string(),
            },
            expect: Settlement::Changed,
        },
        Row {
            name: "a duplicate resolve submission does not reserve twice",
            setup: Setup::MergeWaitAlreadyReserved,
            mode: AppMode::Select,
            tui: TuiCommand::ResolveMerge("c1".to_string()),
            v2: CommandSpec::ResolveMerge {
                change_id: "c1".to_string(),
            },
            expect: Settlement::NoOp,
        },
        Row {
            name: "resolve of a stale target is refused without a reservation",
            setup: Setup::Bare,
            mode: AppMode::Select,
            tui: TuiCommand::ResolveMerge("c1".to_string()),
            v2: CommandSpec::ResolveMerge {
                change_id: "c1".to_string(),
            },
            expect: Settlement::Failed(ErrorCode::TargetIneligible),
        },
    ]
}

#[tokio::test]
async fn tui_and_v2_settle_every_lifecycle_intent_identically() {
    for row in rows() {
        let (tui_effects, tui_reported) =
            through_tui(row.setup, row.mode.clone(), row.tui.clone()).await;
        let (v2_effects, v2_settlement) = through_v2(row.setup, row.mode.clone(), row.v2).await;

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
        assert_eq!(
            tui_reported,
            row.expect.is_reported_to_the_operator(),
            "{}: the TUI must surface exactly the refusals and no-ops /api/v2 reports",
            row.name
        );
    }
}
