## REMOVED Requirements

### Requirement: stats-overview-api-contract-test

Removed with the server statistics API.

#### Scenario: No statistics API contract

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server statistics overview route exists

### Requirement: stats-overview-frontend-resilience-test

Removed with the React dashboard.

#### Scenario: No overview frontend

**Given**: Packaged interfaces
**When**: dashboards are inspected
**Then**: No standalone overview frontend exists

### Requirement: proposal-session-messages-endpoint

Removed with server proposal sessions.

#### Scenario: No proposal messages endpoint

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server proposal messages endpoint exists

### Requirement: UI State REST API

Removed with standalone dashboard state.

#### Scenario: No UI state API

**Given**: The retained router
**When**: routes are enumerated
**Then**: No standalone dashboard UI-state API exists

### Requirement: FullState UI State Inclusion

Removed with standalone dashboard state.

#### Scenario: No UI state in server full state

**Given**: The retained API
**When**: state payloads are inspected
**Then**: No standalone dashboard UI state is included

### Requirement: Dashboard Session Restoration on Reload

Removed with the standalone dashboard.

#### Scenario: No dashboard session restoration

**Given**: Packaged interfaces
**When**: sessions are inspected
**Then**: No standalone dashboard session is restored
