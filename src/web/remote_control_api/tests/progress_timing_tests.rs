//! Archive completion, net-active time, and the run clock on `/api/v2`.
//!
//! The parity cases drive one event sequence through the reducer, the web
//! store, and a TUI `AppState` at identical injected instants. They assert the
//! *existing* TUI behavior: nothing here changes a TUI boundary, so a case that
//! could only pass by changing one would be a blocker, not a fix.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::events::{ExecutionEvent, OperatorCommandEffect};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::state::{OrchestratorState, ReducerCommand};
use crate::tui::types::AppExecutionMode;
use crate::tui::AppState;
use crate::web::remote_control_api::dto::{ChangeResource, InstanceSnapshot};
use crate::web::state::WebState;

type Reducer = Arc<tokio::sync::RwLock<OrchestratorState>>;

fn change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 2,
        last_modified: "1m ago".to_string(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

/// One injected observation instant: monotonic plus its wall sample.
#[derive(Clone, Copy)]
struct Clock {
    base: Instant,
    wall: DateTime<Utc>,
}

impl Clock {
    fn new() -> Self {
        Self {
            base: Instant::now(),
            wall: DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap(),
        }
    }

    fn at(&self, seconds: u64) -> (Instant, DateTime<Utc>) {
        (
            self.base + Duration::from_secs(seconds),
            self.wall + chrono::Duration::seconds(seconds as i64),
        )
    }

    fn wall_at(&self, seconds: u64) -> String {
        self.at(seconds).1.to_rfc3339()
    }
}

/// A web store wired to a reducer, plus a TUI over the same rows.
struct Fixture {
    web: WebState,
    reducer: Reducer,
    tui: AppState,
    clock: Clock,
}

impl Fixture {
    async fn new(ids: &[&str]) -> Self {
        let reducer: Reducer = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            ids.iter().map(|id| id.to_string()).collect(),
            0,
        )));
        let web = WebState::new(&[]);
        web.set_shared_state(reducer.clone()).await;
        let changes: Vec<Change> = ids.iter().map(|id| change(id)).collect();
        web.update(&changes).await;
        Self {
            web,
            reducer,
            tui: AppState::new(changes),
            clock: Clock::new(),
        }
    }

    /// Apply a reducer command (queue admission, resolve reservation, resume).
    async fn command(&self, command: ReducerCommand) {
        self.reducer.write().await.apply_command(command);
    }

    /// Deliver one event to the reducer and the web store at `seconds`.
    async fn web_event(&self, seconds: u64, event: &ExecutionEvent) {
        let (now, wall) = self.clock.at(seconds);
        self.reducer.write().await.apply_execution_event(event);
        self.web.pin_observation_clock_for_tests(now, wall);
        self.web.apply_execution_event(event).await;
    }

    /// The TUI's reducer synchronization at an injected instant.
    ///
    /// The clock half of `set_display_status_cache` — the method it delegates
    /// to — fed the same reducer display statuses the web store observes.
    async fn sync_tui(&mut self, seconds: u64) {
        let (now, _) = self.clock.at(seconds);
        let reducer = self.reducer.read().await;
        for row in &mut self.tui.changes {
            let status = match reducer.display_status(&row.id) {
                "stopped" => "not queued",
                other => other,
            };
            row.observe_active_status_at(status, now);
            row.display_status_cache = status.to_string();
        }
        let archived: HashSet<String> = reducer.archived_changes().clone();
        drop(reducer);
        self.tui.apply_archived_changes_from_reducer(&archived);
    }

    fn snapshot(&self) -> (InstanceSnapshot, u64) {
        let (snapshot, revision, _) = self.web.remote_control().projection().snapshot();
        (snapshot, revision)
    }

    fn row(&self, id: &str) -> ChangeResource {
        self.snapshot()
            .0
            .changes
            .into_iter()
            .find(|change| change.id == id)
            .unwrap_or_else(|| panic!("'{id}' must be projected"))
    }

    /// Accepted Start on both frontends at `seconds`.
    ///
    /// The TUI takes the real `RunDispatched` path, which proves its own guard
    /// decided to reset; the reset is then re-applied at the injected instant so
    /// both frontends hold the same start sample.
    async fn start(&mut self, seconds: u64, ids: &[&str]) {
        let event = run_dispatched(ids);
        let tui_was_running = self.tui.execution_mode == AppExecutionMode::Running;
        let before = self.tui.orchestration_started_at;
        self.tui.handle_orchestrator_event(event.clone());
        if !tui_was_running {
            assert_ne!(self.tui.orchestration_started_at, before, "the TUI reset");
            self.tui.reset_for_run_at(self.clock.at(seconds).0);
        } else {
            assert_eq!(self.tui.orchestration_started_at, before);
        }
        self.web_event(seconds, &event).await;
    }

    /// `Stopped` on both frontends at `seconds`.
    async fn stopped(&mut self, seconds: u64) {
        self.tui.handle_stopped_at(self.clock.at(seconds).0);
        self.web_event(seconds, &ExecutionEvent::Stopped).await;
    }

    /// Assert the TUI run clock and the published run clock agree.
    fn assert_run_clock_parity(&self, started: Option<u64>, frozen: Option<(u64, u64)>) {
        let published = self.snapshot().0.run_clock.expect("run_clock is published");
        assert_eq!(
            self.tui.orchestration_started_at,
            started.map(|s| self.clock.at(s).0),
            "TUI start"
        );
        assert_eq!(published.started_at, started.map(|s| self.clock.wall_at(s)));
        assert_eq!(
            self.tui.orchestration_elapsed,
            frozen.map(|(_, elapsed)| Duration::from_secs(elapsed)),
            "TUI elapsed"
        );
        assert_eq!(
            published.elapsed_ms,
            self.tui
                .orchestration_elapsed
                .map(|elapsed| elapsed.as_millis() as u64),
            "API elapsed equals TUI elapsed"
        );
        assert_eq!(
            published.ended_at,
            frozen.map(|(at, _)| self.clock.wall_at(at))
        );
    }

    /// Assert the TUI row clock and the published net-active timing agree.
    fn assert_net_active_parity(&self, id: &str, seconds: u64) {
        let published = self.row(id).net_active.expect("net_active is published");
        let row = self
            .tui
            .changes
            .iter()
            .find(|row| row.id == id)
            .expect("TUI row");
        assert_eq!(
            Duration::from_millis(published.accumulated_ms),
            row.elapsed_time.unwrap_or_default(),
            "accumulated at +{seconds}s"
        );
        assert_eq!(
            published.active_since.is_some(),
            row.started_at.is_some(),
            "open state at +{seconds}s"
        );
        if let (Some(since), Some(open)) = (&published.active_since, row.started_at) {
            let offset = open.duration_since(self.clock.base).as_secs();
            assert_eq!(
                since,
                &self.clock.wall_at(offset),
                "boundary at +{seconds}s"
            );
        }
        assert_eq!(published.has_history, row.has_execution_history());
    }
}

