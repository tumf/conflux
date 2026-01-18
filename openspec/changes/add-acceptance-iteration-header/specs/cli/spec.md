## MODIFIED Requirements

### Requirement: Orchestration loop runs apply and archive

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.
The orchestrator SHALL execute an acceptance loop after a successful apply and before starting archive.
The acceptance loop SHALL run `acceptance_command` for the change, parse the output text to determine acceptance success, failure, or continue, and route the change accordingly.
- Exit code indicates command execution success, not acceptance verdict.
- Acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
- The acceptance verdict parsing MUST recognize PASS/FAIL/CONTINUE markers even when the marker line includes non-semantic decoration (example: Markdown emphasis or surrounding punctuation).
- When acceptance fails, the orchestrator MUST update tasks.md before returning to the apply loop.
- Task updates MUST either add a new follow-up task or uncheck a previously completed task that must be revisited.
- The acceptance failure reason MUST be recorded in tasks.md together with the task update.
- The apply loop MUST resume with the same iteration counter value (no reset) after acceptance failure.
- If the output indicates CONTINUE, the orchestrator MUST retry acceptance up to `acceptance_max_continues` times.
- If no acceptance marker is present, the orchestrator MUST treat the outcome as CONTINUE and retry according to `acceptance_max_continues`.
- If the CONTINUE limit is exceeded, the orchestrator MUST treat the outcome as FAIL and return to the apply loop.
- The acceptance loop MUST carry forward its iteration counter when returning to the apply loop after acceptance failure.
- The TUI log panel MUST display acceptance log headers as `[{change_id}:acceptance:<iteration>]` when an iteration number is available.
- The TUI log panel MUST display acceptance log headers as `[{change_id}:acceptance]` when no iteration number is available.
- The acceptance loop iteration counter MUST increment on each acceptance attempt, and MUST NOT reset when the apply loop is re-entered due to acceptance failure.

#### Scenario: Acceptance output includes PASS with decoration
- **GIVEN** a change completes an apply iteration successfully
- **WHEN** acceptance output contains a decorated PASS marker such as `**ACCEPTANCE: PASS**`
- **THEN** the orchestrator treats the outcome as PASS
- **AND** tasks.md is not updated for acceptance failure

#### Scenario: Acceptance output with no marker defaults to CONTINUE
- **GIVEN** a change completes an apply iteration successfully
- **WHEN** acceptance output includes no PASS/FAIL/CONTINUE marker
- **THEN** the orchestrator treats the outcome as CONTINUE
- **AND** the retry count is incremented

#### Scenario: Acceptance log header includes iteration
- **WHEN** the TUI receives acceptance output logs with change_id and iteration
- **THEN** the log entry header is displayed as `[{change_id}:acceptance:<iteration>]`

#### Scenario: Acceptance log header without iteration
- **WHEN** the TUI receives acceptance output logs with change_id but no iteration
- **THEN** the log entry header is displayed as `[{change_id}:acceptance]`

#### Scenario: Acceptance iteration continues after failure
- **GIVEN** a change completes an apply iteration successfully
- **AND** acceptance output indicates FAIL
- **WHEN** the orchestrator returns the change to the apply loop
- **THEN** the next acceptance attempt uses the previous acceptance iteration counter without resetting
