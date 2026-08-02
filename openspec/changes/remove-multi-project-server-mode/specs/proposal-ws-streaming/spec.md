## REMOVED Requirements

### Requirement: agent-thought-chunk-ws-message

Removed with proposal-session WebSocket streaming.

#### Scenario: No thought-chunk message

**Given**: The retained router
**When**: proposal streams are enumerated
**Then**: No thought-chunk proposal message exists

### Requirement: thought-chunk-history-annotation

Removed with proposal-session history.

#### Scenario: No thought history annotation

**Given**: The retained product
**When**: proposal history is inspected
**Then**: No server proposal thought annotation exists

### Requirement: proposal-ws-server-message-types

Removed with proposal-session WebSocket streaming.

#### Scenario: No proposal server messages

**Given**: The retained router
**When**: WebSocket messages are enumerated
**Then**: No server proposal-session messages exist

### Requirement: proposal-ws-client-message-id

Removed with proposal-session WebSocket streaming.

#### Scenario: No proposal client message ID

**Given**: The retained router
**When**: proposal messages are inspected
**Then**: No server proposal client message ID exists