fn run_dispatched(ids: &[&str]) -> ExecutionEvent {
    ExecutionEvent::OperatorCommandApplied {
        effect: OperatorCommandEffect::RunDispatched {
            change_ids: ids.iter().map(|id| id.to_string()).collect(),
            explicit_retry: false,
            scheduler_started: true,
        },
    }
}

fn apply_started(id: &str) -> ExecutionEvent {
    ExecutionEvent::ApplyStarted {
        change_id: id.to_string(),
        command: "apply".to_string(),
    }
}

// ============================================================================
// Archive completion
// ============================================================================

#[tokio::test]
async fn remote_progress_timing_archive_complete_follows_the_reducer() {
    let mut fx = Fixture::new(&["alpha", "beta"]).await;
    fx.command(ReducerCommand::AddToQueue("alpha".into())).await;
    let mut step = 0;
    let mut observed = Vec::new();
    for event in [
        apply_started("alpha"),
        ExecutionEvent::ArchiveStarted {
            change_id: "alpha".into(),
            command: "archive".into(),
        },
        ExecutionEvent::ChangeArchived("alpha".into()),
        ExecutionEvent::MergeDeferred {
            change_id: "alpha".into(),
            reason: "base moved".into(),
            auto_resumable: false,
        },
    ] {
        step += 1;
        fx.web_event(step, &event).await;
        fx.sync_tui(step).await;
        let row = fx.row("alpha");
        observed.push((row.display_status.clone(), row.archive_complete));
        // Equal to the TUI cache for every row in the same observation.
        for tui_row in &fx.tui.changes {
            assert_eq!(
                fx.row(&tui_row.id).archive_complete,
                Some(tui_row.archive_complete_cache),
                "'{}' after step {step}",
                tui_row.id
            );
        }
    }
    fx.command(ReducerCommand::ResolveMerge("alpha".into()))
        .await;
    for event in [
        ExecutionEvent::ResolveStarted {
            change_id: "alpha".into(),
            command: "resolve".into(),
        },
        ExecutionEvent::ResolveCompleted {
            change_id: "alpha".into(),
            worktree_change_ids: None,
        },
    ] {
        step += 1;
        fx.web_event(step, &event).await;
        let row = fx.row("alpha");
        observed.push((row.display_status.clone(), row.archive_complete));
    }

    assert_eq!(
        observed,
        vec![
            ("applying".to_string(), Some(false)),
            ("archiving".to_string(), Some(false)),
            ("resolving".to_string(), Some(true)),
            ("merge wait".to_string(), Some(true)),
            ("resolving".to_string(), Some(true)),
            ("merged".to_string(), Some(true)),
        ]
    );
    assert_eq!(fx.row("beta").archive_complete, Some(false));

    // A fresh-process resolve retry: `resolving` with no archive on record.
    let fresh = Fixture::new(&["alpha"]).await;
    fresh
        .web_event(
            1,
            &ExecutionEvent::MergeDeferred {
                change_id: "alpha".into(),
                reason: "base moved".into(),
                auto_resumable: false,
            },
        )
        .await;
    fresh
        .command(ReducerCommand::ResolveMerge("alpha".into()))
        .await;
    fresh
        .web_event(
            2,
            &ExecutionEvent::ResolveStarted {
                change_id: "alpha".into(),
                command: "resolve".into(),
            },
        )
        .await;
    let row = fresh.row("alpha");
    assert_eq!(row.display_status, "resolving");
    assert_eq!(row.archive_complete, Some(false));
}

