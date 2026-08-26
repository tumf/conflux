//! TUI ↔ `/api/v2` adapter parity.
//!
//! Every other test in this change checks one adapter. These check the property
//! the change exists for: for the same lifecycle intent, in the same mode, over
//! the same state, a keypress and a remote command produce the *same* reducer
//! transition, the *same* scheduler dispatch, the *same* accepted / no-op /
//! refused settlement, and the *same* thing on both frontends.
//!
//! Each row is run twice over two independently constructed but identically
//! arranged processes — once through [`handle_tui_command`], once through
//! [`SharedServiceExecutor`]. Both processes carry a real `AppState` *and* a
//! real `WebState` on one dispatch owner, so what is compared is not just the
//! shared decision state but what each frontend would render next and how many
//! authoritative events and projection revisions the command produced. A
//! divergence anywhere (an extra `start_run`, a queue intent applied on only one
//! side, a reservation taken twice, a mode that reached one frontend and not the
//! other, an outcome published twice) fails the comparison as one value, so a
//! future frontend-local shortcut cannot pass silently.
//!
//! Integration-scoped by the shared `WebState` projection; the scheduler is the
//! recording double, the queue is the in-memory `DynamicQueue`, and no
//! repository, process, or network is touched.

use std::sync::Arc;

use super::tests::{create_test_change, AdapterHarness};
use super::*;

use crate::events::ExecutionEvent;
use crate::orchestration::operator_command::ParallelEligibility;
use crate::orchestration::run_control::testing::SchedulerCall;
use crate::tui::state::AppState;
use crate::tui::types::AppExecutionMode;
use crate::web::remote_control_api::dto::{CommandSpec, ErrorCode};
use crate::web::remote_control_api::executor::{RemoteControlExecutor, SharedServiceExecutor};
use crate::web::state::{WebEventSink, WebState};

/// The two changes every row is arranged over.
const CHANGES: [&str; 2] = ["c1", "c2"];

// ============================================================================
// One process, both frontends
// ============================================================================

/// An arranged process with a TUI and a Web frontend on one dispatch owner.
///
/// Two owners here would make every frontend assertion below vacuous: each
/// adapter would be reading back its own writes rather than the other's.
struct Wired {
    harness: AdapterHarness,
    app: AppState,
    web: Arc<WebState>,
    /// Every change the process was arranged over, in the arranged order.
    change_ids: Vec<String>,
    /// The projection revision observed before the command under test ran.
    ///
    /// Revision *counts* are the comparable quantity, not revision values: two
    /// independently constructed processes agree on how many revisions one
    /// command produced, which is the property acceptance criterion 10 states.
    baseline_revision: u64,
}

/// Attach both frontends to an already-arranged harness and settle the baseline.
///
/// `app` arrives with its row caches already painted from the reducer, because
/// what a row arranges differs (parallel eligibility, pre-existing marks) while
/// the wiring never does.
async fn wired(
    change_ids: &[&str],
    mode: AppExecutionMode,
    harness: AdapterHarness,
    mut app: AppState,
) -> Wired {
    app.execution_mode = mode;
    app.warning_message = None;

    let web = Arc::new(WebState::new(&[]));
    web.set_shared_state(harness.state.clone()).await;
    web.set_execution_marks(harness.marks.clone()).await;
    web.set_parallel_runtime(harness.parallel.clone()).await;
    web.set_repo_root(std::path::PathBuf::from("/repo")).await;
    let changes: Vec<_> = change_ids.iter().map(|id| create_test_change(id)).collect();
    web.update_with_mode(&changes, app_mode_string(&mode)).await;
    web.sync_remote_control_projection().await;

    // The Web frontend joins the boundary the TUI is already on, and the same
    // projection is what the coordinator reads its outcome revisions back from.
    harness.attach(Arc::new(WebEventSink::new(web.clone())));
    harness.attach_revisions(web.clone());
    // The arranged frontend mode *is* the arranged process mode: they are one
    // value in production, so arranging only one would exercise a state the
    // process cannot be in.
    harness.core_mode.set(mode.operator_mode());

    let baseline_revision = web.remote_control().projection().revision();
    Wired {
        harness,
        app,
        web,
        change_ids: change_ids.iter().map(|id| (*id).to_string()).collect(),
        baseline_revision,
    }
}

