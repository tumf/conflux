## MODIFIED Requirements
### Requirement: Orchestration loop runs apply and archive
The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.
The orchestrator SHALL execute an acceptance loop after a successful apply and before starting archive.
The acceptance loop SHALL run `acceptance_command` for the change, parse the output text to determine acceptance success, failure, or continue, and route the change accordingly.
- Exit code indicates command execution success, not acceptance verdict.
- Acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
- When acceptance fails, the orchestrator MUST update tasks.md before returning to the apply loop.
- Task updates MUST either add a new follow-up task or uncheck a previously completed task that must be revisited.
- The acceptance failure reason MUST be recorded in tasks.md together with the task update.
- The apply loop MUST resume with the same iteration counter value (no reset) after acceptance failure.
- If the output indicates CONTINUE, the orchestrator MUST retry acceptance up to `acceptance_max_continues` times.
- If no acceptance marker is present, the orchestrator MUST treat the outcome as CONTINUE and retry according to `acceptance_max_continues`.
- If the CONTINUE limit is exceeded, the orchestrator MUST treat the outcome as FAIL and return to the apply loop.

#### Scenario: Acceptance output with no marker defaults to CONTINUE
- **GIVEN** a change completes an apply iteration successfully
- **WHEN** acceptance output includes no PASS/FAIL/CONTINUE marker
- **THEN** the orchestrator treats the outcome as CONTINUE
- **AND** the retry count is incremented