// ============================================================================
// Net-active time
// ============================================================================

#[tokio::test]
async fn remote_progress_timing_net_active_matches_the_tui_row() {
    let mut fx = Fixture::new(&["alpha"]).await;
    fx.start(0, &["alpha"]).await;
    fx.command(ReducerCommand::AddToQueue("alpha".into())).await;

    // Apply and acceptance: one interval, opened once.
    for (t, event) in [
        (1, apply_started("alpha")),
        (
            5,
            ExecutionEvent::AcceptanceStarted {
                change_id: "alpha".into(),
                command: "accept".into(),
            },
        ),
    ] {
        fx.web_event(t, &event).await;
        fx.sync_tui(t).await;
        fx.assert_net_active_parity("alpha", t);
    }
    assert_eq!(
        fx.row("alpha").net_active.unwrap().active_since,
        Some(fx.clock.wall_at(1)),
        "a successive active status keeps the first boundary"
    );

    // Stop before sync: the TUI's stop handler closes first, the reducer's
    // `not queued` synchronization then finds nothing open.
    fx.stopped(10).await;
    fx.sync_tui(12).await;
    fx.assert_net_active_parity("alpha", 12);
    assert_eq!(fx.row("alpha").net_active.unwrap().accumulated_ms, 9_000);

    // Retry, park, re-admission, and a resolve reservation in the sequence.
    // Rows move only through the injected reducer synchronization: a real TUI
    // status handler would stamp its own `Instant::now()` boundary.
    fx.command(ReducerCommand::ResumeStopped("alpha".into()))
        .await;
    fx.command(ReducerCommand::AddToQueue("alpha".into())).await;
    fx.start(20, &["alpha"]).await;
    fx.sync_tui(20).await;
    fx.assert_net_active_parity("alpha", 20);
    fx.web_event(21, &apply_started("alpha")).await;
    fx.sync_tui(21).await;
    fx.assert_net_active_parity("alpha", 21);
    fx.web_event(25, &ExecutionEvent::PersistentSchedulerIdle)
        .await;
    fx.tui
        .handle_orchestrator_event(ExecutionEvent::PersistentSchedulerIdle);
    fx.sync_tui(25).await;
    fx.assert_net_active_parity("alpha", 25);
    fx.web_event(26, &apply_started("alpha")).await;
    fx.sync_tui(26).await;
    fx.assert_net_active_parity("alpha", 26);
    assert_eq!(
        fx.row("alpha").net_active.unwrap().active_since,
        Some(fx.clock.wall_at(21)),
        "re-admission inside one active stretch does not restart the interval"
    );

    for (t, event) in [
        (
            30,
            ExecutionEvent::ArchiveStarted {
                change_id: "alpha".into(),
                command: "archive".into(),
            },
        ),
        (34, ExecutionEvent::ChangeArchived("alpha".into())),
        (
            40,
            ExecutionEvent::MergeDeferred {
                change_id: "alpha".into(),
                reason: "base moved".into(),
                auto_resumable: false,
            },
        ),
    ] {
        fx.web_event(t, &event).await;
        fx.sync_tui(t).await;
        fx.assert_net_active_parity("alpha", t);
    }
    fx.command(ReducerCommand::ResolveMerge("alpha".into()))
        .await;
    let reserved = ExecutionEvent::OperatorCommandApplied {
        effect: OperatorCommandEffect::ResolveReserved {
            change_id: "alpha".into(),
            active: true,
        },
    };
    fx.web_event(41, &reserved).await;
    fx.sync_tui(41).await;
    fx.assert_net_active_parity("alpha", 41);
    for (t, event) in [
        (
            42,
            ExecutionEvent::ResolveStarted {
                change_id: "alpha".into(),
                command: "resolve".into(),
            },
        ),
        (
            47,
            ExecutionEvent::ResolveCompleted {
                change_id: "alpha".into(),
                worktree_change_ids: None,
            },
        ),
    ] {
        fx.web_event(t, &event).await;
        fx.sync_tui(t).await;
        fx.assert_net_active_parity("alpha", t);
    }

    // apply 1..10 = 9s, apply 21..40 = 19s (merge wait closes), resolve 42..47 = 5s.
    let published = fx.row("alpha").net_active.unwrap();
    assert_eq!(published.accumulated_ms, 33_000);
    assert_eq!(published.active_since, None);
    assert!(published.has_history);
}

