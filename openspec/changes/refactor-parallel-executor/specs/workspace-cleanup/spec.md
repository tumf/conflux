## ADDED Requirements

### Requirement: Workspace Cleanup Guard

The system SHALL provide a `WorkspaceCleanupGuard` that ensures workspaces are cleaned up on partial failures using the RAII pattern.

The guard SHALL track created workspaces and automatically clean them up when dropped, unless explicitly committed.

#### Scenario: Normal completion with commit

- **GIVEN** workspaces are created and tracked by the guard
- **WHEN** all operations complete successfully
- **AND** `guard.commit()` is called
- **THEN** the guard SHALL NOT clean up the workspaces on drop

#### Scenario: Early failure without commit

- **GIVEN** workspace A is created and tracked
- **AND** workspace B creation fails
- **WHEN** the guard is dropped (due to error propagation)
- **THEN** workspace A SHALL be cleaned up automatically

#### Scenario: Panic during execution

- **GIVEN** workspaces are created and tracked by the guard
- **WHEN** a panic occurs during execution
- **THEN** the guard SHALL clean up all tracked workspaces during unwinding

### Requirement: Guard Integration with Parallel Executor

The `ParallelExecutor::execute_group()` method SHALL use `WorkspaceCleanupGuard` to ensure resource cleanup on any failure path.

#### Scenario: Workspace creation failure cleanup

- **GIVEN** a group with 5 changes to process
- **WHEN** workspace creation succeeds for the first 3 changes
- **AND** workspace creation fails for the 4th change
- **THEN** the 3 successfully created workspaces SHALL be cleaned up
- **AND** an appropriate error SHALL be returned

#### Scenario: Apply execution failure cleanup

- **GIVEN** a group with 3 changes and 3 successfully created workspaces
- **WHEN** apply execution fails for one workspace
- **THEN** all 3 workspaces SHALL remain (for debugging/retry)
- **AND** the guard SHALL be committed after the apply phase
- **Note:** Cleanup happens after the full group completes, not on individual apply failures

### Requirement: Cleanup Logging

The cleanup guard SHALL log cleanup operations for debugging and monitoring.

#### Scenario: Cleanup on failure

- **WHEN** the guard performs cleanup on drop (uncommitted)
- **THEN** a warning log entry SHALL be emitted for each cleaned workspace
- **AND** the log SHALL include the reason (guard not committed)

#### Scenario: Cleanup errors

- **WHEN** workspace cleanup fails (e.g., workspace already deleted)
- **THEN** the error SHALL be logged but not propagated
- **AND** cleanup SHALL continue for remaining workspaces
