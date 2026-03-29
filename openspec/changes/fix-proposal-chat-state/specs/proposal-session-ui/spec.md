## ADDED Requirements

### Requirement: proposal-session-ui-stable-turn-identity

The Dashboard SHALL preserve each completed assistant turn as a separate message in a proposal session.

#### Scenario: two-sequential-turns

**Given**: A proposal session with one completed assistant response
**When**: The user sends a second prompt and a second assistant response completes
**Then**: Both assistant responses remain visible as distinct messages in chronological order

### Requirement: proposal-session-ui-history-hydration

The Dashboard SHALL restore existing proposal-session messages when reconnecting to or reopening the same session.

#### Scenario: reopen-session-restores-history

**Given**: A proposal session with existing user and assistant messages persisted by the backend
**When**: The user closes and reopens the chat for that same session
**Then**: The prior messages are loaded into the chat list before any new streaming updates are rendered

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model active-turn state explicitly and SHALL enable or disable input based on that state.

#### Scenario: input-disabled-only-during-active-turn

**Given**: An active proposal session and a connected WebSocket
**When**: A prompt is submitted and no completion/error/cancel event has yet been received
**Then**: The input is disabled

#### Scenario: input-reenabled-on-turn-finish

**Given**: An active proposal session with disabled input because a turn is running
**When**: The current turn completes, errors, or is cancelled
**Then**: The input is enabled unless an elicitation dialog remains active or the connection is disconnected