#[tokio::test]
async fn remote_progress_timing_net_active_closes_once_when_sync_precedes_stop() {
    let mut fx = Fixture::new(&["alpha"]).await;
    fx.command(ReducerCommand::AddToQueue("alpha".into())).await;
    fx.web_event(1, &apply_started("alpha")).await;
    fx.sync_tui(1).await;

    // The reducer moves the row out first; the process `Stopped` then finds no
    // open interval on either side.
    fx.web_event(
        6,
        &ExecutionEvent::ChangeStopped {
            change_id: "alpha".into(),
        },
    )
    .await;
    fx.sync_tui(6).await;
    fx.assert_net_active_parity("alpha", 6);
    fx.stopped(9).await;
    fx.sync_tui(9).await;
    fx.assert_net_active_parity("alpha", 9);
    assert_eq!(fx.row("alpha").net_active.unwrap().accumulated_ms, 5_000);
}

// ============================================================================
// Run clock
// ============================================================================

#[tokio::test]
async fn remote_progress_timing_run_clock_boundaries() {
    let fx = Fixture::new(&["alpha"]).await;
    let clock = || fx.snapshot().0.run_clock.unwrap();
    let event = |t, e: ExecutionEvent| {
        let fx = &fx;
        async move { fx.web_event(t, &e).await }
    };

    assert_eq!(clock().started_at, None, "nothing before the first Start");
    assert_eq!(clock().ended_at, None);
    assert_eq!(clock().elapsed_ms, None);

    // From Select.
    event(0, run_dispatched(&["alpha"])).await;
    assert_eq!(clock().started_at, Some(fx.clock.wall_at(0)));
    // While running: no reset.
    event(2, run_dispatched(&["alpha"])).await;
    assert_eq!(clock().started_at, Some(fx.clock.wall_at(0)));
    // Park and fatal error leave it open.
    event(3, ExecutionEvent::PersistentSchedulerIdle).await;
    event(
        4,
        ExecutionEvent::Error {
            message: "boom".into(),
        },
    )
    .await;
    assert_eq!(clock().started_at, Some(fx.clock.wall_at(0)));
    assert_eq!(clock().ended_at, None);
    assert_eq!(clock().elapsed_ms, None);
    // From Error: opens.
    event(5, run_dispatched(&["alpha"])).await;
    assert_eq!(clock().started_at, Some(fx.clock.wall_at(5)));
    // Stopped freezes and the value is retained across reads.
    event(8, ExecutionEvent::Stopped).await;
    assert_eq!(clock().elapsed_ms, Some(3_000));
    assert_eq!(clock().ended_at, Some(fx.clock.wall_at(8)));
    fx.web.sync_remote_control_projection().await;
    assert_eq!(clock().elapsed_ms, Some(3_000));
    // From Stopped: opens and clears the frozen value.
    event(10, run_dispatched(&["alpha"])).await;
    assert_eq!(clock().started_at, Some(fx.clock.wall_at(10)));
    assert_eq!(clock().ended_at, None);
    assert_eq!(clock().elapsed_ms, None);
    // AllCompleted freezes.
    event(17, ExecutionEvent::AllCompleted).await;
    assert_eq!(clock().elapsed_ms, Some(7_000));
    // ResolveCompleted with no processing row while running freezes.
    event(20, run_dispatched(&["alpha"])).await;
    event(
        26,
        ExecutionEvent::ResolveCompleted {
            change_id: "alpha".into(),
            worktree_change_ids: None,
        },
    )
    .await;
    assert_eq!(clock().elapsed_ms, Some(6_000));
    assert_eq!(clock().ended_at, Some(fx.clock.wall_at(26)));
}

