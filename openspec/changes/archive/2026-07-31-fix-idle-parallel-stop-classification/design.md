# Design: truthful parallel stop classification

## Decision

Take one runtime stop snapshot and derive two orthogonal decisions:

- `ProcessReport`: whether registered or reducer-visible in-flight execution justifies `Force stopped` and managed child-process termination reporting.
- `ShutdownBarrier`: whether active execution, a pending background merge/base-lane mutation, or scheduler work must reach its existing safe cancellation boundary before terminal `Stopped`.

A known-empty execution-handle and reducer-active set produces ordinary stop reporting. A pending background merge keeps the shutdown barrier active but never justifies an agent-process force-stop claim. Both reporting classes use the same global cancellation mechanism; the classifier controls reporting and waiting, not whether cleanup runs.

The snapshot is runtime-only and derives from existing in-memory scheduler, reducer, registered execution-handle, and pending-merge state. It must not become durable workflow evidence.

## Current Failure Path

1. First Esc sets `AppMode::Stopping`.
2. A change archives and enters `MergeWait`; no agent command remains active.
3. Second Esc sees only `AppMode::Stopping`, immediately records `ForceStopped`, applies `Stopped`, and logs `Force stopped`.
4. The outer `tokio::select!` observes global cancellation and constructs `OrchestratorError::AgentCommand`.
5. Cancellation is rendered as execution failure and error completion.

This conflates three independent facts: TUI lifecycle mode, scheduler task liveness, and child-process activity.

## Activity Evidence

The implementation must expose one shared runtime snapshot consumed by both second-Esc and `TuiCommand::ForceStop`. The snapshot combines reducer in-flight activity, registered per-change execution handles, and pending background merge/base-lane work under one conservative decision rule. The synchronous key handler may enqueue the shared force-stop command rather than duplicating asynchronous state inspection.

`MergeWait`, `ResolveWait`, queued-only state, deferred merge, and scheduler idle are not process activity. `pending_merge_count > 0` or an occupied base-mutating lane is shutdown activity but not agent-process activity.

The snapshot must fail safe. Any positive execution signal or unavailable execution-handle inspection selects managed active cleanup and force-stop reporting. Only a known-empty execution set permits ordinary stop reporting. Any positive or unavailable shutdown-work signal keeps the shutdown barrier active until cleanup completes or bounded escalation finishes.

## Cancellation Outcome

The outer parallel orchestration boundary must carry operator cancellation as a distinct stopped/cancelled outcome. It must not encode cancellation in the text of `AgentCommand` and later recover it by string matching.

The outer boundary must pin and poll the scheduler future after the cancellation token fires instead of selecting cancellation by dropping that future. Terminal cancellation is selected only after the inner scheduler has observed cancellation, aborted and drained workspace tasks, released registered execution handles, received or safely bounded pending merge/base-lane outcomes, and dropped its workspace guard. If that barrier exceeds its bounded deadline, escalation continues through managed process cleanup while the outcome remains operator cancellation rather than execution failure.

A cancellation outcome:

- may emit one cancellation/stopped diagnostic;
- must suppress execution-failure and normal-completion diagnostics;
- must not emit `AllCompleted`;
- remains idempotent when `Stopped` was already applied by the frontend.

The first transition into `AppMode::Stopped` owns `Processing stopped`. A repeated or late `Stopped` event may reconcile state but must not append another terminal stop message.

A genuine command/service error remains unchanged and continues through the error-completion path.

## Compatibility

The active path retains current cancellation-token propagation and managed process-group cleanup. The proposal changes only the classification and truthful reporting of the scheduler-only case. Existing execution-mark preservation and transient queue reset remain authoritative in the current TUI stopped-state handlers.

## Verification Strategy

Keep tests under one discoverable `idle_parallel_stop` name family so the declared verification proves tests exist before running them. Use test doubles or reducer fixtures for activity state; do not spawn long-lived external agents. Every default-path test must remain below the repository one-second unit-test limit.
