# Design: Queued analysis reconciliation

## Current failure model

The scheduler starts with a local `queued: Vec<Change>` and appends dynamic queue entries from `DynamicQueue`. The TUI/reducer can still show a change as `queued` even when that local vector does not contain the change. In that state the scheduler does not call analysis because the main loop only attempts re-analysis when its local `queued` vector is non-empty.

This proposal treats reducer-visible queued intent as the durable runtime intent for the running process while keeping workspace/OpenSpec change files as the loadable source for `Change` metadata. Dynamic queue notifications remain a wake-up optimization, not the authoritative candidate set.

## Reconciliation approach

Add a scheduler-side reconciliation step before drain/idle checks and before the `!queued.is_empty()` analysis gate:

1. Read shared `OrchestratorState` when available.
2. Identify changes with queued intent and no active/terminal state that would make dispatch invalid.
3. Load corresponding active OpenSpec changes from native change listing.
4. Add missing eligible changes into scheduler-local `queued` if they are not already queued and not truly in-flight.
5. Record explicit skip reasons for reducer-queued changes that cannot be added yet.

The implementation should prefer a small helper on `ParallelExecutor`, for example `reconcile_queued_candidates_from_shared_state(...)`, so tests can exercise candidate ingestion without requiring a full terminal UI session.

## State and constitution constraints

The reconciliation must not introduce out-of-worktree durable workflow-control state. It may use:

- in-memory shared reducer/runtime state for the active process
- OpenSpec active change directories for loadable `Change` metadata
- workspace/git state already allowed by the Constitution

Logs and diagnostics are observability only and must not become workflow-control inputs.

## Diagnostics

When reducer-visible queued work exists and analysis does not start, the scheduler should emit enough information to identify the blocking reason. At minimum, implementation should distinguish:

- no available execution slots
- debounce active
- queued change not loadable from OpenSpec active changes
- change already active/in-flight
- local queue still empty after reconciliation

This diagnostic can be a log entry, a `ParallelEvent::Log`, or a more structured event if one already fits existing event conventions.

## Verification strategy

The most important regression test should avoid calling `perform_reanalysis_and_dispatch()` directly with a pre-populated `queued` vector. Instead it should simulate the failure boundary:

- shared reducer state contains a queued change
- scheduler-local `queued` starts empty
- no dynamic queue notification is required
- available slots are positive
- analyzer invocation or `AnalysisStarted` proves the queued change was ingested

Additional tests should cover recoverability after a skipped dynamic queue pop and diagnostics when analysis does not run.
