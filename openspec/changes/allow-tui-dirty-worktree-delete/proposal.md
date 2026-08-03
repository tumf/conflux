---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/vcs-worktree-operations/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - src/worktree_ops/service.rs
  - src/worktree_ops/git_backend.rs
  - src/tui/state.rs
  - src/tui/types.rs
  - src/tui/key_handlers.rs
  - src/tui/render.rs
verifications:
  - id: dirty-worktree-delete-tests
    requirement: TUI operators can explicitly discard uncommitted worktree changes while ordinary and remote deletion remain fail-closed
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Rust unit and integration test output covering dirty refusal, second confirmation, fresh identity and eligibility revalidation, explicit local discard, teardown behavior, ahead/main/active/unknown/root-busy refusal, and remote API exclusion
    rerun: cargo test worktree_delete && cargo test worktree_ops && cargo test remote_worktree && cargo clippy --all-targets -- -D warnings
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Allow explicit dirty worktree deletion in the TUI

**Change Type**: implementation

## Premise / Context

- TUI worktree deletion currently opens one confirmation, but the shared service then rejects every `DirtyState::Dirty` target.
- The Git removal boundary already invokes `git worktree remove --force`; the blocking behavior is Conflux policy rather than a Git limitation.
- The existing `S` confirmation key controls only `.wt/teardown` skipping and must not become an ambiguous shortcut for discarding uncommitted files.
- Remote `/api/v2` deletion is deliberately fail-closed and forbids force or unsafe recovery controls.
- Repository identity, active-change, main-worktree, unresolved-base-merge, dirty-unknown, and commits-ahead guards remain required.

## Problem / Context

An operator cannot retire a disposable managed worktree when it contains uncommitted or untracked files. The first confirmation implies deletion is possible, but confirmation ends in a warning from `classify_delete_eligibility`. The operator must leave Conflux and manually remove the worktree even though the TUI already owns a confirmation and the lower Git boundary supports forced removal.

Simply allowing dirty deletion on the current `Y` path would make an irreversible data-loss action too easy and could accidentally weaken the shared remote policy. The TUI needs a distinct, explicit discard intent with a second confirmation and fresh revalidation at the mutation boundary.

## Proposed Solution

Keep ordinary `Y` deletion fail-closed for dirty worktrees. When a freshly observed target is dirty, transition the TUI from the ordinary delete confirmation to a dedicated destructive confirmation that states uncommitted and untracked files will be permanently lost. Require a distinct explicit confirmation input before emitting a delete command carrying local discard permission.

Extend the shared delete options with an explicit dirty-discard permission that defaults to false. Only the local TUI destructive-confirmation path may set it true. `classify_delete_eligibility` continues to reject dirty targets unless this permission is present, and still rejects main worktrees, unresolved base merges, unknown dirty state, and commits ahead. Existing active/deleting and branch-identity checks remain in the TUI and service flow.

At execution time, acquire the existing repository mutation guard, observe the target again, verify the confirmed branch identity, and re-run eligibility using the explicit policy before teardown or Git removal. A target that became dirty after ordinary confirmation must not be silently deleted; a target whose identity or eligibility changed after destructive confirmation must be retained. Dirty-discard deletion still runs `.wt/teardown` unless the operator independently selected the existing skip-teardown recovery action.

Record a warning before removal when explicit dirty discard is used. Keep `/api/v2` and WebUI on `DeleteOptions::fail_closed()` and do not add request fields, buttons, force controls, or unsafe recovery capability.

## Acceptance Criteria

1. A clean, eligible TUI worktree retains the existing ordinary confirmation and `Y` deletion behavior.
2. Ordinary deletion of a dirty worktree does not remove it and instead presents a dedicated second confirmation warning that uncommitted and untracked files will be permanently lost.
3. The second confirmation requires a distinct explicit discard action; cancel and unrelated keys preserve the worktree.
4. Explicit local discard removes a still-matching dirty worktree, runs teardown by default, deletes the associated branch best-effort, refreshes the list, and records a warning that dirty content was intentionally discarded.
5. Dirty-discard permission is denied by default and is set only by the TUI destructive-confirmation path.
6. Main worktrees, active/deleting targets, unresolved base merges, unknown dirty state, branch-identity mismatches, and worktrees with commits ahead remain undeletable even after destructive confirmation.
7. A target that changes identity or eligibility between either confirmation and execution is retained with an actionable warning.
8. The existing skip-teardown action remains independent: skipping teardown does not itself permit dirty deletion, and dirty deletion does not itself skip teardown.
9. `/api/v2` and WebUI retain fail-closed dirty deletion and expose no dirty-discard, force, unsafe recovery, path, or branch control.
10. Unit and integration tests prove the normal, destructive, cancellation, stale-state, teardown, and remote-exclusion paths with real service behavior rather than placeholder state transitions.

## Explicit Completion Conditions

- `DeleteOptions` and pure eligibility tests prove dirty permission is explicit, local-only at call sites, and does not waive any other guard.
- Typed TUI modal state distinguishes ordinary deletion from destructive dirty discard and owns the confirmed path and branch identity without hidden durable state.
- TUI key handling and rendering tests prove the warning text, distinct confirmation input, cancellation behavior, and absence of accidental deletion from ordinary `Y` or `S`.
- Service/backend tests prove fresh observation and identity validation occur before teardown and `git worktree remove --force`, including target drift after confirmation.
- Remote API tests prove dirty deletion still returns the existing typed refusal and that request schemas reject unsafe parameters.
- `cargo fmt --check`, the targeted verification rerun, `cargo test`, and `cargo clippy --all-targets -- -D warnings` exit successfully.

## Scope Rationale

TUI modal behavior, shared delete policy, execution revalidation, and regression coverage must ship together. Splitting them could expose a destructive confirmation that cannot execute safely, or introduce a permissive service option before every non-local caller is proven fail-closed.

## Out of Scope

- Force-deleting worktrees with commits ahead.
- Deleting the main worktree or a worktree owned by active orchestration.
- Allowing deletion when dirty state cannot be determined.
- Adding dirty-discard controls to `/api/v2`, WebUI, or a standalone CLI command.
- Automatically committing, stashing, exporting, or backing up dirty files.
- Changing `.wt/teardown` semantics or combining dirty discard with skip-teardown into one permission.
