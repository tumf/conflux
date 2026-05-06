---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/tui-resolve/spec.md
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/tui/state.rs
  - src/tui/command_handlers.rs
---

# Fix resolve wait progress while other changes apply

**Change Type**: implementation

## Premise / Context

- The user observed that pressing `M` on a `merge wait` row changes it to `resolve pending`, but it can remain there while other changes are applying.
- Existing constitutional law requires workflow control state to remain derivable from workspace/git/reducer state, and completion must be backed by repository-verifiable evidence.
- Canonical specs already define `M` as intent-only, with scheduler-owned retry execution through reducer-owned `ResolveWait`.
- Relevant code paths are the TUI `resolve_merge` intent path, TUI command scheduling, reducer `ResolveMerge` handling, and the parallel scheduler retry loop.
- Existing tests cover empty manual resolve startup and queue dispatch during `ResolveWait`, but do not fully cover manual `M` while apply work remains in flight.

## Requested Artifact

Implementation.

## Inferred Request

- Ensure pressing `M` on a `merge wait` change never leaves it stuck indefinitely at `resolve pending` solely because other changes are still applying.
- Preserve scheduler-owned semantics: `M` records retry intent, and the parallel scheduler owns merge/resolve retry execution.
- Keep other queued/applying changes progressing while resolve retry intent is pending.

## Problem / Context

Manual merge retry is currently represented as reducer-owned `ResolveWait`. That is correct, but the user-visible behavior is confusing and possibly stuck when the scheduler is busy with other apply/archive work: the TUI shows `resolve pending`, while no obvious retry progress occurs until later scheduler events.

The intended behavior is stricter: `resolve pending` means the retry intent is durable and scheduler-visible, not forgotten. If apply work is in flight, the scheduler must keep processing that work and then retry the merge as soon as the base-mutating lane and preserved workspace state allow it. If the retry cannot proceed because manual intervention is still required, the state must transition back to `merge wait` with a visible reason instead of remaining pending forever.

## Proposed Solution

Tighten manual resolve retry handling across the TUI, reducer, and parallel scheduler:

1. Add regression coverage for pressing `M` on `MergeWait` while another change is applying or otherwise in flight.
2. Ensure queue notifications or completion events trigger retry dispatch after the in-flight apply/archive work releases scheduler capacity.
3. Ensure retry attempts produce one of three observable outcomes:
   - `resolving` / `merged` when the preserved archived workspace can merge,
   - `merge wait` with a manual reason when the base remains dirty or workspace verification fails,
   - `resolve pending` only while retry is genuinely waiting on scheduler/base-lane capacity.
4. Preserve current behavior where queued changes can continue dispatching when resolve wait exists and normal slots remain available.

## Acceptance Criteria

- Pressing `M` on a `merge wait` row while another change is applying records reducer-owned `ResolveWait` and keeps the row as `resolve pending` only while scheduler retry is legitimately waiting.
- When the other applying/archive work completes, the scheduler retries the pending merge without requiring another `M` keypress.
- The scheduler does not exit while reducer-owned `ResolveWait` remains pending, including the case where apply work was in flight when the user pressed `M`.
- If the retry still cannot proceed because manual intervention is required, the row returns to `merge wait` with an error/warning path that is visible in TUI logs or warning state.
- Queued non-resolve changes continue to be analyzed and dispatched when slots are available; resolve wait must not suppress unrelated apply progress.

## Explicit Completion Conditions

- `src/parallel/orchestration.rs` and/or `src/parallel/queue_state.rs` contain verified scheduler behavior that retries reducer-owned `ResolveWait` after in-flight apply/archive work completes.
- `src/tui/state.rs` and `src/tui/command_handlers.rs` continue to treat `M` as intent-only and do not directly execute merge/resolve outside the scheduler loop.
- Regression tests in `src/parallel/tests/executor.rs` or another parallel/TUI test module fail against a stub/no-op retry path and pass only when `ResolveWait` is actually retried after in-flight work drains.
- TUI-facing tests verify that manual `M` produces `resolve pending`, then either reaches `resolving`/`merged` or returns to `merge wait` on manual deferral.
- `cargo test` or a narrower documented Rust test command covering the touched modules passes.

## Out of Scope

- Changing the intent-only architecture of the `M` key.
- Adding durable workflow control state outside workspace/git/reducer state.
- Reworking the full parallel scheduler architecture beyond the resolve-wait progress bug.
- Altering unrelated acceptance, archive, or rejection-review semantics.
