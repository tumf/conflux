## MODIFIED Requirements
### Requirement: Parallel execution acceptance loop
Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace.
The acceptance loop SHALL parse stdout to determine pass/fail, and MUST NOT use exit code to determine acceptance verdict.
The acceptance prompt MUST include a hardcoded acceptance prompt followed by configured `acceptance_prompt`.
When resuming a workspace that has not completed archive, the orchestrator SHALL re-run acceptance before starting archive, even if tasks are already complete.

**Acceptance state persistence**: Acceptance results are NOT persisted to disk or git commits. Therefore, on resume:
- If workspace state is `Applying` or `Created`: Normal apply+acceptance loop proceeds
- If workspace state is `Applied`: Acceptance MUST be re-run before archive
- If workspace state is `Archiving` (archive files moved but not committed): Acceptance MUST be re-run before archive commit
- If workspace state is `Archived` or `Merged`: Acceptance is not required (archive already complete)

This ensures quality gates are always enforced, even after interruptions.

#### Scenario: Parallel acceptance failure records tail output
- **GIVEN** a change completes an apply iteration successfully in parallel mode
- **WHEN** acceptance output indicates FAIL
- **THEN** the orchestrator returns the change to the apply loop and records the acceptance output tail in tasks.md
