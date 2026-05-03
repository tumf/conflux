# Design

## Overview

The bug occurs at the boundary between reducer-owned lifecycle state and scheduler-local queue reconciliation.

Manual merge deferral is a wait state, not ordinary queued work. The archived workspace has already completed apply/accept/archive. Re-dispatching it through normal queued candidate reconciliation re-enters resume handling and repeatedly attempts the same merge while the base repository remains dirty.

## Current Flow

1. A workspace reaches archive-complete state.
2. Parallel merge handling calls `attempt_merge()`.
3. `base_dirty_reason()` detects uncommitted changes in the base repository.
4. `attempt_merge()` returns `MergeAttempt::Deferred(DeferredMerge::manual(reason))`.
5. `MergeDeferred(auto_resumable=false)` reaches the reducer and UI.
6. The reducer sets or preserves `MergeWait`, but normal `QueueIntent::Queued` can remain set.
7. `reconcile_queued_candidates_from_shared_state()` reads `queued_change_ids()` and re-adds the archived change as an ordinary queued candidate.
8. Resume routing detects `WorkspaceState::Archived` and returns a merge handoff again, causing a tight loop.

## Target Flow

Manual merge deferral must be a terminal scheduler decision until explicit user retry:

1. `MergeDeferred(auto_resumable=false)` sets the reducer wait state to `MergeWait`.
2. The same reducer transition clears normal queue intent.
3. Scheduler queue reconciliation no longer sees the change in `queued_change_ids()`.
4. The UI still shows `merge wait` and allows `M`.
5. `M` applies `ReducerCommand::ResolveMerge`, which is independent from normal queue intent and moves the change to `ResolveWait`.
6. Scheduler-owned retry merges the archived workspace once the base repository is clean.

## Non-Authoritative State Constraint

This design follows `openspec/CONSTITUTION.md` by not introducing durable out-of-worktree workflow state. The change only corrects in-memory reducer intent classification for already repository-visible workflow facts:

- archived workspace state
- base working tree dirty state
- explicit user retry intent

Deleting `~/.local/state/cflx/**` must not be required for correctness and must not be used to break the loop.

## Alternatives Considered

### Filter `MergeWait` inside queue reconciliation

The scheduler could skip changes whose display state is `MergeWait`. This is less direct because queue reconciliation should consume reducer-owned queue intent, not reinterpret lifecycle display state. Clearing queue intent at the reducer transition better preserves a single source of truth.

### Treat dirty-base manual deferral as auto-resumable

This would keep the change in `ResolveWait` and retry automatically. That is unsafe because uncommitted user edits in the base workspace are not guaranteed to become clean without human action and should not cause a busy retry loop.

### Auto-stash or auto-commit base changes

Out of scope and unsafe. The base workspace belongs to the user; Conflux must not modify it to recover from dirty-base merge deferral.
