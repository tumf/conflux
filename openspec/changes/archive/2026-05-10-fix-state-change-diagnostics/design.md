# Design: State-change driven scheduler diagnostics

## Background

Conflux already allows observability-layer dedupe/rate-limiting for repeated scheduler diagnostics. That is not enough for this issue. The scheduler should not manufacture the same diagnostic event repeatedly when nothing relevant changed.

The design goal is to move suppression closer to the event source while preserving the constitutional rule that workflow-control decisions come from workspace and git state, not from caches or logs.

## State model

The scheduler may keep in-memory diagnostic observation state such as:

- last dependency blocker fingerprint per change id
- whether a change has emitted a blocked diagnostic for that exact fingerprint
- whether a corresponding resolved diagnostic has already been emitted after the last blocked fingerprint
- optional last worktree/merge-wait observation fingerprints for diagnostic/log emission only

This state is diagnostic emission state. It is not workflow state.

## Dependency blocker fingerprint

A dependency blocker fingerprint should be stable and comparable. It should include enough information to distinguish materially different blocker states:

- blocked change id
- sorted unresolved dependency ids
- dependency target class for each unresolved dependency, such as queued, in-flight, missing, rejected, or archived/misclassified transitions where applicable
- error classification when dependency evaluation fails

Sorting is preferred so equivalent dependency sets do not produce false changes due to iteration order.

## Event emission rules

- If the current blocker fingerprint equals the last emitted blocker fingerprint for the change, do not emit another `DependencyBlocked` event or user/debug diagnostic.
- If the current blocker fingerprint differs from the last emitted fingerprint, emit a new blocked diagnostic/event and replace the remembered fingerprint.
- If a change with a remembered blocker fingerprint becomes unblocked, emit one `DependencyResolved`, clear the remembered blocker fingerprint, and mark the resolved transition complete.
- If a later loop still sees the change unblocked, do not emit another resolved event.
- If the change becomes blocked again after resolution, it is a new transition and may emit again.

## TUI defensive behavior

The TUI should not rely solely on scheduler correctness. It should avoid appending duplicate user-visible logs when a duplicate event does not cause a display state transition.

For example, receiving `DependencyBlocked` while the row is already displayed as `blocked` should update no visible state and append no duplicate log. Receiving `DependencyResolved` while the row is not displayed as `blocked` should likewise avoid a misleading duplicate log.

## Constitution compliance

Suppression state must not be persisted. It must not be consulted to decide whether a change is executable, should resume, should archive, should accept/reject, or should route to a next action.

Deleting `~/.local/state/cflx/**` must not change the next action selected for the same workspace and git state.

## Verification strategy

Primary verification should be unit/integration tests around the scheduler and TUI state handlers:

- repeated same dependency blocker emits once
- changed dependency blocker emits again
- blocked to resolved emits one resolution
- repeated resolved scans are silent
- TUI duplicate events are log no-ops when display state does not transition

A manual run may be used to demonstrate bounded log growth, but it is not a substitute for repository tests proving the transition behavior.
