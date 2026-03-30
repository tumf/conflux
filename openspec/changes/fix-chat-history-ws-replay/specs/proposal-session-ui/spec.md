## MODIFIED Requirements

### Requirement: proposal-session-ui-turn-state

The Dashboard SHALL model active-turn state explicitly and SHALL enable or disable input based on that state. When the WebSocket connection closes unexpectedly during an active turn, the turn SHALL be treated as failed and the input SHALL be re-enabled.

#### Scenario: input-disabled-only-during-active-turn

**Given**: An active proposal session and a connected WebSocket
**When**: A prompt is submitted and no completion/error/cancel event has yet been received
**Then**: The input is disabled

#### Scenario: input-reenabled-on-turn-finish

**Given**: An active proposal session with disabled input because a turn is running
**When**: The current turn completes, errors, or is cancelled
**Then**: The input is enabled unless an elicitation dialog remains active or the connection is disconnected

#### Scenario: input-reenabled-on-ws-disconnect-during-turn

**Given**: An active proposal session with disabled input because a turn is running
**When**: The WebSocket connection closes unexpectedly
**Then**: The active turn is treated as failed, `isAgentResponding` is set to false, and the input is re-enabled (though still disabled due to disconnected state, it will become usable upon reconnection)
