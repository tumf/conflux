## Context

The scheduler currently has two representations of admitted work: reducer-owned queue intent and a scheduler-local candidate vector. Failed-dependency filtering removes a candidate only from the local vector. Reconciliation then truthfully restores it from reducer intent, but misclassifies that restoration as a new queue edge. The analyzer suppression layer correctly allows explicit queue edges to bypass a matching signature, so fixing suppression globally would damage real dynamic queue behavior.

## Decision

Represent a failed-dependent change as stable blocked queued work rather than as repeatedly skipped-and-removed work.

- Reducer queue intent remains authoritative.
- Scheduler-local candidates retain blocked entries.
- Dispatch classification excludes blocked entries.
- Reconciliation only reports an addition when an ID was not already represented.
- A failed-dependency transition emits bounded observability keyed by dependent ID and failed dependency ID.
- Explicit retry clears the dependency's ephemeral failed marker and arms one reevaluation, but normal repository evidence remains authoritative for dependency resolution.

## State Transitions

1. A fails: mark A failed.
2. B depends on A: retain B in queued candidates, classify B as dependency-blocked, emit one skip/block observation.
3. Scheduler wakes without state change: retain the same classification, emit no queue addition, analyzer attempt, or duplicate skip observation.
4. Operator explicitly retries A: reducer accepts retry, clear A's ephemeral failed marker, and arm one queue/retry edge.
5. A remains unresolved: dependency evidence keeps B blocked.
6. A succeeds and becomes resolved: normal dependency checks allow B to enter dispatch selection.
7. A fails again: mark A failed again and establish a new bounded blocker transition for B.

## Alternatives Rejected

### Suppress every matching `QueueNotification`

This would hide the loop but delay genuine newly queued work, contradicting the existing one-edge immediate-analysis contract.

### Remove reducer queue intent for B

This silently discards operator intent and prevents automatic recovery after A is retried and resolved.

### Deduplicate only the log

This leaves analyzer invocations, reconciliation churn, repository probes, and resource consumption unchanged.

### Keep B blocked forever after A fails

`FailedChangeTracker` is ephemeral process state. Without a clear transition, an accepted same-process retry cannot recover the dependency graph.

## Verification Strategy

Use paused Tokio time and the real scheduler loop. A scripted analyzer and event collector must prove invocation and diagnostic counts, not merely inspect helper return values. Focused unit tests cover tracker transitions and reconciliation accounting. Integration tests cover mixed blocked/independent work and retry outcomes.
