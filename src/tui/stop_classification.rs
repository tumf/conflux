//! Truthful classification of an immediate (second-Esc / `ForceStop`) stop.
//!
//! `AppMode::Stopping` describes TUI lifecycle only. It says nothing about
//! whether an agent process is running, so it must never be used on its own to
//! claim that a process was force-terminated.
//!
//! This module takes one runtime activity snapshot and derives two orthogonal
//! decisions from it:
//!
//! - [`ProcessReport`]: whether registered or reducer-visible in-flight agent
//!   execution justifies force-stop reporting and managed child-process
//!   termination.
//! - [`ShutdownBarrier`]: whether active execution, a pending background merge,
//!   or a base-lane mutation must reach its existing cancellation-safe boundary
//!   before terminal stop is established.
//!
//! Both reporting classes use the same global cancellation mechanism. The
//! classifier controls reporting and waiting, never whether cleanup runs.
//!
//! The snapshot is runtime-only: it derives from in-memory reducer state and the
//! in-memory execution-handle registry, and is never persisted as durable
//! workflow evidence.

use std::sync::Arc;

use crate::orchestration::state::{ExecutionMode, OrchestratorState};
use crate::tui::queue::DynamicQueue;

/// Evidence about registered per-change execution handles.
///
/// A registered handle exists exactly while a workspace task (apply/accept/
/// archive, i.e. an agent command) owns a change, so a known-empty registry is
/// positive evidence that no agent process is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionEvidence {
    /// The registry was inspected successfully and holds `registered` handles.
    Known { registered: usize },
    /// The registry could not be inspected (no registry bound, or a non-parallel
    /// execution mode whose processes are not registered here).
    Unavailable,
}

/// Evidence about scheduler-owned shutdown work that must drain safely.
///
/// Background post-archive merges and base-lane mutations are shutdown work.
/// They are never evidence that an agent process was force-stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownWorkEvidence {
    /// Shutdown work was inspected successfully.
    Known { pending: bool },
    /// Shutdown work could not be inspected.
    Unavailable,
}

/// One shared runtime activity snapshot for an immediate stop decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopActivitySnapshot {
    /// Registered per-change execution handles.
    pub execution_handles: ExecutionEvidence,
    /// Reducer-visible agent execution (applying / accepting / archiving).
    pub reducer_agent_execution_active: bool,
    /// Pending background merge or base-lane mutation owned by the scheduler.
    pub shutdown_work: ShutdownWorkEvidence,
}

/// How the completed stop may be reported to the operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessReport {
    /// An agent process/in-flight execution was active: force-stop reporting and
    /// managed child-process termination apply.
    ForceStopped,
    /// No agent process was active: the scheduler/orchestrator is cancelled
    /// without claiming process termination.
    OrdinaryStop,
}

impl ProcessReport {
    /// True when force-stop reporting is truthful for this stop.
    pub fn is_force_stop(self) -> bool {
        matches!(self, ProcessReport::ForceStopped)
    }
}

/// Whether terminal stop must wait for a scheduler cleanup boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownBarrier {
    /// Cleanup work must reach its safe boundary before terminal stop.
    Required,
    /// Nothing is in flight; terminal stop may be established immediately.
    NotRequired,
}

impl ShutdownBarrier {
    /// True when the caller must wait for the cleanup boundary.
    pub fn is_required(self) -> bool {
        matches!(self, ShutdownBarrier::Required)
    }
}

/// The two orthogonal decisions derived from one [`StopActivitySnapshot`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopClassification {
    /// Truthful process-termination reporting class.
    pub process_report: ProcessReport,
    /// Required shutdown waiting class.
    pub shutdown_barrier: ShutdownBarrier,
}

impl StopActivitySnapshot {
    /// True when the parallel scheduler owns cancellation cleanup for this run.
    ///
    /// Only the parallel scheduler registers execution handles and reaches a
    /// bounded cancellation cleanup barrier, so only it may own the terminal stop
    /// transition. Other execution modes keep their existing stop semantics.
    pub fn scheduler_owns_cleanup(&self) -> bool {
        matches!(self.execution_handles, ExecutionEvidence::Known { .. })
    }

    /// Derive the reporting and waiting decisions for this snapshot.
    ///
    /// Fail-safe rules:
    /// - Any positive execution signal, or unavailable execution evidence,
    ///   selects force-stop reporting and managed active cleanup.
    /// - Only a known-empty execution set permits ordinary stop reporting.
    /// - Any positive or unavailable shutdown-work signal keeps the shutdown
    ///   barrier active.
    pub fn classify(&self) -> StopClassification {
        let execution_active = match self.execution_handles {
            ExecutionEvidence::Known { registered } => registered > 0,
            // Unavailable evidence must never be read as "no process".
            ExecutionEvidence::Unavailable => true,
        } || self.reducer_agent_execution_active;

        let shutdown_work_pending = match self.shutdown_work {
            ShutdownWorkEvidence::Known { pending } => pending,
            ShutdownWorkEvidence::Unavailable => true,
        };

        StopClassification {
            process_report: if execution_active {
                ProcessReport::ForceStopped
            } else {
                ProcessReport::OrdinaryStop
            },
            shutdown_barrier: if execution_active || shutdown_work_pending {
                ShutdownBarrier::Required
            } else {
                ShutdownBarrier::NotRequired
            },
        }
    }
}

