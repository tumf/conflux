## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.
The second and later acceptance attempts MUST focus on the updated file list since the previous acceptance attempt and the previously reported findings, rather than performing a full re-check.
The acceptance prompt for second and later attempts MUST include the updated file list (file paths only) since the previous acceptance attempt.
The acceptance prompt for second and later attempts MUST include the previous acceptance findings and instruct the agent to verify whether those findings are resolved.
The acceptance prompt for second and later attempts MUST instruct the agent to read relevant files as needed; it MUST NOT include diff content.
Acceptance findings MUST exclude `ACCEPTANCE:` markers and the `FINDINGS:` header line from the recorded tail lines.
If acceptance output is BLOCKED, the orchestrator MUST stop apply retries for the change and preserve the workspace for manual follow-up.
If acceptance output is BLOCKED, the change MUST be recorded as a terminal failure for dependency skipping in the current run.
Before allowing archive to start, acceptance MUST verify that the workspace is ready for a real final archive commit under the repository's final-commit quality gates (SHALL). If those readiness checks fail, acceptance MUST return a non-pass verdict and record the blocking gate or command context instead of allowing archive to surface the failure later (MUST).

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

#### Scenario: Acceptance catches archive-readiness blocker before archive
- **GIVEN** apply has produced a workspace that appears functionally complete
- **AND** the final archive commit would be rejected by a repository quality gate such as a pre-commit hook, formatting check, lint check, or test gate
- **WHEN** acceptance evaluates archive-readiness
- **THEN** acceptance returns a non-pass verdict before archive starts
- **AND** acceptance findings identify the blocking gate or command context so the failure is actionable

#### Scenario: Acceptance passes archive-ready workspace to archive
- **GIVEN** apply has produced a workspace with no unresolved acceptance findings
- **AND** the workspace satisfies the repository's final-commit quality gates for archive
- **WHEN** acceptance completes
- **THEN** the change may proceed to archive
- **AND** archive remains responsible for executing and verifying the final archive commit
