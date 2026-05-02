# Design: Conflictless Archived Merge Handoff

## Current Failure Mode

The current Git sequential merge flow enters `resolve_merges_with_retry()` even when `detect_conflicts()` returns no unresolved conflicts.

That creates two coupled false behaviors:

1. A conflict-oriented AI resolve prompt is emitted for a merge that is already merge-ready.
2. After the AI agent performs the ordinary merge commit, post-merge verification can still retry because it expects the archived source branch tip to include the pre-merge base, which is not the right success signal once the merge commit already exists on `main`.

## Desired Behavior

For archived Git merges, the runtime should separate:

- merge-ready, conflictless completion
- real unresolved conflict resolution

The first path should complete the merge commit and validate merged outcome directly.
The second path should continue using the resolve agent and retry loop.

## Minimal Design

Preferred implementation shape:

1. Detect whether unresolved conflicts actually exist before constructing/emitting the resolve prompt.
2. If there are no unresolved conflicts and the repository is in a normal merge-ready state, finish verification on the merge result without launching resolve.
3. In post-merge verification, rely on merge commit evidence and merged ancestry on `main`, not on the archived source branch tip still containing the pre-merge base after merge.
4. Only require worktree-path/conflict-file context when the runtime truly needs AI conflict resolution.

## Constraints

- Must preserve workspace-local workflow state rules from `openspec/CONSTITUTION.md`.
- Must not weaken true-conflict verification or silently swallow real merge conflicts.
- Must stay narrowly scoped to the conflictless archive-to-merge handoff bug, not redesign all merge provenance logic.
