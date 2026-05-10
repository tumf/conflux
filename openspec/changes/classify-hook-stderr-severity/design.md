# Design: Hook Stderr Severity Classification

## Context

Many command-line tools write progress, dependency resolution, or non-fatal diagnostics to stderr while still exiting successfully. Treating all captured stderr as warning-level Conflux output makes successful lifecycle hooks appear problematic.

## Constraints

- Hook stdout/stderr must remain observable.
- Failed hooks must preserve stderr diagnostics for debugging.
- `on_merged` failure behavior must remain a real merge blocker when configured to fail closed.
- Logs and UI state are observability only and must not become workflow-control inputs.

## Approach

Thread hook execution outcome into the hook-output emission path:

1. Capture stdout/stderr as today.
2. When the hook exits successfully, emit stdout and stderr as informational hook output.
3. When the hook exits unsuccessfully or execution errors, emit captured stderr as warning/error context before applying retry and `continue_on_failure` semantics.
4. Keep truncation markers and output size limits unchanged.

## Trade-offs

This preserves potentially useful stderr output while reducing false warning noise. Operators still see successful-hook stderr, but it no longer implies a Conflux warning or required action.
