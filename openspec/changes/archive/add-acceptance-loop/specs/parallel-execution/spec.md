## ADDED Requirements
### Requirement: Parallel Acceptance Loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.

The acceptance loop SHALL parse stdout to determine pass/fail, and MUST NOT use exit code to determine acceptance verdict.

The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.

#### Scenario: Parallel acceptance success proceeds to archive
- **GIVEN** parallel mode and apply completes for change `add-feature`
- **WHEN** acceptance output indicates PASS
- **THEN** archive starts for the workspace

#### Scenario: Parallel acceptance failure returns to apply loop
- **GIVEN** parallel mode and apply completes for change `add-feature`
- **WHEN** acceptance output indicates FAIL with findings
- **THEN** the findings are recorded to apply history
- **AND** apply is retried for the same workspace

#### Scenario: Parallel acceptance command execution failure
- **GIVEN** parallel mode and apply completes for change `add-feature`
- **WHEN** acceptance_command exits non-zero
- **THEN** the workspace is marked failed
- **AND** archive is not executed
