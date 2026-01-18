## MODIFIED Requirements
### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
- When acceptance fails, the orchestrator MUST update tasks.md before returning to the apply loop.
- Task updates MUST either add a new follow-up task or uncheck a previously completed task that must be revisited.
- The acceptance failure reason MUST be recorded in tasks.md together with the task update.
- The apply loop MUST resume with the same iteration counter value (no reset) after acceptance failure.

#### Scenario: Parallel acceptance failure returns to apply loop with task updates
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** acceptance output indicates FAIL with findings
- **THEN** the orchestrator updates tasks.md with a follow-up task or unchecks a completed task
- **AND** the acceptance failure reason is recorded in tasks.md
- **AND** the orchestrator returns the change to the apply loop without resetting the iteration counter
