---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-merge/spec.md
  - src/archive_layout.rs
  - src/history.rs
  - src/parallel/conflict.rs
  - src/parallel/merge.rs
  - src/parallel/tests/conflict.rs
  - src/vcs/git/commands/merge.rs
  - src/embedded_skills.rs
  - skills/cflx-resolve/SKILL.md
verifications:
  - id: resolve-continuation-tests
    requirement: "Sequential resolve retains and validates every worktree path, diagnoses the exact unfinished phase from repository evidence, and accepts only safe terminal integration states"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust unit and temporary-Git integration output covering path plumbing, identity failures, pre-sync, final merge, fast-forward compatibility, resurrection cleanup, bounded history, and embedded skill guidance"
    rerun: "cargo test parallel:: && cargo test history:: && cargo test embedded_skills"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix sequential resolve merge continuation

**Change Type**: implementation

## Problem / Context

`attempt_merge` receives one archived worktree path per `(revision, change_id)` but drops those paths when it calls `merge_and_resolve`. `resolve_merges_with_retry` then reconstructs paths from `workspace_manager.workspaces()`. A preserved or archived worktree can be absent from that process-local list even though its path and Git state still exist. The prompt displays `(unknown)`, and worktree `MERGE_HEAD`, conflicts, branch identity, pre-sync subject, and ancestry checks are silently skipped. Verification falls through to the generic `Missing merge commits for change_ids (...); retrying resolve` reason.

In the observed failure, the change worktree held the incomplete pre-sync while the target branch still held the live OpenSpec change. Because the actual worktree path was lost, three retries could not diagnose the unfinished pre-sync or direct the agent safely to final merge. Queue reconciliation then correctly deferred to manual merge wait after retries were exhausted.

The existing pre-loop target-root shortcut also commits any conflict-free `MERGE_HEAD` before full phase diagnosis. That path can bypass archive resurrection cleanup and pre-sync verification. Finally, current code intentionally accepts an already integrated revision by ancestry when no exact final merge subject exists, while the initial proposal incorrectly implied that an exact subject is always mandatory.

## Proposed Solution

Carry the ordered archived worktree paths through `attempt_merge`, `merge_and_resolve`, `merge_and_resolve_with`, `ResolveMergesWithRetryArgs`, and `resolve_merges_with_retry`. Validate each path against current Git worktree and branch evidence. If a supplied path is stale, rediscover the expected branch through repository-local Git worktree metadata; if evidence remains unavailable, mismatched, detached, or unreadable, classify it explicitly and fail closed instead of skipping checks.

Add a side-effect-free sequential merge state classifier that evaluates each ordered `(revision, change_id, worktree_path)` and the target repository. It will identify the earliest incomplete or unsafe phase, emit bounded actionable continuation, and place the existing target-root conflict-free auto-commit shortcut under the same classification, archive-cleanup, and terminal-verification rules. Git mutations and conflict decisions remain owned by the resolve agent.

Preserve existing idempotent compatibility: an exact `Merge change: <change_id>` commit is valid only when it integrates the expected revision, while a revision already ancestral to target `HEAD` remains accepted without manufacturing a new merge commit. New protocol-driven final merges continue to require the exact subject.

This remains one change because path plumbing, closed-state diagnosis, terminal predicates, embedded skill guidance, and Git-backed regression tests must ship together; any subset would retain the fail-open behavior.

## Acceptance Criteria

1. Every change passed to sequential resolve has an ordered worktree path sourced from `archive_paths` or repository-local Git rediscovery; process-local workspace membership is not required.
2. Missing, stale, unreadable, wrong-branch, detached-HEAD, unexpected-merge-parent, or Git-query-failed evidence is classified explicitly and never skipped or treated as completion.
3. The classifier distinguishes unsafe evidence, unfinished target merge, unfinished worktree pre-sync, missing/invalid pre-sync, missing final merge, resurrection cleanup required, and complete integration, and evaluates multiple changes in declared merge order.
4. `MERGE_HEAD` guidance is emitted only after its identity is proven: target-root merge identity must include the expected change revision, and worktree pre-sync identity must include the expected target state. Unknown merge identity never receives a blind commit instruction.
5. A pre-sync-complete/final-merge-missing state names the change, branch, validated worktree path, target branch, exact `Merge change: <change_id>` subject, and whether resurrection cleanup will be required, without instructing the agent to repeat pre-sync.
6. Archive identity uses `archive_layout::find_valid_archive_entry` and `invalid_layout_error`. Before final merge it considers the validated change worktree/branch archive evidence plus target live evidence; during or after final merge it uses the target worktree/index-visible tree. Invalid or unrelated archive entries never authorize live-directory removal.
7. Final success requires no unfinished merge or conflict, valid pre-sync evidence when required, expected revision integration, and no live/archive coexistence. An exact final subject must integrate the expected revision; alternatively, an already ancestral revision remains an idempotent fast-forward/already-integrated success.
8. The target-root conflict-free `MERGE_HEAD` shortcut cannot bypass phase identity, resurrection cleanup, or terminal verification, and multi-change processing preserves per-change ordered evidence.
9. Retry diagnostics use the existing output/history surfaces, retain no more than the configured retry count, cap each captured stream tail at 2 KiB, and cap the complete injected resolve context at 8 KiB on UTF-8 boundaries.
10. Agent exit status and prose remain non-authoritative; all routing and completion decisions remain derivable from workspace file state, Git state, and base comparison.

## Explicit Completion Conditions

- `src/parallel/merge.rs` passes ordered worktree paths through the full merge/resolve call chain and applies one documented terminal integration policy in both retry verification and `verify_merge_commits`.
- `src/parallel/conflict.rs` uses a closed side-effect-free state classifier, removes fail-open `Option` path skips, and routes the pre-loop target merge shortcut through the same safety predicates.
- `src/archive_layout.rs` helpers are reused for exact/date-prefixed archive validation; nested/invalid layouts fail closed.
- `src/history.rs` enforces the 2 KiB per-stream and 8 KiB complete-context limits without splitting UTF-8.
- `skills/cflx-resolve/SKILL.md` aligns its live/archive predicate with runtime validation and requires resumption from the diagnosed phase without blind commits.
- Tests cover empty manager workspace lists with valid passed paths, stale-path rediscovery, missing paths, wrong branch, detached HEAD, wrong `MERGE_HEAD`, Git errors, wrong-parent exact subjects, fast-forward/already-integrated success, multi-change order, target auto-commit bypass prevention, pre/post-final resurrection, bounded history, and actual embedded skill content.
- `cargo test parallel:: && cargo test history:: && cargo test embedded_skills` passes.

## Out of Scope

- Making Conflux automatically complete conflict-free pre-sync or final merges.
- Guaranteeing that an external resolve agent will always converge; this change guarantees correct fail-closed diagnosis, continuation, and terminal verification.
- Bypassing Git hooks, rewriting branch history, changing queue reconciliation after genuine retry exhaustion, or repairing the currently preserved failed worktree.
- Cleaning up duplicate canonical `merge-attempt-resolve-priority` blocks; that pre-existing spec hygiene issue is unrelated.
