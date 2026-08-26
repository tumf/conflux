---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/mark_settlement.rs
  - src/orchestration/operator_coordinator.rs
  - src/orchestration/operator_command.rs
  - src/tui/command_handlers/mark_settlement_tests.rs
  - openspec/specs/operator-command-execution/spec.md
verifications:
  - id: running-mark-settlement-regression
    requirement: A mark accepted while the persistent scheduler is already running becomes queue intent after the stability window and wakes analysis exactly once.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: change-acceptance
    automation: src/tui/command_handlers/mark_settlement_tests.rs
    evidence: Focused Rust tests covering the production owner wiring, concurrent active work, timer settlement, queue mutation, and scheduler notification.
    rerun: cargo test --locked running_mark_reanalysis
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix running mark settlement admission

**Change Type**: implementation

## Problem / Context

A live Conflux v0.6.298 owner accepted an execution mark for `add-stt-ccr-loop-runner` while another change was active. More than the 10-second stability window later, the coherent owner snapshot still reported:

- `app_mode: running`
- `scheduler_running: true`
- `execution_marked: true`
- `queue_intent: not_queued`
- `display_status: not queued`
- no blocker and parallel eligibility true

The existing canonical requirement already says this target must gain queue intent without another Start. Existing paused-time adapter tests pass, so they do not cover the production boundary that failed to reconcile the target.

## Proposed Solution

Trace and repair the production path from an accepted individual, bulk, or API mark mutation through `ExecutionMarkStore::arm_settlement`, the process-local settlement task, application runtime binding, queue mutation, and scheduler wake.

Keep execution marks, queue intent, and lifecycle control separate. Do not enqueue directly in a frontend and do not send or synthesize Start. Preserve the 10-second coalescing window and changed-target scope.

Add a deterministic regression using production-equivalent owner wiring where:

1. the persistent scheduler is already running;
2. one unrelated change remains active;
3. a second ordinary eligible change is marked through the shared operator/API path;
4. the stability window expires;
5. the second change becomes queued and produces exactly one scheduler reanalysis edge.

The regression must also expose an explicit typed/logged reason if settlement cannot arm or complete. A mark accepted during a live scheduler must not silently remain `not queued` indefinitely.

## Acceptance Criteria

- An accepted mark for an ordinary eligible `not queued` change settles into queue intent while the owner is already running; no additional Start is required.
- Concurrent active work does not prevent the independent marked target from settling into the queue.
- The settlement batch remains target-scoped and preserves unrelated marks and explicit queue intent.
- A real queue-membership change wakes scheduler analysis exactly once; a no-op settlement wakes it zero times.
- Failure to bind, arm, spawn, upgrade, or execute the settlement runtime is observable with a stable reason instead of a silent mark-only result when the owner reports a live command-capable scheduler.
- TUI Space/bulk mark and `cflx client`/API mark reach the same production settlement behavior.
- `cargo test --locked running_mark_reanalysis` exits 0.

## Explicit Completion Conditions

- The root cause is demonstrated by a regression that fails before the implementation change and passes afterward.
- The regression exercises the production owner/application binding and a live persistent scheduler with concurrent active work, not only a recording scheduler flag or isolated classifier.
- Source changes stay within shared mark-settlement, owner/application wiring, and their tests/observability.
- The focused verification runs in normal repository test execution and exits 0.

## Out of Scope

- Changing Start semantics.
- Merging execution marks with queue intent.
- Durable persistence of marks or pending settlement batches.
- Retrying terminal-error changes from a mark.
- Editing or restarting the affected downstream `diffusion-kkc` owner as part of this change.
