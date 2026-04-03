## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL determine the next non-terminal step from the worktree state and MUST NOT start archive directly.

**Acceptance state persistence**: Acceptance results are NOT persisted to disk or git commits. Therefore, on resume:
- If the resumed worktree is terminal (`Archived`, `Merged`, or rejected): apply/acceptance are not required.
- If the resumed worktree is non-terminal and its worktree-local `tasks.md` progress is 100%: acceptance MUST be re-run before archive.
- If the resumed worktree is non-terminal and its worktree-local `tasks.md` progress is below 100% or unavailable: the orchestrator MUST resume with apply instead of archive.

This ensures quality gates are always enforced, even after interruptions.

- The second and later acceptance attempts MUST focus on the updated file list since the previous acceptance attempt and the previously reported findings, rather than performing a full re-check.
- The acceptance prompt for second and later attempts MUST include the updated file list (file paths only) since the previous acceptance attempt.
- The acceptance prompt for second and later attempts MUST include the previous acceptance findings and instruct the agent to verify whether those findings are resolved.
- The acceptance prompt for second and later attempts MUST instruct the agent to read relevant files as needed; it MUST NOT include diff content.
- Acceptance failures SHALL record findings using stdout/stderr tail lines without parsing `FINDINGS:` structure.
- Acceptance findings MUST exclude `ACCEPTANCE:` markers and the `FINDINGS:` header line from the recorded tail lines.
- Acceptance FAIL logs MUST NOT label tail line counts as "findings"; if counts are shown, they MUST be labeled as tail lines.
- If acceptance output is BLOCKED, the orchestrator MUST stop apply retries for the change and preserve the workspace for manual follow-up.
- If acceptance output is BLOCKED, the change MUST be recorded as a terminal failure for dependency skipping in the current run.

#### Scenario: Resumed worktree with incomplete tasks returns to apply
- **GIVEN** a parallel-mode worktree is resumed and has not completed archive
- **AND** the worktree-local `openspec/changes/{change_id}/tasks.md` progress is below 100%
- **WHEN** the orchestrator chooses the next step for the resumed change
- **THEN** it resumes with apply
- **AND** it does not start archive directly

#### Scenario: Resumed worktree with complete tasks returns to acceptance
- **GIVEN** a parallel-mode worktree is resumed and has not completed archive
- **AND** the worktree-local task progress for the change is 100%
- **WHEN** the orchestrator chooses the next step for the resumed change
- **THEN** it resumes with acceptance
- **AND** archive starts only after that acceptance pass succeeds

### Requirement: Workspace State Detection
Existing workspaces SHALL be classified from worktree state in a way that preserves canonical execution ordering for resume.

Archive-complete terminal detection MAY still use committed file state, but non-terminal resumed worktrees MUST NOT be classified into a direct-archive execution path.

When a reused worktree is not archive-complete:
- the orchestrator MUST inspect worktree-local task progress for the change
- it MUST choose `apply` when progress is below 100% or unavailable
- it MUST choose `acceptance` when progress is 100%
- it MUST NOT choose archive directly

#### Scenario: Non-terminal resumed worktree never routes directly to archive
- **GIVEN** a reused worktree is neither archive-complete nor merged
- **WHEN** resume classification is performed
- **THEN** the next execution step is either apply or acceptance
- **AND** archive is not selected as the first resumed non-terminal step
