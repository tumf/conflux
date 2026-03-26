## Context

The repository has already moved toward reducer-owned lifecycle state in `OrchestratorState`, with TUI rows deriving display status from shared state snapshots. However, the current TUI runner only pushes `ChangesRefreshed` observations into the shared reducer, while manual resolve lifecycle events are handled mostly in TUI-local state.

This creates a split-brain condition:
- local TUI state moves a row to `Merged` when manual resolve completes
- shared reducer can still retain `ResolveWait`
- the next refresh reapplies reducer display state and revives `resolve pending`

## Goals / Non-Goals

- Goals:
  - keep reducer state authoritative for manual resolve completion
  - eliminate stale `ResolveWait` after merge/resolve success
  - preserve current key bindings and resolve queue behavior
- Non-Goals:
  - redesign all TUI state ownership
  - alter merge conflict resolution behavior or Git workflow

## Decisions

- Decision: manual resolve lifecycle events that affect reducer-owned wait/activity/terminal state must be applied to shared reducer state, not only TUI-local state.
  - Rationale: `ResolveWait` is already defined as reducer-owned queued resolve intent, so the same owner must clear it.
- Decision: successful manual resolve completion must clear any residual reducer wait state before refresh-derived display sync runs again.
  - Rationale: refresh is intentionally frequent and should reconcile observations, not resurrect stale queued resolve intent.

## Risks / Trade-offs

- Risk: double-applying events could regress terminal state if reducer transitions are not idempotent.
  - Mitigation: rely on reducer idempotency guarantees and add regression tests for duplicate/manual event paths.
- Risk: touching both runner and command handler paths may create overlapping reducer writes.
  - Mitigation: document the single intended event ingestion path in tests and code comments where needed.

## Verification Plan

- Add reducer/TUI tests covering manual resolve success followed by refresh.
- Validate that stale `ResolveWait` cannot survive `ResolveCompleted` or `MergeCompleted`.
- Run formatting, lint, and tests after implementation.
