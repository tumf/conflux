## NEW Requirements
### Requirement: Enter Key Operation Guidance

The TUI MUST display warning logs when the Enter key is ignored in Worktrees view, explaining the reason for rejection.

#### Scenario: Warning When Enter Is Ignored Outside Worktrees View

- **GIVEN** the TUI is displaying a view other than Worktrees
- **WHEN** the user presses the Enter key
- **THEN** the TUI outputs "Enter ignored: not in Worktrees view" to the warning log

#### Scenario: Warning When Enter Is Ignored Due to No Worktree Selection

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** no worktree is currently selected
- **WHEN** the user presses the Enter key
- **THEN** the TUI outputs "Enter ignored: no worktree selected" to the warning log

#### Scenario: Warning When Enter Is Ignored Due to Missing worktree_command Configuration

- **GIVEN** the TUI is displaying the Worktrees view
- **AND** worktree_command is not configured
- **WHEN** the user presses the Enter key
- **THEN** the TUI outputs "Enter ignored: worktree_command not configured" to the warning log
