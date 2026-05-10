## MODIFIED Requirements

### Requirement: REQ-OBS-001 Command Execution Logging

The bundled Conflux log mining helper MUST be able to scan marker-selected runtime logs incrementally for actionable errors, manual operation markers, and resolve/merge timeline markers without requiring whole-file buffering. The helper output MUST remain observability-only and MUST NOT be used as a workflow-control input.

#### Scenario: large marker-selected log is mined without whole-file buffering

- **GIVEN** a log root contains `.last-checked` and a large `.log` file whose mtime is newer than the marker
- **WHEN** an operator runs `python3 scripts/cflx-log-mine.py --log-root <log-root> --top 30`
- **THEN** the helper emits the standard report sections for top error/warning groups, manual operation markers, action timeline markers, and recommended follow-up queries
- **AND** the scanner does not need to read the entire log file into memory before processing hits
- **AND** no mined log output affects scheduling, resume routing, acceptance, archive, merge, or next-action behavior

#### Scenario: change-id filtering remains compatible under streaming scan

- **GIVEN** a marker-selected log contains events for change `alpha` and unrelated events for change `beta`
- **WHEN** an operator runs `python3 scripts/cflx-log-mine.py --log-root <log-root> --change-id alpha --format json`
- **THEN** returned grouped examples, manual events, and action events are limited to hits whose text or captured context includes `alpha`
- **AND** the JSON report keeps the existing top-level keys used by consumers

#### Scenario: grouped diagnostics continue to normalize volatile local details

- **GIVEN** a marker-selected log line contains volatile values such as local absolute paths, process ids, project ids, branch names, or change ids
- **WHEN** the helper groups the diagnostic
- **THEN** the group key normalizes those volatile values consistently with existing behavior
- **AND** the helper does not write confidential mined log content into repository-tracked proposal or test artifacts
