## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

Parallel execution MUST persist enough workspace-local acceptance state to distinguish `pending`, `running`, `passed`, and non-pass terminal outcomes for the latest apply revision.
Archive MUST NOT start unless the latest acceptance state for the current workspace revision is durably recorded as `passed`.
If the orchestrator restarts after acceptance started but before a final verdict is recorded, the resumed workspace MUST treat that acceptance attempt as incomplete and MUST rerun acceptance before archive.
The workspace-local acceptance state artifact `.cflx/acceptance-state.json` MUST be excluded from Git dirty-worktree checks via the workspace's effective `info/exclude`, without requiring a repository-tracked `.gitignore` entry.

#### Scenario: Acceptance state artifact stays out of dirty-worktree checks

- **GIVEN** a workspace has no repository-tracked ignore rule for `.cflx/acceptance-state.json`
- **AND** Conflux is about to save durable acceptance state in that workspace
- **WHEN** the state file is created or updated
- **THEN** Conflux ensures the workspace's effective `info/exclude` ignores `.cflx/acceptance-state.json`
- **AND** `git status --porcelain` for that workspace does not report `.cflx/acceptance-state.json` as an untracked file
