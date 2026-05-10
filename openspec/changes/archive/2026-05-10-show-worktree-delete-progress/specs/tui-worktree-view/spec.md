## ADDED Requirements

### Requirement: Worktree deletion progress is visible in TUI

When a user confirms manual worktree deletion from the TUI Worktrees view, the TUI MUST immediately show that deletion is in progress for the target worktree until the deletion command completes.

The progress state MUST be transient UI state only. It MUST NOT be persisted or used as an input to scheduler dispatch, resume routing, acceptance, archive, or next-action decisions.

#### Scenario: Normal delete shows deleting row immediately

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** a non-main worktree is selected
- **WHEN** the user confirms deletion with `Y`
- **THEN** the selected worktree row shows `[Deleting...]` before the deletion command completes
- **AND** the TUI logs that worktree deletion has started

#### Scenario: Skip-teardown delete shows deleting row immediately

- **GIVEN** the TUI is displaying the worktree deletion confirmation modal
- **AND** the selected worktree has a teardown script that may be skipped
- **WHEN** the user confirms skip-teardown deletion with `S`
- **THEN** the selected worktree row shows `[Deleting...]` before the deletion command completes
- **AND** the TUI logs that deletion started with skip-teardown

#### Scenario: Deleting worktree suppresses duplicate target actions

- **GIVEN** a worktree row is marked as deleting in the TUI
- **WHEN** the user attempts to delete, merge, open, or edit that same worktree row
- **THEN** the TUI does not emit the normal target action command
- **AND** the TUI displays or logs a warning that the worktree is already being deleted

#### Scenario: Successful delete clears progress state

- **GIVEN** a worktree row is marked as deleting in the TUI
- **WHEN** the worktree deletion command succeeds
- **THEN** the deleting marker is cleared
- **AND** the existing worktree refresh behavior updates the Worktrees list
- **AND** existing worktree and branch deletion success or warning logs remain available

#### Scenario: Failed delete clears progress state and allows retry

- **GIVEN** a worktree row is marked as deleting in the TUI
- **WHEN** the worktree deletion command fails
- **THEN** the deleting marker is cleared
- **AND** the existing failure popup or error log is shown
- **AND** the worktree can be selected for deletion again

#### Scenario: Deletion progress remains UI-only

- **GIVEN** a worktree row is marked as deleting in the TUI
- **WHEN** the scheduler, resume routing, acceptance, archive, or next-action selection logic runs
- **THEN** those workflow decisions do not read or depend on the TUI deletion progress marker
