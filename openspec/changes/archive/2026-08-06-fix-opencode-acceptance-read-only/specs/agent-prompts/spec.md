## MODIFIED Requirements

### Requirement: Acceptance review MUST be read-only and runtime MUST own FAIL follow-up

Acceptance guidance MUST define acceptance as read-only review and MUST NOT instruct the acceptance agent to edit `tasks.md`. When acceptance returns FAIL, runtime MUST own persistence of one `## Current Acceptance Follow-up` section, replacing prior runtime-managed numbered follow-up sections rather than accumulating attempt history.

The current section MUST represent repository-fixable findings from the latest FAIL as normalized unchecked tasks. A stable identity that reappears in the latest FAIL MUST be reopened even if a prior follow-up marked it complete. External or non-mockable blockers MUST be retained as non-checkbox metadata with evidence and next action. Runtime MUST NOT write verdict protocol markers into `tasks.md`.

#### Scenario: runtime writes one current follow-up

- **GIVEN** acceptance returns repository-fixable FAIL findings
- **WHEN** runtime persists follow-up work
- **THEN** `tasks.md` contains exactly one current runtime-managed acceptance section
- **AND** repository findings appear once as tasks
- **AND** prior numbered runtime follow-up sections are replaced

#### Scenario: Acceptance review remains read-only

- **GIVEN** acceptance guidance is used to review a change
- **WHEN** the reviewer prepares a FAIL verdict
- **THEN** the reviewer returns actionable findings without editing `tasks.md`
- **AND** runtime persists the current follow-up without writing verdict protocol markers

#### Scenario: external blocker is not converted into implementation work

- **GIVEN** a FAIL includes a repository defect and an external blocker
- **WHEN** runtime writes current follow-up
- **THEN** only the repository defect is a checkbox task
- **AND** the external blocker is metadata with evidence and next action

#### Scenario: OpenCode adapter delegates follow-up persistence

- **GIVEN** the tracked OpenCode Acceptance command adapter is used to review a change
- **WHEN** the reviewer prepares a FAIL verdict
- **THEN** the adapter requires the reviewer to return all actionable findings without editing `tasks.md` or `## Current Acceptance Follow-up`
- **AND** the adapter delegates normalized follow-up persistence to Conflux runtime
- **AND** the adapter does not instruct the reviewer to derive an attempt number or create a numbered `## Acceptance #N Failure Follow-up` section
