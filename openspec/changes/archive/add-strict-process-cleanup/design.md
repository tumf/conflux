## Context

cflx executes agent commands through a shell (`sh -c ...`) and relies on process-group/session isolation to prevent job-control stalls and to support cancellation.

In practice, some commands can exit while leaving background processes running (e.g. `cmd & exit 0`). When this happens, the orchestrator currently may stop monitoring after the parent exits and fail to terminate the remaining group members on the success path.

## Goals / Non-Goals

### Goals

- cflx owns and enforces full lifecycle cleanup for agent command executions.
- No processes from the spawned command's process group remain after completion.
- Behavior is tool-agnostic (no special casing for `opencode`/`claude`).
- Works consistently across serial, parallel, and TUI modes.

### Non-Goals

- Killing processes that intentionally escape supervision by creating a new session/process group after spawn.
- Adding tool-specific shutdown logic (HTTP dispose calls, agent-specific flags).

## Decisions

### 1) Strict post-completion cleanup

Decision: Always run a post-completion cleanup sequence on the isolated process group/session created for the command, regardless of exit status.

Rationale:
- Prevents orphaned background work.
- Aligns with "launcher owns cleanup" and avoids per-agent special casing.

### 2) Cleanup sequence

Decision: Use a cooperative-then-forced sequence:

1. Send SIGTERM to the process group.
2. Sleep briefly (configurable, small).
3. Send SIGKILL to the process group.
4. Optionally verify absence via `killpg(pgid, 0)` (Unix).

Rationale:
- SIGTERM allows graceful shutdown.
- SIGKILL ensures termination of stubborn processes.
- Verification enables deterministic regression tests.

### 3) Configuration escape hatch

Decision: Add `command_strict_process_cleanup` (default: true). When false, keep best-effort cleanup on cancellation/timeout but do not enforce post-completion cleanup on success.

Rationale:
- Allows debugging workflows where the agent intentionally starts long-running background work.

## Risks / Trade-offs

- Stricter default behavior may terminate background processes that some users expected to persist.
  - Mitigation: documented opt-out config.

- Some tools may daemonize (create a new session). Those processes can escape PGID-based cleanup.
  - Mitigation: log a warning when verification indicates survivors, and keep Future Work open for optional process-tree traversal.

## Migration Plan

1. Add config flag and wire it into the execution layer.
2. Add reusable cleanup helper in the process management module.
3. Apply cleanup helper to all execution paths.
4. Add regression tests.

## Open Questions

- None.
