## Context

The scheduler stores a `ReanalysisReason` across loop iterations. Explicit scheduler events replace it, but the ordinary 500 ms timer branch does not. This is safe only for debounceable reasons. `ResolveCompletion`, `RepairCandidate`, and `SlotRecovery` bypass debounce, so retaining any of them after its first evaluation converts a state-transition edge into repeated level-triggered analysis.

The canonical parallel-execution behavior intentionally allows dependency analysis at zero capacity. Removing that behavior would regress queued work discovery during resolve. The correction must therefore consume edge-triggered reasons, not broadly gate analysis on capacity.

`should_reanalyze(true)` is unconditional in the current implementation, so an edge-triggered bypass reason offered to a queued reanalysis block starts analysis. Automatic resolve capacity is counted through an RAII guard; the guard decrements on every exit path, while merge/workspace result channels and the bounded timer/debounce loop provide scheduler wake/re-evaluation paths. The implementation must audit these paths rather than relying on a sticky reason as an accidental liveness mechanism.

## Decision

`ResolveCompletion`, `RepairCandidate`, and `SlotRecovery` are one-shot reasons. `QueueNotification` keeps its existing candidate-addition reconciliation/reset behavior because it has separate logic that distinguishes a real addition from a wake without one.

The scheduler loop passes the current reason by value into `perform_reanalysis_and_dispatch`. After the queued reanalysis/dispatch evaluation returns, the loop replaces an unchanged one-shot reason with `Initial`. `Initial` is the existing non-bypass state for iterations after the first, so no new enum variant or state machine is needed.

Consumption belongs to the loop-owned `reanalysis_reason`, not inside `perform_reanalysis_and_dispatch`. This preserves the helper's direct-call semantics and existing tests that intentionally invoke it repeatedly with a supplied reason.

A loop that skips the queued reanalysis/dispatch block does not consume the reason at that point. A later explicit queue/reconciliation edge may replace it with its own reason; otherwise the next eligible queued evaluation can consume it. The implementation must test this queued-empty boundary rather than infer it.

## Capacity-Recovery Audit

Implementation must enumerate every path that can reduce scheduler-accounted occupancy or pending work, including:

- workspace task completion and `join_set` delivery;
- manual resolve completion and scheduler notification;
- automatic conflict/merge resolution RAII counter release on success, failure, and early return;
- background merge result delivery for merged, deferred, and failed outcomes;
- pending merge counter decrement;
- deferred resolve/reject wait retry completion;
- slot transition from zero to positive detected by `calculate_available_slots` / `last_available_slots`.

For each distinct path, repository evidence must prove one of:

1. it produces an explicit scheduler event that re-arms immediate evaluation;
2. it is observed by `SlotRecovery`; or
3. queued work remains eligible for bounded timer/debounce re-evaluation without permanent starvation.

The audit may reveal a narrowly missing wake edge. Any correction must remain in runtime scheduler signaling and be covered locally; it must not introduce durable workflow state.

## Invariants

- Every real completion, repair-candidate, or slot-recovery edge remains able to trigger immediate reanalysis.
- An edge is consumed only after the loop actually evaluates queued work with that reason.
- An edge is not replayed by timer wakes.
- Zero-capacity analysis remains legal, but ordinary apply dispatch remains capacity-gated.
- Capacity recovery remains autonomous on success, failure, deferred, and early-return paths.
- Explicit queue additions retain their current bypass semantics.
- `perform_reanalysis_and_dispatch` and direct-call tests do not acquire hidden event-lifetime behavior.
- Runtime-only bookkeeping remains non-authoritative and Constitution-compatible.

## Rejected Alternatives

### Skip all analysis at zero capacity

Rejected because canonical behavior requires queued candidates to remain analyzable during active resolve, and existing tests encode that contract.

### Debounce completion, repair, or slot-recovery edges

Rejected because a genuine state transition can integrate dependency evidence or release capacity and must be evaluated immediately once.

### Deduplicate analysis by queued-set signature

Rejected because the same queued set can require a new analysis after repository-visible dependency integration. Event consumption is the correct boundary; input-signature caching risks suppressing legitimate progress.

### Consume inside `perform_reanalysis_and_dispatch`

Rejected because the helper receives the reason by value and is directly tested as a single evaluation. Trigger lifetime belongs to the scheduler loop that owns the mutable reason.

### Add a new consumed/timer enum variant

Rejected because `Initial` already behaves as a non-bypass reason after iteration one. A new variant adds state without changing required behavior.

### Increase the timer duration

Rejected because it reduces frequency without removing the replay bug.

## Test Strategy

Use an analyzer test double with an invocation counter and controlled analysis result. Exercise the scheduler loop or a deterministic loop-owned transition helper, not only direct calls to `perform_reanalysis_and_dispatch` and not only diagnostic output.

Use Tokio paused time where timer behavior is involved. Required cases:

- one completion edge, zero capacity, and multiple 500 ms timer advances produce exactly one analysis;
- a second completion edge re-arms analysis and capacity recovery dispatches queued work;
- a queued-empty iteration does not consume an edge before a later eligible evaluation;
- one repair-candidate edge followed by timer advances does not repeat analysis;
- slot recovery remains immediate and one-shot;
- zero-capacity first analysis still retains queued work and suppresses apply;
- success, deferred/failure, and early-return capacity-release mechanisms remain live according to the capacity-recovery audit.

Default tests must remain under one second; tests that cannot practically meet that limit must use the repository heavy-test policy.
