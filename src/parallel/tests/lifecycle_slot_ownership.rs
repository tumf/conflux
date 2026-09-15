//! One admitted change owns one concurrency slot, continuously.
//!
//! The defect these tests pin down is a lifecycle-ownership defect, not an
//! atomic-counter ordering one: the scheduler used to release a change's permit
//! when its apply/acceptance/archive task returned and to drop it from
//! capacity accounting before the independently spawned background merge
//! settled. A queued change could then take the apparent free slot while the
//! previous change was still merging or resolving, so a configured limit of
//! three could present three applying changes plus one resolving change.
//!
//! Every sequence below therefore asserts *occupancy at each observable state*
//! rather than only the end state. The archive-to-merge case in particular
//! fails on the old accounting, where the returning task's release made
//! occupancy drop to two under a limit of three.
//!
//! Evidence class: unit. Slot ownership, its transitions, and the reducer
//! evidence reconciliation runs on are exercised through in-memory doubles —
//! the slot owner itself and an in-process `OrchestratorState` — with no Git,
//! process, network, or real workspace boundary, and no timing sleeps: the
//! states other tests would hold open with a barrier are constructed directly
//! from the same transitions production applies.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use crate::events::ExecutionEvent;
use crate::orchestration::operator_command::is_active_status;
use crate::orchestration::state::{OrchestratorState, ReducerCommand};
use crate::parallel::lifecycle_slots::{
    merge_result_transition, workspace_completion_transition, LifecycleSlots, SlotPhase,
    SlotTransition,
};
use crate::parallel::ParallelExecutor;

use super::support::create_test_config;

/// The configured limit every sequence below runs under.
const MAX: usize = 3;

fn ids(values: &[&str]) -> HashSet<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

/// Three admitted changes, each owning its one slot from workspace admission.
async fn three_admitted() -> LifecycleSlots {
    let mut slots = LifecycleSlots::new(MAX);
    for change_id in ["alpha", "beta", "gamma"] {
        assert!(
            slots.occupy_now(change_id, SlotPhase::Workspace).await,
            "admission takes exactly one slot per change"
        );
    }
    assert_eq!(slots.len(), MAX);
    assert_eq!(slots.available(MAX, &ids(&[])), 0);
    slots
}

/// An executor whose only job here is to own slots and read reducer evidence.
///
/// The repository path is never touched: none of the methods under test run a
/// VCS command, create a workspace, or read the filesystem.
fn evidence_executor(state: &Arc<tokio::sync::RwLock<OrchestratorState>>) -> ParallelExecutor {
    let mut executor = ParallelExecutor::new(
        PathBuf::from("/lifecycle-slot-ownership-unit-test"),
        create_test_config(),
        None,
    );
    executor.set_shared_orchestrator_state(state.clone());
    executor
}

#[tokio::test]
async fn archive_completion_transfers_the_slot_to_background_merge() {
    // The regression. `alpha` finishes archive and hands off to a background
    // merge; `beta` and `gamma` are still applying. Under the old accounting
    // the returning task released the permit and `handle_workspace_completion`
    // removed `alpha` from capacity membership, leaving one apparently free
    // slot for a queued change. Occupancy must stay at three.
    let mut slots = three_admitted().await;
    let in_flight = ids(&["beta", "gamma"]);

    let released = slots.apply_transition(
        "alpha",
        workspace_completion_transition(false, false, /* has_final_revision */ true),
    );

    assert!(!released, "an archived change is still admitted");
    assert_eq!(
        slots.phase("alpha"),
        Some(SlotPhase::Merge),
        "the same slot moves to background merge instead of being dropped"
    );
    assert_eq!(
        slots.len(),
        MAX,
        "admitted lifecycles: two applying plus one merging"
    );
    assert_eq!(
        slots.available(MAX, &in_flight),
        0,
        "a queued change must not be dispatched into a slot that is still owned"
    );
}

