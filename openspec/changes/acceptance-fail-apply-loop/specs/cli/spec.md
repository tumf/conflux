## MODIFIED Requirements
### Requirement: Orchestration loop runs apply and archive
The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.
The orchestrator SHALL execute an acceptance loop after a successful apply and before starting archive.
The acceptance loop SHALL run `acceptance_command` for the change, parse the output text to determine acceptance success or failure, and route the change accordingly.
- Exit code indicates command execution success, not acceptance verdict.
- Acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
- When acceptance fails, the orchestrator MUST update tasks.md before returning to the apply loop.
- Task updates MUST either add a new follow-up task or uncheck a previously completed task that must be revisited.
- The acceptance failure reason MUST be recorded in tasks.md together with the task update.
- The apply loop MUST resume with the same iteration counter value (no reset) after acceptance failure.

#### Scenario: Acceptance failure returns to apply loop with task updates
- **GIVEN** a change completes an apply iteration successfully
- **WHEN** acceptance output indicates FAIL with findings
- **THEN** the orchestrator updates tasks.md with a follow-up task or unchecks a completed task
- **AND** the acceptance failure reason is recorded in tasks.md
- **AND** the orchestrator returns the change to the apply loop without resetting the iteration counter