#[tokio::test]
async fn remote_progress_timing_run_clock_persistent_idle_readmission_keeps_start() {
    let mut fx = Fixture::new(&["alpha"]).await;
    fx.start(0, &["alpha"]).await;
    fx.command(ReducerCommand::AddToQueue("alpha".into())).await;

    fx.tui
        .handle_orchestrator_event(ExecutionEvent::PersistentSchedulerIdle);
    fx.web_event(3, &ExecutionEvent::PersistentSchedulerIdle)
        .await;
    assert_eq!(fx.snapshot().0.app_mode, "select");
    assert_eq!(fx.tui.execution_mode, AppExecutionMode::Select);
    fx.assert_run_clock_parity(Some(0), None);

    fx.tui.handle_orchestrator_event(apply_started("alpha"));
    fx.web_event(5, &apply_started("alpha")).await;
    assert_eq!(fx.snapshot().0.app_mode, "running");
    assert_eq!(fx.tui.execution_mode, AppExecutionMode::Running);
    fx.assert_run_clock_parity(Some(0), None);
}

#[tokio::test]
async fn remote_progress_timing_run_clock_resolve_reserved_keeps_start() {
    let mut fx = Fixture::new(&["alpha"]).await;
    fx.start(0, &["alpha"]).await;
    fx.stopped(4).await;
    fx.assert_run_clock_parity(Some(0), Some((4, 4)));

    let reserved = ExecutionEvent::OperatorCommandApplied {
        effect: OperatorCommandEffect::ResolveReserved {
            change_id: "alpha".into(),
            active: true,
        },
    };
    fx.tui.handle_orchestrator_event(reserved.clone());
    fx.web_event(9, &reserved).await;
    assert_eq!(fx.snapshot().0.app_mode, "running");
    assert_eq!(fx.tui.execution_mode, AppExecutionMode::Running);
    fx.assert_run_clock_parity(Some(0), Some((4, 4)));
}

