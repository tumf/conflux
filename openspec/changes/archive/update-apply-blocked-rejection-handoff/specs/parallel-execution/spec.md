## MODIFIED Requirements

### Requirement: ParallelRunService rejection flow on blocked execution

ParallelRunService SHALL support blocked handoff from both acceptance and apply execution phases. When apply execution records a blocker by generating `openspec/changes/<change_id>/REJECTED.md` as a rejection proposal, the runtime SHALL treat the workspace as `apply blocked` even if `tasks.md` still contains unchecked items. An `apply blocked` workspace SHALL proceed to acceptance instead of being retried indefinitely as fresh apply work. Acceptance SHALL decide whether to confirm the rejection proposal, and only a confirmed blocked verdict SHALL execute the rejection flow.

#### Scenario: apply blocker proposal reaches acceptance

- **GIVEN** apply execution generates `openspec/changes/fix-auth/REJECTED.md` with a blocker reason
- **AND** `openspec/changes/fix-auth/tasks.md` still contains unchecked implementation tasks
- **WHEN** the runtime evaluates the apply result
- **THEN** the workspace is treated as `apply blocked`
- **AND** the change proceeds to acceptance instead of looping in apply retries

#### Scenario: confirmed blocked verdict runs rejection flow

- **GIVEN** acceptance receives a change in `apply blocked` state with a rejection proposal
- **WHEN** acceptance confirms the blocked verdict
- **THEN** the rejection flow executes
- **AND** the worktree is cleaned up after rejection completes

#### Scenario: unconfirmed blocker does not trigger rejection flow

- **GIVEN** acceptance receives a change in `apply blocked` state with a rejection proposal
- **WHEN** acceptance does not confirm rejection
- **THEN** the rejection flow does not execute
- **AND** the runtime returns the change to a non-terminal state for further action
