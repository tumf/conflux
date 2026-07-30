## Context

The TUI currently owns orchestration-facing effects that are not intrinsically terminal UI behavior. Queue mutation, cancellation, reducer ordering, hooks, and retry decisions must remain identical when another frontend is added.

## Goals / Non-Goals

### Goals

- One process-local command service for TUI and remote adapters.
- Reducer-owned workflow state and EventSink-owned notifications.
- Cancellation completion before active dequeue.
- Exact queue hook cardinality.
- Existing acceptance-stall reconciliation and retry routing.

### Non-Goals

- A durable command journal or operator-state database.
- Network protocol design.
- Replacing the reducer or execution event model.

## Decisions

### Decision: frontends adapt; the service decides

TUI key handlers map user intent to typed operator commands. They do not independently apply reducer transitions or duplicate lifecycle matrices. The future HTTP layer performs the same mapping.

### Decision: state axes stay separate

An execution mark answers whether the operator wants a change considered at the next applicable boundary. Queue intent answers whether it belongs to the dynamic pending set. Activity and hold/terminal fields answer what Core is doing or why it cannot proceed. `display_status()` remains a projection of authoritative runtime state.

Marks are ephemeral UI/operator state. They are not written outside the workspace and cannot control restart routing.

### Decision: side effects use prepare, effect, commit ordering

For queue mutation, the service validates first, executes the real `DynamicQueue` operation, then applies the matching reducer transition and hook/event effects only when the operation changed state.

For active stop-and-dequeue, cancellation is different:

```text
validate target is active
  -> find cancellation handle
  -> request cancellation
  -> await task/process termination within bound
  -> apply DequeueChange
  -> emit result/event
```

A missing token, cancellation error, or timeout returns failure and leaves the active reducer state unchanged.

### Decision: queue hooks describe actual dynamic mutations

`on_queue_add` and `on_queue_remove` run after successful runtime queue changes. Duplicate operations and initial queue construction are not dynamic mutations and produce no hook.

### Decision: retry remains evidence-aware

Terminal errors use the reducer retry command. Acceptance-stalled retry first reconciles `load_valid_acceptance_stall()` and uses the existing `explicit_retry` path so a valid hold resumes acceptance after workspace preparation. It does not infer completion or rerun apply. Invalid records fail safe and retain repository-derived behavior.

## Risks / Trade-offs

- Moving effects can expose hidden TUI ordering assumptions. Adapter parity tests cover both entry paths.
- Waiting for termination adds bounded latency to dequeue. Returning success early would be unsafe, so timeout is explicit failure.
- Process-local marks disappear at restart by design; repository/worktree evidence continues to determine workflow routing.