// ============================================================================
// Comparable observations
// ============================================================================

/// The shared decision state both adapters must agree on after one command.
#[derive(Debug, PartialEq, Eq)]
struct SharedEffects {
    /// Every scheduler interaction, in order.
    ///
    /// Carries the start/notify counts, the cancellation requests, and the
    /// graceful-stop flag writes, so all three are compared as one sequence
    /// rather than as three independently sampled counters.
    scheduler: Vec<SchedulerCall>,
    /// Reducer display status per change, in a stable order.
    statuses: Vec<(String, String)>,
    /// Reducer queue membership, sorted.
    reducer_queue: Vec<String>,
    /// Explicit-retry edges armed for the scheduler, sorted.
    ///
    /// A one-shot edge separate from ordinary queue intent: a retry that
    /// committed the reducer transition without arming it would start nothing,
    /// and one that armed it twice would release a failed classification the
    /// operator only asked to release once.
    explicit_retries: Vec<String>,
    /// The change owning the single resolver slot, if any.
    active_resolver: Option<String>,
    /// Changes waiting behind the active resolver, in FIFO order.
    queued_resolves: Vec<String>,
    /// The authoritative marked target set.
    marks: Vec<String>,
    /// The one process lifecycle mode both frontends project.
    mode: crate::orchestration::operator_command::OperatorMode,
}

/// Shared decision state *plus* what each frontend would show next and how much
/// the command published.
///
/// The frontend halves are the reason this file exists: the bug it guards
/// against was a command that committed the right reducer transition and then
/// reached only the adapter that submitted it, which no shared-state-only
/// comparison can see.
#[derive(Debug, PartialEq, Eq)]
struct Effects {
    shared: SharedEffects,
    /// What the TUI would render on its next pass.
    tui_mode: AppExecutionMode,
    /// Rows the TUI would paint as marked.
    tui_marked_rows: Vec<String>,
    /// Whether the TUI would paint an active manual resolve.
    tui_resolving: bool,
    /// The `app_mode` a `/api/v2` client would read.
    web_mode: String,
    /// The marks a `/api/v2` client would read, from the published projection.
    web_marks: Vec<String>,
    /// Whether a `/api/v2` client would see an active manual resolve.
    web_resolving: bool,
    /// How many authoritative dispatches the command produced.
    dispatches: usize,
    /// How many projection revisions the command produced.
    revisions: u64,
}

impl Wired {
    fn row(&self, change_id: &str) -> &crate::tui::state::ChangeState {
        self.app
            .changes
            .iter()
            .find(|change| change.id == change_id)
            .expect("the arranged change exists")
    }

    async fn shared_effects(&self) -> SharedEffects {
        let (statuses, reducer_queue) = {
            let guard = self.harness.state.read().await;
            let statuses = self
                .change_ids
                .iter()
                .map(|id| (id.clone(), guard.display_status(id).to_string()))
                .collect();
            let mut queued = guard.queued_change_ids();
            queued.sort();
            (statuses, queued)
        };
        SharedEffects {
            scheduler: self.harness.scheduler.calls(),
            statuses,
            reducer_queue,
            // Draining is the only read the queue offers, and this is the last
            // observation of the process, so nothing downstream can miss it.
            explicit_retries: self
                .harness
                .queue
                .drain_explicit_retries()
                .await
                .into_iter()
                .map(|edge| edge.change_id)
                .collect(),
            active_resolver: self.harness.resolves.active(),
            queued_resolves: self.harness.resolves.waiting(),
            marks: self.harness.marks.marked_ids(),
            mode: self.harness.core_mode.get(),
        }
    }

