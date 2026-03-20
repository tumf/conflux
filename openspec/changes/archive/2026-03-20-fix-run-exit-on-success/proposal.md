# Change: Fix `cflx run` exit after successful completion

## Problem/Context

- `cflx run` can log successful completion and remain alive instead of exiting.
- The current run-mode control loop in `src/main.rs` keeps waiting for restart or stop signals after a successful orchestration result.
- With `--web`, additional background tasks exist for HTTP serving and refresh/control handling, so successful completion must close run-scoped tasks cleanly rather than leaving the process parked in the Tokio runtime.

## Proposed Solution

- Update run-mode lifecycle behavior so a successful orchestration result causes `cflx run` to exit promptly with status code 0.
- Preserve explicit stop and error handling behavior separately from normal success completion.
- Define cleanup expectations for run-scoped background tasks, including web-monitoring tasks started by `cflx run --web`.
- Add regression coverage for both plain `cflx run` success and `cflx run --web` success.

## Acceptance Criteria

- When orchestration completes successfully with no remaining work, `cflx run` exits promptly without waiting for an external signal.
- When `cflx run --web` completes successfully, the process also exits promptly and does not remain alive due to web-monitoring tasks.
- The successful path still logs completion before exiting.
- Existing web retry/stop controls for non-success states are not expanded by this change.

## Out of Scope

- Redesigning run-mode retry semantics after orchestration errors.
- Changing TUI lifecycle behavior.
- Introducing new user-facing CLI flags for lifecycle control.
