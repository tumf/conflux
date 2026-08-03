---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/vcs-worktree-operations/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - src/worktree_ops/service.rs
  - src/worktree_ops/git_backend.rs
  - src/worktree_ops.rs
  - src/tui/types.rs
  - src/tui/state/modal_logic.rs
  - src/tui/state/worktree_action_logic.rs
  - src/tui/key_handlers.rs
  - src/tui/command_handlers.rs
  - src/tui/runner.rs
  - src/tui/render.rs
  - src/web/remote_control_api/worktrees.rs
  - tests/e2e_git_worktree_tests.rs
verifications:
  - id: dirty-worktree-delete-tests
    requirement: TUI operators can explicitly discard known dirty worktree content while safety observations and remote deletion remain fail-closed
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Non-empty Rust unit, integration, real-Git, API schema, and OpenAPI checks covering the Y/S/X state machine, shared guard, identity and ref drift, observation failures, teardown revalidation, branch retention, and remote exclusion
    rerun: cargo test --lib tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --lib tui_dirty_worktree_delete && cargo test --lib dirty_discard -- --list | grep -q dirty_discard && cargo test --lib dirty_discard && cargo test --lib remote_worktree_dirty_discard -- --list | grep -q remote_worktree_dirty_discard && cargo test --lib remote_worktree_dirty_discard && cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete && make check-openapi && cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Allow explicit dirty worktree deletion in the TUI

**Change Type**: implementation

## Premise / Context

- TUI deletion currently submits immediately after one confirmation; the shared service rejects known dirty state.
- The Git boundary already uses `git worktree remove --force`, while `S` means only skip teardown.
- `WorktreeInfo` does not carry dirty state, so escalation must originate from a fresh service observation rather than stale TUI projection.
- Current local deletion permits unknown dirty state, and ahead/base-merge observation failures can collapse to safe-looking values; this change intentionally makes destructive deletion fail closed.
- `/api/v2` intentionally exposes no force or unsafe recovery controls.

## Problem / Context

Operators cannot retire disposable managed worktrees containing uncommitted changes. Allowing the current `Y` path to bypass the dirty guard would risk data loss, while merely adding dialog state would not solve observation failures, teardown-induced drift, branch-ref races, or separate TUI/Web service guards.

## Proposed Solution

Ordinary `Y` or `S` submits deletion with known-dirty discard disabled. The shared service observes under one repository-scoped guard used by TUI and `/api/v2`. A clean eligible target is deleted; only `WorktreeOpError::Dirty` returns to the TUI adapter as escalation into typed `ConfirmDirtyDiscard`. No dirty field is added to `WorktreeInfo` or remote DTOs.

`ConfirmDirtyDiscard` carries path, expected Git worktree identity, branch, HEAD, and selected `skip_teardown`. Uppercase `X` alone grants known-dirty discard. `N` and `Esc` cancel; `Y`, `S`, lowercase `x`, and unrelated keys do not mutate. `Y` captures `skip_teardown=false`; `S` captures `true`, but never grants dirty discard by itself.

Replace local unknown-dirty fail-open behavior with two policies: ordinary deletion refuses known and unknown dirty state; explicit discard waives only known `DirtyState::Dirty`. Base branch, commits-ahead, base-merge, Git identity, branch ref, or other safety observations that cannot be determined also refuse deletion.

Split teardown from Git removal. Observe and validate expected identity/ref, main, merge, ahead, and dirty facts before teardown; re-observe safety-critical facts after teardown or immediately before removal when skipped. Emit a structured warning, then remove the worktree. Delete the branch only if its ref still matches the validated OID and safe reachability is reconfirmed; otherwise retain it and warn.

Known dirty means tracked/index changes and non-ignored untracked entries reported with explicit untracked-file status mode. Ignored-only content may classify clean; the ordinary dialog continues warning that the directory and generated/ignored contents may be removed. Full ignored-file discovery is out of scope.

Conflux serializes its own worktree mutations with the shared guard and rejects detectable drift immediately before removal. External Git processes are outside the atomic boundary; the proposal does not claim filesystem/Git transactions against them.

## Acceptance Criteria

1. Clean eligible targets retain ordinary `Y` deletion; `S` independently skips teardown.
2. `Y` on known dirty returns `Dirty` and opens destructive confirmation with teardown enabled; uppercase `X` confirms removal.
3. `S` on known dirty opens the same confirmation with teardown skipped; uppercase `X` confirms both explicit permissions.
4. In the destructive modal, `Y`, `S`, lowercase `x`, unrelated keys, `N`, and `Esc` never delete; cancellation retains content.
5. Unknown dirty, ahead, base-merge, identity, branch-ref, or required observation state never escalates to destructive confirmation and remains undeleted.
6. Main, active/deleting, identity/ref mismatched, unresolved-merge, and known-ahead targets remain undeletable.
7. TUI and `/api/v2` use one repository-scoped service/guard; detectable Conflux mutation races are serialized.
8. Teardown and Git removal are separate; post-teardown drift is revalidated before forced removal.
9. Branch cleanup retains and warns when the ref moved or safe reachability cannot be reconfirmed.
10. Structured warning records path, branch, `dirty_discard=true`, and `skip_teardown` immediately before forced removal.
11. Remote DTO, OpenAPI, and WebUI expose no dirty-discard, force, skip-teardown, path, or branch mutation controls.
12. Non-empty unit/integration filters and a real-Git heavy test prove success, refusal, drift, teardown, branch retention, and remote exclusion.

## Explicit Completion Conditions

- Typed state and tests prove the exact `D → Y/S → Dirty → X` flow without adding dirty state to projections.
- Safety observations use explicit unknown states or equivalent errors and fail closed.
- One injected repository-scoped service/guard is shared by TUI and Web runtime paths.
- Backend phases support post-teardown observation before removal and ref-safe branch cleanup.
- Status uses explicit non-ignored untracked reporting; ignored-only behavior is documented and tested.
- Verification commands prove matching new tests exist before running them.
- The declared rerun, `cflx openspec validate allow-tui-dirty-worktree-delete --strict --evidence warn`, and archive gate pass.

## Scope Rationale

The interaction, shared concurrency boundary, safety observations, teardown split, and branch retention must ship atomically; partial delivery could expose a destructive control without a trustworthy final guard.

## Out of Scope

- Stash, backup, automatic commit, or dirty-content fingerprinting.
- Full ignored-file enumeration or protection.
- Force deletion of known or unknown commits ahead.
- Remote/WebUI unsafe deletion controls.
- Durable workflow state or guarantees against concurrent external Git processes.
- Cleanup of unrelated duplicate canonical teardown requirements.
