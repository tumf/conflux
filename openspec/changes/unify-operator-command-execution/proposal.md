---
change_type: implementation
priority: high
dependencies: []
references:
  - "openspec/CONSTITUTION.md"
  - "openspec/specs/frontend-abstraction/spec.md"
  - "openspec/specs/hooks/spec.md"
  - "openspec/specs/tui-state-management/spec.md"
  - "openspec/specs/orchestration-state/spec.md"
  - "src/orchestration/state.rs"
  - "src/tui/command_handlers.rs"
  - "src/tui/queue.rs"
  - "src/parallel/dispatch.rs"
  - "src/execution/state.rs"
verifications:
  - id: operator-command-local
    requirement: Shared operator commands, queue side effects, cancellation ordering, and retry routing are covered by repository-local tests.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/command_handlers.rs
    evidence: cargo test output for operator_command cases
    rerun: cargo test operator_command
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: unify operator command execution

**Change Type**: implementation

## Problem / Context

TUI controls currently combine reducer updates, dynamic queue mutation, cancellation, hooks, and retry routing inside frontend-specific handlers. A second remote-control frontend would either duplicate that behavior or diverge on lifecycle guards and side-effect ordering.

The shared frontend contract already requires Core state changes through `ReducerCommand` and notifications through `EventSink`. Operator actions therefore need one process-local application service that both TUI and future API adapters call without introducing a second workflow state machine or durable control database.

## Proposed Solution

Add a process-local operator command service that owns lifecycle validation and coordinates reducer commands with runtime side effects. TUI handlers become adapters to this service; a later API change consumes the same service.

The service will:

- represent execution marks, queue intent, activity, hold state, terminal state, and `display_status()` as separate fields rather than collapsing them into one frontend flag;
- keep execution marks process-local and initialize every change as unmarked after process restart;
- use `ReducerCommand` for authoritative runtime transitions and `EventSink` for resulting notifications;
- centralize dynamic queue add/remove, per-change cancellation, retry, and hook dispatch;
- preserve dependency-blocked queue intent instead of rejecting it, while preserving `stalled` for resumable non-dependency holds;
- prevent direct execution-mark mutation in `Error` mode and require explicit retry commands;
- allow mark mutation in `Select` and `Stopped`, use queue intent for ordinary Running rows, and allow mark-only mutation during `MergeWait` or `ResolveWait`;
- cancel and confirm active work termination before applying `ReducerCommand::DequeueChange`;
- route terminal error retry through `ReducerCommand::RetryError` and acceptance-stalled retry through the existing reconciled acceptance-hold path without rerunning apply;
- invoke `on_queue_add` and `on_queue_remove` exactly once only after a real dynamic queue mutation, never for initial start or no-op requests.

## Acceptance Criteria

1. TUI operator actions and direct service calls produce the same reducer transitions, events, queue mutations, and hook outcomes.
2. Execution marks are process-local operator intent, restart as `false`, and remain distinct from queue intent and lifecycle status.
3. A dependency-ineligible queue addition remains queued with `blocked` status; resumable non-dependency holds remain `stalled`; no `gated` display status is introduced.
4. Invalid mode/action combinations fail without reducer or runtime side effects.
5. Active `stop_and_dequeue` reports success only after the target cancellation token exists, cancellation is issued, and task/process termination is confirmed; failure or timeout preserves active state.
6. Queue hooks run once after successful dynamic mutation and not for initial queue population, duplicate add/remove, or failed mutation.
7. Terminal error retry and acceptance-stalled retry follow their existing repository/reconciled-hold semantics; bulk retry does not consume unsupported or mismatched holds.
8. Existing TUI key behavior and existing external API routes remain backward compatible.

## Explicit Completion Conditions

- A shared service and command/result types exist outside frontend-specific rendering code and are used by TUI handlers.
- Tests exercise success, no-op, invalid lifecycle, dependency-blocked queueing, hook cardinality, missing cancellation token, cancellation failure/timeout, terminal retry, acceptance-stalled retry, and restart mark reset.
- `DynamicQueue::force_kill` result semantics are no longer treated as proof of process termination; active dequeue has an observable completion handshake.
- `cargo test operator_command`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and strict OpenSpec validation pass.

## Out of Scope

- HTTP, SSE, WebSocket, authentication, or OpenAPI routes.
- Worktree create/delete/merge operations.
- Durable execution marks or external workflow-control storage.
- New lifecycle status vocabulary or changes to serial mode.
