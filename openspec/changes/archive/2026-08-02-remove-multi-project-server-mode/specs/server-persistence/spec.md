## REMOVED Requirements

### Requirement: Server SQLite Database

Removed with server persistence.

#### Scenario: No server database

**Given**: Production modules
**When**: persistence backends are inspected
**Then**: No server SQLite database exists

### Requirement: Change Event Persistence

Removed with the server database.

#### Scenario: No server event persistence

**Given**: A local process
**When**: events are emitted
**Then**: No server database stores them

### Requirement: Log Entry Persistence

Removed with server persistence.

#### Scenario: No server log table

**Given**: Local logging
**When**: logs are persisted
**Then**: No server SQLite log table is used

### Requirement: Log Rotation

Removed with server database logs.

#### Scenario: No server database rotation

**Given**: Local logging
**When**: rotation occurs
**Then**: No server SQLite rows are rotated

### Requirement: Change State Persistence

Removed with multi-project server state.

#### Scenario: No server change-state persistence

**Given**: A local run
**When**: state changes
**Then**: No server database owns it

### Requirement: Statistics Overview API

Removed with the standalone dashboard.

#### Scenario: No server statistics overview

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server statistics overview route exists

### Requirement: UI State Persistence

Removed with the standalone dashboard.

#### Scenario: No server UI state persistence

**Given**: Packaged interfaces
**When**: UI state stores are enumerated
**Then**: No standalone dashboard UI state database exists

### Requirement: Proposal Session Persistence

Removed with server proposal sessions.

#### Scenario: No proposal session table

**Given**: Production persistence
**When**: schemas are inspected
**Then**: No server proposal session table exists

### Requirement: Proposal Session Message Persistence

Removed with server proposal sessions.

#### Scenario: No proposal message table

**Given**: Production persistence
**When**: schemas are inspected
**Then**: No server proposal message table exists
