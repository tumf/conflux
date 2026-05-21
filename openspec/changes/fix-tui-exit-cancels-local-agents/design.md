# Design: TUI Exit Cancels Local Agent Work

## Current Behavior

Local TUI startup creates a TUI-scoped cancellation token and starts local orchestrator work only after user action. At TUI shutdown, the runner cancels the TUI token and the orchestrator cancellation token, aborts the auto-refresh task, and waits up to a short grace period for the orchestrator task. If the orchestrator task is still running after that wait, the current cleanup path logs a warning but does not abort the task.

Because Tokio join handles detach tasks when dropped, a timed-out local orchestrator task can continue to run. If that task owns or waits on agent command streams, the underlying agent command can continue modifying worktrees after the TUI process appears to have ended from the operator's perspective.

## Design Goals

- Local TUI exit must be a decisive cancellation boundary for local work launched by that TUI.
- Remote TUI exit must remain a client disconnect, not a remote stop command.
- Cancellation must reach the process-group cleanup path already used by `AiCommandRunner` and `ManagedChild`.
- Cancellation must remain runtime-only and must not affect repository-visible workflow routing decisions except through actual workspace/git changes produced before cancellation took effect.

## Proposed Architecture

### 1. Local cleanup helper

Introduce or refactor a runner-level helper around local orchestrator cleanup. The helper should accept the local orchestrator handle, its cancellation token, and mode information. On local mode shutdown it should:

1. cancel the orchestrator token;
2. wait a bounded grace period for natural cleanup;
3. if the task is still running, abort the join handle;
4. await or poll the aborted handle enough to avoid silently detaching it;
5. emit an operator-visible log/tracing message that distinguishes graceful completion from forced abort.

The helper should be unit-testable without a real terminal by using controlled futures/channels.

### 2. Remote mode separation

Remote mode already uses the remote client to issue explicit control commands when users press start/stop controls. Shutdown of the TUI client should not reuse local force-stop logic against the remote server. The runner should keep remote subscription and local bridge tasks tied to the TUI cancellation token, but it must not call remote stop endpoints during ordinary quit cleanup.

### 3. Streaming cancellation propagation

The active agent command handle must be terminated when cancellation is observed during long awaits. The apply loop already receives a cancellation token but checks it mainly at iteration boundaries. The implementation should select on cancellation while:

- waiting for output lines;
- waiting for completion-grace deadlines;
- draining or waiting for child completion when possible.

When cancellation wins, call the existing `StreamingChildHandle::terminate()` path and return a cancellation error/result instead of continuing to retry or moving to the next stage.

The same pattern should be shared or mirrored for archive, acceptance, resolve, and analysis paths that stream or wait on child commands.

### 4. Parallel cancellation propagation

Parallel execution passes a cancellation token into the parallel service. The implementation must ensure cancellation is checked before dispatching new ordinary work, while waiting for scheduler notifications, and in workspace tasks that own child command handles. In-flight tasks should terminate their active children before reporting stopped/cancelled state.

### 5. Verification strategy

Prefer stub commands and controlled futures over real AI agents. The most important regression test is not visual TUI rendering; it is that cleanup cannot drop a live join handle and allow it to send later events or start later work.

Tests that require real process groups or longer timing should be minimized. If a test cannot reliably run under one second, mark it behind `heavy-tests` according to repository policy.

## Alternatives Considered

### Only increase the cleanup timeout

Rejected. A longer timeout reduces frequency but does not fix the leak, because the task is still detached if it misses the timeout.

### Kill all matching agent processes from TUI shutdown

Rejected as a primary mechanism. It risks killing unrelated processes. The implementation should use owned child handles and process groups created by Conflux.

### Make TUI quit leave local work running intentionally

Rejected for local mode. Local TUI work was launched from the interactive session, and hidden continuation violates operator expectations. Remote mode remains the explicit case where client quit does not imply server stop.
