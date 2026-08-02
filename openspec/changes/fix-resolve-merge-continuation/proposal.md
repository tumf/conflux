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
  - src/vcs/git/commands/worktree.rs
  - src/embedded_skills.rs
  - skills/cflx-resolve/SKILL.md
verifications:
  - id: resolve-continuation-tests
    requirement: "Sequential resolve retains ordered worktree evidence, validates batch and commit identity, durably removes resurrected live changes, and accepts only clean committed terminal states"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust unit and temporary-Git integration output covering path plumbing, pre-sync and final parentage, batch ownership, Git evidence views, forward cleanup commits, bounded resolve history, and embedded skill guidance"
    rerun: "cargo test parallel:: && cargo test history:: && cargo test vcs::git::commands:: && cargo test embedded_skills"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix sequential resolve merge continuation

**Change Type**: implementation

## Problem / Context

`attempt_merge` receives one archived worktree path per `(revision, change_id)` but drops those paths when it calls `merge_and_resolve`. `resolve_merges_with_retry` then reconstructs paths and pre-sync metadata from `workspace_manager.workspaces()`. A preserved archived worktree can be absent from that process-local list even though its path and Git state still exist. The prompt displays `(unknown)`, and worktree `MERGE_HEAD`, conflicts, branch identity, pre-sync subject, and ancestry checks are silently skipped. Verification falls through to generic `Missing merge commits for change_ids (...); retrying resolve` guidance.

The current pre-loop target-root shortcut also commits any conflict-free `MERGE_HEAD` before full identity, pre-sync, resurrection, and terminal checks. Existing verification accepts ancestry-only already-integrated revisions, but exact-subject commits are not required to prove the expected parent topology. Post-final live/archive coexistence also lacks a durable forward-cleanup protocol, so staged deletion could otherwise be mistaken for completion.

## Proposed Solution

Carry an ordered batch item containing `revision`, `change_id`, and `archive_path` through `attempt_merge`, `merge_and_resolve`, `merge_and_resolve_with`, `ResolveMergesWithRetryArgs`, and `resolve_merges_with_retry`. Validate or repository-locally rediscover each worktree through Git metadata. Missing, ambiguous, detached, mismatched, or unreadable identity fails closed.

Replace ad hoc checks with a batch-aware, side-effect-free classifier. It first determines any target `MERGE_HEAD` owner, then evaluates items in order using explicit required target state, pre-sync topology, final-merge topology, archive evidence view, and clean committed terminal predicates. Process-local `workspace.base_revision` is no longer required.

Keep Git mutations with the resolve agent. A resurrection discovered before the final commit is removed inside that final merge. A resurrection discovered after the final commit is removed by a new forward commit with exact subject `Cleanup resurrected change: <change_id>`; amend and history rewrite remain forbidden. Terminal success is based only on clean index/worktree state and committed target `HEAD` evidence.

Preserve idempotent compatibility: when no exact final-subject candidate exists, an expected revision already ancestral to target `HEAD` is accepted as historical already-integrated evidence and does not require reconstruction of an unavailable pre-sync. New protocol commits must satisfy exact parent topology.

This remains one change because path retention, batch topology, durable cleanup, terminal truth, prompt guidance, and regression tests are inseparable safety boundaries.

## Acceptance Criteria

