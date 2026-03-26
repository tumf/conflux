## ADDED Requirements

### Requirement: Focused blocked rows remain readable

When the TUI displays a change row that is visually blocked or dimmed to communicate a restriction, the focused-row highlight SHALL preserve readable contrast for the row's primary text and badges. The blocked/dimmed meaning and the focused/cursor meaning MUST remain visually distinguishable from each other.

This requirement applies to the Changes list in both Select and Running views, including rows that are marked as parallel-ineligible because the Git working tree has uncommitted or untracked files.

#### Scenario: Focused uncommitted row in Select view remains legible

- **GIVEN** the TUI is in the Changes list Select view
- **AND** a change row is marked as parallel-ineligible because uncommitted or untracked files exist
- **AND** the cursor is on that row
- **WHEN** the row is rendered
- **THEN** the change ID and blocked badge remain readable
- **AND** the row still appears visually blocked compared with a normal actionable row
- **AND** the cursor/focus state remains visually apparent

#### Scenario: Focused uncommitted row in Running view remains legible

- **GIVEN** the TUI is in the Changes list Running view
- **AND** a change row is marked as parallel-ineligible because uncommitted or untracked files exist
- **AND** the cursor is on that row
- **WHEN** the row is rendered
- **THEN** the change ID, badges, and progress/status text remain readable
- **AND** the blocked state remains distinguishable from the focus state

#### Scenario: Unfocused blocked rows remain visually de-emphasized

- **GIVEN** the TUI displays a blocked or dimmed change row
- **AND** the cursor is on a different row
- **WHEN** the list is rendered
- **THEN** the blocked row remains visually de-emphasized relative to the focused row
- **AND** the contrast fix does not make blocked rows appear like normal actionable rows
