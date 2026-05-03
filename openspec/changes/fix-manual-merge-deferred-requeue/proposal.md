---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/state.rs
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
  - src/parallel/dispatch.rs
  - openspec/specs/orchestration-state/spec.md
---

# Fix manual merge-deferred archived workspace requeue loop

**Change Type**: implementation

## Problem / Context

In parallel mode, an archived workspace may fail to merge because the base repository working tree is dirty. This is a manual-recovery condition: the user must clean or commit the base workspace, then explicitly retry merge resolution with `M`.

A real run against `/Users/tumf/wakumo/avacus/avacuscc-dbot` showed `fix-dbot-skill-toggle-switch` repeatedly cycling through:

- resumed workspace state detected as `Archived`
- archive-complete resume handoff emits archive/merge handling
- merge attempt is deferred because the base working tree has uncommitted changes
- scheduler queue reconciliation re-adds the same change candidate
- the cycle repeats rapidly

Repository inspection indicates the reducer can leave normal `QueueIntent::Queued` in place after `MergeDeferred(auto_resumable=false)`. Scheduler reconciliation then treats the merge-wait change as ordinary queued work instead of manual merge wait.

## Proposed Solution

Make manual merge deferral consume normal queue intent while preserving explicit merge-wait retry behavior.

When the reducer processes `MergeDeferred(auto_resumable=false)`:

- keep the change in `WaitState::MergeWait`
- clear normal queued intent for that change
- remove the change from the reducer-owned resolve-wait queue
- leave it eligible for explicit user retry via `ReducerCommand::ResolveMerge` / TUI `M`

When the reducer processes `MergeDeferred(auto_resumable=true)`, preserve the existing retry semantics:

- keep or move the change into `WaitState::ResolveWait`
- keep it in reducer-owned retry intent
- allow scheduler-owned retry after merge/resolve capacity becomes available

The scheduler should no longer re-dispatch an archived merge-wait change as ordinary queued work while the base working tree remains dirty.

## Acceptance Criteria

- A manual dirty-base merge deferral for an archived change displays as `merge wait` and does not remain in `queued_change_ids()`.
- Scheduler queue reconciliation does not re-add a manual merge-deferred change as an ordinary queued candidate.
- Pressing `M` after the base workspace is cleaned still transitions the change from `merge wait` to `resolve pending` and allows scheduler-owned merge retry.
- Auto-resumable merge deferrals, such as another resolve/merge in progress, continue to enter `resolve pending` and remain retryable without a manual `M` press.
- The fix is covered by reducer-level and scheduler-reconciliation regression tests that would fail if queue intent remained set after manual merge deferral.

## Explicit Completion Conditions

This proposal is complete when:

- `src/orchestration/state.rs` updates `ExecutionEvent::MergeDeferred` handling so `auto_resumable=false` clears normal queue intent and resolve-wait membership while preserving `MergeWait`.
- Existing `ResolveMerge` behavior continues to allow explicit retry from `MergeWait`.
- Tests demonstrate that manual `MergeDeferred` removes a change from `queued_change_ids()` while auto-resumable `MergeDeferred` remains in `resolve_wait_change_ids()`.
- Tests demonstrate that scheduler reconciliation does not re-add a manual merge-deferred change from reducer queue intent.
- The relevant Rust test targets pass locally.

## Out of Scope

- Changing how `base_dirty_reason()` detects dirty base repositories.
- Automatically stashing, committing, or discarding user changes in the base workspace.
- Changing merge conflict resolution behavior after an explicit `M` retry.
- Changing serial-mode archive terminal semantics.
