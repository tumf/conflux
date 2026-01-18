## MODIFIED Requirements
### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
- The acceptance verdict parsing MUST recognize PASS/FAIL/CONTINUE markers even when the marker line includes non-semantic decoration (example: Markdown emphasis or surrounding punctuation).
- When acceptance fails, the orchestrator MUST update tasks.md before returning to the apply loop.
- Task updates MUST either add a new follow-up task or uncheck a previously completed task that must be revisited.
- The acceptance failure reason MUST be recorded in tasks.md together with the task update.
- The apply loop MUST resume with the same iteration counter value (no reset) after acceptance failure.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

#### Scenario: Parallel acceptance output includes PASS with decoration
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** acceptance output contains a decorated PASS marker such as `**ACCEPTANCE: PASS**`
- **THEN** the orchestrator treats the outcome as PASS
- **AND** tasks.md is not updated for acceptance failure
