## MODIFIED Requirements
### Requirement: Orchestration loop runs apply and archive
The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.
The orchestrator SHALL execute an acceptance loop after a successful apply and before starting archive.
The acceptance loop SHALL run `acceptance_command` for the change, parse the output text to determine acceptance success or failure, and route the change accordingly.
- Exit code indicates command execution success, not acceptance verdict.
- Acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.

#### Scenario: Acceptance success proceeds to archive
- **GIVEN** a change completes an apply iteration successfully
- **WHEN** acceptance output indicates PASS
- **THEN** the orchestrator proceeds to archive for that change

#### Scenario: Acceptance failure returns to apply loop
- **GIVEN** a change completes an apply iteration successfully
- **WHEN** acceptance output indicates FAIL with findings
- **THEN** the orchestrator returns the change to the apply loop and records the findings

#### Scenario: Acceptance command execution failure
- **GIVEN** a change completes an apply iteration successfully
- **WHEN** the acceptance_command exits with non-zero status
- **THEN** the orchestrator records the command failure and returns the change to the apply loop
