## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The Project SHALL own the resolve wait queue in shared orchestration state (`OrchestratorState`) rather than in TUI-local lifecycle state. This is a Core-owned state; Frontend implementations (TUI, Web) SHALL NOT maintain independent copies of the resolve queue.

`ResolveWait` SHALL represent reducer-owned queued resolve intent while another resolve is active within the same Project.

Manual resolve lifecycle events that clear or complete queued resolve intent MUST also be applied to the shared orchestration reducer before later refresh-driven display reconciliation can occur.

Frontend MAY cache the resolve queue state for rendering purposes, but the cache MUST be derived from the Core's `OrchestratorState` and MUST NOT serve as the source of truth for resolve ordering or execution decisions.

#### Scenario: Resolve wait queue is reducer-owned

- **GIVEN** one Change within a Project is currently resolving
- **AND** the user requests resolve for another Change in `MergeWait` within the same Project
- **WHEN** the reducer processes the command
- **THEN** the second Change enters `ResolveWait`
- **AND** the change_id is stored in the Project's shared resolve wait queue
- **AND** Frontend render caches are updated from this Core state

#### Scenario: ResolveWait is not reconstructed from workspace only

- **GIVEN** a Change has an archived workspace that is still ahead of base
- **WHEN** the system rebuilds state from workspace observation alone
- **THEN** the reducer may recover `MergeWait`
- **AND** the reducer does not recover `ResolveWait` unless the Project's shared resolve wait queue contains that Change

#### Scenario: Frontend does not own resolve queue independently

- **GIVEN** TUI or Web UI needs to display the resolve queue
- **WHEN** the resolve queue is accessed for rendering
- **THEN** the displayed queue is derived from `OrchestratorState.resolve_wait_queue`
- **AND** Frontend does not maintain a separate FIFO queue that diverges from Core state
