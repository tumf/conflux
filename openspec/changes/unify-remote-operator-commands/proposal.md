---
change_type: implementation
priority: high
dependencies:
  - expose-authoritative-operator-snapshot
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - src/orchestration/operator_command.rs
  - src/tui/command_handlers.rs
  - src/web/remote_control_api/executor.rs
  - src/web/remote_control_api/commands.rs
verifications:
  - id: shared-command-tests
    requirement: TUI and v2 lifecycle intents produce the same admitted outcome, reducer transition, scheduler side effect, and event
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Table-driven service and remote-control command test output
    rerun: cargo test --features web-monitoring --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Unify remote operator command execution

**Change Type**: implementation

## Problem / Context

Several `/api/v2` lifecycle commands report success when a control message was enqueued, before the shared application service has accepted the intent or produced its scheduler effect. Start does not consume the authoritative marked target set, retry planning does not prove scheduler dispatch, and resolve does not expose TUI-equivalent single-resolver queue behavior. `result_revision` can therefore precede the effect that the command claims.

## Proposed Solution

Move start, retry, graceful stop, cancel stop, force stop, and resolve behind shared process-local application services used by TUI and v2 adapters. Settle command records only after admission has produced an actual changed, no-op, or failed outcome and the resulting projection revision includes the synchronous decision fields. Preserve idempotent replay, expected-revision fencing, cancellation-first dequeue, and safe-boundary force-stop classification.

## Acceptance Criteria

1. Equivalent TUI and v2 intents call the same service and produce equivalent reducer transitions, scheduler wake/spawn behavior, events, and errors.
2. Start consumes the authoritative marked target set at the admitted revision and fails or no-ops when no eligible target exists.
3. Retry routes error, stalled acceptance, and resumable external holds correctly and proves scheduler dispatch; unsupported holds remain unchanged.
4. Resolve enforces one active resolver, FIFO queued resolve, duplicate prevention, stale-target rejection, and actual scheduler wake/spawn.
5. Stop, cancel stop, and force stop enforce the TUI mode matrix and return truthful safe-boundary classification.
6. Command records distinguish `succeeded`, `no-op`, and `failed`; their result revision includes the corresponding decision-state mutation.

## Explicit Completion Conditions

- TUI command handlers and v2 executor are thin adapters over shared application services for all in-scope lifecycle commands.
- No command is settled as succeeded solely because an internal channel send succeeded.
- Table-driven tests compare TUI and v2 outcomes for valid, invalid-mode, stale, duplicate, empty-target, scheduler-live, scheduler-idle, and failure cases.
- `cargo test --features web-monitoring operator_command remote_control_api::tests::command_tests` passes.

## Out of Scope

- Parallel-mode toggle and eligibility, handled by `add-remote-parallel-control`.
- Worktree safety-policy changes.
- Browser confirmation dialogs or UI controls.
- Durable command intent across process restart.
