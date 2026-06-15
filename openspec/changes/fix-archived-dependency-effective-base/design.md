# Design: Effective dependency base for archived dependencies

## Background

`fix-dependency-dispatch-after-merge` fixed a real safety issue: dependents must not dispatch merely because a dependency has been archived. They must wait until merge evidence proves the dependency's artifacts are available to the branch/context used for subsequent work.

The current implementation checks that evidence through `is_dependency_resolved()` using the workspace manager's `original_branch`. That is correct only when the original branch is also the integration base. In stacked orchestration, Conflux may accumulate merge/archive commits on a branch other than the startup branch. In that mode, checking only the startup branch can keep dependents blocked after the dependency is already merged into the effective orchestration context.

## Design Goals

- Preserve fail-closed behavior before merge evidence exists.
- Avoid reverting to archive-only dispatch satisfaction.
- Make the branch/tree used for archived dependency checks explicit and testable.
- Keep workflow-control decisions derivable from repository-visible workspace/git/base tree evidence.
- Align analysis, dispatch gating, and diagnostics so operators can understand why a dependency remains blocked.

## Proposed Model

Introduce an explicit effective dependency base concept in the scheduler dispatch path.

The effective dependency base is the repository-visible branch or tree reference that represents the accumulated integration context for ordinary apply dispatch. In simple runs this remains the original branch. In stacked orchestration, it is the branch/context that already includes completed dependency merge/archive commits and from which newly dispatched work should derive availability assumptions.

Archived dependency dispatch checks then become:

1. Classify dependency target from repository-visible evidence.
2. If dependency is missing, rejected, queued, in-flight, active-but-not-queued, or terminal-error, preserve existing fail-closed behavior.
3. If dependency is archived, verify merge evidence with `is_merged_to_base(dep_id, repo_root, effective_dependency_base)`.
4. Dispatch only if the check succeeds.

## Implementation Notes

Potential touch points:

- `src/parallel/queue_state.rs::is_dependency_resolved` currently obtains `original_branch` from `workspace_manager.ensure_original_branch_initialized()` and passes it to `is_merged_to_base`.
- `src/execution/state.rs::is_merged_to_base` already accepts a branch/tree reference parameter; the fix should prefer changing the caller-side base selection over weakening this function's archive/remove checks.
- `src/vcs/git/mod.rs` may need to expose or preserve the integration branch/context used by workspace creation and post-archive merge flow.
- Tests should model distinct original and effective integration refs so the regression cannot pass accidentally when both point to the same branch.

## Safety Considerations

The change must not make `DependencyTargetClass::Archived` equivalent to resolved. The required proof remains merge evidence in the selected effective base. If no effective integration base can be determined from repository-visible state, the scheduler should fail closed or retain current original-branch behavior with an explicit diagnostic.

## Verification Strategy

- Regression: archived dependency not merged into effective base remains blocked.
- New stacked regression: archived dependency merged into effective integration branch unblocks a dependent even when original branch has not advanced.
- Diagnostic regression: blocked archived dependency names the checked base or otherwise explains archived-but-not-merged rather than implying a missing dependency.
- Existing dependency-dispatch tests continue to pass.
