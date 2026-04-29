## ADDED Requirements

### Requirement: Optional design documents MUST NOT be surfaced as apply or acceptance errors

`openspec/changes/<change-id>/design.md` is an optional context artifact. When apply or acceptance context gathering attempts to read it and the file does not exist, the runtime MUST treat that outcome as a skip/informational condition and continue processing. The absence of `design.md` MUST NOT be surfaced as a user-visible change error or be counted as a change failure by itself.

#### Scenario: missing optional design doc is skipped without error
- **GIVEN** an active change contains `proposal.md` and `tasks.md`
- **AND** `openspec/changes/<change-id>/design.md` does not exist
- **WHEN** apply or acceptance context gathering reads proposal-side artifacts
- **THEN** the runtime records the design read as skipped or informational
- **AND** change processing continues without emitting a user-visible `Error: File not found` for `design.md`
- **AND** the change is not marked failed solely because `design.md` is absent

#### Scenario: required proposal artifacts still fail when missing
- **GIVEN** apply or acceptance context gathering reads proposal-side artifacts
- **WHEN** `proposal.md` or `tasks.md` is missing
- **THEN** the runtime emits a failure outcome
- **AND** the missing required artifact is surfaced as an error distinct from optional `design.md` absence
