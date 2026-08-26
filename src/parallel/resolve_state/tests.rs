//! Merge-authorization tests for sequential resolve classification.
//!
//! Every case runs against in-memory doubles: no process, no filesystem, no
//! repository. What is under test is the decision — whether repository-visible
//! task evidence authorizes a final merge — not Git itself.

use super::fixtures::{
    archived, archived_tasks, item, live, live_tasks, presynced_repo, tasks_markdown, FakeEvidence,
    FakeRepo,
};
use super::{
    classify_batch, classify_batch_with_latch, read_task_completion, BatchState,
    MergeAuthorizationLatch, TaskCompletion,
};

/// Assert a diagnosis carries nothing an agent could execute as a final merge.
fn assert_no_merge_instruction(diagnosis: &str) {
    for forbidden in [
        "git merge",
        "Merge change:",
        "git commit",
        "--no-ff",
        "git rm",
    ] {
        assert!(
            !diagnosis.contains(forbidden),
            "withheld merge guidance must not contain '{}': {}",
            forbidden,
            diagnosis
        );
    }
    assert!(
        diagnosis.contains("required_action: none"),
        "withheld merge guidance must state that no action is required: {}",
        diagnosis
    );
}

fn single_item() -> Vec<super::SequentialMergeItem> {
    vec![item("ws-a", "change-a", "/wt/a")]
}

// ---------------------------------------------------------------------------
// Repository-derived task completion evidence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn archived_task_evidence_reports_its_own_progress() {
    let mut repo = presynced_repo();
    repo.file("a_tip", &archived_tasks("change-a"), &tasks_markdown(6, 7));
    let evidence = FakeEvidence::new(repo, "t");
    let tree = vec![archived("change-a"), archived_tasks("change-a")];

    let completion = read_task_completion(&evidence, "change-a", "a_tip", &tree).await;

    assert_eq!(
        completion,
        TaskCompletion::Incomplete {
            source: archived_tasks("change-a"),
            completed: 6,
            total: 7,
        }
    );
}

#[tokio::test]
async fn active_task_evidence_is_read_when_the_change_is_not_yet_archived() {
    let mut repo = FakeRepo::default();
    repo.commit("a_tip", "Work", &[])
        .file("a_tip", &live_tasks("change-a"), &tasks_markdown(6, 7));
    let evidence = FakeEvidence::new(repo, "t");
    let tree = vec![live("change-a"), live_tasks("change-a")];

    let completion = read_task_completion(&evidence, "change-a", "a_tip", &tree).await;

    assert_eq!(
        completion,
        TaskCompletion::Incomplete {
            source: live_tasks("change-a"),
            completed: 6,
            total: 7,
        }
    );
}

#[tokio::test]
async fn complete_task_evidence_names_every_source_it_proved() {
    let mut repo = FakeRepo::default();
    repo.commit("a_tip", "Work", &[])
        .file("a_tip", &live_tasks("change-a"), &tasks_markdown(2, 2))
        .file("a_tip", &archived_tasks("change-a"), &tasks_markdown(7, 7));
    let evidence = FakeEvidence::new(repo, "t");
    let tree = vec![live_tasks("change-a"), archived_tasks("change-a")];

    let completion = read_task_completion(&evidence, "change-a", "a_tip", &tree).await;

    match completion {
        TaskCompletion::Complete { sources, total } => {
            assert_eq!(total, 9);
            assert!(sources.contains(&live_tasks("change-a")), "{:?}", sources);
            assert!(
                sources.contains(&archived_tasks("change-a")),
                "{:?}",
                sources
            );
        }
        other => panic!("expected complete task evidence, got {:?}", other),
    }
}

#[tokio::test]
async fn one_incomplete_list_outweighs_a_complete_sibling() {
    let mut repo = FakeRepo::default();
    repo.commit("a_tip", "Work", &[])
        .file("a_tip", &live_tasks("change-a"), &tasks_markdown(6, 7))
        .file("a_tip", &archived_tasks("change-a"), &tasks_markdown(7, 7));
    let evidence = FakeEvidence::new(repo, "t");
    let tree = vec![live_tasks("change-a"), archived_tasks("change-a")];

    assert_eq!(
        read_task_completion(&evidence, "change-a", "a_tip", &tree).await,
        TaskCompletion::Incomplete {
            source: live_tasks("change-a"),
            completed: 6,
            total: 7,
        },
        "the merge integrates every list at once, so the weakest one decides"
    );
}

