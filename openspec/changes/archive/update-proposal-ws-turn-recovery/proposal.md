---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/proposal-session-ui/spec.md
  - openspec/specs/proposal-session-backend/spec.md
  - openspec/specs/proposal-ws-streaming/spec.md
  - dashboard/src/hooks/useProposalChat.ts
  - src/server/api.rs
---

# Change: Stabilize Proposal Session WebSocket Turn Recovery

**Change Type**: implementation

## Why

The server mode WebUI currently treats any unexpected proposal-session WebSocket disconnect during an active turn as an immediate turn failure. In practice, transient network interruptions, browser tab suspension, proxy/NAT idle timeouts, and long-running idle gaps during agent work can all drop the WebSocket even when the underlying proposal session is still healthy on the server.

This creates a mismatch between backend reality and frontend state:

1. **The frontend fails the turn too early**: `useProposalChat` immediately transitions to `error` on disconnect while status is `submitted` or `streaming`.
2. **The backend can often continue safely**: the proposal session manager keeps session history and can replay messages after reconnection.
3. **There is no heartbeat/keepalive for proposal chat**: proposal-session WebSockets do not proactively detect or prevent idle disconnects.
4. **Reconnect semantics are underspecified**: existing specs require reconnect with history replay, but do not define how active turns should recover versus fail.

The result is the user-visible error `WebSocket disconnected during active turn` even for recoverable disconnects, causing unnecessary interruption and lost trust in the WebUI.

## What Changes

### Frontend

- Update the proposal chat turn-state model so an unexpected disconnect during `submitted` or `streaming` does **not** immediately fail the active turn.
- Add an explicit recovery path for active turns during reconnect, including a reconnecting/recovering UI state and delayed failure only after reconnect is exhausted or recovery proves impossible.
- Require reconnection to reconcile active turn state from server replay/history so the UI returns to `streaming` or `ready` instead of remaining in a stale error state.
- Preserve pending prompt flushing behavior while preventing duplicate prompt execution during reconnect recovery.

### Backend

- Add proposal-session WebSocket heartbeat / keepalive behavior so otherwise idle but healthy turns are less likely to be disconnected by intermediaries.
- Define server responsibilities for reconnect recovery, including replaying enough information for the client to decide whether an interrupted turn is still active or already complete.
- Ensure the WebSocket contract remains backward compatible for non-recovery-aware clients where practical.

### Specification

- Tighten `proposal-session-ui` requirements around disconnect handling during active turns.
- Extend backend/WebSocket requirements so reconnect recovery and heartbeat expectations are explicit and testable.

## Impact

- Affected specs: `proposal-session-ui`, `proposal-session-backend`
- Affected frontend files: `dashboard/src/hooks/useProposalChat.ts`, related proposal chat UI components and tests
- Affected backend files: `src/server/api.rs`, proposal session manager/state tracking
- Affected tests: proposal chat hook/component tests and proposal session integration/e2e tests

## Acceptance Criteria

1. An unexpected WebSocket disconnect during `submitted` or `streaming` does not immediately mark the active turn as failed.
2. The proposal chat UI shows a reconnecting/recovering state while automatic reconnection is in progress.
3. If reconnection succeeds and the interrupted turn is still active, streaming resumes or the UI re-enters a recoverable in-progress state without requiring the user to resend the prompt.
4. If reconnection succeeds after the turn already completed on the server, the UI reconciles to `ready` using replay/history without surfacing a false active-turn failure.
5. If reconnection exceeds the configured retry budget or the server cannot recover the turn, the UI transitions to `error` with a terminal recovery failure.
6. Proposal-session WebSockets use heartbeat/keepalive behavior sufficient to reduce idle disconnect risk during long-running turns.
7. Reconnect recovery does not duplicate already-submitted prompts.

## Out of Scope

- Redesigning the main dashboard `/api/v1/ws` state stream.
- Adding automatic reconnect to terminal session WebSockets.
- Guaranteeing uninterrupted delivery across full browser refresh or process restart beyond existing persisted-history behavior.
