---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/tui-state/spec.md
  - src/tui/runner.rs
  - src/tui/state/event_handlers/refresh.rs
  - src/orchestration/state.rs
  - src/orchestration/run_control.rs
verifications:
  - id: startup-merge-wait-tests
    requirement: Startup workspace evidence restores reducer-owned merge-wait state and manual resolve dispatch without regressing stronger lifecycle states
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for orchestration state, run-control, and TUI command-handler regression cases
    rerun: cargo test --lib orchestration::state && cargo test --lib orchestration::run_control && cargo test --lib tui::command_handlers
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: repository-quality-gates
    requirement: The Rust implementation remains formatted, lint-clean, and valid across the default test suite
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: successful make fmt, make lint, and make test results
    rerun: make fmt && make lint && make test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix TUI Startup Merge-Wait Resume

**Change Type**: implementation

## Problem / Context

Conflux reconstructs workflow state from workspace and Git evidence when the TUI starts. The refresh loop identifies archived-but-not-yet-merged workspaces and publishes them in `ChangesRefreshed.merge_wait_ids`, but the current reducer reconciliation path does not establish `WaitState::MergeWait` from a fresh idle state. The TUI row cache can therefore display `merge wait` while the shared reducer still reports `not queued`.

Pressing `M` on that row emits `TuiCommand::ResolveMerge`, but `RunControlService::resolve_merge` rejects the command because its authoritative reducer status is not `merge wait`. No resolve reservation is created and the scheduler is neither started nor notified.

This contradicts the workspace-local state law in `openspec/CONSTITUTION.md` and the existing `tui-resolve` requirement that pressing `M` on a merge-wait row creates scheduler-consumable retry intent.

## Proposed Solution

Treat refresh-derived archived-but-not-yet-merged workspace evidence as authoritative reconciliation input for a fresh, fully idle reducer entry. When `ChangesRefreshed.merge_wait_ids` contains a change whose reducer state has no activity, wait, queue intent, or terminal outcome, restore that change to reducer-owned `MergeWait` before the TUI renders it.

Preserve all stronger reducer-owned states. Refresh reconciliation must not demote active resolving/rejecting work, scheduler-owned pending work, queued work, terminal outcomes, or errors to `MergeWait`.

Keep `RunControlService` validation conservative. Once reconciliation makes the reducer and display agree, the existing manual resolve command path can reserve `ResolveWait` and start or notify scheduler-owned retry work without accepting arbitrary `not queued` targets.

The reducer reconciliation and manual resolve behavior form one atomic scope: restoring display state without enabling scheduler dispatch would retain the user-visible failure, while weakening dispatch admission without workspace evidence would broaden unsafe resolve eligibility.

## Acceptance Criteria

- Starting the TUI with an archived-but-not-yet-merged workspace causes the shared reducer and the TUI row to report `merge wait` for that change.
- Pressing `M` on that reconstructed row creates reducer-owned `ResolveWait`, reserves the change for manual resolve, and starts or notifies scheduler-owned retry evaluation.
- A reconstructed manual resolve does not complete as ordinary zero-change success while accepted retry intent remains pending.
- Refresh-derived evidence does not regress `resolving`, `resolve pending`, `rejecting`, `reject pending`, queued, merged, rejected, error, or other terminal state to `merge wait`.
- A change with no archived-but-not-yet-merged workspace evidence remains in its existing reducer state and cannot be resolved merely by submitting a stale or arbitrary command.
- TUI and `/api/v2` manual resolve commands continue to share the same reducer-owned eligibility and scheduler-dispatch behavior.

## Explicit Completion Conditions

- The `ChangesRefreshed` reducer path establishes `MergeWait` only for a fresh idle and not-queued change backed by `merge_wait_ids` workspace evidence.
- Existing active, pending, queued, terminal, and error guards are covered by regression tests and remain unchanged by refresh reconciliation.
- A repository-local test reproduces startup from a fresh `OrchestratorState`, applies only `ChangesRefreshed.merge_wait_ids`, submits manual resolve, and observes an active resolve reservation plus scheduler dispatch.
- TUI command-handler coverage verifies the startup-equivalent `M` path reaches `resolve pending` without the scheduler-state rejection warning.
- `make fmt`, `make lint`, and `make test` pass.

## Out of Scope

- Persisting reducer state outside the workspace or introducing a durable resume database.
- Changing `M` key bindings, TUI navigation, or worktree-view merge behavior.
- Altering merge conflict resolution, dirty-worktree classification, or sequential merge implementation.
- Automatically starting resolve solely from refresh; explicit `M` intent remains required for manual merge waits.
