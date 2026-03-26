## ADDED Requirements

### Requirement: Reducer-Owned Change Runtime State

The system SHALL maintain reducer-owned runtime state for each change in `OrchestratorState`.

The runtime state MUST distinguish at least the following concerns:

- queue intent
- active execution stage
- wait reason
- terminal result
- workspace observation summary

Display status exposed to consumers MAY be derived from this runtime state, but consumers SHALL NOT own an independent lifecycle copy.

#### Scenario: Runtime state preserves queued intent while blocked

- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the runtime state records queued intent
- **AND** the wait reason becomes blocked with dependency details
- **AND** the derived display status is `blocked`

#### Scenario: Runtime state distinguishes merge wait from archived result

- **GIVEN** archive has completed for a change
- **AND** merge is deferred because the worktree is not ready to merge
- **WHEN** the reducer applies the merge-deferred input
- **THEN** the runtime state records terminal archived result
- **AND** the wait reason becomes merge-wait
- **AND** the derived display status is `merge wait`

### Requirement: Reducer Input Precedence and Idempotency

The reducer SHALL accept mutations only through structured inputs: user commands, execution events, and workspace observations.

The reducer MUST be idempotent for duplicate inputs and MUST ignore stale inputs that would regress terminal state.

Execution events SHALL own active-stage and terminal transitions. Workspace observations SHALL reconcile durable wait/recovery state and MUST NOT override an active execution stage.

#### Scenario: Duplicate event is a no-op

- **GIVEN** a change is already in an applying activity state
- **WHEN** the same `ApplyStarted` event is processed again
- **THEN** the reducer leaves the runtime state unchanged
- **AND** no invalid regression occurs

#### Scenario: Late failure does not regress merged state

- **GIVEN** a change is already in terminal merged state
- **WHEN** a stale `ResolveFailed` or `ApplyFailed` event arrives
- **THEN** the reducer ignores the stale event
- **AND** the runtime state remains merged

#### Scenario: Observation does not override active resolve

- **GIVEN** a change is currently resolving
- **WHEN** auto-refresh observes that the worktree is archived and ahead of base
- **THEN** the reducer stores the observation
- **AND** the displayed status remains `resolving`

### Requirement: Resolve Wait Queue Ownership

The system SHALL own the resolve wait queue in shared orchestration state rather than in TUI-local lifecycle state.

`ResolveWait` SHALL represent reducer-owned queued resolve intent while another resolve is active.

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