#[tokio::test]
async fn remote_progress_timing_run_clock_repeated_terminal_refreezes_like_tui() {
    // Stopped, then Stopped again.
    let mut fx = Fixture::new(&["alpha"]).await;
    fx.start(0, &["alpha"]).await;
    fx.stopped(4).await;
    let (_, first) = fx.snapshot();
    fx.assert_run_clock_parity(Some(0), Some((4, 4)));
    fx.stopped(7).await;
    let (snapshot, second) = fx.snapshot();
    fx.assert_run_clock_parity(Some(0), Some((7, 7)));
    assert_eq!(second, first + 1, "one revision for the re-freeze");
    assert_eq!(snapshot.app_mode, "stopped");
    assert_eq!(fx.tui.execution_mode, AppExecutionMode::Stopped);

    // Stopped, then a delayed AllCompleted.
    let mut fx = Fixture::new(&["alpha"]).await;
    fx.start(0, &["alpha"]).await;
    fx.stopped(4).await;
    let (_, first) = fx.snapshot();
    fx.tui.handle_all_completed_at(fx.clock.at(11).0);
    fx.web_event(11, &ExecutionEvent::AllCompleted).await;
    let (snapshot, second) = fx.snapshot();
    fx.assert_run_clock_parity(Some(0), Some((11, 11)));
    assert_eq!(second, first + 1, "one revision for the re-freeze");
    assert_eq!(snapshot.app_mode, "stopped");
    assert_eq!(fx.tui.execution_mode, AppExecutionMode::Stopped);
}

/// Running, one resolver, one other change applying.
async fn resolve_fixture() -> Fixture {
    let mut fx = Fixture::new(&["alpha", "beta"]).await;
    fx.start(0, &["alpha", "beta"]).await;
    fx.command(ReducerCommand::AddToQueue("alpha".into())).await;
    fx.command(ReducerCommand::AddToQueue("beta".into())).await;
    for (t, event) in [
        (1, apply_started("alpha")),
        (2, ExecutionEvent::ChangeArchived("alpha".into())),
        (
            3,
            ExecutionEvent::MergeDeferred {
                change_id: "alpha".into(),
                reason: "base moved".into(),
                auto_resumable: false,
            },
        ),
        (4, apply_started("beta")),
    ] {
        fx.tui.handle_orchestrator_event(event.clone());
        fx.web_event(t, &event).await;
        fx.sync_tui(t).await;
    }
    fx.command(ReducerCommand::ResolveMerge("alpha".into()))
        .await;
    let started = ExecutionEvent::ResolveStarted {
        change_id: "alpha".into(),
        command: "resolve".into(),
    };
    fx.tui.handle_orchestrator_event(started.clone());
    fx.web_event(5, &started).await;
    fx.sync_tui(5).await;
    assert_eq!(fx.snapshot().0.app_mode, "running");
    assert_eq!(fx.tui.execution_mode, AppExecutionMode::Running);
    fx
}

