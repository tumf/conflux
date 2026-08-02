## REMOVED Requirements

### Requirement: proposal-session-config

Server-backed proposal-session configuration is removed.

#### Scenario: No proposal-session server config

**Given**: Current configuration
**When**: supported fields are inspected
**Then**: No server proposal-session configuration is exposed

### Requirement: proposal-session-create

Server proposal-session creation is removed.

#### Scenario: No proposal-session create route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server proposal-session create route exists

### Requirement: proposal-session-list

Server proposal-session listing is removed.

#### Scenario: No proposal-session list route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server proposal-session list route exists

### Requirement: proposal-session-close

Server proposal-session closure is removed.

#### Scenario: No proposal-session close route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server proposal-session close route exists

### Requirement: proposal-session-merge

Server proposal-session merge is removed.

#### Scenario: No proposal-session merge route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server proposal-session merge route exists

### Requirement: proposal-session-websocket

The server proposal-session WebSocket transport is removed.

#### Scenario: No proposal-session WebSocket

**Given**: The retained router
**When**: WebSocket routes are enumerated
**Then**: No proposal-session transport exists

### Requirement: proposal-session-specification-boundaries

Server proposal-session specification guidance is removed with the feature.

#### Scenario: No proposal-session agent boundary

**Given**: Production modules
**When**: proposal-session subprocess integration is inspected
**Then**: No server proposal-session agent boundary exists

### Requirement: proposal-session-change-detection

Server proposal-session change detection is removed.

#### Scenario: No proposal-session change detection

**Given**: Production modules
**When**: proposal detection paths are inspected
**Then**: No server proposal-session watcher exists

### Requirement: proposal-session-inactivity-timeout

Server proposal-session timeout management is removed.

#### Scenario: No proposal-session timeout worker

**Given**: Runtime background tasks
**When**: workers are inspected
**Then**: No proposal-session timeout scanner exists

### Requirement: proposal-session-backend-transport-single-source

The server proposal-session backend transport is removed.

#### Scenario: No proposal transport selection

**Given**: Production configuration
**When**: transports are inspected
**Then**: No server proposal-session transport can be selected

### Requirement: proposal-session-ws-replay-user-messages

Server proposal-session replay is removed.

#### Scenario: No proposal replay stream

**Given**: The retained router
**When**: streams are enumerated
**Then**: No proposal-session replay exists

### Requirement: proposal-session-ui-history-hydration

Server dashboard proposal history hydration is removed.

#### Scenario: No proposal history hydration

**Given**: Packaged interfaces
**When**: proposal chat surfaces are inspected
**Then**: No server dashboard proposal history is present

### Requirement: proposal-session-websocket-turn-recovery

Server proposal-session turn recovery is removed.

#### Scenario: No turn recovery

**Given**: Production modules
**When**: proposal-session recovery paths are inspected
**Then**: No server turn-recovery path exists
