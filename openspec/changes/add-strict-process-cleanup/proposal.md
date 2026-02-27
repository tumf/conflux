# Change: Strict cleanup of agent command process groups

## Why

Agent commands launched by cflx (e.g. via `apply_command`, `archive_command`, `acceptance_command`, `resolve_command`, `analyze_command`) can leave behind processes that remain visible in `ps` even after the orchestrator considers the command complete.

This causes resource leaks (CPU/memory, open files), confusing "stuck" behavior, and long-lived background servers/tasks that outlive the workflow.

The orchestrator MUST treat process lifecycle management as its responsibility: if it started a command, it must ensure that all processes it indirectly created are terminated when the command is done.

## What Changes

- Enforce a single lifecycle contract for agent command execution: each command runs in an isolated process group/session and is strictly cleaned up on completion.
- Perform post-completion cleanup for **all** outcomes (success/failure/cancellation/inactivity-timeout), not only for retries or cancellation paths.
- Add a configuration escape hatch to disable strict cleanup for advanced debugging scenarios.
- Add regression tests that reproduce the leak pattern ("command exits but backgrounds a child") and assert no surviving process-group members.

## Impact

- Affected specs:
  - `process-execution`
  - `configuration`
- Affected code (expected):
  - `src/ai_command_runner.rs` (streaming retry executor lifecycle)
  - `src/process_manager.rs` (process-group termination helpers)
  - `src/agent/runner.rs` and other agent execution call sites (ensure the unified cleanup path is used)
  - `src/config/mod.rs` (new configuration flag)
- Compatibility:
  - Default behavior becomes stricter: background processes started by an agent command will be terminated when the command completes.
  - This is intentional and aligns with “launcher owns cleanup”; users can opt out via config for debugging.