/// How long the stop path waits for reducer state before treating it as
/// unavailable evidence.
///
/// The immediate-stop path must stay responsive, so a reducer that is held by a
/// concurrent writer is reported as unavailable rather than waited on
/// indefinitely. Unavailable evidence fails safe.
const REDUCER_SNAPSHOT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(200);

/// Collect the shared runtime stop snapshot.
///
/// Both the key-driven and the command-driven stop path consume this single
/// function so the two entrypoints cannot drift apart.
pub async fn collect_stop_activity_snapshot(
    dynamic_queue: &DynamicQueue,
    shared_state: &Arc<tokio::sync::RwLock<OrchestratorState>>,
) -> StopActivitySnapshot {
    collect_stop_activity_snapshot_within(dynamic_queue, shared_state, REDUCER_SNAPSHOT_TIMEOUT)
        .await
}

/// [`collect_stop_activity_snapshot`] with an explicit reducer read deadline.
///
/// Only parallel mode registers execution handles in [`DynamicQueue`]; any other
/// execution mode reports [`ExecutionEvidence::Unavailable`] so its existing
/// force-stop behavior is preserved unchanged. A reducer read that does not
/// complete within `reducer_timeout` reports both execution and shutdown-work
/// evidence as unavailable.
pub(crate) async fn collect_stop_activity_snapshot_within(
    dynamic_queue: &DynamicQueue,
    shared_state: &Arc<tokio::sync::RwLock<OrchestratorState>>,
    reducer_timeout: std::time::Duration,
) -> StopActivitySnapshot {
    let Ok(state) = tokio::time::timeout(reducer_timeout, shared_state.read()).await else {
        // Unreadable reducer state is not evidence of an idle scheduler.
        return StopActivitySnapshot {
            execution_handles: ExecutionEvidence::Unavailable,
            reducer_agent_execution_active: false,
            shutdown_work: ShutdownWorkEvidence::Unavailable,
        };
    };
    let parallel = matches!(state.execution_mode(), ExecutionMode::Parallel);
    let reducer_agent_execution_active = state.is_agent_execution_active();
    let shutdown_work = ShutdownWorkEvidence::Known {
        pending: state.is_base_mutating_lane_occupied(),
    };
    drop(state);

    let execution_handles = if parallel {
        ExecutionEvidence::Known {
            registered: dynamic_queue.registered_execution_count().await,
        }
    } else {
        ExecutionEvidence::Unavailable
    };

    StopActivitySnapshot {
        execution_handles,
        reducer_agent_execution_active,
        shutdown_work,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::state::OrchestratorState;
    use tokio_util::sync::CancellationToken;

    fn snapshot(
        execution_handles: ExecutionEvidence,
        reducer_agent_execution_active: bool,
        shutdown_work: ShutdownWorkEvidence,
    ) -> StopActivitySnapshot {
        StopActivitySnapshot {
            execution_handles,
            reducer_agent_execution_active,
            shutdown_work,
        }
    }

    #[test]
    fn idle_parallel_stop_registered_handle_selects_force_stop_and_barrier() {
        let classification = snapshot(
            ExecutionEvidence::Known { registered: 1 },
            false,
            ShutdownWorkEvidence::Known { pending: false },
        )
        .classify();

        assert_eq!(classification.process_report, ProcessReport::ForceStopped);
        assert_eq!(classification.shutdown_barrier, ShutdownBarrier::Required);
    }

    #[test]
    fn idle_parallel_stop_reducer_activity_selects_force_stop_without_handles() {
        let classification = snapshot(
            ExecutionEvidence::Known { registered: 0 },
            true,
            ShutdownWorkEvidence::Known { pending: false },
        )
        .classify();

        assert_eq!(classification.process_report, ProcessReport::ForceStopped);
        assert_eq!(classification.shutdown_barrier, ShutdownBarrier::Required);
    }

    #[test]
    fn idle_parallel_stop_empty_execution_set_selects_ordinary_stop() {
        let classification = snapshot(
            ExecutionEvidence::Known { registered: 0 },
            false,
            ShutdownWorkEvidence::Known { pending: false },
        )
        .classify();

        assert_eq!(classification.process_report, ProcessReport::OrdinaryStop);
        assert_eq!(
            classification.shutdown_barrier,
            ShutdownBarrier::NotRequired
        );
    }

    #[test]
    fn idle_parallel_stop_pending_merge_keeps_barrier_without_force_stop() {
        let classification = snapshot(
            ExecutionEvidence::Known { registered: 0 },
            false,
            ShutdownWorkEvidence::Known { pending: true },
        )
        .classify();

        assert_eq!(classification.process_report, ProcessReport::OrdinaryStop);
        assert_eq!(classification.shutdown_barrier, ShutdownBarrier::Required);
    }

    #[test]
    fn idle_parallel_stop_unavailable_execution_evidence_fails_safe() {
        let classification = snapshot(
            ExecutionEvidence::Unavailable,
            false,
            ShutdownWorkEvidence::Known { pending: false },
        )
        .classify();

        assert_eq!(classification.process_report, ProcessReport::ForceStopped);
        assert_eq!(classification.shutdown_barrier, ShutdownBarrier::Required);
    }

    #[test]
    fn idle_parallel_stop_unavailable_shutdown_evidence_keeps_barrier() {
        let classification = snapshot(
            ExecutionEvidence::Known { registered: 0 },
            false,
            ShutdownWorkEvidence::Unavailable,
        )
        .classify();

        assert_eq!(classification.process_report, ProcessReport::OrdinaryStop);
        assert_eq!(classification.shutdown_barrier, ShutdownBarrier::Required);
    }

    fn parallel_state(change_ids: Vec<String>) -> Arc<tokio::sync::RwLock<OrchestratorState>> {
        let mut state = OrchestratorState::new(change_ids, 5);
        state.set_execution_mode(ExecutionMode::Parallel);
        Arc::new(tokio::sync::RwLock::new(state))
    }

    #[tokio::test]
    async fn idle_parallel_stop_snapshot_reports_registered_execution_handles() {
        let queue = DynamicQueue::new();
        queue
            .register_kill_token("change-a".to_string(), CancellationToken::new())
            .await;
        let state = parallel_state(vec!["change-a".to_string()]);

        let snapshot = collect_stop_activity_snapshot(&queue, &state).await;

        assert_eq!(
            snapshot.execution_handles,
            ExecutionEvidence::Known { registered: 1 }
        );
        assert_eq!(
            snapshot.classify().process_report,
            ProcessReport::ForceStopped
        );
    }

    #[tokio::test]
    async fn idle_parallel_stop_snapshot_reports_merge_wait_as_ordinary_stop() {
        let queue = DynamicQueue::new();
        let state = parallel_state(vec!["change-a".to_string()]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::MergeDeferred {
                change_id: "change-a".to_string(),
                reason: "manual".to_string(),
                auto_resumable: false,
            });
        }

        let snapshot = collect_stop_activity_snapshot(&queue, &state).await;

        assert_eq!(
            snapshot.execution_handles,
            ExecutionEvidence::Known { registered: 0 }
        );
        assert!(!snapshot.reducer_agent_execution_active);
        assert_eq!(
            snapshot.shutdown_work,
            ShutdownWorkEvidence::Known { pending: false }
        );
        let classification = snapshot.classify();
        assert_eq!(classification.process_report, ProcessReport::OrdinaryStop);
        assert_eq!(
            classification.shutdown_barrier,
            ShutdownBarrier::NotRequired
        );
    }

    #[tokio::test]
    async fn idle_parallel_stop_snapshot_reports_background_merge_as_shutdown_work() {
        let queue = DynamicQueue::new();
        let state = parallel_state(vec!["change-a".to_string()]);
        {
            let mut guard = state.write().await;
            guard.apply_execution_event(&crate::events::ExecutionEvent::ResolveStarted {
                change_id: "change-a".to_string(),
                command: "merge".to_string(),
            });
        }

        let snapshot = collect_stop_activity_snapshot(&queue, &state).await;

        assert!(!snapshot.reducer_agent_execution_active);
        assert_eq!(
            snapshot.shutdown_work,
            ShutdownWorkEvidence::Known { pending: true }
        );
        let classification = snapshot.classify();
        assert_eq!(classification.process_report, ProcessReport::OrdinaryStop);
        assert_eq!(classification.shutdown_barrier, ShutdownBarrier::Required);
    }

    #[tokio::test]
    async fn idle_parallel_stop_snapshot_fails_safe_when_reducer_state_is_unreadable() {
        let queue = DynamicQueue::new();
        let state = parallel_state(vec!["change-a".to_string()]);
        let blocker = state.clone();
        let held = blocker.write_owned().await;

        let snapshot = collect_stop_activity_snapshot_within(
            &queue,
            &state,
            std::time::Duration::from_millis(5),
        )
        .await;
        drop(held);

        assert_eq!(snapshot.execution_handles, ExecutionEvidence::Unavailable);
        assert_eq!(snapshot.shutdown_work, ShutdownWorkEvidence::Unavailable);
        let classification = snapshot.classify();
        assert_eq!(classification.process_report, ProcessReport::ForceStopped);
        assert_eq!(classification.shutdown_barrier, ShutdownBarrier::Required);
    }

    #[tokio::test]
    async fn idle_parallel_stop_snapshot_treats_serial_mode_as_unavailable_evidence() {
        let queue = DynamicQueue::new();
        let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
            vec!["change-a".to_string()],
            5,
        )));

        let snapshot = collect_stop_activity_snapshot(&queue, &state).await;

        assert_eq!(snapshot.execution_handles, ExecutionEvidence::Unavailable);
        assert_eq!(
            snapshot.classify().process_report,
            ProcessReport::ForceStopped
        );
    }
}
