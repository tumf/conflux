## MODIFIED Requirements

### Requirement: Processing Item Spinner Animation

The TUI SHALL display phase-specific vocabulary for active work: `preparing`, `applying`, `accepting`, `rejecting`, `archiving`, and `resolving`. `preparing` SHALL mean that a scheduler-admitted change is creating, recreating, setting up, inspecting, or waiting to start its repository-derived workflow operation after acquiring an execution slot. It SHALL display `blocked` for both dependency waits and validated external prerequisite waits, with detail that identifies the blocker kind. It SHALL display `stalled` for no-progress or exhausted execution holds. When an iteration number applies, the display SHALL retain the `status:iteration` format. TUI, WebUI, and `/api/v2` SHALL project the same reducer-owned active status.

#### Scenario: Dependency wait displays blocked

- **GIVEN** a change waits on an unarchived proposal dependency
- **WHEN** the TUI renders the change
- **THEN** its status is `blocked`
- **AND** its detail identifies a dependency blocker

#### Scenario: External prerequisite displays blocked

- **GIVEN** the orchestrator has validated an external prerequisite blocker
- **WHEN** the TUI renders the change
- **THEN** its status is `blocked`
- **AND** its detail exposes the external category, unblock condition, and next action

#### Scenario: Exhausted execution displays stalled

- **GIVEN** automatic execution stopped after no progress or retry exhaustion
- **WHEN** the TUI renders the change
- **THEN** its status is `stalled`
- **AND** the row is not described as waiting on a dependency or external prerequisite

#### Scenario: Worktree setup displays preparing

- **GIVEN** a queued change has acquired a parallel execution-slot permit and passed stop and terminal gates
- **WHEN** the scheduler begins force-recreate cleanup, managed worktree creation, or `.wt/setup`
- **THEN** the shared status becomes `preparing` before the potentially slow preparation starts
- **AND** TUI, WebUI, and `/api/v2` display `preparing` while preparation remains in progress
- **AND** the status does not claim that Apply has started

#### Scenario: Preparation advances to the repository-derived phase

- **GIVEN** a change is displayed as `preparing`
- **WHEN** workspace preparation completes and repository evidence selects the next workflow operation
- **THEN** the status changes to that operation's active vocabulary
- **AND** an Apply route displays `applying` with its applicable iteration
- **AND** a resumed acceptance, rejection, archive, or resolve route does not emit a false Apply transition

#### Scenario: Preparation failure is visible

- **GIVEN** a change is displayed as `preparing`
- **WHEN** worktree creation or `.wt/setup` fails
- **THEN** the change transitions to `error`
- **AND** the operator receives a diagnostic identifying the failed preparation step

#### Scenario: Preparing is active for safety controls

- **GIVEN** a change is displayed as `preparing`
- **WHEN** an operator requests dequeue or managed-worktree deletion
- **THEN** the system treats the change as active execution
- **AND** managed-worktree deletion remains refused
- **AND** if inline preparation has no termination handle, immediate dequeue is refused while the stop mark remains recorded
- **AND** after preparation returns, the recorded stop prevents operation-agent startup and the change leaves `preparing` through a reducer-visible stopped or cleared transition

#### Scenario: Preparing clears on pre-operation exit

- **GIVEN** a change has emitted `preparing`
- **WHEN** global cancellation or a pre-spawn early return ends dispatch before another operation-started event
- **THEN** the reducer receives a clearing, stopped, or terminal transition
- **AND** the change does not remain indefinitely displayed as `preparing`

#### Scenario: Preparing is not durable routing state

- **GIVEN** a process stops while a change is displayed as `preparing`
- **WHEN** Conflux starts again with the same workspace files and Git state
- **THEN** the next action is derived from workspace and repository evidence
- **AND** no persisted `preparing` observation, log, metric, or elapsed duration controls routing

#### Scenario: Setup duration is observable

- **GIVEN** a managed worktree contains `.wt/setup`
- **WHEN** Conflux runs the setup script
- **THEN** it emits one setup-start diagnostic
- **AND** success emits one completion diagnostic with elapsed duration
- **AND** failure emits one actionable failure diagnostic
- **AND** these diagnostics do not change workflow routing
