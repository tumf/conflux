## MODIFIED Requirements

### Requirement: Orchestration loop runs apply and archive
The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.
The orchestrator SHALL execute an acceptance loop after a successful apply and before starting archive.
The acceptance loop SHALL run `acceptance_command` for the change, parse the output text to determine acceptance success, failure, or continue, and route the change accordingly.
- Exit code indicates command execution success, not acceptance verdict.
- Acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
- If the output indicates CONTINUE, the orchestrator MUST retry acceptance up to `acceptance_max_continues` times.
- If the CONTINUE limit is exceeded, the orchestrator MUST treat the outcome as FAIL and return to the apply loop.

#### Scenario: Acceptance continue retries
- **GIVEN** a change completes an apply iteration successfully
- **WHEN** acceptance output indicates CONTINUE
- **THEN** the orchestrator retries acceptance for the same change
- **AND** the retry count is incremented

#### Scenario: Acceptance continue limit exceeded
- **GIVEN** acceptance has returned CONTINUE `acceptance_max_continues` times for a change
- **WHEN** the next acceptance output indicates CONTINUE
- **THEN** the orchestrator treats the outcome as FAIL
- **AND** the change returns to the apply loop
