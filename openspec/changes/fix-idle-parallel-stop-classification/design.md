# Design: truthful parallel stop classification

## Decision

Use explicit process/execution activity evidence to choose between two operator-stop effects:

- `ActiveExecution`: cancel active work and retain managed child-process cleanup.
- `SchedulerOnly`: cancel the parallel scheduler/orchestrator without claiming process termination.

The classification is runtime-only and derives from existing in-memory scheduler, reducer, and registered execution-handle state. It must not become durable workflow evidence.

## Current Failure Path

1. First Esc sets `AppMode::Stopping`.
2. A change archives and enters `MergeWait`; no agent command remains active.
3. Second Esc sees only `AppMode::Stopping`, immediately records `ForceStopped`, applies `Stopped`, and logs `Force stopped`.
4. The outer `tokio::select!` observes global cancellation and constructs `OrchestratorError::AgentCommand`.
5. Cancellation is rendered as execution failure and error completion.

This conflates three independent facts: TUI lifecycle mode, scheduler task liveness, and child-process activity.

## Activity Evidence

The implementation should expose the narrowest existing runtime query that can answer whether cancellation targets active execution. Suitable evidence includes reducer in-flight state and registered per-change execution/cancellation handles. `MergeWait`, `ResolveWait`, queued-only state, deferred merge, and scheduler idle are not process activity.

The query must fail safe. If repository runtime state says work is in flight but handle inspection is temporarily unavailable, cancellation should retain the active cleanup path rather than risk orphaning a process. A known empty active set uses the scheduler-only path.

## Cancellation Outcome

The outer parallel orchestration boundary must carry operator cancellation as a distinct stopped/cancelled outcome. It must not encode cancellation in the text of `AgentCommand` and later recover it by string matching.

A cancellation outcome:

- may emit one cancellation/stopped diagnostic;
- must suppress execution-failure and normal-completion diagnostics;
- must not emit `AllCompleted`;
- remains idempotent when `Stopped` was already applied by the frontend.

A genuine command/service error remains unchanged and continues through the error-completion path.

## Compatibility

The active path retains current cancellation-token propagation and managed process-group cleanup. The proposal changes only the classification and truthful reporting of the scheduler-only case. Existing execution-mark preservation and transient queue reset remain authoritative in the current TUI stopped-state handlers.

## Verification Strategy

Keep tests under one discoverable `idle_parallel_stop` name family so the declared verification proves tests exist before running them. Use test doubles or reducer fixtures for activity state; do not spawn long-lived external agents. Every default-path test must remain below the repository one-second unit-test limit.
