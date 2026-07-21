## ADDED Requirements

### Requirement: Acceptance follow-up rendering uses normalized finding scopes

Serial and parallel execution MUST use the shared normalized finding representation when runtime updates acceptance follow-up state. Repository-fixable findings MUST affect task completion; external blockers MUST remain non-checkbox metadata. Both modes MUST produce equivalent follow-up and prompt context for equivalent observations.

#### Scenario: serial and parallel render equivalent mixed findings

- **GIVEN** serial and parallel receive equivalent repository and external findings
- **WHEN** each persists follow-up and builds the next acceptance context
- **THEN** both produce the same repository task identities
- **AND** both preserve the same external blocker metadata
- **AND** neither replays prior attempt history

#### Scenario: re-reported identity reopens despite detail changes

- **GIVEN** a repository finding was completed in the current follow-up
- **AND** the latest FAIL reports the same stable identity with changed descriptive detail
- **WHEN** runtime updates the section
- **THEN** the finding is reopened as unchecked
- **AND** identities absent from the latest payload are removed
