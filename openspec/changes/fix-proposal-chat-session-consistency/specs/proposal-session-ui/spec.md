## MODIFIED Requirements

### Requirement: proposal-session-ui-history-hydration

The Dashboard SHALL restore existing proposal-session messages through an idempotent history restoration flow. Initial session open and browser reload SHALL hydrate from REST history as the authoritative baseline for visible chat state. WebSocket replay/recovery events SHALL be reconciled against stable message/turn identity as updates to that baseline (not blind append), so the same logical message or assistant turn is never rendered more than once. The UI SHALL ignore stale history restoration results that do not belong to the currently active `projectId/sessionId` session.

#### Scenario: reopen-session-does-not-duplicate-history

**Given**: A proposal session with persisted user and assistant messages
**When**: The user closes and reopens that same session
**Then**: The prior messages are restored exactly once in chronological order without duplicate assistant turns

#### Scenario: reload-does-not-duplicate-restored-turns

**Given**: A proposal session with previously completed assistant turns
**When**: The user reloads the browser and the chat restores session history
**Then**: Each logical turn appears once even if multiple transport or hydration steps occur during restoration

#### Scenario: stale-history-response-ignored-after-session-switch

**Given**: The user switches from proposal session A to proposal session B while session A history loading is still in flight
**When**: The delayed history response for session A arrives after session B is already active
**Then**: The session A history response is ignored and the visible chat continues to show only session B messages

### Requirement: proposal-session-ui-multi-session

The Dashboard shall support multiple simultaneous proposal sessions per project with tab-based switching, and each session tab SHALL maintain strict conversation isolation. Chat state, replayed history, and asynchronous updates from one proposal session SHALL NOT appear in another session's chat view.

#### Scenario: switch-between-sessions-without-cross-session-leakage

**Given**: Two active proposal sessions for the same project with different message histories
**When**: The user switches from the first session tab to the second
**Then**: The chat view shows only the second session's conversation and no delayed updates from the first session are rendered into it
