## MODIFIED Requirements
### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

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

#### Scenario: Resume forces acceptance before archive
- **GIVEN** a workspace is resumed after interruption
- **AND** archive is not yet complete for the change
- **WHEN** the orchestrator resumes processing
- **THEN** acceptance_command is executed before any archive command

**Implementation Notes:**
- Acceptance results are not persisted across orchestrator sessions
- When resuming in `Applied` or `Archiving` states, acceptance must re-run before archive
- This ensures quality checks are not skipped even if acceptance completed before interruption
- Acceptance history is cleared after successful archive to prevent history bloat
