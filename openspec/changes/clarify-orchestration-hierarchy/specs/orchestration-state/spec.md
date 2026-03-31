## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

The Project SHALL maintain reducer-owned runtime state for each Change in `OrchestratorState`.

The system follows a three-tier hierarchy: `Orchestration 1--* Project 1--* Change`.

- **Orchestration**: The top-level runtime that manages one or more Projects.
- **Project**: A set of Changes sharing one `OrchestratorState` instance, one resolve queue, and one execution mode (Serial or Parallel). `OrchestratorState` is the single source of truth for a Project's lifecycle.
- **Change**: A unit of work within a Project, tracked by `ChangeRuntimeState` inside the Project's `OrchestratorState`.

The runtime state MUST distinguish at least the following concerns:

- queue intent
- active execution stage (per-Change: Applying, Accepting, Archiving, Resolving, Idle)
- wait reason
- terminal result
- workspace observation summary
- execution mode (Serial or Parallel)

Display status exposed to consumers MAY be derived from this runtime state, but consumers SHALL NOT own an independent lifecycle copy.

`is_resolving_active()` operates at Project scope: it returns true when any Change within the Project has `ActivityState::Resolving`. This Project-scoped flag SHALL be used exclusively for resolve operation serialization (ensuring only one resolve runs at a time within a Project). It SHALL NOT be used to block apply, accept, archive, or other non-resolve operations on other Changes within the same Project.

#### Scenario: Runtime state preserves queued intent while blocked

- **GIVEN** a Change is queued for execution within a Project
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the runtime state records queued intent
- **AND** the wait reason becomes blocked with dependency details
- **AND** the derived display status is `blocked`

#### Scenario: Runtime state distinguishes merge wait from archived result

- **GIVEN** archive has completed for a Change in parallel execution mode
- **WHEN** the reducer applies the `ChangeArchived` event
- **THEN** the wait reason becomes merge-wait
- **AND** the terminal state remains `None` (not yet terminal)
- **AND** the derived display status is `merge wait`

#### Scenario: is_resolving_active does not block non-resolve operations

- **GIVEN** Change A within a Project has `ActivityState::Resolving`
- **AND** Change B within the same Project is queued for apply
- **WHEN** the orchestrator checks whether to start Change B's apply
- **THEN** Change B's apply proceeds regardless of Change A's resolving state
- **AND** `is_resolving_active()` is not consulted for this decision

### Requirement: Resolve Wait Queue Ownership

The Project SHALL own the resolve wait queue in shared orchestration state (`OrchestratorState`) rather than in TUI-local lifecycle state.

`ResolveWait` SHALL represent reducer-owned queued resolve intent while another resolve is active within the same Project.

Manual resolve lifecycle events that clear or complete queued resolve intent MUST also be applied to the shared orchestration reducer before later refresh-driven display reconciliation can occur.

#### Scenario: Resolve wait queue is reducer-owned

- **GIVEN** one Change within a Project is currently resolving
- **AND** the user requests resolve for another Change in `MergeWait` within the same Project
- **WHEN** the reducer processes the command
- **THEN** the second Change enters `ResolveWait`
- **AND** the change_id is stored in the Project's shared resolve wait queue

#### Scenario: ResolveWait is not reconstructed from workspace only

- **GIVEN** a Change has an archived workspace that is still ahead of base
- **WHEN** the system rebuilds state from workspace observation alone
- **THEN** the reducer may recover `MergeWait`
- **AND** the reducer does not recover `ResolveWait` unless the Project's shared resolve wait queue contains that Change
