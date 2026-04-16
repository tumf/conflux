## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail/continue/blocked, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include the configured acceptance prompt context required for the current change and revision.
When resuming a workspace that has not completed archive, the orchestrator SHALL determine the next non-terminal step from the worktree state and MUST NOT start archive directly.

**Acceptance state persistence**: Acceptance results are NOT persisted to disk or git commits. Therefore, on resume:
- If the resumed worktree is terminal (`Archived`, `Merged`, or rejected): apply/acceptance are not required.
- If the resumed worktree is non-terminal and its worktree-local `tasks.md` progress is 100%: acceptance MUST be re-run before archive.
- If the resumed worktree is non-terminal and its worktree-local `tasks.md` progress is below 100% or unavailable: the orchestrator MUST resume with apply instead of archive.

This ensures archive handoff guardrails are always enforced, even after interruptions.

- The second and later acceptance attempts MUST focus on the updated file list since the previous acceptance attempt and the previously reported findings, rather than performing a full re-check.
- The acceptance prompt for second and later attempts MUST include the updated file list (file paths only) since the previous acceptance attempt.
- The acceptance prompt for second and later attempts MUST include the previous acceptance findings and instruct the agent to verify whether those findings are resolved.
- The acceptance prompt for second and later attempts MUST instruct the agent to read relevant files as needed; it MUST NOT include diff content.
- Acceptance failures SHALL record findings using stdout/stderr tail lines without parsing `FINDINGS:` structure.
- Acceptance findings MUST exclude `ACCEPTANCE:` markers and the `FINDINGS:` header line from the recorded tail lines.
- Acceptance FAIL logs MUST NOT label tail line counts as "findings"; if counts are shown, they MUST be labeled as tail lines.
- If acceptance output is BLOCKED, the orchestrator MUST stop apply retries for the change and preserve the workspace for manual follow-up.
- If acceptance output is BLOCKED, the change MUST be recorded as a terminal failure for dependency skipping in the current run.
- Before allowing archive to start, acceptance MUST verify that the workspace is ready for the real final archive commit under the target repository's actual commit path (SHALL). If those readiness checks fail, acceptance MUST return a non-pass verdict and record the blocking commit-path context instead of allowing archive to surface the failure later (MUST).
- Acceptance MUST NOT assume that pre-commit hooks, tests, linters, or formatters exist in every repository (MUST NOT).
- Acceptance MAY treat a hook, command, or other gate as relevant only when it actually blocks the archive commit path for the current repository (MAY).

#### Scenario: Acceptance catches archive commit-path blocker before archive

- **GIVEN** apply has produced a workspace that appears functionally complete
- **AND** the final archive commit would be rejected by an actual blocker on the repository's commit path
- **WHEN** acceptance evaluates archive-readiness
- **THEN** acceptance returns a non-pass verdict before archive starts
- **AND** acceptance findings identify the blocking commit-path context so the failure is actionable

#### Scenario: Acceptance does not invent repo-wide toolchain gates

- **GIVEN** a repository does not define test, lint, format, or pre-commit execution as part of the actual archive commit path
- **WHEN** acceptance evaluates archive-readiness
- **THEN** acceptance does not fail merely because such gates are absent or not independently run
- **AND** archive-readiness remains centered on whether the archive commit can succeed

#### Scenario: Acceptance passes archive-ready workspace to archive

- **GIVEN** apply has produced a workspace with no unresolved acceptance findings
- **AND** the workspace satisfies the repository's actual final-commit path for archive
- **WHEN** acceptance completes
- **THEN** the change may proceed to archive
- **AND** archive remains responsible for executing and verifying the final archive commit
