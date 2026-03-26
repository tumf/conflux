## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL own the resolve wait queue in shared orchestration state rather than in TUI-local lifecycle state.

`ResolveWait` SHALL represent reducer-owned queued resolve intent while another resolve is active.

Manual resolve lifecycle events that clear or complete queued resolve intent MUST also be applied to the shared orchestration reducer before later refresh-driven display reconciliation can occur.

#### Scenario: Resolve wait queue is reducer-owned

- **GIVEN** one change is currently resolving
- **AND** the user requests resolve for another change in `MergeWait`
- **WHEN** the reducer processes the command
- **THEN** the second change enters `ResolveWait`
- **AND** the change_id is stored in the shared resolve wait queue

#### Scenario: ResolveWait is not reconstructed from workspace only

- **GIVEN** a change has an archived workspace that is still ahead of base
- **WHEN** the system rebuilds state from workspace observation alone
- **THEN** the reducer may recover `MergeWait`
- **AND** the reducer does not recover `ResolveWait` unless the shared resolve wait queue contains that change

#### Scenario: Manual resolve completion clears reducer-owned resolve wait

- **GIVEN** the user has triggered manual resolve for a change that entered `ResolveWait`
- **AND** the shared reducer currently derives display status `resolve pending`
- **WHEN** the manual resolve completes successfully and the merge result becomes terminal
- **THEN** the shared reducer clears the queued resolve wait for that change
- **AND** subsequent `ChangesRefreshed` reconciliation does not derive `resolve pending` for the merged change
