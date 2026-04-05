## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

Parallel execution MUST persist enough acceptance state to distinguish `pending`, `running`, `passed`, and non-pass terminal outcomes for the latest apply revision.
Archive MUST NOT start unless the latest acceptance state for the current workspace revision is durably recorded as `passed`.
If the orchestrator restarts after acceptance started but before a final verdict is recorded, the resumed workspace MUST treat that acceptance attempt as incomplete and MUST rerun acceptance before archive.
The durable acceptance state artifact MUST NOT be written inside the Git worktree being evaluated for apply/acceptance/archive/merge progression.

#### Scenario: Acceptance state is durable without creating a worktree file

- **GIVEN** a parallel workspace has completed apply
- **WHEN** Conflux records acceptance state for resume and archive safety
- **THEN** the state remains durably available across restart
- **AND** no `.cflx/acceptance-state.json` file is created inside that workspace's Git worktree

#### Scenario: Interrupted acceptance still reruns before archive without worktree artifact

- **GIVEN** a parallel workspace recorded acceptance as `running`
- **AND** the process stopped before recording a final verdict
- **WHEN** the workspace is resumed
- **THEN** the orchestrator reruns acceptance before archive
- **AND** it does so without relying on a worktree-local acceptance-state file

#### Scenario: Stale durable pass for another revision does not unlock archive

- **GIVEN** external acceptance state exists for a workspace
- **AND** that state is `passed`
- **AND** the stored revision does not match the workspace's current revision
- **WHEN** resume routing or archive preconditions are evaluated
- **THEN** the orchestrator MUST NOT treat that state as a valid pass for archive
- **AND** it reruns acceptance before archive
