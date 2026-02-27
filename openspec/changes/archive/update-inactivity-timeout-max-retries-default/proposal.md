# Change: Set Default Inactivity Timeout Max Retries to 3

## Problem/Context

Streaming command execution can be terminated by the inactivity timeout when stdout/stderr stays silent for the configured duration (default: 900s).
Today, retries after an inactivity timeout are disabled by default (`command_inactivity_timeout_max_retries = 0`). This leads to avoidable failures in cases where the command was still making progress but did not emit output.

## Proposed Solution

- Change the default value of `command_inactivity_timeout_max_retries` from `0` to `3`.
- Keep the option to disable retries by explicitly setting `command_inactivity_timeout_max_retries: 0` in config.

## Acceptance Criteria

- When `command_inactivity_timeout_max_retries` is not configured, the orchestrator retries a command that hit inactivity timeout up to 3 times.
- When `command_inactivity_timeout_max_retries: 0` is configured, no inactivity-timeout retry is attempted.
- Configuration parsing/merge behavior remains unchanged.

## Out of Scope

- Changing the inactivity timeout duration (e.g. 900s) or its kill grace behavior.
- Improving the correctness of inactivity timeout detection near command completion.

## Notes / Trade-offs

- Increasing the default retry count increases worst-case wall-clock time for truly stuck commands (e.g. up to ~4x for repeated timeouts).
- Retrying can re-run side-effectful commands; users can set retries to `0` if this is undesirable.
