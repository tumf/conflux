## MODIFIED Requirements

### Requirement: ParallelRunService rejection flow on blocked execution

ParallelRunService SHALL support blocked handoff from both acceptance and apply execution phases. When apply execution records a blocker by generating `openspec/changes/<change_id>/REJECTED.md` as a rejection proposal, the runtime SHALL transition the workspace into a dedicated `rejecting` stage even if `tasks.md` still contains unchecked implementation tasks. A workspace in `rejecting` SHALL NOT enter the normal acceptance flow. Instead, the runtime SHALL run rejection review and require one of two outcomes: `confirm_rejection` or `resume_apply`.

`confirm_rejection` SHALL execute the rejection flow and finalize the change as rejected after the base branch records `openspec/changes/<change_id>/REJECTED.md`. `resume_apply` SHALL delete the worktree-local `REJECTED.md`, append at least one non-rejection recovery task to the worktree-local `tasks.md`, and return the change to apply so that the blocker is addressed as normal implementation work.

Parallel rejection handling SHALL NOT rely on `openspec resolve <change_id>` and SHALL NOT merge additional worktree files into the base branch. When rejection is confirmed, the base branch SHALL receive only `openspec/changes/<change_id>/REJECTED.md`.

#### Scenario: apply rejection proposal enters rejecting stage

- **GIVEN** apply execution generates `openspec/changes/fix-auth/REJECTED.md` with a blocker reason
- **AND** `openspec/changes/fix-auth/tasks.md` still contains unchecked implementation tasks
- **WHEN** the runtime evaluates the apply result
- **THEN** the workspace enters `rejecting`
- **AND** the change does not enter the normal acceptance flow
- **AND** apply does not immediately retry the same change

#### Scenario: rejecting confirms rejection

- **GIVEN** parallel execution is reviewing a change in `rejecting`
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists in the worktree
- **WHEN** rejecting returns `confirm_rejection`
- **THEN** the rejection flow commits `openspec/changes/fix-auth/REJECTED.md` on the base branch
- **AND** the workspace result is returned as rejected
- **AND** no further resolve step is required to finalize the rejection

#### Scenario: rejecting resumes apply after dismissing reject proposal

- **GIVEN** parallel execution is reviewing a change in `rejecting`
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists in the worktree
- **WHEN** rejecting returns `resume_apply`
- **THEN** the worktree-local `openspec/changes/fix-auth/REJECTED.md` is removed
- **AND** `openspec/changes/fix-auth/tasks.md` gains at least one unchecked task describing a non-rejection recovery action
- **AND** the change returns to `applying`

#### Scenario: rejected worktree changes are not merged to base

- **GIVEN** a rejected worktree contains code, tasks, and spec changes in addition to `REJECTED.md`
- **WHEN** rejecting confirms rejection and the rejection flow completes
- **THEN** the base branch receives only `openspec/changes/fix-auth/REJECTED.md`
- **AND** the remaining worktree-only files are discarded with worktree cleanup

## ADDED Requirements

### Requirement: Parallel rejecting resume semantics

Parallel execution SHALL restore a non-terminal workspace containing `openspec/changes/<change_id>/REJECTED.md` into `rejecting` on resume instead of `accepting` or `applying`.

#### Scenario: resumed rejection proposal re-enters rejecting

- **GIVEN** parallel execution resumes a workspace that has not completed archive
- **AND** the workspace contains `openspec/changes/fix-auth/REJECTED.md`
- **WHEN** the orchestrator determines the next non-terminal step
- **THEN** the next step is `rejecting`
- **AND** the workspace does not run the normal acceptance loop before rejection review
