---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/queue_state.rs
  - src/openspec.rs
  - src/parallel/tests/manual_resolve.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-06-13-fix-spawned-retry-lane-release/tasks.md
---

# Fix Dynamic Queue Ingestion Repo-Root Resolution and Self-Referential Test Fixture

**Change Type**: implementation

## Problem/Context

`cargo test --lib parallel::tests` is currently red on `main` (deterministic, 3/3
reproductions). The Task 6 regression test backfilled by
`2026-06-13-fix-spawned-retry-lane-release`
(`src/parallel/tests/manual_resolve.rs::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve`)
pushes the dynamic-queue id `fix-spawned-retry-lane-release` — the id of the change that
introduced it — and waits for the "Dynamically added to parallel execution" event.
Dynamic-queue ingestion (`check_dynamic_queue_and_add_changes`,
`src/parallel/queue_state.rs:1645`) validates candidates via the cwd-based
`crate::openspec::list_changes_native()`, which lists ACTIVE changes in the host
repository's `openspec/changes/`. The change was active while the apply ran (test green),
but archiving it (commit `b2db95d2`) moved it to `openspec/changes/archive/`, so the
lookup now reports `candidate_not_found`, the awaited event never fires, and the test
times out after 500ms. Two defects:

1. **Self-referential test fixture.** The regression test depends on the host
   repository's own OpenSpec change state, which is guaranteed to change at archive time.
   Quality gates passed during apply but the merged final state is red.
2. **cwd-based change listing in scheduler ingestion.** `ParallelExecutor` carries a
   configured `repo_root`, but dynamic-queue ingestion candidate validation ignores it
   and resolves `openspec/changes` against the process working directory. Besides making
   a self-contained test impossible, this is incorrect whenever the executor's
   `repo_root` differs from the process cwd. `list_changes_native_from(&Path)` already
   exists (`src/openspec.rs:391`) and is the correct call.

Tracked as beads issue `conflux-o63`.

## Proposed Solution

1. Make dynamic-queue ingestion resolve OpenSpec changes from the executor's configured
   `repo_root`: replace the `list_changes_native()` call at
   `src/parallel/queue_state.rs:1645` with
   `list_changes_native_from(&self.repo_root)`.
2. Make the gated-resolve scheduler-loop test self-contained: build a temp-dir fixture
   containing a synthetic ACTIVE change under `openspec/changes/<synthetic-id>/`, point
   the executor's `repo_root` at the temp dir, and push the synthetic id to the dynamic
   queue. The test must pass regardless of the host repository's OpenSpec state.
3. Add focused regression coverage for repo-root resolution of ingestion candidate
   validation (present under `repo_root` → ingested; absent → `candidate_not_found`
   reconciliation log), independent of process cwd.

Other cwd-based `list_changes_native()` call sites (orchestrator, TUI, web, merge paths)
are intentionally untouched; this change fixes only the scheduler dynamic-queue
ingestion site that the broken test exercises.

## Acceptance Criteria

- `cargo test --lib parallel::tests::manual_resolve` passes on a tree where
  `fix-spawned-retry-lane-release` is archived (i.e., current `main` state), and the
  gated-resolve test no longer references any real change id from the host repository.
- Dynamic-queue ingestion validates candidates against the executor's configured
  `repo_root`: a change directory that exists only under a non-cwd `repo_root` is
  ingested; a candidate absent under `repo_root` produces the existing
  `candidate_not_found` reconciliation log and is not queued.
- The gated-resolve test still verifies the original Task 6 behaviors: dynamic ingest
  while the resolve gate is held, `AnalysisStarted` within the bounded window,
  `dispatch_capacity_zero_after_analysis` diagnostic, and no `ApplyStarted`.
- No behavior change for ingestion when `repo_root` equals the process cwd.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs::check_dynamic_queue_and_add_changes` calls
  `list_changes_native_from(&self.repo_root)`; `rg "list_changes_native\(\)" src/parallel/queue_state.rs`
  no longer matches the dynamic-queue ingestion site.
- `scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` constructs its own
  temp-dir OpenSpec fixture and contains no occurrence of the string
  `fix-spawned-retry-lane-release` (or any other real change id of this repository).
- A test exists that fails if ingestion candidate validation regresses to cwd-based
  lookup (fixture change exists only under the temp `repo_root`, not under the process
  cwd).
- `cargo test --lib parallel::tests` passes on the final tree; each new/modified
  default-suite test completes in under 1 second (AGENTS.md rule).

## Out of Scope

- Migrating the remaining cwd-based `list_changes_native()` call sites
  (`src/orchestrator.rs`, `src/main.rs`, `src/parallel/merge.rs`,
  `src/parallel/queue_state.rs:1224`, TUI/web) to `repo_root`-based resolution —
  separate cleanup with its own risk assessment.
- The spawned-retry give-up lane-release defect (`conflux-m8d`) — handled by the
  separate change `fix-retry-giveup-lane-release`.
- Any change to debounce, capacity gating, or analysis targeting semantics.