/// The TUI's resolve-side settlement at an injected instant: the handler's own
/// row rewrite, then `try_transition_to_select_at` — what
/// `handle_resolve_completed` does with an empty resolve queue.
fn tui_resolve_settles_at(tui: &mut AppState, target: &str, rewrite: &str, now: Instant) {
    if let Some(row) = tui.changes.iter_mut().find(|row| row.id == target) {
        row.display_status_cache = rewrite.to_string();
    }
    tui.try_transition_to_select_at(now);
}

#[tokio::test]
async fn remote_progress_timing_run_clock_resolve_settlement_matches_tui() {
    // ResolveCompleted while another row is still applying: no freeze anywhere.
    let mut fx = resolve_fixture().await;
    let completed = ExecutionEvent::ResolveCompleted {
        change_id: "alpha".into(),
        worktree_change_ids: None,
    };
    fx.tui.handle_orchestrator_event(completed.clone());
    fx.web_event(8, &completed).await;
    fx.sync_tui(8).await;
    fx.assert_run_clock_parity(Some(0), None);

    // The other row becomes non-processing; a late ResolveFailed for the
    // already-merged row is a no-op in both frontends.
    let failed_b = ExecutionEvent::ProcessingError {
        id: "beta".into(),
        error: "boom".into(),
    };
    fx.tui.handle_orchestrator_event(failed_b.clone());
    fx.web_event(9, &failed_b).await;
    fx.sync_tui(9).await;
    let late = ExecutionEvent::ResolveFailed {
        change_id: "alpha".into(),
        error: "late".into(),
    };
    fx.tui.handle_orchestrator_event(late.clone());
    fx.web_event(11, &late).await;
    assert_eq!(fx.tui.execution_mode, AppExecutionMode::Running);
    fx.assert_run_clock_parity(Some(0), None);
    assert_eq!(fx.row("alpha").display_status, "merged");

    // ResolveCompleted with no other active row freezes both at one instant.
    let mut fx = resolve_fixture().await;
    let done_b = ExecutionEvent::ProcessingError {
        id: "beta".into(),
        error: "boom".into(),
    };
    fx.tui.handle_orchestrator_event(done_b.clone());
    fx.web_event(6, &done_b).await;
    fx.sync_tui(6).await;
    // Decision parity through the real TUI handler on a twin.
    let mut twin = resolve_fixture().await;
    twin.tui.handle_orchestrator_event(done_b.clone());
    twin.web_event(6, &done_b).await;
    twin.sync_tui(6).await;
    twin.tui.handle_orchestrator_event(completed.clone());
    assert!(
        twin.tui.orchestration_elapsed.is_some(),
        "the real handler freezes"
    );
    assert_eq!(twin.tui.execution_mode, AppExecutionMode::Select);
    // Numeric parity at the injected instant.
    tui_resolve_settles_at(&mut fx.tui, "alpha", "merged", fx.clock.at(12).0);
    fx.web_event(12, &completed).await;
    fx.assert_run_clock_parity(Some(0), Some((12, 12)));

    // MergeCompleted outside a resolve lifecycle freezes nothing on either side.
    let mut fx = Fixture::new(&["alpha"]).await;
    fx.start(0, &["alpha"]).await;
    let merged = ExecutionEvent::MergeCompleted {
        change_id: "alpha".into(),
        revision: "abc".into(),
    };
    assert!(!fx.tui.is_resolving() && !fx.tui.has_queued_resolves());
    fx.tui.handle_orchestrator_event(merged.clone());
    fx.web_event(5, &merged).await;
    assert!(!fx.snapshot().0.is_resolving);
    fx.assert_run_clock_parity(Some(0), None);
}

// ============================================================================
// Revision discipline, ChangeTiming, restart
// ============================================================================

