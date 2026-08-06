## MODIFIED Requirements

### Requirement: Apply completion grace requires stable repository completion

When runtime observes an Apply completion condition while the owned Apply command is still running, it MAY start a bounded grace period before terminating the child. A task-completion condition is eligible for that grace only when task progress was incomplete at the start of the active command dispatch and became complete during that dispatch. Task progress that was already complete when a stage, task-format, final-commit-hook, or other task-complete repair command began MUST NOT by itself start, refresh, or finalize completion grace for that command.

Blocked and rejecting handoff conditions remain eligible regardless of task progress at dispatch start. Runtime MUST re-evaluate the same eligible repository completion condition when the grace period expires and MUST terminate the child only if that condition remains present. If an eligible completion disappears or changes during the grace period, runtime MUST cancel or restart the deadline for the current eligible condition and continue Apply. After grace-driven termination, runtime MUST preserve the existing process-group cleanup and quiescence gate before repository finalization or handoff.

Dispatch-start eligibility MAY be retained as ephemeral in-memory state for the lifetime of the owned command. Restart routing MUST remain derived from workspace file state, Git state, and base-branch evidence and MUST NOT depend on persisted dispatch state.

#### Scenario: Active dispatch reaches stable task completion

- **GIVEN** task progress is incomplete when an Apply command is dispatched
- **AND** the active command makes task progress complete while the owned child remains running
- **WHEN** the task-complete condition remains present through the bounded grace period
- **THEN** runtime terminates the lingering owned process group through the existing cleanup path
- **AND** repository finalization may continue only after process-group quiescence is confirmed

#### Scenario: Transient task completion does not terminate Apply

- **GIVEN** task progress is incomplete when an Apply command is dispatched
- **AND** `tasks.md` becomes complete while the Apply child remains running
- **AND** runtime starts its completion grace period
- **AND** `tasks.md` becomes incomplete before the grace period expires
- **WHEN** runtime rechecks repository state at the deadline
- **THEN** it does not terminate the child based on the stale completion observation
- **AND** Apply continues until an eligible completion condition remains stable or the child exits

#### Scenario: Pre-complete stage repair outlives completion grace

- **GIVEN** task progress is already complete when finalization rejects an unstaged or untracked workspace
- **AND** Conflux dispatches an Apply command to repair explicit staging
- **WHEN** the repair command remains active longer than the configured completion grace before staging the intended files
- **THEN** the pre-existing task completion does not terminate that repair command
- **AND** the command may complete normally before finalization is retried

#### Scenario: Pre-complete task-format repair outlives completion grace

- **GIVEN** all checkboxes are complete but worktree-local task-format validation requires another Apply command
- **WHEN** the task-format repair remains active longer than the configured completion grace before correcting `tasks.md`
- **THEN** the pre-existing task completion does not terminate that repair command
- **AND** Acceptance remains undispatched until the repair exits and task-format validation succeeds

#### Scenario: Pre-complete final-commit-hook repair outlives completion grace

- **GIVEN** all tasks are complete and a hook-enabled final Apply commit was rejected with repository-fixable diagnostics
- **AND** Conflux dispatches the required repair Apply command
- **WHEN** the repair remains active longer than the configured completion grace before resolving the rejection
- **THEN** the pre-existing task completion does not terminate that repair command
- **AND** finalization is retried through the existing hook-enabled commit path only after the repair command exits and process-group quiescence is confirmed

#### Scenario: Repair dispatch still terminates for blocked handoff

- **GIVEN** task progress was already complete when an Apply repair command began
- **AND** that active command creates a valid `APPLY_BLOCKED` handoff artifact and leaves its child running
- **WHEN** the blocked condition remains stable through completion grace
- **THEN** runtime terminates the lingering owned process group through the existing cleanup path
- **AND** Apply returns the blocked handoff rather than treating pre-existing task completion as success

#### Scenario: Repair dispatch still terminates for rejecting handoff

- **GIVEN** task progress was already complete when an Apply repair command began
- **AND** that active command creates `REJECTED.md` and leaves its child running
- **WHEN** the rejecting condition remains stable through completion grace
- **THEN** runtime terminates the lingering owned process group through the existing cleanup path
- **AND** Apply returns the rejecting handoff rather than treating pre-existing task completion as success

#### Scenario: Pre-complete repair failure remains a failed attempt

- **GIVEN** task progress was already complete when an Apply repair command began
- **AND** the command exits non-zero without completing its repair or creating a recognized handoff
- **WHEN** runtime classifies the command result
- **THEN** pre-existing task completion does not make the command success-equivalent
- **AND** existing failure, permission, retry, stall, and iteration-budget policy remains authoritative

#### Scenario: Restart recomputes repair without persisted dispatch policy

- **GIVEN** the process stops before a task-complete repair is finished
- **WHEN** Conflux resumes from the preserved workspace
- **THEN** it re-derives the next Apply action from workspace file state, Git state, and base-branch evidence
- **AND** no persisted dispatch-start completion value or out-of-worktree workflow-control state is required
