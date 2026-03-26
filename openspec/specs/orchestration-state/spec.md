## Purpose

Provide a single, reducer-owned model for tracking the runtime lifecycle of each change across serial and parallel execution modes. All display status is derived from this shared state; consumers never own an independent lifecycle copy.

## Requirements

### Requirement: Reducer-Owned Change Runtime State

The system SHALL maintain reducer-owned runtime state for each change in `OrchestratorState`.

The runtime state MUST distinguish at least the following concerns:

- queue intent
- active execution stage
- wait reason
- terminal result
- workspace observation summary
- execution mode (Serial or Parallel)

Display status exposed to consumers MAY be derived from this runtime state, but consumers SHALL NOT own an independent lifecycle copy.

#### Scenario: Runtime state preserves queued intent while blocked

- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the runtime state records queued intent
- **AND** the wait reason becomes blocked with dependency details
- **AND** the derived display status is `blocked`

#### Scenario: Runtime state distinguishes merge wait from archived result

- **GIVEN** archive has completed for a change in parallel execution mode
- **WHEN** the reducer applies the `ChangeArchived` event
- **THEN** the wait reason becomes merge-wait
- **AND** the terminal state remains `None` (not yet terminal)
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

### Requirement: Execution Mode Determines Archive Terminal Semantics

The system SHALL support two execution modes — Serial and Parallel — that determine how `ChangeArchived` events affect terminal state.

In Serial mode, `ChangeArchived` SHALL set the terminal state to `Archived` (a terminal state from which no further transitions occur).

In Parallel mode, `ChangeArchived` SHALL set the wait state to `MergeWait` (a non-terminal state) to allow the subsequent merge step to transition the change to `Merged`.

#### Scenario: Serial mode treats archive as terminal

- **GIVEN** the orchestrator is running in Serial execution mode
- **WHEN** a change receives a `ChangeArchived` event
- **THEN** the terminal state becomes `Archived`
- **AND** the derived display status is `archived`
- **AND** subsequent `MergeCompleted` events for this change are ignored

#### Scenario: Parallel mode treats archive as merge-wait

- **GIVEN** the orchestrator is running in Parallel execution mode
- **WHEN** a change receives a `ChangeArchived` event
- **THEN** the wait state becomes `MergeWait`
- **AND** the terminal state remains `None`
- **AND** the derived display status is `merge wait`

#### Scenario: Parallel mode archive then merge completes lifecycle

- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a change has received a `ChangeArchived` event (currently in `MergeWait`)
- **WHEN** a `MergeCompleted` event is received for the change
- **THEN** the terminal state becomes `Merged`
- **AND** the derived display status is `merged`


### Requirement: Parallel Resume Applies Archive-Complete Wait Semantics

In Parallel execution mode, when a resumed workspace is already archive-complete, the shared lifecycle state SHALL apply the same wait semantics as a `ChangeArchived` transition.

This resume-time archive-complete transition MUST preserve the user-visible merge-wait lifecycle and MUST NOT fall back to `not queued` before merge handling has been attempted.

#### Scenario: Resume-time archived change becomes merge wait

- **GIVEN** the orchestrator is running in Parallel execution mode
- **AND** a reused workspace is detected as already archived but not yet merged
- **WHEN** the parallel resume path reports archive-complete completion for that change
- **THEN** the wait state becomes `MergeWait`
- **AND** the derived display status is merge wait
- **AND** the change does not regress to `not queued` during the restart flow


#
