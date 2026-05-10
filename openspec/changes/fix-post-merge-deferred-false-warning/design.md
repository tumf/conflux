# Design: Post-merge deferred false warning suppression

## Context

The observed failure is a race between successful merge cleanup and a duplicate archived-workspace merge/resume path. The successful path emitted `MergeCompleted` and removed the worktree. A stale path then evaluated archive completeness against the archived worktree and produced a manual `MergeDeferred` reason, even though the change was already integrated into base.

## Constraints

- Workflow decisions must remain repository/workspace-derived per `openspec/CONSTITUTION.md`.
- Logs and UI state must not become authoritative control inputs.
- Legitimate manual blockers must remain visible.

## Approach

Use base-branch integration as the authoritative idempotency guard:

1. Before classifying archive verification failure as manual merge deferral, check whether the change's revision/archive evidence is already integrated into base.
2. If integrated, suppress the manual deferral, clear scheduler retry membership, and allow cleanup/idempotent success handling.
3. During queue reconciliation, avoid adding archived dirty workspaces for changes already active in the post-archive merge lane.
4. In the reducer, ensure `MergeCompleted` is terminal enough that later stale `MergeDeferred` cannot regress display state.

## Trade-offs

This avoids durable dedupe state and uses git/base evidence instead. The trade-off is that the guard must be applied in every stale duplicate path that can emit `MergeDeferred`; regression tests should pin both direct merge retry and queue reconciliation behavior.
