---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/state.rs
  - src/tui/orchestrator.rs
  - src/tui/state/event_handlers/errors.rs
  - src/tui/state/event_handlers/completion.rs
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Restrict Resolve Pending Blockers

**Change Type**: implementation

## Problem / Context

`resolve pending` currently appears in cases broader than the intended lifecycle semantics. In particular, archived changes can be moved into `ResolveWait` through auto-resumable merge-deferred handling even when the only other active work is applying or accepting, or when no eligible blocker exists.

The intended behavior is narrower: `resolve pending` should mean the change is ready for merge/resolve retry but cannot start because another change is actively occupying the resolve/rejection coordination lane. That lane is limited to active `resolving` or active `rejecting` work from another change.

This proposal follows the Conflux Constitution: the change must not introduce out-of-worktree durable workflow state, and completion must be proven by repository-verifiable tests and implementation evidence.

## Proposed Solution

Restrict `ResolveWait` / `resolve pending` creation to cases where another non-terminal change is actively `resolving` or `rejecting`.

Update reducer, TUI post-archive dispatch, and parallel merge-deferred handling so:

- another active `resolving` change may cause an archived/merge-deferred change to enter `ResolveWait`
- another active `rejecting` change may cause an archived/merge-deferred change to enter `ResolveWait`
- active `applying`, `accepting`, or `archiving` changes do not cause `ResolveWait`
- terminal `rejected` changes do not cause `ResolveWait`
- when no eligible blocker exists, post-archive handling attempts merge immediately or leaves the change in manual `MergeWait` if merge is deferred for dirty base or another manual intervention reason
- queued/manual user resolve intent remains supported when the selected row is already in `MergeWait`, but the automatic post-archive path must not use `ResolveWait` as a generic waiting state

## Acceptance Criteria

- `OrchestratorState` only derives automatic `ResolveWait` for archived/deferred changes when another active non-terminal change is `Resolving` or `Rejecting`.
- TUI post-archive dispatch does not emit an auto-resumable resolve-pending event solely because another change is applying, accepting, archiving, terminal rejected, or absent.
- Parallel deferred merge handling does not classify a deferred merge as auto-resumable by parsing human-readable reason strings.
- A change already in `MergeWait` can still be explicitly requested for resolve by user/scheduler intent through `ReducerCommand::ResolveMerge`.
- Rejecting-driven `ResolveWait` has a retry trigger when rejection review completes or fails, so queued merge retries are not stranded.
- Specs for orchestration state and parallel execution describe the narrower blocker set.

## Explicit Completion Conditions

- Source changes are made in reducer/TUI/parallel paths that currently create or consume `ResolveWait`.
- Unit tests in reducer/TUI/parallel modules cover resolving, rejecting, applying, accepting, terminal rejected, and no-active-blocker cases.
- At least one test proves rejection review completion/failure triggers retry of resolve-pending changes.
- `cargo test` passes for the touched module tests or a narrower documented command that exercises all new tests.
- `cflx openspec validate restrict-resolve-pending-blockers --strict --evidence warn` passes before implementation is accepted.

## Out of Scope

- Changing the meaning of manual `M` key resolve intent for rows already in `merge wait`.
- Introducing durable scheduler state outside workspace/git/base-branch-derived state.
- Reworking the entire merge conflict resolution algorithm.
