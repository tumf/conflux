## MODIFIED Requirements

### Requirement: WebSocket change list uses reducer-derived status

The WebSocket change list payload SHALL use `ChangeRuntimeState.display_status()` as the authoritative source for each change's status field when orchestrator state is available.

The system SHALL NOT derive display status from `detect_workspace_state()` independently when the reducer is available, as this creates divergent lifecycle copies that violate the orchestration-state specification.

When a change directory contains `openspec/changes/<change_id>/REJECTED.md`, the dashboard-facing change list SHALL still include that change if the change has a proposal directory, and the payload status SHALL be `rejected` even when the reducer has no in-memory runtime entry for the change.

#### Scenario: Reducer-available path replaces workspace-derived status

- **GIVEN** a project is running with orchestrator state available
- **AND** a change has `ActivityState::Accepting` in the reducer
- **WHEN** the WebSocket builds the change list payload
- **THEN** the status field is `accepting`
- **AND** `detect_workspace_state()` is NOT called for status derivation

#### Scenario: Reducer-unavailable fallback maps accurately

- **GIVEN** orchestrator state is unavailable for a project
- **AND** workspace detection reports `WorkspaceState::Applied`
- **WHEN** the WebSocket builds the change list payload
- **THEN** the status field is `applied` (not `archiving`)

#### Scenario: Rejected marker is surfaced in dashboard payload

- **GIVEN** `openspec/changes/fix-auth/proposal.md` exists
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** the reducer has no runtime entry for `fix-auth`
- **WHEN** the WebSocket builds the change list payload
- **THEN** the payload includes `fix-auth`
- **AND** the status field is `rejected`
