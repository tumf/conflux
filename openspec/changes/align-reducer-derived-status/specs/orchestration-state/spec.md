## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

Display status exposed to consumers MAY be derived from this runtime state, but consumers SHALL NOT own an independent lifecycle copy.

Server-mode WebSocket API (`/api/v1/ws`) SHALL derive per-change display status exclusively from `ChangeRuntimeState.display_status()` when the orchestrator state is available. A fallback to workspace file-state detection is permitted only when orchestrator state is unavailable, and the fallback MUST NOT map intermediate workspace states to misleading display statuses.

#### Scenario: WebSocket reports reducer-derived status for active change

- **GIVEN** a change is in `Accepting` activity stage in the reducer
- **WHEN** a WebSocket client requests the change list
- **THEN** the change status is reported as `accepting`
- **AND** the status matches `ChangeRuntimeState.display_status()`

#### Scenario: WebSocket fallback does not report misleading archiving

- **GIVEN** orchestrator state is unavailable
- **AND** workspace detection reports `Applied` state
- **WHEN** a WebSocket client requests the change list
- **THEN** the change status is NOT reported as `archiving`
- **AND** the change status reflects the intermediate state accurately

## MODIFIED Requirements

### Requirement: WebSocket change status consistency with TUI

Server-mode WebSocket API SHALL produce the same set of display status strings as `ChangeRuntimeState.display_status()`. The system MUST NOT maintain a separate mapping from workspace states to display strings that diverges from the reducer-derived status vocabulary.

#### Scenario: All display statuses are representable in WebSocket payloads

- **GIVEN** the reducer can produce any of: `not queued`, `queued`, `blocked`, `applying`, `accepting`, `rejecting`, `archiving`, `resolving`, `merge wait`, `resolve pending`, `archived`, `merged`, `rejected`, `error`, `stopped`
- **WHEN** a WebSocket client receives a change list
- **THEN** the status field for each change is one of the above values
