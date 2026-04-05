## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

Parallel execution MUST persist enough workspace-local acceptance state to distinguish `pending`, `running`, `passed`, and non-pass terminal outcomes for the latest apply revision.
Archive MUST NOT start unless the latest acceptance state for the current workspace revision is durably recorded as `passed`.
If the orchestrator restarts after acceptance started but before a final verdict is recorded, the resumed workspace MUST treat that acceptance attempt as incomplete and MUST rerun acceptance before archive.
For implementation changes, resumed workspaces with unchecked items under `## Implementation Tasks` MUST be routed back to apply before acceptance or archive routing is considered.

#### Scenario: Incomplete implementation tasks force Apply on resume

- **GIVEN** a resumed workspace belongs to an implementation change
- **AND** `tasks.md` still contains unchecked items under `## Implementation Tasks`
- **WHEN** resume routing is evaluated
- **THEN** the workspace is routed to apply
- **AND** it is not routed to acceptance

#### Scenario: Completed tasks keep existing acceptance routing

- **GIVEN** a resumed workspace belongs to an implementation change
- **AND** all items under `## Implementation Tasks` are complete
- **AND** the latest durable acceptance state is `pending`, `running`, or `failed`
- **WHEN** resume routing is evaluated
- **THEN** the workspace is routed to acceptance
- **AND** archive is not started
