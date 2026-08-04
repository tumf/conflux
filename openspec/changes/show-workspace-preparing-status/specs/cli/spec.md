## MODIFIED Requirements

### Requirement: Processing Item Spinner Animation

The TUI SHALL display phase-specific vocabulary for active work: `preparing`, `applying`, `accepting`, `archiving`, and `resolving`. `preparing` SHALL mean that a scheduler-admitted change is creating, recreating, setting up, or inspecting its managed workspace before the repository-derived workflow operation starts. It SHALL display `blocked` for both dependency waits and validated external prerequisite waits, with detail that identifies the blocker kind. It SHALL display `stalled` for no-progress or exhausted execution holds. When an iteration number applies, the display SHALL retain the `status:iteration` format. TUI, WebUI, and `/api/v2` SHALL project the same reducer-owned active status.

#### Scenario: Worktree setup displays preparing

- **GIVEN** a queued change has passed dependency and capacity selection
- **WHEN** the scheduler begins managed worktree creation or runs `.wt/setup`
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
- **AND** destructive mutation does not proceed until owned preparation cancellation is confirmed

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
