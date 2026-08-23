# Design

## Boundary

`ApplyIterationLimit` answers why one invocation stopped. It is retained so the finish path and operator can inspect exact attempts/max evidence. It must not answer whether a later explicit operator command may create a new invocation.

The current implementation couples those concerns through `RunBoundaryLiveness`: a persistent scheduler remains live after a change-local failure, so the diagnostic record stays an action gate forever. The promised “retry after the run closes” event never occurs during normal TUI ownership.

## Decision

Use intent, not scheduler lifetime, to prevent loops:

- Automatic paths cannot consume terminal `error`: queue reconciliation, delayed mark settlement, ordinary queue add, and generic scheduler notification remain non-retry paths.
- Explicit paths can consume terminal `error`: `RetryChange`, bulk retry, and retry-class Start apply `ReducerCommand::RetryError`, publish one target-specific explicit-retry edge, and start or wake the scheduler as already specified.
- The retained Apply-limit record remains diagnostic. Starting a new explicit execution boundary resets budget through the existing invocation-local accounting path.

This removes the operator-action use of `active_apply_iteration_limit`, its TUI cache/guidance, and the `/api/v2` `apply_iteration_limit_active` blocked reason for settled terminal errors. It does not require deleting the reducer record before the finish path has observed it.

## Safety properties

1. No automatic retry: no path without explicit retry intent applies `RetryError`.
2. One retry per intent: the existing serialized command transaction and target-specific explicit-retry edge remain authoritative.
3. Fresh budget: retry creates a later invocation rather than redispatching inside the exhausted invocation.
4. Truthful UI/API: both project the shared retry classifier instead of a scheduler-liveness exception.
5. Workspace-local routing: restart behavior and next-action selection remain derived from terminal workspace/Git evidence; retained process-local diagnostics do not control routing.

## Regression shape

The tight regression fixture keeps a command-capable persistent scheduler live, records an exhausted `ApplyIterationLimit`, and settles the target into terminal `error`. Before the fix, `RetryChange` and Start classification return `apply_iteration_limit_active`. After the fix:

- no action occurs before explicit intent;
- individual retry is accepted once;
- Start-selected retry is accepted once;
- bulk retry accepts the limited target and unrelated retryable targets;
- API action eligibility and TUI guidance report retry availability.
