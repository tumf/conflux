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
  - src/events.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/run_control.rs
  - src/orchestration/state.rs
  - src/orchestrator.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/executor.rs
  - src/tui/orchestrator.rs
  - src/tui/run_supervisor.rs
  - src/tui/runner.rs
verifications:
  - id: explicit-recovery-intent-tests
    requirement: "Every frontend processes only explicit targets or current reducer intent while preserving repository-derived archived-dirty recovery, revocation, and lane-wait behavior"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust unit and temporary-Git integration output covering production refresh ordering, analyzer and lifecycle exclusion, Git immutability, TUI/CLI/remote equivalence, dequeue and requeue, selected archived-dirty recovery, lane waits, and terminal stop gates"
    rerun: "cargo test parallel::tests::executor && cargo test tui::orchestrator && cargo test orchestration::state && cargo test orchestration::run_control && cargo test tui::command_handlers"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent unselected worktree recovery from entering operator runs

**Change Type**: implementation

## Problem / Context

TUI selection and shared operator marks correctly produce an explicit target list. The parallel scheduler later performs a repository-wide archived-dirty worktree scan inside `reconcile_queued_candidates_from_shared_state`. That scan appends a recoverable worktree ID to the same list as reducer-queued IDs even when no frontend selected, queued, retried, or resolved that change.

The observed run started one selected change, then discovered an unselected `Archiving (files moved, commit incomplete)` workspace, expanded dependency analysis to two changes, skipped completed apply work, and started archive commit finalization for the unselected change. Repository evidence correctly identified a recoverable phase but incorrectly became implicit operator intent.

`OrchestratorState::initial_change_ids` cannot serve as the admission gate. Catalog initialization and `ChangesRefreshed(all_changes)` call `add_dynamic_change`, so the set can contain every active change. It also has no revocation path when queue intent is removed or a change is dequeued.

Archived-dirty recovery remains required after explicit operator intent. TUI, CLI, and remote frontends share the scheduler and must produce identical eligibility. Manual resolve/reject lane waits and terminal retry rules remain reducer-owned and distinct from ordinary queued work.

## Proposed Solution

Remove worktree-wide discovery as a source of ordinary execution intent. Repository/worktree discovery may classify and display recoverable state, but it must not append a change ID to scheduler-local queued or analysis candidates.

Use existing explicit scheduler inputs instead of adding a new membership store:

- initial TUI, CLI, or remote targets enter through the shared start boundary and initial scheduler-local candidates;
- accepted Running-mode queue additions and terminal-error retries produce reducer `QueueIntent::Queued`;
- `RemoveFromQueue` and `DequeueChange` revoke ordinary queued eligibility immediately;
- accepted `ResolveMerge` and rejection-review handoff remain separate reducer-owned `ResolveWait` and `RejectWait` lane intent.

When an explicitly targeted or reducer-queued ID is no longer loadable from the active OpenSpec catalog, reconciliation may inspect its preserved workspace. If repository evidence proves archived-dirty state, the scheduler may reconstruct a repair candidate and resume the evidence-derived archive-finalization or archive-complete phase. `ChangesRefreshed`, catalog discovery, and unrelated worktree scans never grant eligibility.

Add one production-order regression that initializes selected `fresh`, applies `ChangesRefreshed` containing `fresh` and archived-dirty `stale`, runs reconciliation and analysis, captures lifecycle events, and compares `stale` Git/worktree state before and after. Positive, revocation, frontend-equivalence, lane-wait, merged-residue, and terminal-error cases complete the boundary.

This remains one change because intent ownership, scheduler reconciliation, canonical archived-dirty semantics, and cross-frontend regression coverage must ship atomically. Splitting them would either preserve the bypass or strand explicitly requested recovery.

## Acceptance Criteria

1. An archived-dirty worktree with no initial explicit target, `QueueIntent::Queued`, `ResolveWait`, or `RejectWait` is excluded from scheduler-local queued work, dependency analysis, apply, acceptance, archive finalization, resolve/reject handling, and post-archive merge.
2. `ChangesRefreshed(all_changes)`, catalog refresh, workspace observation, and repository-wide worktree discovery do not create ordinary execution eligibility.
3. Excluding an unrequested workspace does not change its HEAD, branch ref, index, worktree status, or files and does not emit lifecycle-start/completion events for it.
4. An initial explicit target from TUI, CLI, or remote Start can recover its archived-dirty workspace and resumes from repository-visible evidence without rerunning completed phases.
5. An accepted dynamic queue addition or explicit terminal-error retry produces reducer queued intent and enables the same recovery path.
6. `RemoveFromQueue`, successful stop-and-dequeue, and `DequeueChange` prevent worktree rediscovery from re-adding ordinary work during the same run; a later explicit requeue enables recovery again.
7. TUI and remote mark-plus-Start produce the same initial target eligibility, and TUI/remote queue commands produce the same reducer queued eligibility. CLI explicit targets obey the same scheduler boundary, while unrelated CLI-visible worktrees remain excluded.
8. Manual `MergeWait` advances only after accepted `ResolveMerge`; reducer-owned `ResolveWait` and `RejectWait` remain independently consumable when the ordinary queue is empty.
9. An explicitly eligible already-merged worktree remains terminal residue, and an explicitly visible terminal-error worktree remains blocked until `RetryError`; these stop gates must be proven after the explicit-intent boundary is passed.
10. A run with no ordinary queued, active, resolve-wait, or reject-wait intent may drain or complete even when unrequested archived-dirty residue exists.
11. No durable execution mark, queue intent, recovery allowlist, or out-of-worktree workflow-control state is introduced. After restart, explicit process-local intent is required again, and identical workspace evidence chooses the same resume phase once that intent is supplied.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` no longer converts repository-wide worktree discovery into reducer/scheduler queued IDs. It resolves archived-dirty candidates only for initial explicit candidates or reducer IDs whose current intent makes them eligible.
- `src/orchestration/state.rs` keeps catalog membership separate from current queue/wait intent; `ChangesRefreshed` may register display/runtime entries but cannot make them dispatchable.
- `src/orchestration/operator_command.rs` and `src/orchestration/run_control.rs` remain the frontend-neutral sources of accepted start, queue, dequeue, retry, and resolve intent; no parallel admission service is added.
- TUI, CLI, and remote start/queue tests prove identical explicit-intent semantics at their shared runtime boundaries.
- A temporary-Git production-order test covers selected initialization, all-change refresh, reconciliation, captured analyzer input, lifecycle-event absence, and immutable `stale` repository/worktree evidence.
- Tests cover initial and dynamic archived-dirty recovery, remove/dequeue non-reacquisition, explicit requeue, empty-queue resolve/reject waits, explicitly eligible merged residue, and terminal-error retry-only behavior.
- `cargo test parallel::tests::executor && cargo test tui::orchestrator && cargo test orchestration::state && cargo test orchestration::run_control && cargo test tui::command_handlers`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Removing archived-dirty phase classification, observability, or explicit recovery.
- Persisting execution marks or reducer intent across process restart.
- Automatically selecting or queuing interrupted changes when any frontend starts.
- Changing archive commands, archive layout, retry budgets, merge conflict resolution, or rejection-review semantics.
- Adding a new durable or process-local admission allowlist when existing start, queue, retry, and lane-wait intent is sufficient.