#[tokio::test]
async fn phase_transitions_never_leave_the_change_unowned_or_double_counted() {
    // Every observable state of one change's whole lifecycle: admitted,
    // merging, deferred into a wait, and settled. The change ID appears in
    // occupancy exactly once until settlement, and exactly zero times after.
    let mut slots = LifecycleSlots::new(MAX);
    slots.occupy_now("alpha", SlotPhase::Workspace).await;
    let occupancy = |slots: &LifecycleSlots| {
        slots
            .members()
            .iter()
            .filter(|change_id| change_id.as_str() == "alpha")
            .count()
    };

    assert_eq!(occupancy(&slots), 1, "admitted");
    slots.apply_transition("alpha", workspace_completion_transition(false, false, true));
    assert_eq!(occupancy(&slots), 1, "archived, merging");
    slots.apply_transition(
        "alpha",
        merge_result_transition(/* merged */ false, /* run_fatal */ false),
    );
    assert_eq!(occupancy(&slots), 1, "merge deferred, retained");
    assert!(
        slots.apply_transition("alpha", merge_result_transition(true, false)),
        "the merge that settles the lifecycle releases the slot"
    );
    assert_eq!(occupancy(&slots), 0, "settled");
    assert_eq!(slots.available(MAX, &ids(&[])), MAX);
}

#[tokio::test]
async fn terminal_workspace_outcomes_release_the_slot() {
    // A workspace task that ends the admitted lifecycle — a terminal error, a
    // rejection, or a completion with nothing left to integrate — releases,
    // because no later owner exists to hold the slot for.
    for (errored, rejected, has_revision, label) in [
        (true, false, false, "terminal error"),
        (false, true, false, "rejected"),
        (false, false, false, "nothing to merge"),
    ] {
        let mut slots = three_admitted().await;
        assert!(
            slots.apply_transition(
                "alpha",
                workspace_completion_transition(errored, rejected, has_revision),
            ),
            "{label} ends the admitted lifecycle"
        );
        assert_eq!(slots.len(), MAX - 1, "{label} returns exactly one slot");
        assert_eq!(
            slots.available(MAX, &ids(&["beta", "gamma"])),
            1,
            "{label} makes exactly one slot available"
        );
    }
}

#[tokio::test]
async fn automatic_resolve_runs_inside_the_slot_its_change_already_owns() {
    // `alpha` hit a conflict inside its background merge and is resolving, so
    // both a phase counter and lifecycle membership can see it at once. It is
    // one admitted change and must be counted once: subtracting the resolve
    // counter as well would report less capacity than exists, and would still
    // have reserved nothing for the conflict-free merge that owns most of the
    // interval.
    let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["alpha".to_string(), "beta".to_string()],
        0,
    )));
    let mut executor = evidence_executor(&state);
    executor
        .lifecycle_slots
        .occupy_now("alpha", SlotPhase::Merge)
        .await;
    executor
        .lifecycle_slots
        .occupy_now("beta", SlotPhase::Workspace)
        .await;
    executor
        .get_auto_resolve_counter()
        .store(1, std::sync::atomic::Ordering::SeqCst);

    assert_eq!(
        executor.calculate_available_slots(MAX, &ids(&["beta"])),
        1,
        "two admitted changes under a limit of three leave one slot, \
         however many phases of them are observable"
    );
}

#[tokio::test]
async fn merge_wait_retains_its_slot_until_the_operator_settles_it() {
    // Bounded resolve was exhausted: `alpha` is parked in `merge wait`. That is
    // backpressure by design — admitting a newer worktree here is exactly what
    // produced the divergence the wait exists to report — so the slot stays
    // occupied until settlement, and manual resolve runs inside it.
    let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
        0,
    )));
    let mut executor = evidence_executor(&state);
    for change_id in ["alpha", "beta", "gamma"] {
        executor
            .lifecycle_slots
            .occupy_now(change_id, SlotPhase::Workspace)
            .await;
    }
    executor
        .lifecycle_slots
        .apply_transition("alpha", merge_result_transition(false, false));

    state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "alpha".to_string(),
            reason: "bounded resolve exhausted".to_string(),
            auto_resumable: false,
        });
    let snapshot = executor.capture_reducer_work_snapshot().await;
    executor.reconcile_retained_lifecycle_slots(&snapshot);

    assert_eq!(
        executor.calculate_available_slots(MAX, &ids(&["beta", "gamma"])),
        0,
        "a retained merge wait keeps applying backpressure at full capacity"
    );

    // The operator resolves it and the merge lands.
    state
        .write()
        .await
        .apply_command(ReducerCommand::ResolveMerge("alpha".to_string()));
    assert!(
        executor
            .lifecycle_slots
            .apply_transition("alpha", merge_result_transition(true, false)),
        "merge settlement releases the retained slot exactly once"
    );
    assert_eq!(
        executor.calculate_available_slots(MAX, &ids(&["beta", "gamma"])),
        1,
        "settlement is what returns the capacity, not the wait"
    );
}

