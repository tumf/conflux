## Context

`AiCommandRunner::execute_streaming_with_retry()` currently spawns the real command in a background task and returns a dummy child process. This design makes cancellation and cleanup unreliable, especially for shell pipelines where killing only the shell can orphan children.

## Goals

- Streaming execution returns a handle that controls the real child (or process group).
- Cancellation and inactivity timeouts terminate the whole command tree.
- Retry does not leak children between attempts.
- Keep behavior compatible with existing stagger/retry configuration.

## Non-Goals

- Replacing shell command strings with structured argv everywhere.
- Changing the external AI agent CLIs.

## Decision: Own the Real Child Process

We will refactor the streaming+retry API so it does not rely on a dummy placeholder process.

Two viable approaches:

1. **Refactor CommandQueue streaming-with-retry to return the real child** and drive retries in the caller.
2. **Move streaming retry orchestration into CommandQueue** but keep a single `ManagedChild` abstraction that always points to the current real child and can terminate the process group.

This change prefers (1) as it reduces indirection: spawning, streaming, and waiting occur in one place and can be correctly associated with the returned handle.

## Process Tree Termination

- Unix: create a new process group for the spawned command (`setpgid`) and terminate via `killpg`.
- Windows: attach the process to a job object with `KILL_ON_JOB_CLOSE` and terminate via job close.

The termination mechanism must work for `sh -c` pipelines so that children like `claude` and `jq` are not orphaned.

## Risks / Trade-offs

- Changing process management can be platform-sensitive; implement feature-gated unit tests and keep fallback behavior explicit.
- The retry loop must ensure previous attempt processes are fully terminated before spawning the next.

## Migration Plan

1. Introduce a new streaming API that returns a real-child `ManagedChild`.
2. Update call sites (parallel apply/acceptance) to use the new API.
3. Remove the dummy-child implementation once all call sites migrate.
