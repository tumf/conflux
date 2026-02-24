# Change: Refactor AiCommandRunner Streaming to Track Real Child Processes

## Problem / Context

In parallel execution, AI commands are executed via `AiCommandRunner::execute_streaming_with_retry()`.
The current implementation returns a *dummy* child process (`cat`/`findstr`) while the real command runs in a background task.

This breaks process ownership and lifecycle management:

- Termination and waiting often target the dummy process, not the real command.
- When a shell pipeline is used (e.g. `sh -c "cc-stream ... | cc-stream-filter"`), killing the shell can orphan pipeline children (e.g. `claude`, `jq`).
- Orphaned processes can stall the orchestration and prevent acceptance from starting.

## Proposed Solution

Refactor streaming execution so the orchestrator always owns and controls the *real* child process (or process group) for streaming commands.

Key changes:

- Replace the dummy-child design with a real `ManagedChild` representing the spawned command.
- Ensure cancellation/timeout terminates the full command tree (process group on Unix; job object on Windows).
- Preserve existing CommandQueue behaviors (stagger + retry + inactivity timeout) while making lifecycle control correct.
- Improve observability for timeouts and termination (PID/PGID, operation type, change_id, cwd).

## Acceptance Criteria

- Streaming commands return a handle to the real command process (not a dummy placeholder).
- Calling terminate/cancel on the streaming handle stops the entire command tree and does not leave orphaned pipeline processes.
- Inactivity timeout termination stops pipeline children (no `PPID=1` `claude`/`jq` leftovers).
- Existing retry semantics and stagger semantics remain correct.
- Tests cover cancellation/termination behavior for a pipeline command.

## Out of Scope

- Changing user-facing configuration format (beyond adding optional knobs if required for correctness).
- Rewriting external wrappers (`cc-stream`, `cc-stream-filter`).

## Impact

- Affected specs: `openspec/specs/command-queue/spec.md`
- Affected code:
  - `src/ai_command_runner.rs`
  - `src/command_queue.rs`
  - `src/process_manager.rs` (and platform-specific process-tree kill helpers)
  - Parallel execution paths that rely on streaming handles
