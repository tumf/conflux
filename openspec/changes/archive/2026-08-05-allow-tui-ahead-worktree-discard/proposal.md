---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/vcs-worktree-operations/spec.md
  - openspec/specs/tui-worktree-view/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - src/worktree_ops/service.rs
  - src/worktree_ops/git_backend.rs
  - src/tui/types.rs
  - src/tui/state.rs
  - src/tui/key_handlers.rs
  - src/tui/render.rs
  - src/tui/command_handlers.rs
verifications:
  - id: local-tests
    requirement: Local TUI ahead-discard authorization, safety revalidation, branch deletion, and remote fail-closed behavior are covered before integration
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output covering worktree service, TUI modal/key handling, and remote worktree operation tests
    rerun: cargo test --all-features
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Allow explicit TUI discard of ahead worktrees and branches

**Change Type**: hybrid

## Problem / Context

Conflux currently refuses to delete any managed worktree whose branch has commits ahead of the base branch. This protects unreachable commits, but it also leaves obsolete worktrees undeletable even when a local operator intentionally wants to discard both the worktree and its unmerged local branch.

The local TUI already distinguishes ordinary deletion, teardown skipping, and explicit dirty-worktree discard. It lacks a separate authorization for discarding commits ahead of base. Remote deletion is intentionally fail-closed and must not gain a destructive override.

## Proposed Solution

Add a local-TUI-only destructive confirmation for deleting a managed worktree together with its local branch when that branch has commits ahead of base.

The shared deletion policy will represent commits-ahead discard as an independent permission from known-dirty discard and teardown skipping. An ordinary `Y` or `S` deletion attempt against an ahead worktree will return typed evidence to the TUI and open a dedicated confirmation. Only uppercase `X` in that confirmation may authorize loss of ahead commits. If the worktree is also dirty, the confirmation and authorization must explicitly cover both uncommitted changes and ahead commits rather than composing permissions implicitly.

Immediately before removal, the service will re-observe the worktree and reconfirm its identity, branch, HEAD, branch ref, dirty state, commits-ahead state, and base merge state. Teardown remains enabled unless independently skipped. After worktree removal, the explicitly authorized branch will be deleted only if its ref still points to the confirmed HEAD. Ref movement or an observation failure retains the branch and reports partial success.

Remote API and WebUI behavior remains fail-closed: no new request parameter, force mode, or ahead-discard capability is exposed.

This is one atomic change because the policy flag, typed refusal, confirmation UI, final safety checks, and branch deletion behavior must ship together to avoid either an unusable confirmation or an unsafe backend permission.

## Acceptance Criteria

1. Ordinary local deletion of a clean ahead worktree removes nothing and opens a dedicated destructive confirmation based on a fresh service observation.
2. The confirmation names the path, branch, HEAD, whether teardown will run, and that unmerged commits, the worktree, and the local branch will be permanently deleted without stash, backup, or merge.
3. A dirty and ahead worktree presents one confirmation that explicitly names both categories of data loss; authorization covers both only through that confirmation.
4. `Y`, `S`, lowercase `x`, and unrelated keys cannot grant ahead discard. Only uppercase `X` can submit the destructive intent; `N` and Escape cancel it.
5. Known-dirty discard, ahead-commit discard, and teardown skipping remain independent permissions. No one permission silently grants another.
6. Immediately before removal, changed or unknown identity, branch, HEAD/ref, dirty, ahead, or base-merge facts refuse deletion and preserve both worktree and branch.
7. A teardown failure preserves both worktree and branch unless teardown was independently skipped before the destructive confirmation.
8. After successful worktree removal, the branch is deleted through an atomic compare-and-delete conditional on the confirmed HEAD OID. A moved, missing, or unverifiable ref retains the branch and reports partial success.
9. Main, active, already-deleting, detached, or merge-busy targets remain ineligible regardless of destructive permissions.
10. Remote API and WebUI continue to report ahead worktrees as undeletable and reject unsafe parameters; they expose no ahead-discard permission.
11. Tests exercise clean-ahead success, dirty-ahead success, ahead with unknown dirty state, inert confirmation keys, cancellation, independent permissions, teardown failure, concurrent root-busy refusal, safety drift, worktree removal failure, branch deletion failure/ref drift, atomic compare-and-delete, remote error mapping, remote path redaction, and remote refusal.

## Explicit Completion Conditions

- `DeleteOptions` and the TUI delete intent model carry separate known-dirty and commits-ahead permissions, with remote construction leaving both disabled.
- The shared service returns typed ahead-target evidence before teardown/removal and accepts it only from the local destructive path.
- TUI modal state, rendering, key handling, revalidation, logs, and progress state implement the dedicated uppercase-`X` confirmation flow.
- The Git backend supports explicit deletion of the confirmed ahead branch without weakening ordinary merged-only cleanup.
- Service and TUI tests prove that stale or unknown repository evidence cannot reach worktree or branch removal.
- Remote worktree projection and command tests prove the API remains fail-closed.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` pass.

## Out of Scope

- Automatic merge, stash, backup, tag creation, or remote publication before discard.
- Ahead-discard controls in `/api/v2`, WebUI, or other remote clients.
- Automatic orchestration cleanup of ahead worktrees without an interactive local operator decision.
- Recovery of commits after the branch has been explicitly deleted.
