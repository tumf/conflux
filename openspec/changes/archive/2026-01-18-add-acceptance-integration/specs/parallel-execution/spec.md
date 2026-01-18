## MODIFIED Requirements
### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.

#### Scenario: Parallel acceptance success proceeds to archive
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** acceptance output indicates PASS
- **THEN** the orchestrator proceeds to archive in that workspace

#### Scenario: Parallel acceptance failure returns to apply loop
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** acceptance output indicates FAIL with findings
- **THEN** the orchestrator returns the change to the apply loop and records the findings

#### Scenario: Parallel acceptance command execution failure
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** the acceptance_command exits with non-zero status
- **THEN** the orchestrator records the command failure and returns the change to the apply loop