    async fn effects(&self) -> Effects {
        let shared = self.shared_effects().await;
        let (snapshot, _, _) = self.web.remote_control().projection().snapshot();
        let mut web_marks: Vec<String> = snapshot
            .changes
            .iter()
            .filter(|change| change.execution_marked)
            .map(|change| change.id.clone())
            .collect();
        web_marks.sort();
        let web = self.web.get_state().await;

        Effects {
            shared,
            tui_mode: self.app.execution_mode,
            tui_marked_rows: self
                .app
                .changes
                .iter()
                .filter(|change| change.selected)
                .map(|change| change.id.clone())
                .collect(),
            tui_resolving: self.app.is_resolving(),
            web_mode: web.app_mode,
            web_marks,
            web_resolving: web.is_resolving,
            dispatches: self.harness.dispatch_count(),
            revisions: self.web.remote_control().projection().revision() - self.baseline_revision,
        }
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
    /// Both changes are in a merge wait and `c1` holds the single resolver slot.
    ///
    /// The queued-reservation case: `c2` is a valid resolve target that must
    /// take a FIFO position instead of a second resolver, and must not dispatch
    /// the scheduler.
    BothMergeWaitingFirstReserved,
    /// Both changes are marked and queued; `c2` is not parallel-eligible.
    MarkedWithIneligible,
    /// A live scheduler with in-flight agent execution and pending cleanup.
    ///
    /// The force-stop class that must *not* publish a terminal stop yet: the
    /// scheduler owns the cancellation-safe boundary.
    LiveSchedulerWithInFlightExecution,
    /// `c1` is queued in a live run, so a dequeue has something to cancel.
    QueuedInLiveRun,
    /// `c1` is applying in a live run with no registered execution handle.
    ///
    /// The cancellation case a dequeue must refuse: an active change whose
    /// termination cannot be proven must not be dequeued on either adapter.
    ActiveWithoutCancellationHandle,
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
        Setup::MergeWait => merge_wait(harness, "c1").await,
        Setup::MergeWaitAlreadyReserved => {
            merge_wait(harness, "c1").await;
            harness
                .run_control
                .resolve_merge("c1")
                .await
                .expect("the first resolve of a merge-wait change is accepted");
        }
        Setup::BothMergeWaitingFirstReserved => {
            merge_wait(harness, "c1").await;
            merge_wait(harness, "c2").await;
            harness
                .run_control
                .resolve_merge("c1")
                .await
                .expect("the first resolve of a merge-wait change is accepted");
        }
        Setup::ActiveWithoutCancellationHandle => {
            harness.scheduler.set_running(true);
            harness
                .state
                .write()
                .await
                .apply_execution_event(&ExecutionEvent::ApplyStarted {
                    change_id: "c1".to_string(),
                    command: "apply".to_string(),
                });
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

async fn merge_wait(harness: &AdapterHarness, change_id: &str) {
    harness
        .state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: change_id.to_string(),
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

/// Build the arranged process one row is evaluated over.
///
/// Both drivers call this, so the only difference between the two sides of a
/// comparison is which adapter translated the operator's intent.
async fn arranged(setup: Setup, mode: AppExecutionMode) -> Wired {
    let harness = AdapterHarness::new(&CHANGES);
    arrange(&harness, setup).await;
    // Captured before the app is built: binding an `AppState` republishes the
    // eligibility set from its own rows, which is exactly what production does.
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

    wired(&CHANGES, mode, harness, app).await
}

/// What one adapter did to one arranged process.
///
/// `before` is what the arrangement produced, observed through exactly the same
/// lens as `after`. Keeping both is what stops a row from passing vacuously: a
/// row declared `Changed` whose effects equal its own arrangement changed
/// nothing, and a row declared `NoOp` or `Failed` whose effects moved committed
/// something it was not allowed to.
struct Run {
    before: Effects,
    after: Effects,
    /// What the adapter told the operator, when it told them anything.
    report: Option<String>,
}

/// Run one intent through the TUI adapter and report what it settled as.
///
/// The report is the operator-facing message the TUI surfaced, which is its
/// counterpart to the v2 summary detail.
async fn through_tui(setup: Setup, mode: AppExecutionMode, command: TuiCommand) -> Run {
    let mut wired = arranged(setup, mode).await;
    let before = wired.effects().await;

    let is_two_phase = matches!(command, TuiCommand::DequeueChange(_));
    wired.harness.run(&mut wired.app, command).await;
    if is_two_phase {
        // Stop-and-dequeue deliberately runs off the ordered submission queue so
        // a stuck termination cannot hold later keypresses; joining it here is
        // what the runner's next frames would do.
        wired
            .harness
            .await_feedback(&mut wired.app, std::time::Duration::from_secs(5))
            .await;
    }

    let report = wired.app.warning_message.clone();
    Run {
        before,
        after: wired.effects().await,
        report,
    }
}

/// Run one intent through the `/api/v2` executor and report what it settled as.
///
/// The report is the summary detail, the only place v2 names a consequence the
/// command did not ask for.
async fn through_v2(
    setup: Setup,
    mode: AppExecutionMode,
    command: CommandSpec,
) -> (Run, Settlement) {
    let mut wired = arranged(setup, mode).await;
    let before = wired.effects().await;
    let executor = SharedServiceExecutor::new(wired.harness.application.clone(), wired.web.clone());

    let (settlement, detail) = match executor.execute(&command).await {
        Ok(summary) if summary.changed => (Settlement::Changed, summary.detail),
        Ok(summary) => (Settlement::NoOp, summary.detail),
        Err(failure) => (
            Settlement::Failed(failure.error_code),
            Some(failure.message),
        ),
    };
    // One TUI event-processing pass, exactly as the runner performs it: what the
    // other frontend renders next is part of what this row compares.
    wired.harness.deliver(&mut wired.app).await;

    (
        Run {
            before,
            after: wired.effects().await,
            report: detail,
        },
        settlement,
    )
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
            // A live run admits only marked retry routes, and these marks carry
            // no retryable evidence: the refusal is target-shaped rather than
            // mode-shaped, and both adapters must reach the same one.
            name: "start under a live run is refused when no mark is retryable",
            setup: Setup::MarkedWithLiveScheduler,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Failed(ErrorCode::TargetIneligible),
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
            // Admission owns the refusal now: a mark is accepted whatever the
            // row's status, so Error-mode retry with nothing retryable marked
            // is rejected at start rather than quietly settling as a no-op.
            name: "retry without retryable evidence is rejected without effects",
            setup: Setup::Marked,
            mode: AppExecutionMode::Error,
            tui: TuiCommand::StartProcessing(Vec::new()),
            v2: CommandSpec::Start,
            expect: Settlement::Failed(ErrorCode::TargetIneligible),
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
            name: "force stop escalates a pending graceful stop",
            setup: Setup::LiveScheduler,
            mode: AppExecutionMode::Stopping,
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
            name: "a second resolve target takes a FIFO position, not a second resolver",
            setup: Setup::BothMergeWaitingFirstReserved,
            mode: AppExecutionMode::Select,
            tui: TuiCommand::ResolveMerge("c2".to_string()),
            v2: CommandSpec::ResolveMerge {
                change_id: "c2".to_string(),
            },
            expect: Settlement::Changed,
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
        Row {
            name: "queue add for an already queued row changes nothing on either adapter",
            setup: Setup::QueuedInLiveRun,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::AddToQueue("c1".to_string()),
            v2: CommandSpec::SetQueueIntent {
                change_id: "c1".to_string(),
                queued: true,
            },
            expect: Settlement::NoOp,
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
            name: "stop-and-dequeue of an unprovable termination commits no dequeue",
            setup: Setup::ActiveWithoutCancellationHandle,
            mode: AppExecutionMode::Running,
            tui: TuiCommand::DequeueChange("c1".to_string()),
            v2: CommandSpec::StopAndDequeue {
                change_id: "c1".to_string(),
            },
            expect: Settlement::Failed(ErrorCode::TargetIneligible),
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
        let tui = through_tui(row.setup, row.mode, row.tui.clone()).await;
        let (v2, v2_settlement) = through_v2(row.setup, row.mode, row.v2).await;

        assert_eq!(
            v2_settlement, row.expect,
            "{}: /api/v2 settlement must match the declared outcome",
            row.name
        );
        assert_eq!(
            tui.after, v2.after,
            "{}: the TUI and /api/v2 must produce the same reducer, scheduler, mark, resolver, \
             frontend, and event effects",
            row.name
        );

        // What the declared settlement *means* for the process, checked against
        // the row's own arrangement rather than against a hand-written expected
        // value. A refused or no-op command must leave every one of these
        // untouched — including the event and revision counters, which is where
        // "no side effect" and "published nothing" become the same claim.
        for (adapter, run) in [("the TUI", &tui), ("/api/v2", &v2)] {
            match row.expect {
                Settlement::Changed => assert_ne!(
                    run.after, run.before,
                    "{}: {adapter} declared a change and produced none",
                    row.name
                ),
                Settlement::NoOp | Settlement::Failed(_) => assert_eq!(
                    run.after, run.before,
                    "{}: {adapter} must leave the process exactly as it found it",
                    row.name
                ),
            }
        }

        // A refusal or a no-op is reported by both adapters; so is a consequence
        // the command did not ask for. Anything else is a plain success and must
        // be silent on both sides, or one frontend is telling an operator
        // something the other is hiding.
        let must_report = row.expect.is_reported_to_the_operator() || row.notice.is_some();
        assert_eq!(
            tui.report.is_some(),
            must_report,
            "{}: the TUI must surface exactly what /api/v2 reports, got {:?}",
            row.name,
            tui.report
        );

        if let Some(cleared) = row.notice {
            // Naming the change is the point: an operator whose intent vanished
            // must be able to tell that from a lost command, on both adapters.
            let tui_message = tui
                .report
                .expect("a consequence row surfaces a TUI message");
            assert!(
                tui_message.contains(cleared),
                "{}: the TUI must name '{cleared}', got {tui_message:?}",
                row.name
            );
            let v2_detail = v2.report.expect("a consequence row carries a v2 detail");
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
/// it: Start notifies instead of spawning and opens the run episode from the
/// accepted outcome, marks stay mark-only, graceful stop wakes the parked
/// waiter, the controls stay discoverable, and an idle-origin cancel-stop —
/// one that never had an accepted Start — returns to Ready.
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
        .set_execution_mark("c1", true)
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
        AppExecutionMode::Running,
        "an accepted Start opens the run episode the operator asked for"
    );
    assert!(
        !app.persistent_scheduler_idle,
        "the accepted Start closes the idle presentation episode"
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
//
// The comparison stops at the shared decision state rather than at the two
// frontends. The keypress path writes its classified marks straight to the
// shared store instead of submitting one `SetAllExecutionMarks` intent, so it
// publishes no outcome of its own and the two sides genuinely differ in event
// and revision cardinality. Converging that path means moving bulk
// classification behind the coordinator, which the proposal leaves to the
// separate bulk/parallel-control work; asserting it here would be asserting on
// a design this change does not own.

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
            // Archive alone is not terminal: it hands the row to the
            // post-archive resolve lane, which is still a legitimate next-run
            // target. The merge is what makes the row terminal.
            "merged" => {
                guard.apply_execution_event(&ExecutionEvent::ChangeArchived((*id).to_string()));
                guard.apply_execution_event(&ExecutionEvent::MergeCompleted {
                    change_id: (*id).to_string(),
                    revision: "deadbeef".to_string(),
                });
            }
            "not queued" => {}
            other => panic!("unsupported bulk-mark arrangement status '{other}'"),
        }
    }
}

/// Build the arranged process one bulk-mark case is evaluated over.
async fn arranged_bulk(rows: &[BulkRow], marked: &[&str], mode: AppExecutionMode) -> Wired {
    let ids: Vec<&str> = rows.iter().map(|(id, ..)| *id).collect();
    let harness = AdapterHarness::new(&ids);
    arrange_bulk(&harness, rows, marked).await;

    let mut app = harness.app(&ids);
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

    wired(&ids, mode, harness, app).await
}

async fn bulk_through_tui(
    rows: &[BulkRow],
    marked: &[&str],
    mode: AppExecutionMode,
) -> SharedEffects {
    let mut wired = arranged_bulk(rows, marked, mode).await;

    // The key handler classifies and records target-scoped mark writes; the
    // runner loop applies those through the shared service and then feeds the
    // emitted queue commands back through it, in that order.
    let commands = wired.app.toggle_all_marks();
    for (change_id, marked) in wired.app.take_pending_mark_writes() {
        wired
            .harness
            .run_control
            .operator()
            .apply_execution_mark(&change_id, marked)
            .await;
    }
    wired.app.sync_execution_marks_from_store();
    for command in commands {
        wired.harness.run(&mut wired.app, command).await;
    }

    wired.shared_effects().await
}

async fn bulk_through_v2(
    rows: &[BulkRow],
    marked: &[&str],
    mode: AppExecutionMode,
) -> (SharedEffects, Settlement) {
    let mut wired = arranged_bulk(rows, marked, mode).await;
    let executor = SharedServiceExecutor::new(wired.harness.application.clone(), wired.web.clone());

    let settlement = match executor
        .execute(&CommandSpec::SetAllExecutionMarks {})
        .await
    {
        Ok(summary) if summary.changed => Settlement::Changed,
        Ok(_) => Settlement::NoOp,
        Err(failure) => Settlement::Failed(failure.error_code),
    };
    wired.harness.deliver(&mut wired.app).await;

    (wired.shared_effects().await, settlement)
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
            name: "an uncommitted row joins the target set: eligibility is a start-time fact",
            rows: vec![("c1", "not queued", true), ("c2", "not queued", false)],
            marked: vec![],
            mode: AppExecutionMode::Select,
            expect: Settlement::Changed,
        },
        Case {
            name: "running mode marks the active row too, and writes no queue intent",
            rows: vec![("c1", "not queued", true), ("c2", "applying", true)],
            marked: vec![],
            mode: AppExecutionMode::Running,
            expect: Settlement::Changed,
        },
        Case {
            name: "an already-queued row is marked without its queue membership moving",
            rows: vec![("c1", "queued", true), ("c2", "applying", true)],
            marked: vec!["c1"],
            mode: AppExecutionMode::Running,
            expect: Settlement::Changed,
        },
        Case {
            name: "a fully marked non-terminal set unmarks without dequeuing anything",
            rows: vec![("c1", "queued", true), ("c2", "applying", true)],
            marked: vec!["c1", "c2"],
            mode: AppExecutionMode::Running,
            expect: Settlement::Changed,
        },
        Case {
            name: "a terminal-only target set changes nothing on either side",
            rows: vec![("c1", "rejected", true), ("c2", "merged", true)],
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

// ============================================================================
// Change-scoped failure vs. process mode
// ============================================================================

/// One change failing must not disable the mark controls of the whole process.
///
/// Driven through the authoritative dispatcher and a whole runner frame — event
/// dispatch, reducer transition, mark reconciliation, frontend delivery, Core
/// mode adoption — because the incident lived *between* those steps: the TUI row
/// handler already kept the mode, and the next frame then adopted a Core mode
/// that had promoted the change-scoped failure to process-wide Error. Calling
/// the row handler alone would have passed throughout the bug.
#[tokio::test]
async fn processing_error_keeps_bulk_mark_available() {
    const RUNNING_RUN: [&str; 3] = ["alpha", "beta", "gamma"];

    let harness = AdapterHarness::new(&RUNNING_RUN);
    {
        let mut guard = harness.state.write().await;
        for id in ["alpha", "beta"] {
            guard.apply_command(crate::orchestration::state::ReducerCommand::AddToQueue(
                id.to_string(),
            ));
        }
        // `alpha` is the change that is really executing when it fails.
        guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "apply".to_string(),
        });
    }
    harness
        .marks
        .replace(["alpha".to_string(), "beta".to_string()]);

    let mut app = harness.app(&RUNNING_RUN);
    app.apply_display_statuses_from_reducer(&harness.state.read().await.all_display_statuses());
    app.sync_execution_marks_from_store();
    let mut wired = wired(&RUNNING_RUN, AppExecutionMode::Running, harness, app).await;

    // The authoritative dispatch: one reducer transition, one mark
    // reconciliation, one delivery to each frontend.
    wired
        .harness
        .dispatcher
        .dispatch(ExecutionEvent::ProcessingError {
            id: "alpha".to_string(),
            error: "acceptance command attempts exhausted".to_string(),
        })
        .await;
    // The rest of the frame, including the per-frame Core-mode adoption that
    // used to overwrite the row handler's correct answer.
    wired.harness.deliver(&mut wired.app).await;

    assert_eq!(
        wired.harness.core_mode.get(),
        crate::orchestration::operator_command::OperatorMode::Running,
        "a change-scoped failure leaves the one process mode alone"
    );
    assert_eq!(
        wired.app.execution_mode,
        AppExecutionMode::Running,
        "so the frame the TUI adopts stays Running"
    );
    assert_eq!(
        wired.web.get_state().await.app_mode,
        "running",
        "and the other frontend agrees at the same dispatch"
    );
    assert_eq!(
        wired.harness.status("alpha").await,
        "error",
        "the failed change still receives its change-level Error transition"
    );
    assert!(
        !wired.harness.marks.is_marked("alpha"),
        "and its stale execution intent is revoked by the same dispatch"
    );
    assert!(
        !wired.row("alpha").selected,
        "the row follows the reconciled store rather than its stale cache"
    );
    assert!(
        wired.harness.marks.is_marked("beta"),
        "an unrelated change's mark is untouched by another change's failure"
    );

    // `x`, exactly as the runner drives it: the key handler plans and records
    // target-scoped mark writes, the loop applies them through the shared
    // service, then feeds the emitted queue commands back through it.
    let commands = wired.app.toggle_all_marks();
    for (change_id, marked) in wired.app.take_pending_mark_writes() {
        wired
            .harness
            .run_control
            .operator()
            .apply_execution_mark(&change_id, marked)
            .await;
    }
    wired.app.sync_execution_marks_from_store();
    for command in commands {
        wired.harness.run(&mut wired.app, command).await;
    }

    let report = wired.app.warning_message.clone().unwrap_or_default();
    assert!(
        !report.contains("recovery is owned by retry"),
        "the Error-mode bulk rejection must not fire for a change-scoped failure: {report}"
    );
    assert!(
        !report.contains("Error mode"),
        "and no Error-mode block of any wording may be reported: {report}"
    );

    // The unrelated eligible row really moved: the plan marks every visible
    // non-terminal row. The mark is all it moves — Running mode no longer
    // carries a queue intent along with it.
    assert!(
        wired.harness.marks.is_marked("gamma"),
        "an unrelated eligible row is still bulk-markable"
    );
    assert!(wired.row("gamma").selected);
    assert_eq!(
        wired.harness.status("gamma").await,
        "not queued",
        "and the mark wrote no Running-mode queue intent"
    );
    assert!(
        wired.harness.marks.is_marked("beta"),
        "an already-marked eligible row keeps its mark"
    );

    // `alpha` is governed by the existing Running-mode rules for an error row —
    // the explicit-retry mark alias — rather than by anything this change added.
    // What the change removed is the process-wide Error mode that would have
    // refused the whole operation before any row was classified.
    assert_eq!(
        wired.harness.status("alpha").await,
        "error",
        "the failed change stays in change-level Error"
    );
}
