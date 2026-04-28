## MODIFIED Requirements

### Requirement: Native validator owns behavior-centric proposal checks

The native `cflx openspec validate` implementation MUST remain resilient when parsing behavior-task verification metadata from valid UTF-8 proposal files. Checkbox tasks MAY express verification ownership and evidence either inline as `(verification: ...)` or as an indented standalone `verification:` continuation line attached to the immediately preceding checkbox task. Validation MUST preserve UTF-8 character boundaries when reporting invalid task content so proposal validation returns structured findings instead of panicking.

#### Scenario: Standalone verification continuation line is accepted

- **GIVEN** a checkbox task line is followed by an indented standalone `verification: unit - ...` continuation line
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** the validator treats the continuation line as verification metadata for the preceding checkbox task
- **AND** evidence and ownership checks evaluate that verification text the same way they evaluate inline `(verification: ...)` notes

#### Scenario: Standalone verification line with multi-byte text does not panic

- **GIVEN** a checkbox task line is followed by an indented standalone `verification:` continuation line containing multi-byte UTF-8 text
- **AND** a legacy preview/reporting path would otherwise cut through a multi-byte character
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** validation does not panic
- **AND** it either accepts the verification metadata or emits a structured validation finding

#### Scenario: Invalid task content still reports findings on UTF-8 boundaries

- **GIVEN** a `tasks.md` line is still invalid task content after standalone verification parsing rules are applied
- **AND** the reported preview must be truncated for display
- **WHEN** `cflx openspec validate <change-id> --strict` runs
- **THEN** the reported preview is truncated on UTF-8 character boundaries
- **AND** validation returns a structured finding instead of a char-boundary panic
