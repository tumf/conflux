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
use crate::web::remote_control_api::executor::RemoteControlExecutor;
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
    /// The one process lifecycle mode both frontends project.
    mode: crate::orchestration::operator_command::OperatorMode,
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
        mode: harness.core_mode.get(),
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
    /// A live scheduler with in-flight agent execution and pending cleanup.
    ///
    /// The force-stop class that must *not* publish a terminal stop yet: the
    /// scheduler owns the cancellation-safe boundary.
    LiveSchedulerWithInFlightExecution,
    /// `c1` is queued in a live run, so a dequeue has something to cancel.
    QueuedInLiveRun,
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
        Setup::LiveSchedulerWithInFlightExecution => {
            use crate::tui::stop_classification::{ExecutionEvidence, ShutdownWorkEvidence};
            harness.scheduler.set_running(true);
            harness
                .scheduler
                .set_activity(crate::tui::stop_classification::StopActivitySnapshot {
                    execution_handles: ExecutionEvidence::Known { registered: 1 },
                    reducer_agent_execution_active: true,
                    shutdown_work: ShutdownWorkEvidence::Known { pending: true },
                });
        }
        Setup::QueuedInLiveRun => {
            harness.scheduler.set_running(true);
            let mut guard = harness.state.write().await;
            guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                "c1".to_string(),
            ));
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

    let is_two_phase = matches!(command, TuiCommand::DequeueChange(_));
    harness.run(&mut app, command).await;
    if is_two_phase {
        // Stop-and-dequeue deliberately runs off the ordered submission queue so
        // a stuck termination cannot hold later keypresses; joining it here is
        // what the runner's next frames would do.
        harness
            .await_feedback(&mut app, std::time::Duration::from_secs(5))
            .await;
    }

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

    // Wired the way production wires it: the remote adapter submits typed intent
    // to a coordinator over the *same* reducer, services, and scheduler the TUI
    // harness drives, so the two sides of this comparison differ only in which
    // adapter translated the operator's intent.
    let (executor, _application) = crate::web::remote_control_api::executor::wired_for_test(
        harness.state.clone(),
        harness.run_control.clone(),
        web_state.clone(),
        harness.core_mode.clone(),
    );
    harness.core_mode.set(mode.operator_mode());

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
        // ── force stop safe-boundary classes ────────────────────────────────
        Row {
            name: "force stop with in-flight execution waits for the safe boundary",
            setup: Setup::LiveSchedulerWithInFlightExecution,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::ForceStop,
            v2: CommandSpec::ForceStop,
            expect: Settlement::Changed,
            notice: None,
        },
        // ── queue intent ────────────────────────────────────────────────────
        Row {
            name: "queue add in a live run commits the same intent on both adapters",
            setup: Setup::LiveScheduler,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::AddToQueue("c1".to_string()),
            v2: CommandSpec::SetQueueIntent {
                change_id: "c1".to_string(),
                queued: true,
            },
            expect: Settlement::Changed,
            notice: None,
        },
        Row {
            name: "queue remove takes the same intent back out on both adapters",
            setup: Setup::QueuedInLiveRun,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::RemoveFromQueue("c1".to_string()),
            v2: CommandSpec::SetQueueIntent {
                change_id: "c1".to_string(),
                queued: false,
            },
            expect: Settlement::Changed,
            notice: None,
        },
        // ── stop and dequeue ────────────────────────────────────────────────
        Row {
            name: "stop-and-dequeue clears exactly its target on both adapters",
            setup: Setup::QueuedInLiveRun,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::DequeueChange("c1".to_string()),
            v2: CommandSpec::StopAndDequeue {
                change_id: "c1".to_string(),
            },
            expect: Settlement::Changed,
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

    // The key handler classifies and records target-scoped mark writes; the
    // runner loop applies those through the shared service and then feeds the
    // emitted queue commands back through it, in that order.
    let commands = app.toggle_all_marks();
    for (change_id, marked) in app.take_pending_mark_writes() {
        harness
            .run_control
            .operator()
            .apply_execution_mark(&change_id, marked)
            .await;
    }
    app.sync_execution_marks_from_store();
    for command in commands {
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

    let (executor, _application) = crate::web::remote_control_api::executor::wired_for_test(
        harness.state.clone(),
        harness.run_control.clone(),
        web_state.clone(),
        harness.core_mode.clone(),
    );
    harness.core_mode.set(mode.operator_mode());
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