#[tokio::test]
async fn task_evidence_fails_closed_when_it_cannot_establish_completion() {
    // No task list at all.
    let evidence = FakeEvidence::new(presynced_repo(), "t");
    let completion =
        read_task_completion(&evidence, "change-a", "a_tip", &[archived("change-a")]).await;
    assert!(
        matches!(completion, TaskCompletion::Unestablished { .. }),
        "a missing task list must never authorize a merge: {:?}",
        completion
    );

    // Listed in the tree but with no readable blob behind it.
    let mut orphan = presynced_repo();
    orphan.tree_only("a_tip", "openspec/changes/change-a/tasks.md");
    let evidence = FakeEvidence::new(orphan, "t");
    let completion =
        read_task_completion(&evidence, "change-a", "a_tip", &[live_tasks("change-a")]).await;
    assert!(
        matches!(completion, TaskCompletion::Unestablished { .. }),
        "an unreadable task list must fail closed: {:?}",
        completion
    );

    // A readable list that records no tasks at all is ambiguous, not complete.
    let mut empty = presynced_repo();
    empty.file(
        "a_tip",
        &archived_tasks("change-a"),
        "## Implementation Tasks\n",
    );
    let evidence = FakeEvidence::new(empty, "t");
    let completion = read_task_completion(
        &evidence,
        "change-a",
        "a_tip",
        &[archived_tasks("change-a")],
    )
    .await;
    assert!(
        matches!(completion, TaskCompletion::Unestablished { .. }),
        "0/0 tasks is ambiguous evidence, not proof of completion: {:?}",
        completion
    );

    // An evidence-layer read failure is the same answer.
    let evidence =
        FakeEvidence::new(presynced_repo(), "t").unreadable("a_tip", &archived_tasks("change-a"));
    let completion = read_task_completion(
        &evidence,
        "change-a",
        "a_tip",
        &[archived_tasks("change-a")],
    )
    .await;
    assert!(
        matches!(completion, TaskCompletion::Unestablished { .. }),
        "a failed read must fail closed: {:?}",
        completion
    );
}

#[tokio::test]
async fn another_changes_task_list_is_not_this_changes_evidence() {
    let mut repo = FakeRepo::default();
    repo.commit("a_tip", "Work", &[])
        .file("a_tip", &archived_tasks("change-b"), &tasks_markdown(7, 7))
        .file(
            "a_tip",
            "openspec/changes/archive/2026-08-03/change-a/tasks.md",
            &tasks_markdown(7, 7),
        )
        .file(
            "a_tip",
            "openspec/changes/archive/prefix-change-a/tasks.md",
            &tasks_markdown(7, 7),
        );
    let evidence = FakeEvidence::new(repo, "t");
    let tree = vec![
        archived_tasks("change-b"),
        "openspec/changes/archive/2026-08-03/change-a/tasks.md".to_string(),
        "openspec/changes/archive/prefix-change-a/tasks.md".to_string(),
    ];

    let completion = read_task_completion(&evidence, "change-a", "a_tip", &tree).await;

    assert!(
        matches!(completion, TaskCompletion::Unestablished { .. }),
        "a sibling change, a nested layout, and a suffix collision are not this change's evidence: {:?}",
        completion
    );
}

// ---------------------------------------------------------------------------
// Merge authorization in batch classification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn archived_change_with_an_incomplete_task_is_not_authorized() {
    let mut repo = presynced_repo();
    repo.file("a_tip", &archived_tasks("change-a"), &tasks_markdown(6, 7));
    let evidence = FakeEvidence::new(repo, "t").worktree("ws-a", "/wt/a", "a_tip");
    let items = single_item();

    let state = classify_batch(&evidence, &items, "base").await;

    match &state {
        BatchState::MergeNotAuthorized { change_id, reason } => {
            assert_eq!(change_id, "change-a");
            assert!(reason.contains("6/7"), "{}", reason);
        }
        other => panic!("expected merge not authorized, got {:?}", other),
    }
    assert_eq!(state.phase(), "merge_not_authorized");
    assert!(
        !state.allows_agent_action(),
        "an unfinished change must not start another agent attempt"
    );
    assert_no_merge_instruction(&state.diagnosis());
}

