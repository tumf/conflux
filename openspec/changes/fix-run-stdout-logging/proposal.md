---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/main.rs
  - src/vcs/commands.rs
  - src/vcs/git/commands/worktree.rs
  - tests/logs_command_tests.rs
  - openspec/specs/observability/spec.md
---

# Suppress DEBUG and TRACE logs from `cflx run` stdout

**Change Type**: implementation

## Premise / Context

- `cflx run` currently initializes runtime logging with stdout enabled in `src/main.rs`.
- The stdout tracing layer has no level filter, so internal `debug!` events and dependency `TRACE` events can appear in the terminal.
- Persistent file logging is observability output and must remain available for debugging without becoming workflow-control state.
- The constitution requires workflow decisions to remain workspace-local and completion to be repository-verifiable.

## Requested Artifact

implementation

## Problem / Context

Operators running `cflx run` can see noisy internal logs such as `DEBUG Executing git command: worktree list --porcelain` and `TRACE registering event source with poller` in normal terminal output. These logs obscure user-facing run progress and make the CLI harder to read.

The existing persistent log file should continue to capture detailed diagnostics for troubleshooting. The change should only reduce default stdout verbosity for `cflx run` and other stdout-enabled runtime paths that use the same logging initializer.

## Proposed Solution

Add an explicit stdout logging filter so the terminal-facing tracing layer emits only `INFO` and above by default, while the file logging layer continues to persist detailed diagnostics at its existing level.

The minimal expected implementation is in `src/main.rs`: apply a level filter to the stdout tracing layer inside `init_logging(true)` without changing the persistent file layer or the `logs` subcommand.

## Acceptance Criteria

- `cflx run` terminal output does not show internal `DEBUG` events by default.
- `cflx run` terminal output does not show dependency `TRACE` events by default.
- User-facing `INFO`, `WARN`, and `ERROR` runtime messages remain visible on stdout.
- Persistent Conflux log files continue to receive diagnostic entries at the existing detailed level.
- `cflx logs` remains read-only and does not initialize runtime logging.
- No new CLI flag or configuration option is added for this change.

## Explicit Completion Conditions

- `src/main.rs` constrains the stdout tracing layer to `INFO` or higher while leaving the file layer unchanged.
- A repository-verifiable test or equivalent focused check proves stdout filtering suppresses `DEBUG`/`TRACE` and preserves `INFO` output.
- Existing log viewer behavior is protected by the existing logs command tests or an equivalent targeted check.
- The proposal validates with strict OpenSpec validation and implementation-evidence warnings.

## Out of Scope

- Adding a new `--verbose`, `--quiet`, or logging configuration flag.
- Changing VCS command log classification from `debug!`.
- Removing or reducing persistent file logging.
- Changing TUI log-panel behavior.
