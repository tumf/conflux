## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

**Acceptance state persistence**: Acceptance results are NOT persisted to disk or git commits. Therefore, on resume:
- If workspace state is `Applying` or `Created`: Normal apply+acceptance loop proceeds
- If workspace state is `Applied`: Acceptance MUST be re-run before archive
- If workspace state is `Archiving` (archive files moved but not committed): Acceptance MUST be re-run before archive commit
- If workspace state is `Archived` or `Merged`: Acceptance is not required (archive already complete)

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

**Archive commit creation**: When `ensure_archive_commit()` detects a dirty working tree after the archive command, it SHALL first attempt a direct `git add -A && git commit` with message `"Archive: {change_id}"` without invoking the AI resolve command. If the direct commit succeeds and `is_archive_commit_complete()` returns true, the archive commit is considered finalized. If the direct commit fails (e.g., due to pre-commit hooks modifying files or rejecting the commit), the system SHALL fall back to the AI resolve command for recovery.

#### Scenario: Parallel acceptance retry narrows to updated files and prior findings
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **AND** acceptance output indicates CONTINUE
- **WHEN** the orchestrator runs a subsequent acceptance attempt for the same change
- **THEN** the acceptance prompt includes only the updated file list since the previous acceptance attempt (no diff content)
- **AND** the acceptance prompt includes the prior acceptance findings for verification
- **AND** the acceptance prompt instructs the agent to read files as needed to confirm fixes

#### Scenario: Parallel acceptance failure logging uses tail lines
- **GIVEN** acceptance output tail includes `ACCEPTANCE: FAIL` and `FINDINGS:` lines
- **WHEN** the orchestrator records the acceptance failure
- **THEN** the recorded findings exclude the acceptance markers and `FINDINGS:` header
- **AND** logs do not report "N findings" based on tail line count

#### Scenario: Acceptance blocked preserves workspace and stops apply
- **GIVEN** acceptance output indicates `ACCEPTANCE: BLOCKED`
- **WHEN** the orchestrator processes the acceptance result
- **THEN** the workspace is preserved for manual follow-up
- **AND** apply retries for the change are stopped in the current run

#### Scenario: Direct archive commit succeeds without AI resolve
- **GIVEN** a change has been archived (files moved to archive directory)
- **AND** the working tree is dirty with uncommitted archive changes
- **WHEN** `ensure_archive_commit()` is called
- **THEN** the system executes `git add -A && git commit -m "Archive: {change_id}"` directly
- **AND** the AI resolve command is NOT invoked
- **AND** `is_archive_commit_complete()` returns true

#### Scenario: Direct archive commit fails and falls back to AI resolve
- **GIVEN** a change has been archived (files moved to archive directory)
- **AND** the working tree is dirty with uncommitted archive changes
- **AND** a pre-commit hook rejects or modifies the commit
- **WHEN** `ensure_archive_commit()` attempts the direct commit
- **AND** the direct commit fails (non-zero exit code)
- **THEN** the system logs a warning about the direct commit failure
- **AND** the system falls back to the AI resolve command
- **AND** the AI resolve command attempts to finalize the archive commit
