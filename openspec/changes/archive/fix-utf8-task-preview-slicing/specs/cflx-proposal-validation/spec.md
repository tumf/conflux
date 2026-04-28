## MODIFIED Requirements

### Requirement: Native validator owns behavior-centric proposal checks

The native `cflx openspec validate` implementation MUST remain resilient when rendering task-preview text from valid UTF-8 proposal files. Preview truncation used in validation findings MUST preserve character boundaries so proposal validation reports structured results instead of panicking.

#### Scenario: Bare task warning with multi-byte characters does not panic

- **GIVEN** a change `tasks.md` contains a bare task line long enough to trigger the `Possible task without checkbox` preview
- **AND** the preview cutoff would fall inside a multi-byte UTF-8 character such as `§`
- **WHEN** `cflx openspec validate <change-id> --strict` runs
- **THEN** validation does not panic
- **AND** it reports the `Possible task without checkbox` finding normally

#### Scenario: Bare task preview truncates on character boundaries

- **GIVEN** a bare task line exceeds the validator preview length
- **WHEN** `cflx openspec validate <change-id> --strict` runs
- **THEN** the reported preview is truncated on UTF-8 character boundaries
- **AND** the preview remains human-readable instead of containing partial code points
