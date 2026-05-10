# Design: deferred background merge result status

## Context

Parallel post-archive merge execution currently has three semantic outcomes:

1. The change is merged into the base branch.
2. The merge is deferred because a blocker remains or a retry is needed.
3. The merge task fails due to an unexpected error.

The task result channel currently collapses at least the first two outcomes into a successful `Ok(())` task completion. That is inaccurate for observability and makes queue-state follow-up behavior unable to distinguish completed work from pending deferred work.

## Approach

Add a small explicit result type at the boundary between merge execution and queue-state handling. The type should carry enough information for queue state to log accurately and choose follow-up behavior without inspecting user-facing log text.

Suggested shape:

- `Merged`
- `Deferred { reason, auto_resumable }`
- `Failed { error }` or keep transport-level `Err(String)` for failures

The merge executor should continue to emit detailed events such as `MergeDeferred`, `WorkspaceStatusUpdated`, and `MergeCompleted` where they are produced today. Queue state should treat the returned outcome as task bookkeeping only.

## Workflow-state constraints

The outcome must be derived from the merge attempt return value and repository/workspace checks that already drive merge behavior. It must not inspect persistent logs, TUI-only state, or out-of-worktree durable state.

## Compatibility

Actual successful merges, including idempotent already-merged cases, continue to trigger success-only queue follow-up. Deferred merge attempts remain pending and visible, but no longer claim successful completion.
