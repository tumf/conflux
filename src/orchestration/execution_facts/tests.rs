//! Unit tests for the process-local execution-facts store.
//!
//! Unit-scoped by construction: the reducer and the store are both in-memory
//! values, timestamps are injected, and nothing here touches Git, a process, a
//! filesystem, a socket, or a clock.

use super::*;

use chrono::TimeZone;

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000 + seconds, 0)
        .single()
        .expect("fixed instant")
}

fn state_with(change_ids: &[&str]) -> OrchestratorState {
    OrchestratorState::new(change_ids.iter().map(|id| (*id).to_string()).collect(), 0)
}

fn apply_started(change_id: &str) -> ExecutionEvent {
    ExecutionEvent::ApplyStarted {
        change_id: change_id.to_string(),
        command: "agent apply".to_string(),
    }
}

fn acceptance_started(change_id: &str) -> ExecutionEvent {
    ExecutionEvent::AcceptanceStarted {
        change_id: change_id.to_string(),
        command: "agent accept".to_string(),
    }
}

fn apply_completed(change_id: &str, revision: &str) -> ExecutionEvent {
    ExecutionEvent::ApplyCompleted {
        change_id: change_id.to_string(),
        revision: revision.to_string(),
    }
}

/// Drive one event through the reducer and the store the way the authoritative
/// dispatch boundary does: reducer first, store second, over the same instant.
///
/// The dispatch identity is derived from the instant so each test event carries
/// a distinct one without a clock or a global counter.
fn dispatch(
    store: &ExecutionFactsStore,
    state: &mut OrchestratorState,
    event: ExecutionEvent,
    now: DateTime<Utc>,
) {
    state.apply_execution_event(&event);
    store.observe(now.timestamp() as u64, &event, Some(state), now);
}

#[test]
fn agent_execution_observability_phase_apply_to_acceptance_records_completed_apply() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha"]);

    dispatch(&store, &mut state, apply_started("alpha"), at(0));
    assert_eq!(store.change("alpha").current_phase, ExecutionPhase::Apply);
    assert_eq!(store.change("alpha").phase_started_at, Some(at(0)));
    assert_eq!(store.change("alpha").last_completed_phase, None);

    dispatch(
        &store,
        &mut state,
        apply_completed("alpha", "abc123"),
        at(5),
    );
    dispatch(&store, &mut state, acceptance_started("alpha"), at(6));

    let facts = store.change("alpha");
    assert_eq!(facts.current_phase, ExecutionPhase::Acceptance);
    assert_eq!(facts.phase_started_at, Some(at(6)));
    assert_eq!(facts.last_completed_phase, Some(ExecutionPhase::Apply));
    assert_eq!(facts.last_completed_at, Some(at(5)));
    assert_eq!(facts.apply_commit_oid.as_deref(), Some("abc123"));
    assert_eq!(facts.execution_state, ChangeExecutionState::Active);
}

/// A phase the reducer left without publishing its typed completion is not a
/// completed phase. This is the difference between "Apply finished" and "Apply
/// was abandoned", and a store that watched only the reducer would lose it.
#[test]
fn agent_execution_observability_phase_failure_does_not_complete_apply() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha"]);

    dispatch(&store, &mut state, apply_started("alpha"), at(0));
    dispatch(
        &store,
        &mut state,
        ExecutionEvent::ApplyFailed {
            change_id: "alpha".to_string(),
            error: "boom".to_string(),
        },
        at(1),
    );

    let facts = store.change("alpha");
    assert_eq!(facts.last_completed_phase, None);
    assert_ne!(facts.current_phase, ExecutionPhase::Apply);
}

/// An empty `ApplyCompleted.revision` is not evidence and is never retained.
#[test]
fn agent_execution_observability_apply_commit_empty_revision_is_not_retained() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha"]);

    dispatch(&store, &mut state, apply_started("alpha"), at(0));
    dispatch(&store, &mut state, apply_completed("alpha", "   "), at(1));

    assert_eq!(store.apply_commit_oid("alpha"), None);
    assert_eq!(
        store.change("alpha").last_completed_phase,
        Some(ExecutionPhase::Apply),
        "the completion itself is still a typed fact"
    );
}

