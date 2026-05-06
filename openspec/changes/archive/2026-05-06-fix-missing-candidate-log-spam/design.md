# Design: Bound missing-candidate queue reconciliation diagnostics

## Root Cause

`reconcile_queued_candidates_from_shared_state()` already calls `emit_queue_reconciliation_diagnostic()` for `candidate_not_found`, and that helper suppresses repeated `(change_id, reason)` user-visible diagnostics. However, the same branch also calls `warn!` unconditionally before the helper. That unconditional structured warning is mirrored into debug log files on every scheduler loop and defeats the existing bounded diagnostic intent.

## Design

Use a single bounded diagnostic decision for missing reducer-queued candidates:

- Preserve the first `candidate_not_found` observation as a warning-level diagnostic.
- Suppress repeated warning-level emissions for the same `(change_id, reason)` until the in-memory diagnostic state is reset by process restart or equivalent runtime lifecycle.
- If repeated observations need to remain traceable, emit them at `debug!` or as a bounded summary that does not flood WARN-level log-mining output.
- Do not mutate reducer queue intent, active state, resolve-wait state, or scheduler-local queue insertion decisions as part of logging suppression.

## Constitution Alignment

The proposal uses only runtime-ephemeral observability state. It does not introduce durable workflow state and does not use logs, caches, metrics, or suppression state as workflow-control input.

## Verification Strategy

- Regression test repeated missing-candidate reconciliation and count emitted diagnostics.
- Re-run existing scheduler reconciliation tests for `already_active`, missing candidate observability, and recovery after a change leaves active/in-flight state.
- Manual inspection confirms no branch uses suppression state to decide scheduling behavior.
