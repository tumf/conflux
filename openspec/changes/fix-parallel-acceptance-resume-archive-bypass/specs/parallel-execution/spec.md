## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

Parallel execution MUST persist enough workspace-local acceptance state to distinguish `pending`, `running`, `passed`, and non-pass terminal outcomes for the latest apply revision.
Archive MUST NOT start unless the latest acceptance state for the current workspace revision is durably recorded as `passed`.
If the orchestrator restarts after acceptance started but before a final verdict is recorded, the resumed workspace MUST treat that acceptance attempt as incomplete and MUST rerun acceptance before archive.

#### Scenario: Interrupted acceptance is rerun before archive on resume

- **GIVEN** a parallel workspace has completed apply and tasks are 100% complete
- **AND** the orchestrator recorded acceptance as `running`
- **AND** the process stopped before recording `ACCEPTANCE: PASS` or `ACCEPTANCE: FAIL`
- **WHEN** the workspace is resumed
- **THEN** the orchestrator reruns acceptance before archive
- **AND** it MUST NOT start archive from the interrupted acceptance state

#### Scenario: Applied workspace without durable acceptance pass cannot archive

- **GIVEN** a resumed parallel workspace is detected as `Applied`
- **AND** tasks are complete
- **AND** the latest durable acceptance state is `pending`, `running`, or `failed`
- **WHEN** resume routing is evaluated
- **THEN** the workspace is routed to acceptance
- **AND** archive is not started

#### Scenario: Archiving workspace without durable acceptance pass falls back to acceptance

- **GIVEN** a resumed parallel workspace is detected as `Archiving`
- **AND** archive files exist in the worktree
- **AND** the latest durable acceptance state for the current revision is not `passed`
- **WHEN** the orchestrator evaluates whether to continue archive
- **THEN** it MUST NOT run archive
- **AND** it reruns acceptance first

#### Scenario: Archive guard rejects missing acceptance pass for current revision

- **GIVEN** a parallel workspace is about to start archive
- **AND** tasks are complete
- **AND** no durable acceptance `passed` state exists for the current workspace revision
- **WHEN** archive preconditions are checked
- **THEN** the archive command is not executed
- **AND** the orchestrator logs that acceptance must be rerun before archive