/// A fresh store is a fresh incarnation: nothing survives a restart.
#[test]
fn agent_execution_observability_apply_commit_restart_leaves_no_fact() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha"]);
    dispatch(
        &store,
        &mut state,
        apply_completed("alpha", "abc123"),
        at(0),
    );
    assert_eq!(store.apply_commit_oid("alpha").as_deref(), Some("abc123"));

    let restarted = ExecutionFactsStore::new();
    assert_eq!(restarted.apply_commit_oid("alpha"), None);
    assert!(!restarted.snapshot().has_active_work());
}

/// The typed push episode is the only phase the reducer does not own.
#[test]
fn agent_execution_observability_phase_push_opens_and_closes() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha"]);

    dispatch(
        &store,
        &mut state,
        ExecutionEvent::PushStarted {
            change_id: "alpha".to_string(),
            remote: "origin".to_string(),
            branch: "alpha".to_string(),
        },
        at(0),
    );
    assert_eq!(store.change("alpha").current_phase, ExecutionPhase::Push);
    assert!(store.snapshot().has_active_work());

    dispatch(
        &store,
        &mut state,
        ExecutionEvent::PushCompleted {
            change_id: "alpha".to_string(),
            remote: "origin".to_string(),
            branch: "alpha".to_string(),
        },
        at(1),
    );
    let facts = store.change("alpha");
    assert_eq!(facts.current_phase, ExecutionPhase::None);
    assert_eq!(facts.last_completed_phase, Some(ExecutionPhase::Push));
}

/// Every process-level episode opens on its start event and closes on its
/// terminal, and only an open episode counts as active work.
#[test]
fn agent_execution_observability_phase_process_activities_open_and_close() {
    let cases: Vec<(ExecutionEvent, ExecutionEvent, ProcessActivity)> = vec![
        (
            ExecutionEvent::AnalysisStarted {
                remaining_changes: 2,
                attempt_id: "attempt-1".to_string(),
            },
            ExecutionEvent::AnalysisCompleted { groups_found: 1 },
            ProcessActivity::DependencyAnalysis,
        ),
        (
            ExecutionEvent::MergeStarted {
                revisions: vec!["r1".to_string()],
            },
            ExecutionEvent::MergeCompleted {
                change_id: "alpha".to_string(),
                revision: "r1".to_string(),
            },
            ProcessActivity::BaseBranchMerge,
        ),
        (
            ExecutionEvent::ConflictResolutionStarted,
            ExecutionEvent::ConflictResolutionCompleted,
            ProcessActivity::ConflictResolution,
        ),
        (
            ExecutionEvent::BranchMergeStarted {
                branch_name: "alpha".to_string(),
            },
            ExecutionEvent::BranchMergeCompleted {
                branch_name: "alpha".to_string(),
            },
            ProcessActivity::BranchMerge,
        ),
        (
            ExecutionEvent::CleanupStarted {
                workspace: "ws".to_string(),
            },
            ExecutionEvent::CleanupCompleted {
                workspace: "ws".to_string(),
            },
            ProcessActivity::WorkspaceCleanup,
        ),
    ];

    for (start, terminal, activity) in cases {
        let store = ExecutionFactsStore::new();
        store.observe(0, &start, None, at(0));
        assert_eq!(
            store.snapshot().activities,
            vec![activity],
            "{} must open on its typed start event",
            activity.as_str()
        );
        assert!(store.snapshot().has_active_work());

        store.observe(1, &terminal, None, at(1));
        assert!(
            store.snapshot().activities.is_empty(),
            "{} must close on its typed terminal event",
            activity.as_str()
        );
        assert!(!store.snapshot().has_active_work());
    }
}

