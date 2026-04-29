## ADDED Requirements

### Requirement: Acceptance follow-up persistence failure must not override primary acceptance failure

When acceptance returns a non-pass verdict with findings, the runtime SHALL preserve that acceptance verdict as the primary outcome even if follow-up persistence into `tasks.md` degrades.

The runtime SHALL attempt to persist acceptance follow-up findings to the canonical tasks location for the workspace. If the active change tasks path does not exist, the runtime MAY explore an archived tasks location or another canonical fallback.

Failure to persist follow-up findings MUST NOT by itself convert an acceptance `FAIL` into a terminal execution `Error` unless the primary acceptance outcome itself is indeterminate.

If persistence degrades, the runtime SHALL record the explored path(s) and expose the persistence issue as supplemental warning/error context rather than replacing the acceptance diagnosis.

#### Scenario: active tasks path missing does not override acceptance fail
- **GIVEN** acceptance returns `FAIL` with findings for change `alpha`
- **AND** `openspec/changes/alpha/tasks.md` does not exist in the workspace
- **WHEN** the runtime attempts to persist acceptance follow-up findings
- **THEN** the primary outcome remains acceptance `FAIL`
- **AND** the runtime does not convert the change into terminal execution `Error` solely because the active tasks path is missing

#### Scenario: archived tasks path can receive acceptance follow-up
- **GIVEN** acceptance returns `FAIL` with findings for change `beta`
- **AND** the active tasks path is absent
- **AND** an archived tasks file for the same change exists in the workspace
- **WHEN** the runtime persists acceptance follow-up findings
- **THEN** the runtime appends the follow-up to the archived tasks file
- **AND** the primary outcome remains acceptance `FAIL`

#### Scenario: missing all tasks paths reports degradation as supplemental context
- **GIVEN** acceptance returns `FAIL` with findings for change `gamma`
- **AND** neither an active tasks path nor an archived tasks path exists
- **WHEN** the runtime attempts to persist the follow-up
- **THEN** the runtime reports which canonical paths were explored
- **AND** the persistence failure is surfaced as supplemental degradation context
- **AND** the primary acceptance diagnosis is not replaced by a generic tasks-file execution error
