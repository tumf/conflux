# Change: Update CLI Hook Output Visibility

## Problem / Context

- The current hook implementation captures hook `stdout` and `stderr`, but normal `cflx run` does not guarantee that captured hook output is shown in the user-visible CLI log stream.
- TUI and parallel execution already route hook command/output logs through event-driven log views, which creates an observability gap between interactive and non-interactive execution.
- Existing specs describe hook execution and TUI log visibility, but they do not clearly require equivalent output visibility for normal CLI runs.

## Proposed Solution

- Require `cflx run` to emit hook command logs and captured hook output to the normal user-visible CLI log path for all hook types.
- Standardize CLI hook log ordering so users see the hook command first, then any captured output, then success/failure status.
- Require captured `stdout` and `stderr` to remain visible even when the hook later fails, with truncation explicitly marked when output must be shortened.
- Align CLI run behavior with existing TUI and parallel hook observability expectations without changing hook configuration semantics.

## Acceptance Criteria

- Running `cflx run` with a configured hook that writes to `stdout` shows that output in the CLI log stream.
- Running `cflx run` with a configured hook that writes only to `stderr` shows that output in the CLI log stream.
- Global hooks such as `on_start` and `on_finish` also surface their captured output in CLI mode, even when no `change_id` exists.
- If a hook exits non-zero, any captured output is still shown before the failure result is reported.
- If hook output is truncated for display, the CLI log explicitly indicates truncation.

## Out of Scope

- Changing hook configuration format, placeholders, or timeout semantics.
- Redesigning the broader CLI logging system outside hook-related command/output visibility.
