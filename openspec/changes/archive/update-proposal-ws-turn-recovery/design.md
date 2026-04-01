## Context

Proposal-session chat in server mode already supports reconnect with exponential backoff and message history replay, but the active-turn lifecycle still treats any disconnect during `submitted` or `streaming` as an immediate failure. Meanwhile the backend retains proposal session history and can often continue processing the turn. This proposal aligns the frontend turn state machine with backend recovery capabilities.

## Goals / Non-Goals

- Goals:
  - Prevent false active-turn failure on transient disconnects
  - Make reconnect recovery explicit in both UI and backend behavior
  - Reduce idle disconnect frequency with heartbeat/keepalive
  - Prevent duplicate prompt execution across reconnect
- Non-Goals:
  - Rework the main dashboard state WebSocket
  - Rework terminal session WebSocket reconnect behavior
  - Introduce a wholly new proposal chat transport

## Decisions

- Decision: Active-turn disconnects become recoverable-first, not fail-fast.
  - Rationale: The server may still hold session state and finish the turn, so immediate UI failure is often wrong.
- Decision: The frontend must distinguish transient reconnecting/recovering from terminal error.
  - Rationale: Users need accurate status and the state machine must avoid conflating recoverable transport loss with irrecoverable turn failure.
- Decision: The server must provide heartbeat/keepalive plus replay/recovery semantics sufficient for reconciliation.
  - Rationale: Reconnect alone is not enough if idle connections are allowed to die frequently or replay lacks enough context to infer turn completion.
- Decision: Duplicate prompt prevention remains mandatory during reconnect recovery.
  - Rationale: Recovery must never trade reliability for double execution.

## Alternatives Considered

- Alternative: Keep fail-fast behavior and only improve error messaging.
  - Rejected because it preserves the false-failure bug and forces needless user retries.
- Alternative: Recover entirely on the client with no backend changes.
  - Rejected because idle disconnect risk and replay ambiguity remain server-side concerns.
- Alternative: Expand the proposal to all server-mode WebSockets.
  - Rejected for now to keep scope focused on the direct active-turn failure path.

## Risks / Trade-offs

- Added turn-state complexity in the frontend → Mitigate with explicit state-machine tests.
- Replay-based reconciliation may be ambiguous if server metadata is too weak → Mitigate by specifying minimum recovery semantics in backend requirements.
- Heartbeat traffic adds small overhead → Acceptable trade-off for improved stability.

## Migration Plan

1. Update specs for recoverable disconnect semantics and backend heartbeat/recovery responsibilities.
2. Implement frontend state-machine changes and reconnect reconciliation.
3. Implement backend heartbeat/recovery support.
4. Add integration/e2e tests for disconnect during active turns.

## Open Questions

- Whether recovery needs a new explicit server message for turn recovery status, or whether replay/history plus `turn_complete` is sufficient.
- Whether heartbeat should be app-level JSON messages, protocol ping/pong frames, or both.