#[tokio::test]
async fn active_change_with_an_incomplete_task_is_not_authorized() {
    let mut repo = presynced_repo();
    // The live directory survives alongside the archive, as it does before
    // resurrection cleanup, and it is the one recording unfinished work.
    repo.file("a_tip", &live_tasks("change-a"), &tasks_markdown(6, 7));
    let evidence = FakeEvidence::new(repo, "t").worktree("ws-a", "/wt/a", "a_tip");
    let items = single_item();

    let state = classify_batch(&evidence, &items, "base").await;

    match &state {
        BatchState::MergeNotAuthorized { change_id, reason } => {
            assert_eq!(change_id, "change-a");
            assert!(reason.contains(&live_tasks("change-a")), "{}", reason);
        }
        other => panic!("expected merge not authorized, got {:?}", other),
    }
    assert_no_merge_instruction(&state.diagnosis());
}

#[tokio::test]
async fn missing_task_evidence_is_not_authorized() {
    let mut repo = presynced_repo();
    repo.tree("a_tip", &[&archived("change-a")]);
    let evidence = FakeEvidence::new(repo, "t").worktree("ws-a", "/wt/a", "a_tip");

    let state = classify_batch(&evidence, &single_item(), "base").await;

    match &state {
        BatchState::MergeNotAuthorized { reason, .. } => {
            assert!(reason.contains("no task evidence"), "{}", reason)
        }
        other => panic!("expected merge not authorized, got {:?}", other),
    }
    assert_no_merge_instruction(&state.diagnosis());
}

#[tokio::test]
async fn complete_tasks_preserve_the_existing_final_merge_path() {
    let evidence = FakeEvidence::new(presynced_repo(), "t").worktree("ws-a", "/wt/a", "a_tip");

    let state = classify_batch(&evidence, &single_item(), "base").await;

    match &state {
        BatchState::FinalMergeMissing {
            change_id,
            required_target_state,
            ..
        } => {
            assert_eq!(change_id, "change-a");
            assert_eq!(required_target_state, "t");
        }
        other => panic!("expected the existing final merge path, got {:?}", other),
    }
    assert!(state.allows_agent_action());
    assert!(
        state.diagnosis().contains("Merge change: change-a"),
        "a complete change keeps its exact per-change merge guidance: {}",
        state.diagnosis()
    );
}

#[tokio::test]
async fn conflict_resolution_completion_does_not_authorize_the_merge() {
    let mut repo = presynced_repo();
    repo.file("a_tip", &archived_tasks("change-a"), &tasks_markdown(6, 7));
    let items = single_item();

    // While the worktree still holds conflicts the batch is agent-actionable:
    // resolving them is legitimate work.
    let conflicted = FakeEvidence::new(repo.clone(), "t")
        .worktree("ws-a", "/wt/a", "a_tip")
        .worktree_conflicted("/wt/a", &["src/main.rs"]);
    let state = classify_batch(&conflicted, &items, "base").await;
    assert!(
        matches!(state, BatchState::PreSyncUnfinished { .. }),
        "conflict resolution must stay available while the merge is withheld: {:?}",
        state
    );
    assert!(state.allows_agent_action());

    // Once they are resolved and nothing else changed, the merge is still not
    // authorized, and the batch has become non-agent-actionable.
    let resolved = FakeEvidence::new(repo, "t").worktree("ws-a", "/wt/a", "a_tip");
    let state = classify_batch(&resolved, &items, "base").await;
    assert!(
        matches!(state, BatchState::MergeNotAuthorized { .. }),
        "resolving a conflict never authorizes a final merge: {:?}",
        state
    );
    assert!(!state.allows_agent_action());
    assert_no_merge_instruction(&state.diagnosis());
}

#[tokio::test]
async fn withheld_authorization_never_reports_the_batch_complete() {
    let mut repo = presynced_repo();
    repo.file("a_tip", &archived_tasks("change-a"), &tasks_markdown(0, 7));
    let evidence = FakeEvidence::new(repo, "t").worktree("ws-a", "/wt/a", "a_tip");

    let state = classify_batch(&evidence, &single_item(), "base").await;

    assert!(!state.is_complete(), "{:?}", state);
    assert!(!state.allows_agent_action(), "{:?}", state);
}

// ---------------------------------------------------------------------------
// Monotonic in-process refusal latch
// ---------------------------------------------------------------------------

#[test]
fn latch_is_monotonic_and_authorizes_nothing() {
    let latch = MergeAuthorizationLatch::new();

    assert_eq!(latch.refusal("change-a"), None);
    assert_eq!(latch.refused_count(), 0);

    latch.refuse("change-a", "first reason");
    latch.refuse("change-a", "second reason");

    assert_eq!(
        latch.refusal("change-a").as_deref(),
        Some("first reason"),
        "a refusal is never rewritten by a later one"
    );
    assert_eq!(latch.refused_count(), 1);
    assert_eq!(
        latch.refusal("change-b"),
        None,
        "refusing one change says nothing about another"
    );
}

