---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/process-execution/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/execution/apply.rs
  - src/ai_command_runner.rs
  - src/process_manager.rs
  - tests/process_cleanup_test.rs
verifications:
  - id: apply-process-group-barrier-tests
    requirement: Apply completion cannot enter repository finalization until the owned process group is confirmed quiescent
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Rust test output covering graceful exit, forced termination, descendant-held Git lock release, cleanup timeout, and acceptance suppression
    rerun: cargo test --features heavy-tests --test process_cleanup_test apply_completion
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Wait for Apply process-group cleanup before Git finalization

**Change Type**: hybrid

## Problem / Context

When Apply completion remains stable through the grace period, Conflux asks the streaming command runner to terminate the owned process group and then proceeds to the WIP snapshot and final Apply commit. The cleanup path waits for the process-group leader, but that is not sufficient proof that every descendant has stopped using the managed worktree. A descendant can briefly retain the worktree `index.lock` after the leader exits.

A production run reproduced this ordering: Conflux sent SIGTERM to the Apply process group after the 30-second completion grace period, began WIP and final commit processing immediately afterward, and the final `git commit --amend` failed because the managed worktree `index.lock` still existed. The repository-level singleton lock was working correctly; this race occurred within one Conflux-owned Apply lifecycle.

## Proposed Solution

Make successful Apply completion cleanup a repository-finalization barrier. The command runner must not report completion to the Apply loop until it has terminated the owned process group, waited for the leader, verified that no owned process-group members remain, and used the existing force-kill path when graceful termination does not quiesce the group.

Return a typed cleanup outcome that distinguishes confirmed quiescence from cleanup timeout or inability to verify. The Apply loop may create a WIP snapshot, run cleanup review, or create the final Apply commit only after confirmed quiescence. An unconfirmed cleanup is an Apply failure and must not dispatch Acceptance.

This change remains one proposal because process cleanup and the Apply handoff barrier are not independently correct: cleanup evidence has no effect unless the orchestration boundary consumes it, while gating on the current leader-only result preserves the race.

## Acceptance Criteria

1. Stable task completion or rejecting handoff still uses the bounded Apply completion grace period.
2. After grace expiry, Conflux completes graceful-then-forceful cleanup and confirms that the owned process group has no remaining members before any Conflux-owned index-mutating Git operation starts in that worktree.
3. A descendant that holds the managed worktree `index.lock` during termination cannot race the subsequent WIP snapshot or final Apply commit.
4. If process-group quiescence cannot be confirmed within the cleanup budget, Apply fails with actionable cleanup diagnostics and does not create a WIP/final commit, start cleanup review, or dispatch Acceptance.
5. Natural command completion and explicit cancellation preserve their existing result semantics while using the same truthful process-group cleanup evidence where strict cleanup applies.
6. No durable workflow state is introduced; restart routing remains derivable from workspace files and Git state.

## Explicit Completion Conditions

- `ManagedChild` or its process handle exposes a bounded cleanup result that proves process-group quiescence rather than only leader exit.
- `AiCommandRunner` awaits that result for completion-grace termination and does not send a successful completion status when cleanup is unconfirmed.
- The shared Apply loop treats confirmed cleanup as a hard precondition for WIP snapshot, cleanup review, final Apply commit, and Acceptance handoff.
- Unix integration coverage spawns a leader plus descendant, has the descendant hold a real managed-worktree `index.lock`, triggers Apply completion cleanup, and proves no Conflux Git finalization starts before the lock-owning descendant exits.
- Failure coverage proves a non-quiescent group produces an Apply failure and zero Acceptance dispatches.
- Long-running real-process tests are gated by `heavy-tests`; default tests remain under the repository one-second policy.
- `cargo test --features heavy-tests --test process_cleanup_test apply_completion` and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Retrying final Apply Git operations after process-group cleanup is confirmed.
- Deleting or bypassing Git lock files.
- Preventing independent external tools from operating on a managed worktree.
- Changing the repository-level single-Conflux-instance lock.
- Adding out-of-worktree durable workflow state.
