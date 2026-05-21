---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/runner.rs
  - src/tui/key_handlers.rs
  - src/tui/command_handlers.rs
  - src/execution/apply.rs
  - src/ai_command_runner.rs
  - src/parallel/orchestration.rs
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/process-execution/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Fix TUI Exit Cancelling Local Agent Work

**Change Type**: implementation

## Problem / Context

Users have observed that after running `cflx tui`, local agent work can continue even after the TUI has been exited. The current local TUI cleanup path cancels the TUI-level cancellation tokens and waits briefly for the orchestrator task, but if that task does not finish within the grace period it only logs a warning and drops the join handle. A dropped Tokio `JoinHandle` detaches the task rather than stopping it, so local orchestration and child agent commands can continue after the visible TUI session is gone.

This is dangerous for an agent orchestrator because the operator reasonably expects exiting the local TUI to stop the work that was launched from that TUI session. The fix must preserve the existing remote TUI semantics: closing a remote `cflx tui --server ...` client must not implicitly stop server-side work.

The change must also respect the Conflux constitution: workflow control remains derived from repository-visible workspace/git state, and cancellation bookkeeping must remain runtime-only rather than becoming durable workflow-control state.

## Proposed Solution

Make local TUI shutdown a decisive cancellation boundary for work launched by that TUI session:

- Treat local TUI quit as force-stop-equivalent for local orchestration tasks started from the TUI.
- Cancel the orchestrator cancellation token and ensure the local orchestrator `JoinHandle` cannot outlive TUI cleanup; abort it after a bounded grace period if it does not finish.
- Ensure active streaming agent commands observe cancellation while output is being drained or while waiting for child completion, and terminate the associated process group through existing process cleanup mechanisms.
- Ensure parallel execution observes cancellation and does not leave scheduler/workspace tasks or agent child processes running after local TUI exit.
- Keep remote TUI quit read-only with respect to remote execution: it closes local UI/subscription tasks but does not send stop/force-stop commands to the remote server unless the user explicitly requests them.

## Acceptance Criteria

- Exiting local `cflx tui` while local orchestration is active cancels the local orchestrator and prevents it from continuing work after TUI cleanup finishes.
- If the local orchestrator task does not finish within the bounded TUI cleanup grace period, the task is aborted and no detached local orchestration task remains.
- Active AI command streaming reacts to cancellation while waiting for output or child completion and terminates the process group using the existing SIGTERM/SIGKILL cleanup path on Unix.
- Parallel execution cancellation propagates to in-flight workspace tasks and prevents new ordinary work dispatch after local TUI shutdown begins.
- Remote `cflx tui --server ...` exit only tears down the local TUI client, WebSocket subscription, auto-refresh, and rendering loop; it does not implicitly stop remote server-side work.
- New verification covers local TUI cleanup, streaming command cancellation, parallel cancellation propagation, and remote quit non-stop behavior.

## Explicit Completion Conditions

This change is complete only when repository evidence shows all of the following:

- `src/tui/runner.rs` local cleanup cancels local orchestrator work and aborts any still-running local orchestrator task after a bounded grace period.
- Local quit handling in `src/tui/key_handlers.rs` or the TUI cleanup path has clear force-stop-equivalent semantics for local active work.
- Streaming command loops in `src/execution/apply.rs` and relevant archive/acceptance/resolve execution paths observe cancellation while waiting for output or child status and call `StreamingChildHandle::terminate()` or equivalent process-group cleanup.
- Parallel scheduler/executor cancellation paths stop in-flight work and prevent post-cancel dispatch.
- Remote mode TUI shutdown does not call remote stop/force-stop control endpoints implicitly.
- Unit/integration tests exercise cancellation without relying on real AI agents; tests use stub commands or fixtures and keep default tests under the repository's one-second guidance, using `heavy-tests` only where unavoidable.
- `cargo fmt`, targeted Rust tests, and OpenSpec strict validation pass.

## Out of Scope

- Changing repository-visible resume or archive routing semantics.
- Introducing durable cancellation state under `~/.local/state/cflx` or any other out-of-worktree location.
- Automatically stopping remote server-side work when a remote TUI client exits.
- Replacing the existing process-group cleanup implementation beyond what is needed to make TUI shutdown cancellation reach it.
