---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/hooks/spec.md
  - src/hooks.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - scripts/bump.sh
  - .cflx.jsonc
  - ~/.local/state/cflx/logs/conflux-bda270b8/2026-05-04.log
---

# Block merged transition on on_merged hook failure

**Change Type**: implementation

## Problem / Context

`on_merged` is specified to run after repository-visible merge success and before the change transitions to terminal `Merged` status. In the current parallel merge paths, the hook is invoked before `MergeCompleted`, but hook failure is only logged as a warning and the merged transition still proceeds.

A concrete regression already occurred on `prevent-self-referential-validation-tasks`: `on_merged` ran `make bump-patch`, hit Git lock and dirty-state problems, returned non-zero, and still allowed `MergeCompleted` to be emitted. This left `Cargo.toml`, `Cargo.lock`, and `docs/openapi.yaml` modified in `main` while the change was displayed as merged. That violates the intent of `on_merged` as a pre-merged-transition gate and undermines truthful completion.

The current lock protection is also too weak for repo-mutating hooks. `HookRunner` only waits for root `.git/index.lock`, does not fail closed when the lock remains, and offers little observability about whether the lock was already present, was created during hook execution, or was caused by nearby Conflux Git activity.

## Proposed Solution

Treat `on_merged` as a real success gate when `continue_on_failure=false`.

- If `on_merged` fails, Conflux must not emit `MergeCompleted`, must not transition the change to terminal `Merged`, and must preserve an operator-visible non-terminal or error state that reflects hook failure.
- Strengthen pre-hook write-safety checks for repo-mutating `on_merged` commands, especially around root-repo Git lock contention and immediate post-merge cleanup races.
- Improve diagnostics so logs clearly show whether Conflux waited for lock release, whether the wait timed out, and which local Git conditions blocked the hook.

This change must remain constitution-compliant: the fix cannot depend on hidden durable workflow-control state. It may use logs and diagnostics for observability, but merge/resolve/reject routing must still derive from workspace/git/base-tree facts.

## Acceptance Criteria

- When `hooks.on_merged` fails and `continue_on_failure=false`, Conflux does not emit `MergeCompleted` and does not transition the change to terminal `Merged`.
- The change remains in a visible failure/blocking state that explains `on_merged` failure and does not falsely appear merged.
- Deferred merge retry success, immediate parallel merge success, and manual resolve success all obey the same gate: no `merged` transition before successful `on_merged` completion when failure is non-continuable.
- `on_merged` preflight logging clearly records whether root `.git/index.lock` was already present, whether wait succeeded or timed out, and whether repo-mutating preconditions were unsafe.
- A hook failure caused by root-repo lock contention is reproducibly covered by tests or deterministic simulation, and the regression fails without the fix.
- The change does not add hidden out-of-worktree durable workflow-control state.

## Explicit Completion Conditions

- `src/parallel/merge.rs` and `src/parallel/queue_state.rs` no longer swallow `run_hook(HookType::OnMerged, ...)` failure when `continue_on_failure=false`; instead they block `MergeCompleted` and prevent reducer `Merged` transition.
- The code path that currently logs `on_merged hook failed for ...` and then sends `ParallelEvent::MergeCompleted` is replaced or guarded so failure cannot fall through to merged success.
- `src/hooks.rs` includes stronger, repository-verifiable diagnostics around root `.git/index.lock` waiting and repo-mutating hook preflight outcome.
- Tests in hook/parallel/reducer modules prove that `MergeCompleted` is absent after failing `on_merged`, and that user-visible status is not `merged` in that scenario.
- Targeted Rust tests and OpenSpec validation pass.

## Out of Scope

- Redesigning release versioning policy or changing what `make bump-patch` does.
- Converting `on_merged` into a batch hook that runs once per orchestration instead of once per merged change.
- Generalizing all hook types to the same gating semantics unless needed to preserve consistency for `on_merged`.