/// A process terminal closes every open episode, so a lane that died with the
/// process cannot leave the API reporting work forever.
#[test]
fn agent_execution_observability_phase_process_terminal_closes_activities() {
    for terminal in [
        ExecutionEvent::Stopped,
        ExecutionEvent::AllCompleted,
        ExecutionEvent::Error {
            message: "fatal".to_string(),
        },
    ] {
        let store = ExecutionFactsStore::new();
        store.observe(0, &ExecutionEvent::ConflictResolutionStarted, None, at(0));
        store.observe(1, &terminal, None, at(1));
        assert!(store.snapshot().activities.is_empty());
        assert!(!store.snapshot().has_active_work());
    }
}

/// A parked persistent scheduler is alive without any admitted work.
#[test]
fn agent_execution_observability_phase_persistent_idle_is_not_active_work() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha"]);
    dispatch(
        &store,
        &mut state,
        ExecutionEvent::PersistentSchedulerIdle,
        at(0),
    );

    let snapshot = store.snapshot();
    assert!(!snapshot.has_active_work());
    assert_eq!(snapshot.change("alpha").current_phase, ExecutionPhase::None);
}

/// A graceful stop over an active change is `stopping`, not `active`.
#[test]
fn agent_execution_observability_phase_graceful_stop_marks_stopping() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha"]);

    dispatch(&store, &mut state, apply_started("alpha"), at(0));
    assert_eq!(
        store.change("alpha").execution_state,
        ChangeExecutionState::Active
    );

    dispatch(&store, &mut state, ExecutionEvent::Stopping, at(1));
    assert_eq!(
        store.change("alpha").execution_state,
        ChangeExecutionState::Stopping
    );
    assert_eq!(
        store.change("alpha").current_phase,
        ExecutionPhase::Apply,
        "a pending stop does not end the phase that is still running"
    );
}

/// Terminal reducer outcomes classify ahead of activity and waits.
#[test]
fn agent_execution_observability_phase_terminal_states_classify_first() {
    let cases = [
        (TerminalState::Merged, ChangeExecutionState::Completed),
        (TerminalState::Pushed, ChangeExecutionState::Completed),
        (
            TerminalState::Error("x".to_string()),
            ChangeExecutionState::Failed,
        ),
        (
            TerminalState::Rejected("x".to_string()),
            ChangeExecutionState::Failed,
        ),
        (TerminalState::Stopped, ChangeExecutionState::Stopped),
    ];
    for (terminal, expected) in cases {
        let runtime = ChangeRuntimeState {
            terminal: terminal.clone(),
            activity: ActivityState::Applying,
            ..ChangeRuntimeState::default()
        };
        assert_eq!(
            project_execution_state(&runtime, ExecutionPhase::Apply, false),
            expected,
            "{terminal:?} must classify ahead of the active phase"
        );
    }
}

/// A tracked row with no queue intent, no wait, no activity, and no terminal is
/// explicitly unknown rather than mapped onto a state the reducer never claimed.
#[test]
fn agent_execution_observability_phase_unclassified_row_is_unknown() {
    let runtime = ChangeRuntimeState::default();
    assert_eq!(
        project_execution_state(&runtime, ExecutionPhase::None, false),
        ChangeExecutionState::Unknown
    );

    let queued = ChangeRuntimeState {
        queue_intent: QueueIntent::Queued,
        ..ChangeRuntimeState::default()
    };
    assert_eq!(
        project_execution_state(&queued, ExecutionPhase::None, false),
        ChangeExecutionState::Queued
    );

    let waiting = ChangeRuntimeState {
        wait_state: WaitState::MergeWait,
        ..ChangeRuntimeState::default()
    };
    assert_eq!(
        project_execution_state(&waiting, ExecutionPhase::None, false),
        ChangeExecutionState::Waiting
    );
}

