---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/tui/key_handlers.rs
  - src/tui/state/selection_logic.rs
  - src/tui/command_handlers.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/operator_coordinator.rs
  - src/tui/queue.rs
  - src/parallel/queue_state.rs
  - src/parallel/tests/manual_resolve.rs
verifications:
  - id: running-mark-reanalysis-tests
    requirement: Running-mode execution marks settle into the current queue after a stable interval and trigger dependency analysis without unsafe cancellation
    phase: pre-integration
    owner: conflux-acceptance
    trigger: apply-completion
    automation: Makefile
    evidence: Passing output from the focused running_mark_reanalysis Rust tests
    rerun: cargo test --lib running_mark_reanalysis
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Restore Running Mark Reanalysis

**Change Type**: implementation

## Problem / Context

Running-mode execution marks previously fed the active parallel run: a newly marked `not queued` change became reducer-visible queue intent, `DynamicQueue` notified the scheduler, and dependency analysis could proceed while unrelated apply or resolve work remained active. Commit `e3167311` separated Space and bulk `x` marks from current-run queue mutation. The current implementation now stores only future-run mark intent, so marking a newly discovered or omitted change during a run cannot add it to that run.

That behavior conflicts with the existing parallel scheduler contract for queue-notification reanalysis and with the active-resolve scenario that expects a TUI-marked change to enter scheduler analysis before resolve completes. It also removes the earlier operator-facing stability behavior: users expect rapid mark edits to settle before the queue plan changes, rather than requiring another Start command that is unavailable during Running mode.

## Proposed Solution

Add one process-local, event-driven Running-mode mark reconciliation boundary shared by equivalent operator frontends.

After any accepted execution-mark mutation while the process is in Running mode, retain the latest mark set and restart one 10-second stability deadline. No queue intent changes before that deadline. When the mark set has remained unchanged for the full interval:

- add each loadable ordinary `not queued` change that is marked to reducer queue intent through the existing admitted queue service;
- remember which pending queue memberships were created by this reconciliation episode so a later stable unmark may remove only those still-pending memberships;
- never remove explicitly queued work, active work, retry/resolve intent, merge/resolve waiters, terminal rows, or changes that are otherwise ineligible for ordinary queue admission;
- emit no cancellation or stop request from mark reconciliation.

A real queue addition must use the existing `DynamicQueue` notification path. Once the stable mark set creates a new scheduler-local candidate, the existing explicit queue-addition edge starts dependency analysis without a second 10-second queue debounce. Analysis may run while resolve is active or ordinary dispatch capacity is zero; apply dispatch remains capacity-gated.

The stability deadline, latest observed mark set, and reconciliation provenance are process-local and disposable. They must not become workspace-external durable workflow state, and an idle process with no unsettled marks must not poll the repository or scheduler.

## Acceptance Criteria

- During Running mode, marking one or more eligible `not queued` changes with Space, bulk `x`, or an equivalent shared operator command does not mutate queue intent during the first 10 seconds after the latest accepted mark change.
- Every additional accepted mark change before settlement restarts the single stability deadline, and only the final stable mark set is reconciled.
- At settlement, marked loadable ordinary changes enter reducer queue intent and `DynamicQueue` exactly once; duplicate reconciliation is a no-op.
- A stable unmark removes only still-pending queue membership previously created by mark reconciliation. Explicit queue membership and active/admitted work remain untouched.
- Active, error, retry, merge-wait, resolve-wait, terminal, and ordinary ineligible rows may retain their execution-mark behavior but gain no queue, retry, resolve, stop, or cancellation side effect from reconciliation.
- A settled queue addition wakes the scheduler and starts queued-only dependency analysis without another queue debounce, including while a resolve task is active and while ordinary dispatch capacity is zero.
- Available capacity permits normal dispatch from the new analysis result; zero capacity suppresses dispatch without suppressing analysis.
- Select, Stopping, Stopped, and Error modes do not reconcile execution marks into the active queue.
- Restart discards unsettled marks, stability timing, and mark-admission provenance without changing next-action routing for identical workspace and Git state.
- Deterministic tests use paused time, channels, or state transitions rather than short wall-clock correctness thresholds and complete within the repository's default one-second test target.

## Explicit Completion Conditions

The change is complete when repository evidence shows all of the following:

- The shared operator/orchestration boundary owns one event-driven 10-second mark-set stability deadline for Running mode and exposes no second durable authority.
- Stable reconciliation routes additions and eligible provenance-bound removals through the existing queue service, reducer transition, hooks, and scheduler notification rather than mutating frontend display state directly.
- TUI Space and bulk `x` reach the shared behavior without adding a new queue key or requiring a second Start action.
- Focused unit tests prove deadline reset, final-snapshot reconciliation, duplicate no-op behavior, mode/status exclusions, provenance-safe unmark, and restart-empty state.
- Focused integration tests prove a real `DynamicQueue` addition reaches scheduler analysis during active resolve and at zero capacity, with dispatch occurring only when capacity becomes available.
- `cargo test --lib running_mark_reanalysis` passes.

## Scope Rationale

Mark stability and current-run queue/analysis wiring remain one proposal because neither half produces the requested behavior independently: stability without admission is still mark-only, while admission without stability violates the requested settle period.

## Out of Scope

- Adding a dedicated queue key or changing configured Start controls.
- Making execution marks durable across process restart.
- Cancelling active work through Space or bulk `x`; `K` remains the change-scoped termination control.
- Replacing the scheduler's general queue debounce, analysis-signature, blocked-only, or persistent-idle policy.
- Changing dependency analyzer output or dispatch ordering.
- Treating wait/error marks as implicit retry or resolve commands.

The tracked Rust pre-commit hooks are path-scoped, so proposal-only commit creation does not own Rust validation. Requirement-specific focused tests remain explicit implementation evidence.
