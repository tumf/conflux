# Change: Improve Inactivity Timeout Diagnostics and Error Messages

## Problem / Context

Conflux intentionally terminates long-running commands when they produce no stdout/stderr for a configured interval ("inactivity timeout").

When this fires, the end user often sees errors like:

- `Apply failed ... exit code: None`

This is technically correct (a process terminated by signal may not have a numeric exit code), but it is not actionable. The current logs record that the inactivity timeout fired and that signals were sent, but they do not reliably provide enough context to diagnose *why* the command went silent (e.g., upstream LLM stall, pipeline filter blocking, orphaned subprocesses, etc.).

Additionally, if force-kill fails (e.g., `EPERM`), the user sees a confusing partial story: termination was attempted, but the reason and the observable state are not clearly summarized.

## Proposed Solution

Improve observability and user-facing error messages for inactivity timeouts.

Key changes:

- Log a structured, high-signal summary when inactivity timeout triggers (timeout seconds, grace seconds, operation, change id, cwd, pid/pgid, last-activity age).
- Log a structured summary for each termination step (SIGTERM / SIGKILL), including success/failure and errno when applicable.
- When a command is terminated by inactivity timeout, surface a clear error message that includes:
  - that the termination was *intentional* (timeout)
  - the configured timeout value
  - the operation/change id context
  - signal-termination semantics ("terminated by signal") instead of only `exit code: None`

## Acceptance Criteria

- When inactivity timeout triggers, logs contain enough information to answer: "what command was running, where, for what change/op, and how long since last output?"
- The user-facing error message for inactivity timeout is explicit and actionable (contains "inactivity timeout" plus timeout seconds and context), even when `ExitStatus.code()` is `None`.
- If SIGKILL fails (e.g., `EPERM`), the logs capture the failure in a structured way (signal, target, errno, context) and do not obscure the root cause.
- No behavior change to *when* inactivity timeout triggers (only diagnostics and messaging).

## Out of Scope

- Changing default timeout values.
- Changing the process management strategy (process groups / job objects) beyond logging/diagnostics.
- Refactoring external wrappers like `cc-stream` / `cc-stream-filter`.

## Impact

- Affected specs: `openspec/specs/command-queue/spec.md`
- Likely affected code:
  - `src/ai_command_runner.rs`
  - `src/command_queue.rs`
  - `src/process_manager.rs`
  - `src/tui/state.rs` (user-facing error formatting)
