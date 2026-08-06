//! Table-driven coverage for event-driven execution-mark reconciliation.
//!
//! Unit-scoped by construction: the reducer, the mark store, and the mutation
//! guard are all in-memory, so nothing here touches a repository, a process, a
//! clock, or a network boundary.

use std::collections::HashSet;
use std::sync::Arc;

use super::*;
use crate::events::{ExecutionEvent, RejectionOutcome, StalledBlocker};
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::state::OrchestratorState;

/// A reconciler over a fresh store seeded with `marked`.
fn reconciler(marked: &[&str]) -> (ExecutionMarkReconciler, Arc<ExecutionMarkStore>) {
    let marks = Arc::new(ExecutionMarkStore::new());
    for id in marked {
        marks.set(id, true);
    }
    let reconciler = ExecutionMarkReconciler::new(marks.clone(), Arc::new(ParallelRuntime::new()));
    (reconciler, marks)
}

fn state(change_ids: &[&str]) -> OrchestratorState {
    OrchestratorState::new(change_ids.iter().map(|id| id.to_string()).collect(), 10)
}

/// The exact ordering [`crate::events::dispatch_event_with_marks`] performs:
/// capture pre-state, apply, reconcile against post-state.
fn apply(
    state: &mut OrchestratorState,
    reconciler: &ExecutionMarkReconciler,
    event: &ExecutionEvent,
) -> Vec<String> {
    let pre = capture_pre_state(event, state);
    state.apply_execution_event(event);
    match pre {
        Some(pre) => reconciler.reconcile(event, &pre, state),
        None => Vec::new(),
    }
}

