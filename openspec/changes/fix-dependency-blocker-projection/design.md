# Design: Dependency blocker projection

## Decision

Separate current-state reconciliation from edge-triggered diagnostics.

The scheduler already computes unresolved dependencies to decide dispatch. That classification must update the shared reducer/runtime projection every time it is coherently evaluated. A blocker-fingerprint store may decide whether to emit another log or compatibility event, but it must never decide whether current blocker state exists.

## State contract

For an admitted change with unresolved dependencies:

- queue intent remains `queued`;
- execution state remains `queued` because no execution episode has started;
- lifecycle display is `blocked`;
- blocker kind is `dependency` and includes the current unresolved dependency IDs;
- parallel eligibility is false.

Once repository-visible evidence proves all dependencies resolved, the dependency blocker is removed and the retained queue intent determines the next display/eligibility state.

## Recovery

The projection is reconstructible from workspace and base-branch evidence. Reducer replacement, refresh, or loss of ephemeral diagnostic fingerprints does not require durable external state: the next dependency classification republishes the same current blocker.

## Non-goals

This change does not alter dependency resolution, queue admission, execution marks, scheduling order, or concurrency limits.