#[tokio::test]
async fn remote_progress_timing_time_passing_does_not_advance_the_revision() {
    let fx = Fixture::new(&["alpha"]).await;
    fx.web_event(0, &run_dispatched(&["alpha"])).await;
    fx.command(ReducerCommand::AddToQueue("alpha".into())).await;
    fx.web_event(1, &apply_started("alpha")).await;
    let (first, first_revision) = fx.snapshot();
    assert!(first.changes[0]
        .net_active
        .as_ref()
        .unwrap()
        .active_since
        .is_some());

    // Two reads at later instants with no event in between.
    for later in [100, 5_000] {
        let (now, wall) = fx.clock.at(later);
        fx.web.pin_observation_clock_for_tests(now, wall);
        fx.web.sync_remote_control_projection().await;
        let (snapshot, revision) = fx.snapshot();
        assert_eq!(snapshot, first, "the stored snapshot is unchanged");
        assert_eq!(revision, first_revision);
    }

    // A duplicate identical status event does not move net_active.
    fx.web_event(200, &apply_started("alpha")).await;
    assert_eq!(fx.row("alpha").net_active, first.changes[0].net_active);
    assert_eq!(fx.row("alpha").timing, first.changes[0].timing);
}

#[tokio::test]
async fn remote_progress_timing_change_timing_is_unchanged() {
    let fx = Fixture::new(&["c1"]).await;
    fx.web_event(0, &ExecutionEvent::ProcessingStarted("c1".into()))
        .await;
    let timing = fx.row("c1").timing;
    assert_eq!(timing.started_at, Some(fx.clock.wall_at(0)));
    assert_eq!(timing.completed_at, None);
    assert_eq!(timing.elapsed_ms, None);

    // A later start only fills a gap; it does not restart the run.
    fx.web_event(
        3,
        &ExecutionEvent::AcceptanceStarted {
            change_id: "c1".into(),
            command: "accept".into(),
        },
    )
    .await;
    assert_eq!(fx.row("c1").timing.started_at, Some(fx.clock.wall_at(0)));

    fx.web_event(9, &ExecutionEvent::ChangeArchived("c1".into()))
        .await;
    let timing = fx.row("c1").timing;
    assert_eq!(timing.started_at, Some(fx.clock.wall_at(0)));
    assert_eq!(timing.completed_at, Some(fx.clock.wall_at(9)));
    assert_eq!(timing.elapsed_ms, Some(9_000), "per run, not cumulative");

    // `processing_started` discards the previous run's boundaries, unlike the
    // cumulative net-active clock.
    fx.web_event(20, &ExecutionEvent::ProcessingStarted("c1".into()))
        .await;
    let timing = fx.row("c1").timing;
    assert_eq!(timing.started_at, Some(fx.clock.wall_at(20)));
    assert_eq!(timing.completed_at, None);
    assert_eq!(timing.elapsed_ms, None);
}

#[tokio::test]
async fn remote_progress_timing_restart_starts_from_zero() {
    let fx = Fixture::new(&["alpha"]).await;
    fx.web_event(0, &run_dispatched(&["alpha"])).await;
    fx.command(ReducerCommand::AddToQueue("alpha".into())).await;
    fx.web_event(1, &apply_started("alpha")).await;
    fx.web_event(2, &ExecutionEvent::ChangeArchived("alpha".into()))
        .await;
    fx.web_event(3, &ExecutionEvent::Stopped).await;
    assert_eq!(fx.row("alpha").archive_complete, Some(true));
    assert!(fx.row("alpha").net_active.unwrap().has_history);

    // A new owner process over the same workspace: nothing was persisted.
    let restarted = Fixture::new(&["alpha"]).await;
    restarted.web.sync_remote_control_projection().await;
    let row = restarted.row("alpha");
    assert_eq!(row.archive_complete, Some(false));
    let net_active = row.net_active.unwrap();
    assert_eq!(net_active.accumulated_ms, 0);
    assert_eq!(net_active.active_since, None);
    assert!(!net_active.has_history);
    let run_clock = restarted.snapshot().0.run_clock.unwrap();
    assert_eq!(run_clock.started_at, None);
    assert_eq!(run_clock.ended_at, None);
    assert_eq!(run_clock.elapsed_ms, None);
}
