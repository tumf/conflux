## MODIFIED Requirements

### Requirement: Reducer-Owned Change Runtime State

The system SHALL maintain reducer-owned runtime state for each change in `OrchestratorState`.

The runtime state MUST distinguish at least the following concerns:

- queue intent
- active execution stage
- wait reason
- terminal result
- workspace observation summary
- execution mode (Serial or Parallel)

The active execution stage SHALL include a dedicated `Rejecting` stage distinct from `Applying`, `Accepting`, `Archiving`, and `Resolving`. Display status exposed to consumers MAY be derived from this runtime state, but consumers SHALL NOT own an independent lifecycle copy.

#### Scenario: Runtime state preserves queued intent while blocked

- **GIVEN** a change is queued for execution
- **AND** dependency analysis reports unresolved dependencies
- **WHEN** the reducer applies the dependency-blocked input
- **THEN** the runtime state records queued intent
- **AND** the wait reason becomes blocked with dependency details
- **AND** the derived display status is `blocked`

#### Scenario: Runtime state exposes rejecting as active stage

- **GIVEN** apply execution generated `openspec/changes/fix-auth/REJECTED.md`
- **WHEN** the reducer applies the rejection-review start event
- **THEN** the active execution stage becomes `Rejecting`
- **AND** the derived display status is `rejecting`
- **AND** the change is not shown as `accepting`

### Requirement: Rejected terminal state remains distinct from errors

The terminal result MUST include `Rejected` as a permanent terminal state distinct from `Error`. A rejected change is one where rejecting review has confirmed the specification is unimplementable or otherwise out of scope for completion, requiring a rollback to the base branch with a documented reason.

#### Scenario: rejecting-confirmed change becomes rejected terminal state

- **GIVEN** a change is in `Rejecting`
- **AND** the rejection flow completes (`REJECTED.md` committed and worktree removed)
- **WHEN** the reducer applies the terminal rejection event
- **THEN** the terminal result becomes `Rejected`
- **AND** the derived display status is `rejected`

### Requirement: Rejection proposal dismissal returns to apply with recovery tasks

When rejecting review dismisses a worktree-local `openspec/changes/<change_id>/REJECTED.md` proposal, the runtime SHALL return the change to active apply rather than terminal rejection.

Before returning to apply, the runtime SHALL remove the worktree-local `REJECTED.md` file and ensure `openspec/changes/<change_id>/tasks.md` contains at least one unchecked task describing a non-rejection recovery step.

#### Scenario: dismissing rejection proposal resumes apply

- **GIVEN** a change is currently in `Rejecting`
- **AND** the worktree contains `openspec/changes/fix-auth/REJECTED.md`
- **WHEN** rejecting review dismisses the reject proposal
- **THEN** the worktree-local `REJECTED.md` is removed
- **AND** `openspec/changes/fix-auth/tasks.md` is updated with at least one unchecked recovery task that is not a reject action
- **AND** the active execution stage becomes `Applying`
- **AND** the derived display status is `applying`
