## Context

The scheduler stores a `ReanalysisReason` across loop iterations. Explicit scheduler events replace it, but the ordinary 500 ms timer branch does not. This is safe only for debounceable reasons. `ResolveCompletion` bypasses debounce, so retaining it after its first evaluation converts a single completion edge into repeated level-triggered analysis.

The canonical parallel-execution behavior intentionally allows dependency analysis at zero capacity. Removing that behavior would regress queued work discovery during resolve. The correction must therefore consume the trigger, not broadly gate analysis on capacity.

## Decision

`ResolveCompletion` is a one-shot reason.

After the scheduler has offered that reason to one reanalysis/dispatch evaluation, subsequent loops must not reuse it unless `wait_for_scheduler_event` observes another qualifying completion. Timer-only wakes use the scheduler's ordinary debounceable reason.

The implementation should reset or take the reason at the scheduler-loop boundary where consumption is unambiguous. It should not add a second durable state machine or alter dependency-analysis results.

## Invariants

- Every real completion edge remains able to trigger immediate reanalysis.
- A completion edge is not replayed by timer wakes.
- Zero-capacity analysis remains legal, but ordinary apply dispatch remains capacity-gated.
- Slot recovery remains autonomous.
- Explicit queue additions and repair candidates retain their current bypass semantics.
- Runtime-only bookkeeping remains non-authoritative and Constitution-compatible.

## Rejected Alternatives

### Skip all analysis at zero capacity

Rejected because canonical behavior requires queued candidates to remain analyzable during active resolve, and existing tests encode that contract.

### Debounce `ResolveCompletion`

Rejected because a genuine completion can integrate dependency evidence or release capacity and must be evaluated immediately.

### Deduplicate analysis by queued-set signature

Rejected because the same queued set can require a new analysis after repository-visible dependency integration. Event consumption is the correct boundary; input-signature caching risks suppressing legitimate progress.

### Increase the timer duration

Rejected because it reduces frequency without removing the replay bug.

## Test Strategy

Use an analyzer test double with an invocation counter and controlled analysis result. Exercise scheduler-loop reason transitions rather than testing only diagnostic output. Prefer Tokio paused time or direct deterministic event-loop helpers so default tests remain under one second.
