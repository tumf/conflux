---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/cli.rs
  - src/main.rs
  - src/tui/runner.rs
  - src/tui/command_handlers.rs
  - src/tui/orchestrator.rs
  - src/parallel_run_service.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Enable TUI push post-archive mode

**Change Type**: implementation

## Premise / Context

- `cflx run --push [remote]` already maps CLI input to `PostArchiveAction::PushToRemote { remote }` for parallel run mode.
- Bare TUI startup (`cflx`) and explicit TUI startup (`cflx tui`) currently have no `--push` option.
- TUI parallel execution creates `ParallelRunService` in `src/tui/orchestrator.rs`, but does not call `set_post_archive_action`, so it always uses base-merge post-archive behavior.
- Remote TUI control currently starts server-side `cflx run` through a control API that does not carry push-mode options.
- The constitution requires workflow decisions to stay workspace-local and completion to be repository-verifiable.

## Requested Artifact

implementation

## Problem / Context

Operators can choose push post-archive behavior in non-interactive mode with `cflx run --parallel --push [remote]`, but the equivalent TUI workflow cannot be launched as `cflx --push` or `cflx tui --push`. This creates an inconsistent operator surface: selecting changes in the TUI still uses base-branch merge behavior after archive even when the desired workflow is to push each completed change branch to a remote.

The requested behavior is to make the TUI entrypoint accept the same push post-archive intent as run mode and carry that intent into local TUI parallel orchestration.

## Proposed Solution

Add `--push [remote]` to both bare TUI startup and the explicit `tui` subcommand, using the same remote parsing rules as `run --push`: omitted remote defaults to `origin`, and values containing `:` are rejected before orchestration starts.

Thread the selected post-archive action from CLI parsing through `main`, `tui::run_tui_with_remote`, `TuiCommandContext`, and `run_orchestrator_parallel`, then set it on `ParallelRunService` before execution. Preserve existing serial TUI behavior by only applying the action to parallel TUI execution.

Reject `--push` together with `--server` for TUI startup because the current remote control API does not propagate push-mode semantics to server-side runners. This avoids silent no-op behavior.

## Acceptance Criteria

- `cflx --push` is accepted as a bare TUI launch option and selects remote `origin` for local TUI parallel post-archive push mode.
- `cflx --push upstream` is accepted as a bare TUI launch option and selects remote `upstream`.
- `cflx tui --push` and `cflx tui --push upstream` provide the same behavior for explicit TUI launch.
- `cflx --push origin:main` and `cflx tui --push origin:main` are rejected before TUI orchestration starts with the same branch-selection error semantics as `run --push`.
- When local TUI parallel execution starts with push mode enabled, `ParallelRunService` receives `PostArchiveAction::PushToRemote { remote }` instead of `MergeToBase`.
- Local TUI serial execution remains unaffected by `--push`; no serial archive/push workflow is introduced.
- `--push` combined with TUI remote-server mode is rejected before orchestration starts rather than silently ignored.

## Explicit Completion Conditions

- `src/cli.rs` exposes `--push [remote]` for the top-level TUI default path and for `cflx tui`, sharing the same validation rules as `run --push`.
- `src/main.rs` converts parsed TUI push input to a `PostArchiveAction` and rejects unsupported `--server` combinations before launching the TUI.
- `src/tui/runner.rs`, `src/tui/command_handlers.rs`, and `src/tui/orchestrator.rs` pass the action into local parallel orchestration and call `ParallelRunService::set_post_archive_action` before `run_parallel_with_channel_and_queue_state`.
- Repository-verifiable tests prove CLI parsing/defaulting/rejection and TUI parallel service wiring; tests fail if the action is parsed but not applied to the parallel service.
- Existing `cflx run --push` behavior remains covered and unchanged.
- The proposal validates with strict OpenSpec validation and implementation-evidence warnings.

## Out of Scope

- Adding push post-archive behavior to serial TUI execution.
- Extending remote-server control APIs or server-side runners to accept push-mode options.
- Adding branch override syntax such as `origin:target`.
- Changing the existing push terminal behavior inside `ParallelRunService` or git push implementation.
