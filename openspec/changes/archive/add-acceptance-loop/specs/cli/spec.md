## MODIFIED Requirements
### Requirement: TUI Archive Priority Processing

The TUI running mode SHALL archive all completed changes before starting the next apply operation.

#### Scenario: Archive before next apply
- **WHEN** TUI is in running mode
- **AND** one or more queued changes have reached 100% task completion
- **THEN** all complete changes are archived before any new apply command starts
- **AND** the archive process follows the same hooks (pre_archive, post_archive) as normal archiving

#### Scenario: Multiple complete changes
- **WHEN** TUI is in running mode
- **AND** multiple changes reach 100% completion simultaneously
- **THEN** all complete changes are archived in sequence
- **AND** processing continues only after all complete changes are archived

#### Scenario: Archive on loop iteration
- **WHEN** TUI orchestrator starts a new processing iteration
- **THEN** it first checks for any complete changes in the queue
- **AND** archives all complete changes before selecting the next change to apply

## ADDED Requirements
### Requirement: Acceptance Loop Between Apply and Archive

The orchestrator SHALL execute an acceptance loop after a successful apply and before starting archive.

The acceptance loop SHALL run `acceptance_command` for the change, parse the output text to determine acceptance success or failure, and route the change accordingly.

- Exit code indicates command execution success, not acceptance verdict.
- Acceptance verdict MUST be derived from parsed stdout.
- Acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.

#### Scenario: Acceptance success continues to archive
- **GIVEN** apply completes successfully for change `add-feature`
- **WHEN** acceptance output indicates success
- **THEN** the orchestrator starts archive for `add-feature`

#### Scenario: Acceptance failure returns to apply loop
- **GIVEN** apply completes successfully for change `add-feature`
- **WHEN** acceptance output indicates failure with findings
- **THEN** the orchestrator records the findings in apply history
- **AND** the orchestrator returns `add-feature` to the apply loop

#### Scenario: Acceptance command execution failure
- **GIVEN** apply completes successfully for change `add-feature`
- **WHEN** acceptance_command exits with non-zero status
- **THEN** the orchestrator treats the run as failed
- **AND** apply does not resume until the failure is resolved

### Requirement: Acceptance Output Format

The acceptance_command output MUST be parsed using the following text format.

- Success output MUST include a line `ACCEPTANCE: PASS`.
- Failure output MUST include a line `ACCEPTANCE: FAIL`.
- Findings MUST be listed under a `FINDINGS:` header with one item per line prefixed by `- `.

#### Scenario: PASS output with no findings
- **GIVEN** acceptance_command outputs:
  ```text
  ACCEPTANCE: PASS
  ```
- **THEN** the orchestrator records acceptance as success

#### Scenario: FAIL output with findings
- **GIVEN** acceptance_command outputs:
  ```text
  ACCEPTANCE: FAIL
  FINDINGS:
  - Missing validation for input X
  - Error handling not implemented
  ```
- **THEN** the orchestrator records acceptance as failed
- **AND** the findings are stored for apply history