/// Every reducer activity has exactly one phase projection, and `Idle` is the
/// only one that depends on the typed push episode.
#[test]
fn agent_execution_observability_phase_projects_every_reducer_activity() {
    let cases = [
        (ActivityState::Preparing, ExecutionPhase::Preparing),
        (ActivityState::Applying, ExecutionPhase::Apply),
        (ActivityState::Accepting, ExecutionPhase::Acceptance),
        (ActivityState::Rejecting, ExecutionPhase::RejectionReview),
        (ActivityState::Archiving, ExecutionPhase::Archive),
        (ActivityState::Resolving, ExecutionPhase::Resolve),
        (ActivityState::Idle, ExecutionPhase::None),
    ];
    for (activity, expected) in cases {
        let runtime = ChangeRuntimeState {
            activity: activity.clone(),
            ..ChangeRuntimeState::default()
        };
        assert_eq!(project_phase(&runtime, false), expected);
    }

    // Publication reuses the reducer's `Resolving` activity, so the typed push
    // episode is what distinguishes the two — and only over those activities.
    let idle = ChangeRuntimeState::default();
    assert_eq!(project_phase(&idle, true), ExecutionPhase::Push);
    let resolving = ChangeRuntimeState {
        activity: ActivityState::Resolving,
        ..ChangeRuntimeState::default()
    };
    assert_eq!(project_phase(&resolving, true), ExecutionPhase::Push);
    let applying = ChangeRuntimeState {
        activity: ActivityState::Applying,
        ..ChangeRuntimeState::default()
    };
    assert_eq!(
        project_phase(&applying, true),
        ExecutionPhase::Apply,
        "a newer reducer transition wins over a stale push episode"
    );
}

/// One typed transition refreshes every tracked row, not only its own target.
#[test]
fn agent_execution_observability_phase_refresh_covers_untargeted_changes() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha", "beta"]);

    dispatch(&store, &mut state, apply_started("alpha"), at(0));
    dispatch(&store, &mut state, apply_started("beta"), at(1));
    // A transition addressed at `alpha` alone still re-reads `beta`.
    dispatch(&store, &mut state, apply_completed("alpha", "oid"), at(2));

    let snapshot = store.snapshot();
    assert_eq!(snapshot.change("beta").current_phase, ExecutionPhase::Apply);
    assert!(snapshot.has_active_work());
}

/// A process with both an authoritative dispatch owner and a web projection
/// sink delivers one dispatch to the store twice. The second delivery must not
/// restamp a completion boundary with a later instant.
#[test]
fn agent_execution_observability_phase_repeated_dispatch_is_absorbed_once() {
    let store = ExecutionFactsStore::new();
    let mut state = state_with(&["alpha"]);
    let event = apply_completed("alpha", "abc123");
    state.apply_execution_event(&event);

    assert!(store.observe(7, &event, Some(&state), at(0)));
    assert!(
        !store.observe(7, &event, Some(&state), at(99)),
        "the same dispatch identity is absorbed exactly once"
    );
    assert_eq!(store.change("alpha").last_completed_at, Some(at(0)));
}

/// Wire tokens are contract, so they are asserted rather than derived.
#[test]
fn agent_execution_observability_phase_wire_tokens_are_stable() {
    assert_eq!(ExecutionPhase::Preparing.as_str(), "preparing");
    assert_eq!(ExecutionPhase::Apply.as_str(), "apply");
    assert_eq!(ExecutionPhase::Acceptance.as_str(), "acceptance");
    assert_eq!(ExecutionPhase::RejectionReview.as_str(), "rejection_review");
    assert_eq!(ExecutionPhase::Archive.as_str(), "archive");
    assert_eq!(ExecutionPhase::Resolve.as_str(), "resolve");
    assert_eq!(ExecutionPhase::Push.as_str(), "push");
    assert_eq!(ExecutionPhase::Merge.as_str(), "merge");
    assert_eq!(ExecutionPhase::None.as_str(), "none");
    assert_eq!(ExecutionPhase::Unknown.as_str(), "unknown");

    assert_eq!(ChangeExecutionState::Queued.as_str(), "queued");
    assert_eq!(ChangeExecutionState::Active.as_str(), "active");
    assert_eq!(ChangeExecutionState::Waiting.as_str(), "waiting");
    assert_eq!(ChangeExecutionState::Stopping.as_str(), "stopping");
    assert_eq!(ChangeExecutionState::Stopped.as_str(), "stopped");
    assert_eq!(ChangeExecutionState::Failed.as_str(), "failed");
    assert_eq!(ChangeExecutionState::Completed.as_str(), "completed");
    assert_eq!(ChangeExecutionState::Unknown.as_str(), "unknown");
}
