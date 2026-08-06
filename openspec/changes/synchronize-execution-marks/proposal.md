---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/tui-error-handling/spec.md
  - openspec/specs/cli/spec.md
  - openspec/changes/archive/fix-tui-error-space-requeue/
  - openspec/changes/fix-force-stop-reducer-reconciliation/
  - src/events.rs
  - src/orchestration/operator_command.rs
  - src/tui/state.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/state/event_handlers/output.rs
  - src/tui/state/event_handlers/completion.rs
  - src/web/state.rs
verifications:
  - id: execution-mark-event-regressions
    requirement: "Every system-driven execution-mark revocation updates the shared ExecutionMarkStore before TUI and API projection, preserves unrelated and stop-retained marks, and keeps explicit Error retry behavior coherent"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering failure/rejection/dequeue/refresh mark edges, TUI row parity, same-revision API projection, duplicate idempotency, Error re-mark retry, blocker preservation, and global stop mark retention"
    rerun: "cargo test --lib event_mark_reconciliation_covers_failure_and_rejection_edges -- --list | grep -q event_mark_reconciliation_covers_failure_and_rejection_edges && cargo test --lib event_mark_reconciliation_covers_failure_and_rejection_edges && cargo test --lib tui_event_rows_follow_authoritative_marks -- --list | grep -q tui_event_rows_follow_authoritative_marks && cargo test --lib tui_event_rows_follow_authoritative_marks && cargo test --lib execution_event_clears_authoritative_mark_before_projection -- --list | grep -q execution_event_clears_authoritative_mark_before_projection && cargo test --lib execution_event_clears_authoritative_mark_before_projection && cargo test --lib event_mark_reconciliation_preserves_unrelated_and_stopped_marks -- --list | grep -q event_mark_reconciliation_preserves_unrelated_and_stopped_marks && cargo test --lib event_mark_reconciliation_preserves_unrelated_and_stopped_marks && cargo test --lib error_remark_remains_retryable -- --list | grep -q error_remark_remains_retryable && cargo test --lib error_remark_remains_retryable && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Synchronize execution marks

**Change Type**: implementation

## Problem / Context

`ExecutionMarkStore` is the process-local authoritative target set used by the shared operator command and `/api/v2`. TUI rows also cache the same fact as `ChangeState::selected`.

Several orchestrator event handlers clear only the TUI row cache when a change fails, is rejected, or is dequeued. The shared mark remains set, so the TUI can display `[ ]` while `/api/v2` reports `execution_marked: true` and a later Start reads the stale target. Reducer-derived Error or Rejected transitions that bypass those local handlers can produce the same drift.

Publishing the entire TUI row set after each handler would reverse ownership and allow one frontend cache to overwrite marks changed by another frontend. The shared store must remain authoritative.

## Proposed Solution

Add one process-local execution-mark reconciliation step at the authoritative event-dispatch boundary, after the reducer applies the event and before frontend sinks build TUI or Web projections.

The reconciler SHALL clear only the affected change's shared mark when a typed event creates a mark-revoking edge:

- transition into a recoverable change-level Error through processing, apply, acceptance, archive, push, or rejection-review failure;
- transition into terminal Rejected, including rejected rows discovered by refresh;
- successful per-change dequeue/legacy stop;
- the existing `on_merged` hook-failure transition whose TUI behavior intentionally clears the mark.

The reconciliation is idempotent and preserves marks for unrelated changes, blocked/stalled/dependency-wait rows, completion/archive/merge/push success, process-level `Stopped`, and global fatal `Error` without a target. TUI rows mirror `ExecutionMarkStore` after event handling instead of owning an independent clear decision. Web snapshots read the already-reconciled mark at the same event revision.

Explicit operator re-mark/retry behavior remains unchanged: a change-level Error clears its old mark, can be marked again in supported Running/Stopped flows, and F5/retry commands consume the new explicit intent. Process-level stop continues to preserve marks for resume.

## Split Rationale

This proposal is independent from `restore-ready-on-persistent-idle`. It corrects process-local mark ownership and event ordering without changing scheduler lifetime or frontend Ready/Running transitions. Neither proposal requires repository output from the other, so both may be implemented in parallel with no hard dependency.

## Acceptance Criteria

1. A marked change that transitions into reducer terminal Error has its `ExecutionMarkStore` entry cleared before TUI and Web project the event.
2. Processing, apply, acceptance, archive, push, and rejection-review failure paths use the same mark-revocation rule.
3. `ChangeRejected` and rejected rows introduced by `ChangesRefreshed` clear only their own shared marks.
4. Successful `ChangeDequeued`/legacy `ChangeStopped` and `on_merged` hook failure keep their existing TUI deselection behavior and apply it to the shared store.
5. TUI `selected`, `/api/v2 execution_marked`, and Start target resolution agree immediately after each revoking event.
6. `/api/v2` publishes the new mark value in the same state revision/event envelope as the failure, rejection, dequeue, or refresh transition; no extra mark-only revision is required.
7. Duplicate delivery is idempotent and does not clear unrelated marks or advance a second unchanged state revision.
8. Dependency-blocked, acceptance/external stalled, resolve-wait, merge-wait without the specified hook failure, successful archive/merge/push, and `ChangeSkipped` preserve marks.
9. Process-level `Stopped` and global fatal `Error` preserve the complete mark set so existing resume/retry controls remain valid.
10. After a change-level Error clears the old mark, an explicit supported re-mark creates fresh retry intent and existing F5/Start/retry routing remains unchanged.
11. Execution marks remain process-local, reset on process restart, and do not become workspace or Git authority.

## Explicit Completion Conditions

- One shared event-side policy derives mark revocation from typed event/reducer transition evidence and runs before frontend fan-out in every production orchestration dispatch path.
- The policy is table-tested for every failure family, rejection, rejected refresh, dequeue, legacy stop, `on_merged` hook failure, duplicate events, and all named preservation cases.
- TUI event handling synchronizes row `selected` values from `ExecutionMarkStore` and no longer has an unpaired local-only deselection path.
- A Web event-ownership regression proves the failure/rejection event and `execution_marked: false` share one revision while unrelated marks remain true.
- Cross-adapter tests prove explicit re-mark after Error remains actionable and process-level stop retains marks in both TUI and API.
- The commands declared by `execution-mark-event-regressions` pass.

## Out of Scope

- Persisting execution marks across process restart.
- Converting marks into durable queue, retry, blocker, or workspace evidence.
- Changing `Space`, F5, bulk mark, queue-hook, or retry admission semantics.
- Redesigning `RetryPlan.explicit_retry` or mixed Start behavior.
- Clearing marks on dependency block, stalled hold, ordinary MergeWait/ResolveWait, success completion, archive, merge, push, or process-level stop.
- Changing change-level versus global Error mode classification.
- Changing force-stop reducer reconciliation covered by `fix-force-stop-reducer-reconciliation`.
