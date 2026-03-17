## MODIFIED Requirements

### Requirement: Parallel mode excludes uncommitted changes

Parallel mode SHALL exclude any change that is not present in `HEAD` or that has uncommitted or untracked files under `openspec/changes/<change_id>/` from execution.

When start-time filtering excludes one or more requested changes, the parallel execution layer SHALL return enough information for each caller to reconcile user-visible state correctly. TUI callers SHALL not leave rejected rows displayed as `Queued`, and CLI callers SHALL be able to report when zero requested changes actually started.

#### Scenario: start-time rejection is propagated to callers

- **GIVEN** a caller requests parallel execution for one or more changes
- **AND** at least one requested change fails the latest eligibility check before dispatch begins
- **WHEN** parallel start performs backend filtering
- **THEN** the rejected change is excluded from execution
- **AND** the rejection result is propagated back to the caller
- **AND** the caller can reconcile its user-visible state without pretending the change started

#### Scenario: all requested changes are rejected before start

- **GIVEN** a caller requests parallel execution for one or more changes
- **AND** all requested changes fail the latest eligibility check before dispatch begins
- **WHEN** the backend completes start-time filtering
- **THEN** no changes are dispatched
- **AND** the caller is informed that zero changes started
- **AND** the caller can present the rejection reason to the user
