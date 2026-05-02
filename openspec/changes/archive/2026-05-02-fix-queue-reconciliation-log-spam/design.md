# Design: Queue reconciliation diagnostic deduplication

## Current behavior

The scheduler loop calls `reconcile_queued_candidates_from_shared_state()` every iteration before drain and analysis decisions. That helper reads reducer-visible queued intent and attempts to add eligible changes into the scheduler-local `queued` vector.

When a queued intent is already active or in-flight, the helper correctly skips adding it. The current user-visible log emission is unconditional for that skip reason, so stable active/in-flight state produces repeated identical TUI log entries.

## Design goals

- Preserve queue reconciliation and duplicate-dispatch prevention exactly.
- Make stable healthy conditions quiet enough for TUI use.
- Keep diagnostics available for debugging stuck queued work.
- Avoid using logs, caches, or UI state as workflow-control input.

## Recommended implementation shape

Add a small runtime-only diagnostic suppression layer for queue reconciliation emissions. It can live inside `ParallelExecutor` or an adjacent helper owned by the scheduler.

A minimal shape is:

- key: `(change_id, reason)`
- value: last emitted instant and suppressed count, or only last emitted key when consecutive-only suppression is sufficient
- decision: allow first emission, suppress repeated emissions until state changes or a configured/internal summary interval expires

The scheduler must continue to evaluate active/in-flight status and candidate load status from current reducer and OpenSpec state on every loop. The dedupe layer must only decide whether to emit a log event.

## Reason handling

`already_active` is a normal transient state and should be the quietest reason. It may be emitted once per change/reason and then only in a summary.

`candidate_not_found` is more actionable because it can explain why queued work cannot be analyzed. It should remain user-visible on first observation and may be rate-limited afterward.

Other no-analysis reasons may continue using existing diagnostics unless they produce the same visible flooding pattern. If implementation generalizes dedupe to all queue reconciliation reasons, tests should cover the two known important classes: stable healthy `already_active` and actionable `candidate_not_found`.

## Constitution compatibility

Diagnostic suppression state is runtime-ephemeral observability state. It must not be read by scheduling decisions, resume routing, acceptance gating, archive routing, or next-action selection. Deleting `~/.local/state/cflx/**` or restarting the process may reset suppression, but must not change which change is dispatched next for the same workspace/git/base-tree evidence.

## Verification strategy

Targeted Rust tests should use the existing scheduler/reconciliation test area rather than requiring a full TUI session. Tests should prove:

1. A reducer-queued active/in-flight change is not inserted into local `queued`.
2. Repeating the same reconciliation condition does not emit unbounded identical user-visible log events.
3. Removing the active/in-flight condition allows the change to be inserted on a later reconciliation.
4. Candidate-not-found diagnostics remain observable but bounded.
