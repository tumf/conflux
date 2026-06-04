# Design: dependency dispatch waits for merged evidence

## Decision

`DependencyTargetClass::Archived` remains a repository-visible classification, but it no longer implies dispatch satisfaction. Dispatch satisfaction is derived from base-branch merge evidence.

## Rationale

Archive completion can precede merge/resolve completion. Dependents launched during this window can apply against a base branch that does not yet contain the dependency's canonical archive result. This violates dependency ordering and can create stale worktrees.

The constitution requires workflow-control state to be derivable from workspace file state, workspace git state, and base-branch tree comparison. Base-branch merge verification is already aligned with that law and avoids introducing durable scheduler state.

## Implementation Boundary

The likely implementation point is `ParallelExecutor::select_changes_for_dispatch` in `src/parallel/queue_state.rs`.

Required behavioral shift:

- classify dependency targets as before;
- for `Archived`, do not immediately `continue` as satisfied;
- run the same base-branch merge verification used for other non-terminal dependency classes;
- block when merge verification returns false;
- allow dispatch when merge verification returns true.

## Risks

- Existing tests that assume archive evidence is sufficient must be updated.
- Diagnostics should avoid regressing into generic missing dependency errors.
- Repeated archived-but-not-merged observations should not spam dependency-blocked diagnostics.