#[tokio::test]
async fn terminal_settlement_releases_a_retained_slot_through_reducer_evidence() {
    // Settlement that never produces a background result of its own — a
    // rejected, errored, stopped, or externally merged change — is released by
    // the same pass that reads the reducer, so the recovered capacity is
    // visible to that pass's own dispatch evaluation with no extra wake.
    let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["alpha".to_string()],
        0,
    )));
    let mut executor = evidence_executor(&state);
    state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "alpha".to_string(),
            reason: "bounded resolve exhausted".to_string(),
            auto_resumable: false,
        });

    let snapshot = executor.capture_reducer_work_snapshot().await;
    executor.reconcile_retained_lifecycle_slots(&snapshot);
    assert_eq!(
        executor.calculate_available_slots(MAX, &ids(&[])),
        MAX - 1,
        "the retained wait occupies a slot even though this process never dispatched it"
    );

    state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeCompleted {
            change_id: "alpha".to_string(),
            revision: "abc123".to_string(),
        });
    let snapshot = executor.capture_reducer_work_snapshot().await;
    executor.reconcile_retained_lifecycle_slots(&snapshot);

    assert_eq!(
        executor.calculate_available_slots(MAX, &ids(&[])),
        MAX,
        "terminal settlement returns the capacity"
    );
}

#[tokio::test]
async fn restart_reconstructs_retained_occupancy_from_reducer_evidence() {
    // A fresh process owns no slots. The retained `merge wait` survives as
    // workspace, Git, and reducer evidence, and the first pass that reads that
    // evidence re-establishes its occupancy — no durable slot lease exists to
    // consult (`openspec/CONSTITUTION.md`, law 1).
    let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        vec!["alpha".to_string(), "beta".to_string()],
        0,
    )));
    state
        .write()
        .await
        .apply_execution_event(&ExecutionEvent::MergeDeferred {
            change_id: "alpha".to_string(),
            reason: "manual resolution required".to_string(),
            auto_resumable: false,
        });

    let mut restarted = evidence_executor(&state);
    assert_eq!(
        restarted.calculate_available_slots(MAX, &ids(&[])),
        MAX,
        "a fresh process starts with no reconstructed occupancy"
    );

    let snapshot = restarted.capture_reducer_work_snapshot().await;
    restarted.reconcile_retained_lifecycle_slots(&snapshot);

    assert!(restarted.lifecycle_slots.contains("alpha"));
    assert_eq!(
        restarted.lifecycle_slots.phase("alpha"),
        Some(SlotPhase::Retained)
    );
    assert_eq!(restarted.calculate_available_slots(MAX, &ids(&[])), MAX - 1);
}

#[tokio::test]
async fn unprovable_retained_occupancy_fails_closed_for_new_dispatch() {
    // Restart recovery can find more retained work than there are permits — a
    // limit lowered between runs, or evidence that outlives this process's
    // ceiling. Occupancy it cannot prove suppresses new dispatch rather than
    // rounding down to "capacity is free".
    let mut slots = LifecycleSlots::new(1);
    slots.occupy_now("alpha", SlotPhase::Workspace).await;

    let report = slots.reconcile_retained(&ids(&["beta"]), &ids(&[]), &ids(&[]));

    assert_eq!(report.unavailable, vec!["beta".to_string()]);
    assert!(slots.has_unreserved());
    assert_eq!(slots.available(1, &ids(&["alpha"])), 0);
}

#[tokio::test]
async fn a_retention_the_reducer_has_not_published_yet_is_never_released() {
    // A background result can reach the scheduler before the reducer applies
    // the event that explains it. Absence of wait evidence therefore proves
    // nothing until the wait has been observed once; releasing on it would hand
    // the slot away from a change that is about to appear in `merge wait`.
    let mut slots = LifecycleSlots::new(MAX);
    slots.occupy_now("alpha", SlotPhase::Workspace).await;
    slots.apply_transition("alpha", merge_result_transition(false, false));

    let report = slots.reconcile_retained(&ids(&[]), &ids(&[]), &ids(&[]));
    assert!(
        report.released.is_empty(),
        "an unconfirmed retention is not settlement evidence"
    );
    assert!(slots.contains("alpha"));

    // The wait becomes reducer-visible, then really ends without a terminal
    // outcome of its own — an explicit dequeue back to idle work.
    slots.reconcile_retained(&ids(&["alpha"]), &ids(&[]), &ids(&[]));
    let report = slots.reconcile_retained(&ids(&[]), &ids(&[]), &ids(&[]));

    assert_eq!(report.released, vec!["alpha".to_string()]);
    assert_eq!(slots.available(MAX, &ids(&[])), MAX);
}

