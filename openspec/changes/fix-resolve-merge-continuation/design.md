# Design: Sequential resolve merge continuation

## Context

Sequential integration has two mutation locations: each change worktree is pre-synced with the cumulative target state, then each change branch is integrated into the target repository in declared order. `attempt_merge` owns archive worktree paths, but the current call chain drops them and later depends on an optional process-local workspace list. That loses both path and `workspace.base_revision`, causing verification to skip the repository where incomplete pre-sync evidence actually exists.

## Ordered Batch Input

Define `SequentialMergeItem { revision, change_id, archive_path }` and carry `Vec<SequentialMergeItem>` unchanged through `attempt_merge`, `merge_and_resolve`, `merge_and_resolve_with`, `ResolveMergesWithRetryArgs`, and `resolve_merges_with_retry`.

The supplied path is primary. `src/vcs/git/commands/worktree.rs` validates repository registration, toplevel, exact checked-out branch, and non-detached state. If stale, `git worktree list --porcelain` may rediscover exactly one worktree for the expected branch. Missing or multiple matches, wrong repository/branch, detached HEAD, or Git errors produce `UnsafeEvidence`; no check is skipped.

## Repository-Derived Required Target State

For each item, define required target state `T` without process memory:

- If a target final merge is in progress for the item, `T` is target `HEAD`, the pre-merge first parent while `MERGE_HEAD` names the branch tip.
- If an exact final merge commit already exists, `T` is that commit's first parent.
- If final merge has not started, `T` is current cumulative target `HEAD` after every preceding item has committed completion evidence.

Historical ancestry-only integration has no exact final commit from which to reconstruct `T`; it is accepted as already integrated and exempt from pre-sync reconstruction, while still subject to archive/live and clean-target terminal invariants.

## Pre-Sync Identity

For a non-historical item, validate the worktree branch tip against `T`:

1. If `T` lies on the tip's first-parent lineage, the worktree was created from or advanced directly through `T`; no pre-sync merge commit is required.
2. Otherwise, exactly one reachable commit with exact subject `Pre-sync base into <change_id>` must exist after the item's admitted branch base, have exactly two parents, and have non-first parent exactly `T`.
3. Multiple exact candidates, wrong parent count, wrong non-first parent, or a tip that does not contain the validated pre-sync commit is `PreSyncInvalid`.

The admitted branch base is retained with the batch item only if already available at admission; it is not reconstructed from `workspace_manager.workspaces()`. If unavailable, candidate search is bounded by `merge-base(T, worktree_tip)` and exact topology rather than process memory.

## Batch-Aware Target Merge Ownership

Before per-item classification, inspect the global target repository:

1. If no target `MERGE_HEAD` exists, evaluate items in declared order.
2. If `MERGE_HEAD` exists, match it exactly to one validated item branch tip. Zero or multiple matches are `UnsafeEvidence`.
3. All items before the owner must have committed per-item completion evidence.
4. The owner must be the first incomplete item. If an earlier item is incomplete or a later item appears integrated out of order, fail closed.
5. The target merge owner is evaluated against `T = target HEAD` and valid pre-sync before any `TargetMergeUnfinished` action is returned.

Batch `Complete` requires all items complete, no target `MERGE_HEAD`, no conflicts, and clean target index/worktree including no untracked files.

## Closed States and Order

Classification returns the first state requiring action:

- `UnsafeEvidence`
- `PreSyncUnfinished`
- `PreSyncInvalid`
- `TargetMergeUnfinished`
- `FinalMergeMissing`
- `ResurrectionCleanupRequired`
- `Complete`

Safety, target-owner selection, required `T`, and pre-sync identity are evaluated before target commit guidance. Items are evaluated in declared order after owner selection.

## Exact Final Merge Identity

An exact final candidate is a commit since `base_revision` whose complete subject is `Merge change: <change_id>`. Completion requires:

- exactly one exact candidate;
- exactly two parents;
- first parent exactly `T`;
- non-first parent exactly the validated worktree branch tip;
- target `HEAD` contains that candidate.

If one or more exact candidates exist but topology is invalid or ambiguous, fail closed; ancestry fallback is forbidden. Only when zero exact candidates exist may `is_ancestor(expected_branch_tip, target HEAD)` establish historical already-integrated success.

The retry classifier and `verify_merge_commits` call one shared verifier so their policies cannot diverge.

## Archive Evidence Views

Archive identity uses shared pure name/layout predicates extracted from `archive_layout`; adapters inspect the correct Git view:

- Pre-final: committed validated worktree `HEAD` tree supplies archive evidence; target committed `HEAD` supplies active-live evidence.
- Final merge in progress: target stage-0 index supplies merged live/archive evidence. Any stage 1/2/3 conflict entry is unsafe and cleanup guidance is withheld.
- Post-final: committed target `HEAD` supplies archive/live evidence. Target index and filesystem worktree must exactly match `HEAD` and contain no untracked files before terminal completion.

An active live identity requires `openspec/changes/<change_id>/proposal.md`. Valid archive identity is exact `<change_id>` or `YYYY-MM-DD-<change_id>` with `proposal.md`; nested, unrelated, and suffix-collision entries do not authorize deletion.

## Durable Resurrection Cleanup

Preferred cleanup remains before final merge commit. If committed final integration still contains active live and valid archive forms:

1. Classify `ResurrectionCleanupRequired`.
2. Instruct the resolve agent to remove only `openspec/changes/<change_id>` and commit a forward commit with exact subject `Cleanup resurrected change: <change_id>`.
3. The cleanup commit must have exactly one parent equal to the preceding target `HEAD` and its tree diff must only delete the active live change subtree; valid archive content must remain unchanged.
4. Staged-only, unstaged, mixed, unrelated, amend-based, or dirty cleanup is incomplete.
5. After the commit, rerun full batch terminal verification.

## Target Shortcut

The existing conflict-free target `MERGE_HEAD` shortcut no longer commits directly. It enters owner selection, `T` derivation, pre-sync validation, stage-0 archive inspection, and normal agent continuation. Combined `Merge changes: ...` commits are never generated.

## Resolve-Specific Bounded Continuation

Do not change shared `OutputCollector` defaults used by apply/archive/acceptance. Add resolve-specific byte-safe tail extraction or post-processing:

- each stdout/stderr tail: at most 2 KiB;
- complete `<resolve_context>` including wrapper: at most 8 KiB;
- recorded attempts: at most configured `max_retries`;
- newest structured phase diagnosis is immutable retained content.

Deterministic reduction order is: remove oldest attempts entirely; remove remaining older attempt stream tails; trim newest attempt stdout/stderr detail; retain newest attempt metadata and structured phase diagnosis. Every trim occurs on a UTF-8 boundary. If fixed wrapper plus newest diagnosis alone exceeds 8 KiB, bound individual diagnostic fields during construction so the invariant still holds.

## Retry Contract

The embedded `cflx-resolve` skill requires action only on identity-validated guidance, exact commit subjects, no history rewrite, forward-only post-final cleanup, and re-verification after every commit. `UnsafeEvidence` preserves state and returns without mutation.

## Constitution and Scope

All authority comes from supplied workspace paths, Git worktree metadata, refs, commit parentage, index stages, committed trees, filesystem cleanliness, and base comparison. Process memory and agent prose are non-authoritative. Git mutations remain agent-owned; this change defines diagnosis and truthful completion rather than guaranteeing agent convergence.
