## MODIFIED Requirements

### Requirement: ParallelRunService rejection flow on blocked execution

ParallelRunService SHALL support separate blocked and rejection handoff from apply execution.

When apply execution records a **recoverable blocker**, the runtime SHALL transition the workspace into a dedicated `blocked` activity/state while preserving the worktree, WIP commits, tasks progress, and blocker metadata. A workspace in `blocked` SHALL NOT enter rejection flow, SHALL NOT generate a base-branch rejection marker commit, and SHALL remain resumable.

When apply execution records a **rejection proposal** by generating `openspec/changes/<change_id>/REJECTED.md`, the runtime SHALL transition the workspace into a dedicated `rejecting` stage even if `tasks.md` still contains unchecked implementation tasks. A workspace in `rejecting` SHALL NOT enter the normal acceptance flow. Instead, the runtime SHALL run rejection review and require one of three outcomes: `confirm_rejection`, `resume_apply`, or `block_change`.

The rejecting review operation SHALL end with exactly one dedicated marker line: `REJECTION_REVIEW: CONFIRM`, `REJECTION_REVIEW: RESUME`, or `REJECTION_REVIEW: BLOCK`. Runtime routing SHALL parse that marker instead of relying on `ACCEPTANCE: BLOCKED` for apply-generated rejection proposals.

`confirm_rejection` / `REJECTION_REVIEW: CONFIRM` SHALL execute the rejection flow and finalize the change as rejected after the base branch records `openspec/changes/<change_id>/REJECTED.md`.

`resume_apply` / `REJECTION_REVIEW: RESUME` SHALL delete the worktree-local `REJECTED.md`, append at least one non-rejection recovery task to the worktree-local `tasks.md`, and return the change to apply when the blocker is immediately actionable as normal implementation work.

`block_change` / `REJECTION_REVIEW: BLOCK` SHALL delete the worktree-local `REJECTED.md`, append at least one unresolved recovery task or unblock note to the worktree-local `tasks.md`, preserve the worktree, and transition the change into `blocked` instead of `applying`.

#### Scenario: apply recoverable blocker enters blocked state without rejection proposal
- **GIVEN** apply execution records a recoverable blocker with evidence and unblock actions
- **AND** apply does not generate `openspec/changes/fix-auth/REJECTED.md`
- **WHEN** the runtime evaluates the apply result
- **THEN** the workspace enters `blocked`
- **AND** the worktree, WIP commits, and `tasks.md` progress are preserved
- **AND** the change does not enter `rejecting` or the normal acceptance flow

#### Scenario: apply rejection proposal enters rejecting stage
- **GIVEN** apply execution generates `openspec/changes/fix-auth/REJECTED.md` with a blocker reason
- **AND** `openspec/changes/fix-auth/tasks.md` still contains unchecked implementation tasks
- **WHEN** the runtime evaluates the apply result
- **THEN** the workspace enters `rejecting`
- **AND** the change does not enter the normal acceptance flow
- **AND** apply does not immediately retry the same change

#### Scenario: rejecting review uses dedicated verdict markers including blocked hold
- **GIVEN** a workspace is in `rejecting`
- **WHEN** the rejecting review operation completes successfully
- **THEN** its final marker is exactly one of `REJECTION_REVIEW: CONFIRM`, `REJECTION_REVIEW: RESUME`, or `REJECTION_REVIEW: BLOCK`
- **AND** runtime routing does not require `ACCEPTANCE: BLOCKED` to choose the next step

#### Scenario: rejecting review converts rejected proposal into blocked hold
- **GIVEN** parallel execution is reviewing a change in `rejecting`
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists in the worktree
- **AND** the review determines the change should not be rejected but also should not immediately resume apply
- **WHEN** rejecting returns `block_change`
- **THEN** the worktree-local `openspec/changes/fix-auth/REJECTED.md` is removed
- **AND** `openspec/changes/fix-auth/tasks.md` gains at least one unchecked task or unblock note describing the remaining blocker
- **AND** the change transitions to `blocked`
- **AND** the existing worktree is preserved for later resume

#### Scenario: blocked change resumes apply after explicit retry
- **GIVEN** a change is in `blocked`
- **AND** the recorded unblock condition has been satisfied
- **WHEN** the operator or orchestrator explicitly retries the change
- **THEN** the runtime transitions the change back to `applying`
- **AND** the same worktree and prior WIP context are reused

### Requirement: ParallelRunService rejection flow on blocked execution

After rejecting review completes, the runtime SHALL emit a `RejectionReviewCompleted` execution event with one of `Confirm`, `Resume`, or `Block` outcome. The reducer SHALL use this event to drive the `Rejecting → Rejected`, `Rejecting → Applying`, or `Rejecting → Blocked` transition.

The runtime SHALL NOT leave a change in the `Rejecting` activity stage after rejection review has produced a verdict. If rejection review encounters an error, the runtime SHALL emit a `RejectionReviewFailed` event to transition the change to `Error` terminal state.

#### Scenario: blocked rejection review emits completion event and returns to blocked state
- **GIVEN** a change is in the `Rejecting` activity stage
- **AND** rejection review returns `REJECTION_REVIEW: BLOCK`
- **WHEN** the blocking handoff completes
- **THEN** a `RejectionReviewCompleted` event with `Block` outcome is emitted
- **AND** the reducer transitions the change to `Blocked` activity
- **AND** the worktree remains available for later resume