#[tokio::test]
async fn executing_slots_are_never_released_by_reconciliation() {
    // A workspace task and a background merge each own their own release edge
    // and always reach it. Reconciliation must not race them: between archive
    // and merge completion a row carries neither wait nor queue intent, and
    // treating that as settlement would drop a slot that is still working.
    let mut slots = LifecycleSlots::new(MAX);
    slots.occupy_now("alpha", SlotPhase::Workspace).await;
    slots.occupy_now("beta", SlotPhase::Merge).await;

    let report = slots.reconcile_retained(&ids(&[]), &ids(&[]), &ids(&[]));

    assert!(report.released.is_empty());
    assert_eq!(slots.len(), 2);
}

#[tokio::test]
async fn duplicate_settlement_delivery_never_returns_a_second_slot() {
    let mut slots = three_admitted().await;

    assert!(slots.apply_transition("alpha", merge_result_transition(true, false)));
    assert!(
        !slots.apply_transition("alpha", merge_result_transition(true, false)),
        "the second delivery releases nothing"
    );
    assert_eq!(slots.len(), MAX - 1);
    assert_eq!(slots.available(MAX, &ids(&["beta", "gamma"])), 1);
}

#[tokio::test]
async fn one_change_can_never_own_two_slots() {
    let mut slots = LifecycleSlots::new(MAX);
    assert!(slots.occupy_now("alpha", SlotPhase::Workspace).await);
    assert!(
        !slots.occupy_now("alpha", SlotPhase::Workspace).await,
        "a second admission of the same change is refused"
    );
    assert_eq!(slots.len(), 1);
    assert_eq!(slots.available(MAX, &ids(&["alpha"])), MAX - 1);

    // Reserving retained occupancy for a change that already owns a slot is the
    // same fact, not a second one.
    slots.reconcile_retained(&ids(&["alpha"]), &ids(&[]), &ids(&[]));
    assert_eq!(slots.len(), 1);
}

#[tokio::test]
async fn a_run_fatal_result_leaves_slot_ownership_to_the_aborting_run() {
    let mut slots = three_admitted().await;
    assert_eq!(
        merge_result_transition(false, /* run_fatal */ true),
        SlotTransition::Keep
    );
    slots.apply_transition("alpha", merge_result_transition(false, true));
    assert_eq!(
        slots.len(),
        MAX,
        "the aborting exit clears every slot itself"
    );

    slots.clear();
    assert_eq!(slots.len(), 0);
    assert_eq!(slots.available(MAX, &ids(&[])), MAX);
}

#[tokio::test]
async fn running_counts_and_admission_read_the_same_lifecycle_membership() {
    // The reported overrun was "apply 3 plus resolve 1 under a limit of 3".
    // Every display status a frontend counts as running belongs to a change
    // that owns a lifecycle slot, and admission is computed from that same
    // membership — so the fourth row the header would have to count can never
    // be admitted in the first place.
    let change_ids = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
    let state = Arc::new(tokio::sync::RwLock::new(OrchestratorState::new(
        change_ids.clone(),
        0,
    )));
    let mut executor = evidence_executor(&state);

    {
        let mut guard = state.write().await;
        for change_id in ["alpha", "beta"] {
            guard.apply_execution_event(&ExecutionEvent::ApplyStarted {
                change_id: change_id.to_string(),
                command: "apply".to_string(),
            });
        }
        guard.apply_execution_event(&ExecutionEvent::ResolveStarted {
            change_id: "gamma".to_string(),
            command: "resolve".to_string(),
        });
    }
    for change_id in &change_ids {
        executor
            .lifecycle_slots
            .occupy_now(change_id, SlotPhase::Workspace)
            .await;
    }

    let guard = state.read().await;
    let running = change_ids
        .iter()
        .filter(|change_id| is_active_status(guard.display_status(change_id)))
        .count();
    drop(guard);

    assert_eq!(running, MAX, "two applying plus one resolving");
    assert_eq!(
        executor.lifecycle_slots.len(),
        running,
        "every running row is one admitted lifecycle, counted once"
    );
    assert_eq!(
        executor.calculate_available_slots(MAX, &ids(&["alpha", "beta"])),
        0,
        "no fourth change can be admitted, so no fourth running row can exist"
    );
}
