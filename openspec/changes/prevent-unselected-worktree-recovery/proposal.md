---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/orchestration/state.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/executor.rs
  - src/tui/orchestrator.rs
  - src/tui/state/processing_logic.rs
verifications:
  - id: run-admission-recovery-tests
    requirement: "A TUI parallel run processes only admitted changes while preserving explicit archived-dirty recovery and manual merge retry behavior"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust unit and temporary-Git integration output covering unselected worktree exclusion, selected and dynamically admitted recovery, restart behavior, and manual merge-wait ownership"
    rerun: "cargo test parallel::tests::executor && cargo test tui::orchestrator && cargo test tui::state"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent unselected worktree recovery from entering TUI runs

**Change Type**: implementation

## Problem / Context

The TUI correctly starts with every execution mark cleared and passes only marked IDs into a new parallel run. The scheduler then performs a repository-wide worktree reconciliation. When it finds a non-merged archived-dirty worktree, `reconcile_queued_candidates_from_shared_state` appends that change to scheduler-local queued work even when the operator did not mark it and the reducer has no queue intent for it.

The observed run started one selected change, then queue reconciliation discovered an unselected `Archiving (files moved, commit incomplete)` workspace, expanded analysis to two changes, skipped apply for the recovered workspace, and started archive commit finalization. This bypasses the operator-visible execution-mark boundary.

Archived-dirty recovery itself is required. A change that the operator explicitly admits must still resume from workspace/git/base-tree evidence without rerunning completed phases. Manual `merge wait` also remains an explicit `ResolveMerge` path rather than ordinary queued recovery.

## Proposed Solution

Treat the current reducer run snapshot as the admission boundary for scheduler-owned archived-dirty discovery. Queue reconciliation may recover a workspace-derived archived-dirty candidate only when its change ID has been admitted to the current run through initial TUI selection or a later explicit queue addition. Existing worktrees outside that set remain untouched and available for a future explicit run.

Preserve dynamic queue behavior by continuing to add explicitly queued changes to the reducer snapshot through the existing `add_dynamic_change` path. Preserve empty-queue `ResolveWait` startup and manual `merge wait` retry because those flows carry explicit reducer-owned retry intent rather than ordinary archived-dirty discovery.

Add regression coverage at the queue-reconciliation boundary and the TUI parallel startup boundary. The tests must prove both sides: an unselected recoverable workspace cannot enter analysis or lifecycle execution, while the same workspace becomes recoverable after explicit admission and resumes at the workspace-derived phase.

This remains one change because the TUI admission boundary and scheduler recovery filter must ship together. Filtering without preserving explicit admission would strand valid recovery; changing startup alone would leave the scheduler bypass intact.

## Acceptance Criteria

1. Starting a TUI parallel run with one marked change admits only that change to the initial run snapshot and scheduler-local queued set.
2. A non-merged archived-dirty worktree whose ID is outside the current run snapshot is not added by queue reconciliation, dependency analysis, apply, acceptance, archive finalization, or post-archive merge handling.
3. Excluding an unselected worktree does not delete, clean, commit, merge, or otherwise mutate it.
4. Marking that same change in a later run admits it and resumes from repository-visible workspace state without rerunning phases already proven complete.
5. Adding a change explicitly through the Running-mode queue path admits it through the existing reducer dynamic-change path and allows the same workspace-derived recovery.
6. A manual `merge wait` change remains excluded from ordinary archived-dirty recovery and advances only after accepted `ResolveMerge` intent.
7. Empty ordinary queues with reducer-owned `ResolveWait` continue to start or wake scheduler retry evaluation.
8. Already-merged worktrees remain terminal residue and terminal-error worktrees remain explicit-retry-only.
9. No durable execution mark, recovery allowlist, or out-of-worktree workflow-control state is introduced; restart routing remains derived from workspace/git/base-tree evidence after current-process operator admission.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` gates repository-wide archived-dirty candidate insertion against current-run admission while retaining reducer-queued and reducer-owned lane-wait behavior.
- `src/orchestration/state.rs` remains the process-local source of current-run membership, including initial selected IDs and explicitly added dynamic IDs; no second admission store is introduced.
- `src/tui/orchestrator.rs` continues to initialize the parallel reducer from marked IDs and preserves the empty-ID manual resolve startup path.
- `src/parallel/tests/executor.rs` includes a temporary-Git regression with selected `fresh` and unselected archived-dirty `stale`, proving `stale` is absent from queued work and analysis/lifecycle dispatch until explicitly admitted.
- Tests prove selected restart recovery, Running-mode dynamic admission, manual merge-wait exclusion, terminal-error stop gating, and already-merged residue behavior remain intact.
- `cargo test parallel::tests::executor && cargo test tui::orchestrator && cargo test tui::state`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Removing archived-dirty recovery or requiring users to delete preserved worktrees.
- Persisting execution marks or queue intent across process restart.
- Automatically selecting interrupted changes when the TUI starts.
- Changing archive commands, archive layout, retry budgets, or merge conflict resolution.
- Changing explicit `M` / `ResolveMerge` semantics for manual merge-wait changes.