fn change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 1,
        total_tasks: 1,
        last_modified: "now".to_string(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

fn refresh(
    active: &[&str],
    rejected: &[&str],
    committed: &[&str],
    dirty: &[&str],
) -> ExecutionEvent {
    ExecutionEvent::ChangesRefreshed {
        changes: active.iter().map(|id| change(id)).collect(),
        rejected_changes: rejected.iter().map(|id| change(id)).collect(),
        committed_change_ids: committed.iter().map(|id| id.to_string()).collect(),
        uncommitted_file_change_ids: dirty.iter().map(|id| id.to_string()).collect(),
        worktree_change_ids: HashSet::new(),
        worktree_paths: Default::default(),
        worktree_not_ahead_ids: HashSet::new(),
        merge_wait_ids: HashSet::new(),
    }
}

fn on_merged_failure(change_id: &str) -> ExecutionEvent {
    ExecutionEvent::HookFailed {
        change_id: change_id.to_string(),
        hook_type: crate::hooks::HookType::OnMerged.config_key().to_string(),
        error: "publish script exited 1".to_string(),
    }
}

fn stalled_blocker() -> StalledBlocker {
    StalledBlocker {
        category: "acceptance_finding".to_string(),
        phase: "acceptance".to_string(),
        gate: "acceptance".to_string(),
        error_summary: "unresolved finding".to_string(),
        evidence: vec!["tests/acceptance.rs:1".to_string()],
        unblock_condition: None,
        prerequisite_owner: None,
        next_action: "resolve and retry".to_string(),
        resumable: true,
        worktree_preserved: true,
    }
}

// ── Revoking edges ──────────────────────────────────────────────────────────

/// Every mark-revoking edge clears exactly its own target, and every event that
/// creates no new edge leaves the mark alone.
///
/// The table is the contract: a variant that fails here is a frontend that would
/// render `[ ]` while `/api/v2` still reported `execution_marked: true`.
#[test]
fn event_mark_reconciliation_covers_failure_and_rejection_edges() {
    struct Case {
        name: &'static str,
        /// Events applied before the marks are (re)seeded.
        arrange: Vec<ExecutionEvent>,
        /// Marks set after `arrange`, so a case can express a *fresh* re-mark.
        marked: Vec<&'static str>,
        /// The event under test.
        event: ExecutionEvent,
        /// Marks expected to survive it.
        expected: Vec<&'static str>,
    }

    let alpha = "alpha";
    let beta = "beta";
    let failure_variants: Vec<(&'static str, ExecutionEvent)> = vec![
        (
            "processing failure",
            ExecutionEvent::ProcessingError {
                id: alpha.to_string(),
                error: "boom".to_string(),
            },
        ),
        (
            "apply failure",
            ExecutionEvent::ApplyFailed {
                change_id: alpha.to_string(),
                error: "boom".to_string(),
            },
        ),
        (
            "acceptance failure",
            ExecutionEvent::AcceptanceFailed {
                change_id: alpha.to_string(),
                error: "boom".to_string(),
            },
        ),
        (
            "archive failure",
            ExecutionEvent::ArchiveFailed {
                change_id: alpha.to_string(),
                error: "boom".to_string(),
                reason: None,
                summary: None,
            },
        ),
        (
            "push failure",
            ExecutionEvent::PushFailed {
                change_id: alpha.to_string(),
                remote: "origin".to_string(),
                branch: "cflx/alpha".to_string(),
                error: "boom".to_string(),
            },
        ),
        (
            "rejection-review failure",
            ExecutionEvent::RejectionReviewFailed {
                change_id: alpha.to_string(),
                error: "boom".to_string(),
            },
        ),
    ];

    let mut cases: Vec<Case> = failure_variants
        .iter()
        .map(|(name, event)| Case {
            name,
            arrange: Vec::new(),
            marked: vec![alpha, beta],
            event: event.clone(),
            expected: vec![beta],
        })
        .collect();

    // A late failure that cannot supersede a final outcome is not an edge, even
    // though its variant name ends in `Failed`.
    cases.extend(failure_variants.iter().map(|(name, event)| Case {
        name,
        arrange: vec![ExecutionEvent::MergeCompleted {
            change_id: alpha.to_string(),
            revision: "abc".to_string(),
        }],
        marked: vec![alpha, beta],
        event: event.clone(),
        expected: vec![alpha, beta],
    }));

    cases.extend([
        Case {
            name: "terminal rejection",
            arrange: Vec::new(),
            marked: vec![alpha, beta],
            event: ExecutionEvent::ChangeRejected {
                change_id: alpha.to_string(),
                reason: "blocker".to_string(),
            },
            expected: vec![beta],
        },
        Case {
            name: "rejected marker row introduced by refresh",
            arrange: Vec::new(),
            marked: vec![alpha, beta],
            event: refresh(&[beta], &[alpha], &[alpha, beta], &[]),
            expected: vec![beta],
        },
        Case {
            name: "successful dequeue",
            arrange: Vec::new(),
            marked: vec![alpha, beta],
            event: ExecutionEvent::ChangeDequeued {
                change_id: alpha.to_string(),
            },
            expected: vec![beta],
        },
        Case {
            name: "legacy target-scoped stop",
            arrange: Vec::new(),
            marked: vec![alpha, beta],
            event: ExecutionEvent::ChangeStopped {
                change_id: alpha.to_string(),
            },
            expected: vec![beta],
        },
        Case {
            name: "duplicate dequeue after re-mark",
            arrange: vec![ExecutionEvent::ChangeDequeued {
                change_id: alpha.to_string(),
            }],
            marked: vec![alpha, beta],
            event: ExecutionEvent::ChangeDequeued {
                change_id: alpha.to_string(),
            },
            expected: vec![alpha, beta],
        },
        Case {
            name: "first on_merged hook failure enters merge-wait recovery",
            arrange: Vec::new(),
            marked: vec![alpha, beta],
            event: on_merged_failure(alpha),
            expected: vec![beta],
        },
        Case {
            name: "replayed on_merged hook failure preserves a fresh re-mark",
            arrange: vec![on_merged_failure(alpha)],
            marked: vec![alpha, beta],
            event: on_merged_failure(alpha),
            expected: vec![alpha, beta],
        },
        Case {
            name: "a non-merge hook failure is not a mark edge",
            arrange: Vec::new(),
            marked: vec![alpha, beta],
            event: ExecutionEvent::HookFailed {
                change_id: alpha.to_string(),
                hook_type: "post_apply".to_string(),
                error: "boom".to_string(),
            },
            expected: vec![alpha, beta],
        },
        Case {
            name: "duplicate failure after re-mark keeps the fresh intent",
            arrange: vec![ExecutionEvent::ApplyFailed {
                change_id: alpha.to_string(),
                error: "boom".to_string(),
            }],
            marked: vec![alpha, beta],
            event: ExecutionEvent::ApplyFailed {
                change_id: alpha.to_string(),
                error: "boom".to_string(),
            },
            expected: vec![alpha, beta],
        },
        Case {
            name: "dequeue cannot revoke a rejected row's mark a second time",
            arrange: vec![ExecutionEvent::ChangeRejected {
                change_id: alpha.to_string(),
                reason: "blocker".to_string(),
            }],
            marked: vec![alpha, beta],
            event: ExecutionEvent::ChangeDequeued {
                change_id: alpha.to_string(),
            },
            expected: vec![alpha, beta],
        },
    ]);

    for case in cases {
        let mut reducer = state(&[alpha, beta]);
        let (reconciler, marks) = reconciler(&[]);
        for event in &case.arrange {
            apply(&mut reducer, &reconciler, event);
        }
        for id in &case.marked {
            marks.set(id, true);
        }

        apply(&mut reducer, &reconciler, &case.event);

        let expected: Vec<String> = case.expected.iter().map(|id| id.to_string()).collect();
        assert_eq!(
            marks.marked_ids(),
            expected,
            "{}: unexpected mark set after reconciliation",
            case.name
        );
    }
}

/// Repeating a revoking event changes nothing once the mark is already gone.
#[test]
fn event_mark_reconciliation_is_idempotent_for_repeated_revocation() {
    let mut reducer = state(&["alpha"]);
    let (reconciler, marks) = reconciler(&["alpha"]);
    let event = ExecutionEvent::ChangeRejected {
        change_id: "alpha".to_string(),
        reason: "blocker".to_string(),
    };

    assert_eq!(
        apply(&mut reducer, &reconciler, &event),
        vec!["alpha".to_string()]
    );
    assert!(apply(&mut reducer, &reconciler, &event).is_empty());
    assert!(marks.marked_ids().is_empty());
}

// ── Refresh classification ──────────────────────────────────────────────────

/// One refresh clears only the target it classifies parallel-ineligible.
///
/// The classification uses the canonical eligibility observation and the shared
/// cleanup rule, so a frontend cannot decide a different target set — and the
/// mutation stays target-scoped, so a mark another frontend set on an eligible
/// row survives.
#[test]
fn parallel_ineligible_refresh_revokes_target_mark() {
    let mut reducer = state(&["alpha", "beta"]);
    let (reconciler, marks) = reconciler(&["alpha", "beta"]);

    // `alpha` has uncommitted proposal files; `beta` is committed and clean.
    let event = refresh(&["alpha", "beta"], &[], &["alpha", "beta"], &["alpha"]);
    let revoked = apply(&mut reducer, &reconciler, &event);

    assert_eq!(revoked, vec!["alpha".to_string()]);
    assert_eq!(marks.marked_ids(), vec!["beta".to_string()]);

    // Repeating the same observation is a no-op, and it still cannot reach `beta`.
    assert!(apply(&mut reducer, &reconciler, &event).is_empty());
    assert_eq!(marks.marked_ids(), vec!["beta".to_string()]);

    // A change absent from HEAD is the other ineligibility reason, and it is
    // classified the same way.
    marks.set("beta", true);
    let absent = refresh(&["alpha", "beta"], &[], &["alpha"], &[]);
    apply(&mut reducer, &reconciler, &absent);
    assert!(
        marks.marked_ids().is_empty(),
        "a target absent from HEAD keeps an invalid mark"
    );
}

/// An eligible refresh never revokes anything.
#[test]
fn eligible_refresh_preserves_every_mark() {
    let mut reducer = state(&["alpha", "beta"]);
    let (reconciler, marks) = reconciler(&["alpha", "beta"]);

    let event = refresh(&["alpha", "beta"], &[], &["alpha", "beta"], &[]);
    assert!(apply(&mut reducer, &reconciler, &event).is_empty());
    assert_eq!(
        marks.marked_ids(),
        vec!["alpha".to_string(), "beta".to_string()]
    );
}

// ── Preservation boundaries ─────────────────────────────────────────────────

/// Waits, holds, successes, and both process-level terminal events preserve the
/// complete mark set.
///
/// These are the events an operator's resume and retry controls depend on: a
/// stop that dropped the marked target set would silently discard the run the
/// operator intends to resume.
#[test]
fn event_mark_reconciliation_preserves_unrelated_and_stopped_marks() {
    let preserved: Vec<(&str, ExecutionEvent)> = vec![
        (
            "dependency block",
            ExecutionEvent::DependencyBlocked {
                change_id: "alpha".to_string(),
                dependency_ids: vec!["beta".to_string()],
            },
        ),
        (
            "acceptance-gated stall",
            ExecutionEvent::AcceptanceGated {
                change_id: "alpha".to_string(),
                blocker: stalled_blocker(),
            },
        ),
        (
            "execution hold",
            ExecutionEvent::ExecutionBlocked {
                change_id: "alpha".to_string(),
                blocker: stalled_blocker(),
            },
        ),
        (
            "skipped after a failed dependency",
            ExecutionEvent::ChangeSkipped {
                change_id: "alpha".to_string(),
                reason: "dependency failed".to_string(),
            },
        ),
        (
            "manual merge-wait deferral",
            ExecutionEvent::MergeDeferred {
                change_id: "alpha".to_string(),
                reason: "base dirty".to_string(),
                auto_resumable: false,
            },
        ),
        (
            "auto-resumable resolve wait",
            ExecutionEvent::MergeDeferred {
                change_id: "alpha".to_string(),
                reason: "lane busy".to_string(),
                auto_resumable: true,
            },
        ),
        (
            "resolve failure returns to merge wait",
            ExecutionEvent::ResolveFailed {
                change_id: "alpha".to_string(),
                error: "conflict".to_string(),
            },
        ),
        (
            "archive success",
            ExecutionEvent::ChangeArchived("alpha".to_string()),
        ),
        (
            "merge success",
            ExecutionEvent::MergeCompleted {
                change_id: "alpha".to_string(),
                revision: "abc".to_string(),
            },
        ),
        (
            "push success",
            ExecutionEvent::PushCompleted {
                change_id: "alpha".to_string(),
                remote: "origin".to_string(),
                branch: "cflx/alpha".to_string(),
            },
        ),
        (
            "resumed rejection review",
            ExecutionEvent::RejectionReviewCompleted {
                change_id: "alpha".to_string(),
                outcome: RejectionOutcome::Resume,
            },
        ),
        ("run completion", ExecutionEvent::AllCompleted),
        (
            "global fatal error without a target",
            ExecutionEvent::Error {
                message: "scheduler died".to_string(),
            },
        ),
        ("process-level stop", ExecutionEvent::Stopped),
    ];

    for (name, event) in preserved {
        let mut reducer = state(&["alpha", "beta"]);
        let (reconciler, marks) = reconciler(&["alpha", "beta"]);

        let revoked = apply(&mut reducer, &reconciler, &event);

        assert!(revoked.is_empty(), "{name} revoked a mark it must preserve");
        assert_eq!(
            marks.marked_ids(),
            vec!["alpha".to_string(), "beta".to_string()],
            "{name} did not preserve the complete mark set"
        );
    }
}

/// A process-level stop keeps the resume target set even when the interrupted
/// rows were mid-flight, and the reducer's own stop reconciliation still runs.
#[test]
fn process_stop_retains_marked_resume_targets() {
    let mut reducer = state(&["alpha", "beta"]);
    let (reconciler, marks) = reconciler(&["alpha", "beta"]);

    apply(
        &mut reducer,
        &reconciler,
        &ExecutionEvent::ApplyStarted {
            change_id: "alpha".to_string(),
            command: "apply".to_string(),
        },
    );
    apply(&mut reducer, &reconciler, &ExecutionEvent::Stopped);

    assert_eq!(
        marks.marked_ids(),
        vec!["alpha".to_string(), "beta".to_string()],
        "a process stop must leave every resume target marked"
    );
    assert_eq!(reducer.display_status("alpha"), "not queued");
}

/// Reducer queue intent is queue presentation and never synthesizes a mark.
#[test]
fn queue_intent_never_creates_an_execution_mark() {
    use crate::orchestration::state::ReducerCommand;

    let mut reducer = state(&["alpha"]);
    let (reconciler, marks) = reconciler(&[]);
    reducer.apply_command(ReducerCommand::AddToQueue("alpha".to_string()));
    assert_eq!(reducer.display_status("alpha"), "queued");

    let event = refresh(&["alpha"], &[], &["alpha"], &[]);
    apply(&mut reducer, &reconciler, &event);

    assert!(
        marks.marked_ids().is_empty(),
        "queue intent must not become an execution mark"
    );
}
