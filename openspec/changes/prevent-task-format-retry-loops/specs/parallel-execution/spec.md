## ADDED Requirements

### Requirement: Apply completion MUST validate task format before acceptance

After repository task progress appears complete and before acceptance starts, Conflux MUST deterministically validate the worktree-local `tasks.md` task-format contract. A task-format failure MUST keep the change in apply, MUST NOT consume an acceptance attempt, and MUST provide actionable diagnostics to the subsequent apply attempt.

The gate and its retry decision MUST be derived from workspace file state and Git state. It MUST NOT introduce out-of-worktree durable workflow-control state.

#### Scenario: Malformed completed task file stays in apply

**Given**: all implementation checkboxes are complete
**And**: an active task section contains a top-level non-checkbox evidence bullet
**When**: apply evaluates completion
**Then**: Conflux does not invoke acceptance
**And**: the next apply attempt receives the failing file, line, and task-format diagnostic

#### Scenario: Corrected task file proceeds to acceptance

**Given**: a prior pre-accept task-format check failed
**And**: apply corrects the malformed bullet while preserving completed implementation evidence
**When**: worktree-local task-format validation succeeds
**Then**: Conflux proceeds through the existing cleanup and acceptance handoff
**And**: the repair does not consume an extra acceptance attempt

#### Scenario: Restart derives the same pending repair

**Given**: `tasks.md` remains malformed after process restart
**When**: Conflux resumes from the same workspace and Git state
**Then**: it derives the same apply-before-acceptance action and diagnostic from repository state
**And**: deletion of external logs or runtime state does not alter that next action

#### Scenario: Valid completed task file preserves existing handoff

**Given**: implementation checkboxes are complete and task-format validation succeeds
**When**: apply evaluates completion
**Then**: the existing post-apply cleanup and acceptance workflow continues without an additional agent cycle