1. Every batch item retains its ordered archive worktree path from merge admission, or is rediscovered by exact repository and branch identity; process-local workspace membership and `workspace.base_revision` are not authoritative.
2. The classifier derives required target state `T` from repository evidence: the target pre-merge first parent for an in-progress/existing exact final merge, or current cumulative target `HEAD` after all prior items are complete when final merge has not started.
3. Pre-sync is valid without a merge commit only when `T` is on the validated worktree tip's first-parent lineage. Otherwise exactly one reachable `Pre-sync base into <change_id>` commit must have two parents and non-first parent exactly `T`. Missing, multiple, or wrong-topology candidates fail closed.
4. Before classifying item states, a global target `MERGE_HEAD` owner is uniquely matched by expected branch tip. Items before the owner must already have committed completion evidence, the owner must be the first incomplete item, and any ambiguity or order mismatch fails closed.
5. An exact `Merge change: <change_id>` candidate must be unique since `base_revision`, have exactly two parents, have first parent `T`, and have non-first parent equal to the validated worktree branch tip. Ancestry-only fallback is allowed only when no exact-subject candidate exists.
6. Historical ancestry-only success is exempt from reconstructing pre-sync topology, but still requires archive/live invariants and clean target state.
7. Target or worktree `MERGE_HEAD` guidance is emitted only after branch, owner, and parent identity are proven. The existing conflict-free target shortcut cannot bypass the classifier or create combined `Merge changes: ...` commits.
8. Archive evidence uses distinct Git views: validated worktree `HEAD` before final merge, stage-0 target index during final merge with any conflict stages rejected, and committed target `HEAD` after commit. Filesystem helpers are not used as a substitute for index/commit-tree inspection.
9. Valid archive naming and invalid nested-layout rules are shared with `archive_layout`; exact/date-prefixed entries are accepted, while nested, unrelated, and suffix-collision entries never authorize deletion.
10. Post-final resurrection cleanup is complete only after a forward `Cleanup resurrected change: <change_id>` commit whose sole tree change removes the active live change while preserving the valid archive. Staged-only, unstaged, mixed, or unrelated cleanup remains incomplete.
11. Per-item completion requires valid committed integration and committed archive/live non-coexistence. Batch completion additionally requires every item complete, no `MERGE_HEAD`, no conflicts, and clean target index and worktree including untracked files.
12. Resolve-specific history caps each stdout/stderr tail at 2 KiB and the complete wrapper-inclusive `<resolve_context>` at 8 KiB on UTF-8 boundaries. Deterministic trimming removes oldest attempts, then older stream tails, then newest stream detail while always retaining the newest structured phase diagnosis.
13. Agent exit status and prose remain non-authoritative; routing and completion remain derivable from workspace-local Git and OpenSpec evidence.

## Explicit Completion Conditions

- `src/parallel/merge.rs` passes ordered batch items through the full merge/resolve chain and shares one final-integration verifier with retry completion.
- `src/parallel/conflict.rs` implements required-target-state derivation, global target-merge ownership, ordered classification, durable cleanup diagnosis, and clean batch terminal predicates without optional verification skips.
- `src/vcs/git/commands/worktree.rs` validates and rediscovers exact worktree/branch identity; `src/vcs/git/commands/merge.rs` exposes parent-count/topology, first-parent lineage, exact-candidate, index-stage, committed-tree, and cleanliness evidence needed by the classifier.
- `src/archive_layout.rs` exposes shared pure archive-name/layout classification used by filesystem and Git tree/index adapters.
- `src/history.rs` applies byte caps only to resolve continuation construction, preserving existing apply/archive/acceptance collector behavior.
- `skills/cflx-resolve/SKILL.md` defines identity-validated pre-sync/final actions and the forward cleanup commit protocol.
- Tests cover absent manager entries, stale rediscovery, wrong identity, valid/invalid pre-sync topology, target owner ordering, false exact commits, ancestry fallback, staged-only cleanup, forward cleanup success, HEAD/index/worktree disagreement, bounded history, and actual embedded skill bytes.
- `cargo test parallel:: && cargo test history:: && cargo test vcs::git::commands:: && cargo test embedded_skills` passes.

## Out of Scope

- Conflux-owned automatic pre-sync, final merge, or cleanup commits.
- Guaranteeing convergence of an external resolve agent; this change guarantees fail-closed diagnosis, continuation, durable cleanup requirements, and truthful completion.
- Bypassing hooks, rewriting history, changing genuine retry-exhaustion queue handling, repairing preserved failed worktrees, or cleaning unrelated canonical spec duplication.
