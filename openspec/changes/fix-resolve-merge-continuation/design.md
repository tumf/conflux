# Design: Sequential resolve merge continuation

## Context

Sequential integration has two mutation locations: the change worktree is pre-synced with target state, then the change revision is integrated into the target repository. `attempt_merge` already owns ordered `archive_paths`, but `merge_and_resolve` drops them. `resolve_merges_with_retry` reconstructs paths from `workspace_manager.workspaces()`, an optional process-local view that may omit a preserved archived worktree. Missing map entries produce `(unknown)` prompt locations and skip worktree verification.

This is the primary defect. Generic continuation text is a downstream symptom: without the actual path, Conflux cannot observe the worktree `MERGE_HEAD`, conflicts, branch identity, or pre-sync evidence.

## Decisions

### Preserve ordered worktree identity end to end

Introduce an ordered input record containing `revision`, `change_id`, and `archive_path`, or pass equivalent parallel slices with enforced equal lengths. The record flows through:

1. `ParallelExecutor::attempt_merge`
2. `ParallelExecutor::merge_and_resolve`
3. `ParallelExecutor::merge_and_resolve_with`
4. `ResolveMergesWithRetryArgs`
5. `resolve_merges_with_retry`

The passed path is primary. If it no longer exists, Git worktree metadata may rediscover the path by exact branch ref. Rediscovery must prove repository membership and branch identity. Missing or ambiguous evidence becomes `UnsafeEvidence`; it never falls back to skipping worktree checks.

### Closed state model

For each change in declared merge order, classification returns exactly one state:

- `UnsafeEvidence`: missing/ambiguous path, wrong repository, wrong branch, detached HEAD, Git query failure, invalid archive layout, or merge identity mismatch.
- `TargetMergeUnfinished`: target root has an expected, identity-verified merge in progress; conflicts and cleanup needs are attached.
- `PreSyncUnfinished`: validated worktree has an expected target merge in progress; conflicts and exact pre-sync subject are attached.
- `PreSyncInvalid`: pre-sync is required but subject, parentage, or ancestry evidence is missing or invalid.
- `FinalMergeMissing`: pre-sync is valid but the expected revision is not integrated into target `HEAD`; final subject and cleanup evidence are attached.
- `ResurrectionCleanupRequired`: expected revision is integrated or a final merge is in progress, but target-visible live and valid archived forms coexist.
- `Complete`: terminal predicate passes.

Classification stops at the first non-complete change in input order. Within a change, safety/identity checks run first, then target in-progress merge, worktree pre-sync, pre-sync validity, target integration, resurrection invariant, and terminal completion.

### Merge identity

`MERGE_HEAD` existence alone never authorizes a commit instruction.

- In the target repository, the merge parents/`MERGE_HEAD` must contain the expected change revision for the current ordered item. An unrelated or ambiguous merge is `UnsafeEvidence`.
- In a change worktree, `MERGE_HEAD` must contain the target revision that pre-sync is expected to include, and the checked-out branch must be the expected revision branch. Otherwise classification is `UnsafeEvidence`.
- For multiple changes, only the current ordered item may own an in-progress target merge; combined unauditable merge state is rejected rather than committed with `Merge changes: ...`.

### Final integration policy

Preserve current idempotent ancestry compatibility while making identity explicit:

- A newly executed protocol final merge must use exact subject `Merge change: <change_id>` and the commit must integrate the expected revision.
- If the expected revision is already an ancestor of target `HEAD`, it is accepted as already integrated even when no exact final subject exists. Conflux does not manufacture an empty merge commit.
- An exact-subject commit that does not integrate the expected revision is invalid evidence, not success.
- `resolve_merges_with_retry` and `verify_merge_commits` use the same helper/policy so retry and terminal checks cannot disagree.

### Archive resurrection evidence

Use `archive_layout::invalid_layout_error` before `archive_layout::find_valid_archive_entry`.

Evidence sources are phase-specific:

- Before final merge begins, archive identity may exist only in the validated change worktree/branch tree, while the target working tree still contains `openspec/changes/<change_id>`.
- During final merge, cleanup need is evaluated from the target worktree/index-visible tree after merge content is applied.
- After final integration, terminal verification evaluates the target worktree/index-visible tree and rejects valid archive plus live directory coexistence.

The runtime and `cflx-resolve` skill use the same live-path predicate: the live directory is considered present only when it is the active change identity recognized by the OpenSpec layout, including its `proposal.md`. Invalid, nested, unrelated, or merely similarly suffixed archive directories never authorize deletion.

### Existing target auto-commit shortcut

The pre-loop conflict-free target `MERGE_HEAD` shortcut no longer directly commits and returns. It first enters classification, verifies merge identity, determines resurrection cleanup, and delegates the required action through the same resolve attempt/verification cycle. This preserves agent ownership of Git mutation and prevents multi-change `Merge changes: ...` commits from bypassing per-change evidence.

### Bounded continuation

`ResolveContext` already limits attempt count through orchestration but stream tails are line-bounded only. Add byte bounds:

- stdout tail: at most 2 KiB per attempt;
- stderr tail: at most 2 KiB per attempt;
- complete `<resolve_context>`: at most 8 KiB, retaining newest actionable state and trimming on UTF-8 boundaries;
- recorded attempts: never exceed configured `max_retries`.

Phase diagnosis is formatted separately from agent output and preserved when old stream details must be trimmed.

## Retry Contract

The embedded `cflx-resolve` skill requires the agent to:

- act only on identity-validated phase guidance;
- stop and leave repository evidence intact when guidance reports `UnsafeEvidence`;
- resume from the named incomplete phase instead of repeating verified work;
- complete all subsequent sequential phases in order when no blocker remains;
- apply resurrection cleanup only under the validated runtime predicate;
- re-stage hook-modified files and retry the same exact commit subject;
- never claim success while Git reports conflicts, unfinished merges, or live/archive coexistence.

## Constitution and Safety

All classification inputs are workspace paths, file state, Git worktree metadata, refs, index/working-tree state, and base comparison. Process-local workspace lists may optimize discovery but never determine truth. No durable external state is introduced. Unknown evidence fails closed. Completion remains repository-verifiable.

## Alternatives

### Improve only the generic retry sentence

Rejected. The actual worktree path and evidence are unavailable, so better wording cannot diagnose the phase.

### Make Conflux automatically finish conflict-free merges

Deferred. It broadens mutation ownership and hook recovery. Correct path plumbing and fail-closed diagnosis are sufficient for this change; convergence of an external agent is not promised.

### Require exact final subjects for historical fast-forward states

Rejected. Existing idempotent integration may already be ancestral with no opportunity to create a meaningful merge commit. The policy instead validates exact subjects when present and accepts proven ancestry as already integrated.

## Verification Strategy

Use small temporary Git repositories/worktrees. Tests must exercise actual refs, parentage, `MERGE_HEAD`, worktree discovery, index-visible OpenSpec trees, and embedded skill bytes. Unit tests cover state ordering and bounded UTF-8 formatting. Tests exceeding one second must be optimized or marked heavy under repository policy.
