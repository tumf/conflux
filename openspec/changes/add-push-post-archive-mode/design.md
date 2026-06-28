# Design: Push Post-Archive Mode

## Requested Artifact

implementation

## Completeness Checklist

- User-facing outcome: `cflx run --parallel --push [remote]` pushes completed change branches to a remote instead of merging into base.
- User constraint: branch selection is unavailable; values containing `:` are rejected.
- Repository areas: CLI parsing, orchestrator construction, parallel run service, parallel executor post-archive handling, Git workspace commands, reducer/TUI/Web status mapping, tests.
- Verification: parser unit tests, reducer/status unit tests, post-archive action unit tests, Git integration tests using a local bare remote, and regression tests for unchanged merge mode.
- Rollout: default remains merge mode; push mode is opt-in.
- Non-goals: PR creation, force push, branch rename, serial push mode, remote tracking config changes.

## Current Architecture

Parallel run mode creates one Git worktree/branch per change. After apply, acceptance, and archive complete, `ParallelExecutor::handle_merge_and_cleanup` schedules a background base-lane operation via `attempt_merge`. The merge path verifies archive completion, emits resolve/merge lifecycle events, checks out the original branch, merges the worktree branch into base, runs `on_merged`, emits `MergeCompleted`, and cleans up the worktree.

Push mode should reuse the same pipeline until archive completion, then substitute only the terminal post-archive action. This keeps dependency analysis, apply retry, acceptance, archive verification, cancellation, and workspace recovery behavior unchanged.

## Proposed Model

Introduce a small post-archive action value:

- `PostArchiveAction::MergeToBase`
- `PostArchiveAction::PushToRemote { remote: String }`

The CLI creates this value from `--push [remote]` and rejects any remote value containing `:`. The action propagates into `Orchestrator`, `ParallelRunService`, `ParallelExecutor`, and any spawned background/retry executor that may perform the terminal post-archive operation.

## Push Execution

For a successful archived worktree branch `branch`, push mode executes the equivalent of:

```bash
git push <remote> <branch>:<branch>
```

The destination ref is intentionally the same as the local branch. No base checkout, `git merge`, merge conflict resolution, merge verification, or `on_merged` hook runs in push mode.

Archive-completion verification remains required before push. A failed archive verification remains a deferred/manual outcome as today. A failed push is treated as terminal action failure for that attempt, while preserving the worktree for operator inspection or a future retry.

## Events and State

Add pushed-specific lifecycle reporting rather than reusing merge terminology:

- `PushStarted { change_id, remote, branch }`
- `PushCompleted { change_id, remote, branch, revision }`
- `PushFailed { change_id, remote, branch, error }`

Reducer terminal state should include `Pushed` and expose display status `pushed`. CLI logs, TUI display, and WebState must treat pushed as a successful terminal outcome distinct from merged.

## Constitution Compatibility

The mode does not add durable workflow-control state. Retry/recovery remains based on workspace file state, workspace git state, base/tree comparison where applicable, and remote push command results during execution. Observability events and UI state are not authoritative inputs.

## Trade-offs

- Same-name destination branch is deliberately simpler and matches the user's constraint. It avoids hidden branch-routing state and reduces accidental pushes to a shared base branch.
- Push mode is parallel-only for this change. Serial mode does not use the same worktree branch terminal action and should remain unchanged unless a later proposal requests it.
- `on_merged` is not reused. A later proposal may add an `on_pushed` hook if needed, but conflating push with merge would make hook semantics misleading.
