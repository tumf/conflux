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
