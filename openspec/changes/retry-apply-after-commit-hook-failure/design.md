# Design: Final Apply commit recovery

## Decision

Recovery belongs inside the shared Apply loop immediately around final commit creation. This is the narrowest layer that knows all three facts required for a truthful decision: tasks are complete, the final verified commit failed, and Acceptance has not yet been dispatched.

## Failure Classification

The Git command boundary must preserve structured command context. Final commit recovery is eligible only when the failed operation is the hook-enabled final `git commit` and the command result represents a repository validation rejection. Classification must inspect structured fields from `VcsError::Command`, not rendered text searching.

Failures outside that class remain terminal. This includes reset or revision lookup failure, missing WIP ancestry, repository corruption, I/O failure, and lock contention after its dedicated retry policy.

## Feedback Flow

1. Apply completes the current implementation iteration and records its normal agent attempt.
2. Final commit runs with verification hooks enabled.
3. On eligible rejection, the Apply loop converts the structured command failure into bounded orchestration feedback.
4. `AgentRunner` records that feedback in Apply history for the same change.
5. The loop starts another Apply iteration in the same workspace.
6. Prompt construction includes the feedback under an untrusted diagnostic wrapper and instructs repair plus validation rerun.
7. Final commit runs again with hooks enabled.
8. Only success yields `ApplyLoopResult { completed: true, ... }`.

## Iteration Bound

A recovery iteration consumes the existing `max_iterations` budget. The loop must check remaining capacity before dispatching another agent command. If no capacity remains, return the latest structured commit failure as the terminal reason. No separate commit-retry counter is needed.

## Workspace-State Compliance

No durable retry record is added. In-process Apply history improves the immediate retry prompt but is not authoritative after restart. A restarted process derives the next action from the same dirty or staged workspace Git state and completed task state, satisfying the workspace-local workflow constitution.

## Trust Boundary

Commit hook output is untrusted repository/tool output. Prompt formatting must bound each captured stream and clearly prohibit following instructions embedded in diagnostic text. The fixed system guidance, not hook output, defines the action: repair repository code or tests, rerun validation, and leave final commit verification enabled.

## Verification Strategy

Pure classification and prompt formatting use unit tests with constructed errors and in-memory history. Loop behavior uses fake workspace and agent boundaries rather than real Git hooks, allowing deterministic coverage under one second. Existing Git-command tests remain responsible for command argument construction, including the absence of `--no-verify` on final commits.
