---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/circuit-breaker/spec.md
  - src/orchestration/state.rs
  - src/parallel/queue_state.rs
  - src/parallel/dispatch.rs
  - src/tui/state/selection_logic.rs
---

# Fix Error Retry Gating

**Change Type**: implementation

## Problem / Context

Parallel Conflux runs can return a change to ordinary apply dispatch after an apply, acceptance, archive, or workspace execution error. That makes an errored change appear to stop briefly, then re-enter apply without an explicit operator retry. Existing TUI affordances already imply that error rows require a retry mark, and the reducer already represents `TerminalState::Error`, but the scheduler/resume contract does not make that terminal state authoritative for dispatch gating.

The change must preserve the current recoverable-error model: a delayed success event from the same already-running change may still supersede an error, but a new ordinary apply dispatch must not be created until the operator explicitly requests retry.

## Proposed Solution

Make `TerminalState::Error` a dispatch gate for ordinary apply work across parallel queue reconciliation, workspace resume scans, and TUI/server retry paths. Introduce an explicit retry transition that clears the error terminal state and reintroduces queued intent only when the operator marks/retries the change.

Keep circuit breaker behavior as secondary protection for repeated explicit retries, not as the primary mechanism for stopping automatic error loops.

## Acceptance Criteria

- A parallel apply, acceptance, archive, dispatch, or workspace execution error leaves the affected change displayed as `error` and removes it from ordinary apply dispatch candidates.
- Scheduler reanalysis, queue reconciliation, and workspace resume scans do not resurrect an errored change into apply without explicit retry intent.
- TUI retry-mark/F5 behavior explicitly clears the reducer error terminal and queues the marked change for retry.
- Late success events from an already-running same-change execution may still supersede recoverable error state without requiring retry and without spawning a new apply.
- Changes depending on an errored dependency remain blocked until that dependency is explicitly retried and reaches repository-visible success.
- Verification includes reducer-level, scheduler-level, and TUI retry regression coverage that fails for no-op or placeholder implementations.

## Explicit Completion Conditions

- `src/orchestration/state.rs` exposes or wires a repository-verifiable explicit retry transition that clears `TerminalState::Error`, queue/wait blockers, and relevant stale retry bookkeeping for the target change.
- `src/parallel/queue_state.rs` and resume/dispatch paths exclude terminal-error changes from ordinary apply dispatch, including worktree-derived repair/resume candidates.
- TUI retry paths in `src/tui/state/selection_logic.rs` use the explicit retry transition rather than only mutating local display state.
- Tests exercise an errored parallel change through reanalysis/resume without redispatch, then through explicit retry with redispatch eligibility restored.
- `cargo test` targets covering orchestration state, parallel queue/dispatch, and TUI retry behavior pass.

## Out of Scope

- Changing the existing rule that delayed archive/merge/resolve success may supersede recoverable error state for the same change.
- Replacing the same-error circuit breaker configuration or threshold model.
- Introducing durable workflow-control state outside the workspace/git/base evidence allowed by the Constitution.
