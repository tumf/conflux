# Design: Final Apply commit recovery

## Decision

Recovery belongs inside the shared Apply loop immediately around final commit creation. This is the narrowest layer that knows all three facts required for a truthful decision: tasks are complete, the final verified commit failed, and Acceptance has not yet been dispatched.

## Failure Classification

Classification occurs at a dedicated final-commit call site, not by retrospectively parsing a generic `VcsError::Command`. The final-commit boundary must return a typed outcome equivalent to `Committed`, `HookRejected { command, exit_code, stdout, stderr }`, or `Vcs(VcsError)`. Both the dirty-tree add-and-commit path and the clean-tree amend path must propagate this outcome; amend failure must never be logged and converted to success.

Recovery is eligible only when the final `git commit` process spawned successfully, exited with the designated repository-rejection status, and produced captured diagnostics. The command layer must preserve the actual exit code for this invocation. Spawn failure, failures in `status`, `add`, or staged-snapshot validation, fatal Git status, reset or revision lookup failure, missing WIP ancestry, repository corruption, I/O failure, and lock contention after its dedicated retry policy remain terminal. Rendered error text does not determine eligibility.

## Feedback Flow

1. Apply completes the current implementation iteration and records its normal agent attempt.
2. Final commit runs with verification hooks enabled.
3. On eligible rejection, the Apply loop converts the typed final-commit outcome into bounded orchestration feedback.
4. `AgentRunner` records that feedback in Apply history for the same change through an orchestration-feedback API distinct from normal process `ExitStatus` recording.
5. The loop marks commit repair pending and starts another Apply iteration in the same workspace.
6. While commit repair is pending, the task-complete short circuit is bypassed so one Apply agent command is always dispatched before final commit is retried.
7. Prompt construction includes the feedback under an untrusted diagnostic wrapper and instructs repair plus validation rerun.
8. Final commit runs again with hooks enabled.
9. Only success yields `ApplyLoopResult { completed: true, ... }`.

## Iteration Bound

A recovery iteration consumes the existing `max_iterations` budget. The loop must check remaining capacity before dispatching another agent command. If no capacity remains, return the latest structured commit failure as the terminal reason. No separate commit-retry counter is needed.

## Workspace-State Compliance

No durable retry record is added. In-process Apply history improves the immediate retry prompt but is not authoritative after restart. A rejected add-and-commit path may leave the index staged; the next WIP snapshot or finalization attempt must consume that state without discarding changes. A rejected amend leaves HEAD unchanged. A restarted process derives the next action from workspace files and Git state, satisfying the workspace-local workflow constitution.

## Trust Boundary

Commit hook output is untrusted repository/tool output. Prompt formatting must reuse the existing bounded tail collection behavior and clearly prohibit following instructions embedded in diagnostic text. The fixed system guidance, not hook output, defines the action: repair repository code or tests, rerun validation, and leave final commit verification enabled. This prohibition is scoped to the final commit; the Apply prompt must not universally prohibit `--no-verify` because WIP snapshot policy remains unchanged.

## Verification Strategy

Pure classification and prompt formatting use unit tests with typed outcomes and in-memory history. Loop behavior uses fake agent boundaries to assert an actual repair dispatch and Acceptance gating. Final-commit propagation uses temporary real Git repositories with failing hooks for both add-and-commit and amend paths; these tests must remain under one second or use the repository heavy-test gate. Git-command tests also assert the absence of `--no-verify` on final commits.
