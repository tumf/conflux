## MODIFIED Requirements
### Requirement: Merge Key Hint Display Conditions

TUI Worktree View SHALL display "M: merge" key hint only when ALL of the following conditions are met:
- Not main worktree
- Not detached HEAD
- No merge conflicts
- Has branch name
- Has commits ahead of base branch
- No resolve operation in progress

TUI SHALL NOT display merge key hint when resolve is in progress.

#### Scenario: M key hidden while resolve in progress
- **GIVEN** TUI is in Worktrees view
- **AND** cursor is on a worktree that otherwise meets merge conditions
- **AND** a resolve operation is in progress
- **WHEN** the footer is rendered
- **THEN** the key hints SHALL NOT include "M: merge"
