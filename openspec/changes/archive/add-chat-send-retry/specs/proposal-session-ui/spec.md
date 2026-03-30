## ADDED Requirements

### Requirement: proposal-session-ui-send-retry

The Dashboard SHALL queue user messages sent while the WebSocket is disconnected and automatically send them upon reconnection. Failed sends SHALL be visually indicated with a retry mechanism.

#### Scenario: queue-message-while-disconnected

**Given**: A proposal session with a disconnected WebSocket
**When**: The user submits a message
**Then**: The message appears in the chat list with a "pending" visual indicator and is queued for sending

#### Scenario: auto-send-on-reconnection

**Given**: One or more pending messages are queued
**When**: The WebSocket reconnects
**Then**: The queued messages are sent automatically in chronological order and their status transitions to "sent"

#### Scenario: show-failed-with-retry

**Given**: A pending message that failed to send after reconnection
**When**: The send attempt results in an error
**Then**: The message displays a "failed" visual state with a "Retry" button

#### Scenario: retry-sends-message

**Given**: A failed message with a visible "Retry" button
**When**: The user clicks "Retry"
**Then**: The message status transitions to "pending" and a new send attempt is made