#[tokio::test]
async fn a_latched_refusal_survives_evidence_that_would_authorize_the_merge() {
    let latch = MergeAuthorizationLatch::new();
    let items = single_item();

    // A first classification against incomplete evidence refuses and latches.
    let mut incomplete = presynced_repo();
    incomplete.file("a_tip", &archived_tasks("change-a"), &tasks_markdown(6, 7));
    let evidence = FakeEvidence::new(incomplete, "t").worktree("ws-a", "/wt/a", "a_tip");
    let first = classify_batch_with_latch(&evidence, &items, "base", &latch).await;
    assert!(
        matches!(first, BatchState::MergeNotAuthorized { .. }),
        "{:?}",
        first
    );
    assert_eq!(latch.refused_count(), 1);

    // Even a subsequent classification whose evidence looks complete cannot get
    // the merge instruction back inside the same batch.
    let repaired = FakeEvidence::new(presynced_repo(), "t").worktree("ws-a", "/wt/a", "a_tip");
    let second = classify_batch_with_latch(&repaired, &items, "base", &latch).await;

    match &second {
        BatchState::MergeNotAuthorized { change_id, reason } => {
            assert_eq!(change_id, "change-a");
            assert!(reason.contains("6/7"), "{}", reason);
        }
        other => panic!("a latched refusal must survive, got {:?}", other),
    }
    assert!(!second.allows_agent_action());
    assert_no_merge_instruction(&second.diagnosis());

    // A fresh latch — what the next process gets — recomputes from the
    // workspace alone, so the refusal is ephemeral, not durable state.
    let fresh = classify_batch(&repaired, &items, "base").await;
    assert!(
        matches!(fresh, BatchState::FinalMergeMissing { .. }),
        "authorization must be recomputed from the workspace after restart: {:?}",
        fresh
    );
}

#[tokio::test]
async fn repeated_classification_of_unchanged_evidence_stays_refused() {
    let latch = MergeAuthorizationLatch::new();
    let items = single_item();
    let mut repo = presynced_repo();
    repo.file("a_tip", &archived_tasks("change-a"), &tasks_markdown(6, 7));
    let evidence = FakeEvidence::new(repo, "t").worktree("ws-a", "/wt/a", "a_tip");

    for round in 0..3 {
        let state = classify_batch_with_latch(&evidence, &items, "base", &latch).await;
        assert!(
            matches!(state, BatchState::MergeNotAuthorized { .. }),
            "round {} must stay refused: {:?}",
            round,
            state
        );
        assert!(!state.allows_agent_action(), "round {}", round);
    }
    assert_eq!(
        latch.refused_count(),
        1,
        "one change refused once, however often it is reclassified"
    );
}

#[tokio::test]
async fn blocking_one_item_does_not_authorize_or_mutate_another() {
    // change-a is next in declared order and unfinished; change-b sits behind it.
    let mut repo = FakeRepo::default();
    repo.commit("base", "Base", &[])
        .commit("a1", "Work a", &["base"])
        .commit("a_tip", "Archive change-a", &["a1"])
        .commit("b1", "Work b", &["base"])
        .commit("b_tip", "Archive change-b", &["b1"])
        .tree("base", &[])
        .tree("a_tip", &[&archived("change-a")])
        .tree("b_tip", &[&archived("change-b")])
        .file("a_tip", &archived_tasks("change-a"), &tasks_markdown(6, 7))
        .file("b_tip", &archived_tasks("change-b"), &tasks_markdown(7, 7));
    let evidence = FakeEvidence::new(repo, "base")
        .worktree("ws-a", "/wt/a", "a_tip")
        .worktree("ws-b", "/wt/b", "b_tip");
    let items = vec![
        item("ws-a", "change-a", "/wt/a"),
        item("ws-b", "change-b", "/wt/b"),
    ];
    let latch = MergeAuthorizationLatch::new();

    let state = classify_batch_with_latch(&evidence, &items, "base", &latch).await;

    match &state {
        BatchState::MergeNotAuthorized { change_id, .. } => assert_eq!(change_id, "change-a"),
        other => panic!("expected change-a to be withheld, got {:?}", other),
    }
    let diagnosis = state.diagnosis();
    assert_no_merge_instruction(&diagnosis);
    assert!(
        !diagnosis.contains("change-b"),
        "blocking one item must not name another as actionable: {}",
        diagnosis
    );
    assert_eq!(
        latch.refusal("change-b"),
        None,
        "change-b keeps its own authorization question, unanswered"
    );
}
