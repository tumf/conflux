---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/hooks/spec.md
  - src/hooks.rs
  - src/tui/state/event_handlers/errors.rs
---

# Classify Hook Stderr Severity

**Change Type**: implementation

## Problem / Context

Conflux currently surfaces captured hook `stderr` through warning-level UI/log events even when the hook command exits successfully. That makes normal tool diagnostics written to `stderr` look like a Conflux warning even though the hook outcome was successful and no workflow action is required.

This is not a merge/resolve/retry failure by itself. Manual merge markers and scheduler retry markers remain separate workflow signals, and this change must not use logs or UI state as workflow-control inputs.

## Proposed Solution

Make hook output severity reflect the hook outcome:

- Successful hook executions may still surface captured `stderr`, but should classify it as informational hook output rather than a warning/failure.
- Failed hook executions must continue to surface captured `stderr` as warning/error context before the hook failure is reported.
- Existing `on_merged` failure behavior must remain blocking when `continue_on_failure` is false.
- Output truncation markers and stdout visibility must remain unchanged.

## Acceptance Criteria

- A hook that writes to `stderr` and exits zero does not create a warning-level TUI/log entry solely because stderr was non-empty.
- A hook that writes to `stderr` and exits non-zero still exposes that stderr in warning/error context before the failure is reported.
- `on_merged` hook failures still block the merged transition according to existing hook configuration semantics.
- Hook output visibility remains available in CLI/TUI observability without becoming workflow-control state.

## Explicit Completion Conditions

- `src/hooks.rs` or a helper it calls classifies stderr output using hook success/failure outcome rather than stderr presence alone.
- Regression coverage proves successful stderr is informational and failing stderr remains warning/error-visible.
- Existing hook output truncation tests still pass.
- Existing `on_merged` failure tests in hook and TUI event handling paths still pass.

## Out of Scope

- Changing hook execution order or `continue_on_failure` semantics.
- Suppressing hook stderr entirely.
- Changing merge, resolve, or scheduler retry routing.
- Introducing durable state outside the workspace/git/base-tree model.
