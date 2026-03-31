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

The terminal result MUST include `Rejected` as a permanent terminal state distinct from `Error`. A rejected change is one where acceptance has determined the specification is unimplementable, requiring a rollback to the base branch with a documented reason.

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

#### Scenario: Acceptance Blocked transitions to Rejected terminal state

- **GIVEN** acceptance returns a `Blocked` verdict for a change
- **WHEN** the rejection flow completes (REJECTED.md committed, resolve executed, worktree removed)
- **THEN** the terminal state becomes `Rejected` with the rejection reason
- **AND** the derived display status is `rejected`
- **AND** the change cannot be re-queued via `AddToQueue`

#### Scenario: Rejected change cannot be re-queued

- **GIVEN** a change is in `Rejected` terminal state
- **WHEN** a user or system issues `AddToQueue` for that change
- **THEN** the reducer returns `NoOp`
- **AND** the runtime state remains unchanged

## ADDED Requirements

### Requirement: Rejection Flow Execution

The system SHALL execute a rejection flow when acceptance returns a `Blocked` verdict. The rejection flow MUST perform the following steps in order:

1. Extract the rejection reason from acceptance findings
2. Discard worktree changes and checkout the base branch
3. Generate `openspec/changes/<change_id>/REJECTED.md` containing the rejection reason and timestamp
4. Commit `REJECTED.md` to the base branch with message format `rejected: <change_id> - <one-line summary>`
5. Execute `openspec resolve <change_id>` to mark the change as resolved
6. Delete the worktree

The rejection flow SHALL be used by both serial and parallel execution services.

#### Scenario: Rejection flow generates REJECTED.md and commits to base

- **GIVEN** acceptance has returned `Blocked` for change `fix-auth`
- **WHEN** the rejection flow executes
- **THEN** `openspec/changes/fix-auth/REJECTED.md` is created with the rejection reason
- **AND** a commit is created on the base branch with message starting with `rejected: fix-auth`
- **AND** `openspec resolve fix-auth` is called
- **AND** the worktree for `fix-auth` is deleted

#### Scenario: Rejection flow failure falls back to error state

- **GIVEN** acceptance has returned `Blocked` for a change
- **WHEN** any step of the rejection flow fails (e.g., git commit fails)
- **THEN** the change transitions to `Error` terminal state
- **AND** the worktree is preserved for manual inspection

### Requirement: Rejected Change Exclusion from Change Listing

The system SHALL exclude changes with a `REJECTED.md` file from the active change listing returned by `list_changes_native()`.

This ensures rejected changes are not picked up by `cflx run` or presented as candidates for queue addition.

#### Scenario: Rejected change is excluded from list_changes_native

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **WHEN** `list_changes_native()` is called
- **THEN** `fix-auth` is NOT included in the returned change list

#### Scenario: Non-rejected change with proposal is included

- **GIVEN** `openspec/changes/add-feature/proposal.md` exists
- **AND** `openspec/changes/add-feature/REJECTED.md` does NOT exist
- **WHEN** `list_changes_native()` is called
- **THEN** `add-feature` IS included in the returned change list
