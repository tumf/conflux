---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
  - src/orchestration/state.rs
  - src/parallel/orchestration.rs
  - src/parallel/tests/executor.rs
  - src/parallel/tests/auto_resolve.rs
  - src/parallel/tests/manual_resolve.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-06-13-fix-scheduler-inline-resolve-blocking/proposal.md
---

# Fix Spawned Retry Base-Mutating Lane Release

**Change Type**: implementation

## Problem/Context

`2026-06-13-fix-scheduler-inline-resolve-blocking` moved deferred-merge/rejection-review
retries off the scheduler loop (promotion + `spawn_base_lane_retry_task`) and made
`attempt_merge()` acquire `global_merge_lock()` with `try_lock`, returning an
auto-resumable `Deferred("Merge lane busy")` on contention. Two defects remain:

1. **Lane deadlock on non-Merged retry outcomes (new bug).** Promotion
   (`promote_next_base_mutating_lane_waiter()`, `src/orchestration/state.rs:910`) marks the
   occupant `activity = Resolving`/`Rejecting` with `wait_state = None`. When the spawned
   retry's `attempt_merge()` then defers auto-resumably (e.g. the new "Merge lane busy"
   path while a post-archive merge task holds the global lock), the reducer's
   `MergeDeferred(auto_resumable=true)` application (`src/orchestration/state.rs:1656-1667`)
   intentionally skips state changes for an `is_active()` entry, so the occupant stays
   `Resolving` with `wait_state = None` forever. The scheduler's result handler
   (`handle_merge_result_with_tx`, `src/parallel/queue_state.rs:847-873`) logs Deferred/Err
   outcomes without releasing the lane. Nothing else resets the activity, so
   `is_base_mutating_lane_occupied()` stays true permanently: every later promotion returns
   `None`, all ResolveWait/RejectWait waiters starve, and the stuck change displays
   "resolving" indefinitely. The same stranding occurs when the retry body fails before any
   `ResolveFailed`/`RejectionReviewFailed` event is emitted (workspace-lookup failure paths
   in `retry_deferred_merges_for` / `retry_deferred_rejection_review_for`). This violates
   the canonical requirement "Non-blocking Merge in Scheduler Loop" clauses "Deferred は既存の
   merge/resolve 完了トリガで自動的に再評価されなければならない（MUST）" and "base-mutating
   lane の単一占有は reducer の lane 占有状態によって維持されなければならない（MUST）".

2. **Missing regression coverage (truthful-completion violation).** The prior change's
   tasks.md marked Task 5 (drain test with an in-flight spawned retry), Task 6 (slow/gated
   resolve tests: ingestion + `AnalysisStarted` during resolve, capacity gating, lock
   variant) and Task 7 (deferred-then-retried convergence) as done, but no such tests exist
   in the repository — the only test changes were mechanical `origin` field additions in
   `src/parallel/tests/executor.rs`. The lock-variant test would have caught defect 1.
   Constitution Law 3 (truthful completion) requires repository-verifiable evidence; this
   change backfills it.

Defect 1 and the test backfill are kept in one change because the missing tests are the
verification of the fix, both target the same canonical requirement section (splitting
would guarantee archive-time spec merge conflicts), and the convergence/lock-variant tests
cannot pass without the fix.

## Proposed Solution

1. **Reducer-owned lane release.** Add an `OrchestratorState` method that releases the
   base-mutating lane for a non-terminal occupant after a non-Merged spawned-retry
   outcome: reset `activity` to `Idle`, restore `wait_state` to the origin's wait kind
   (`ResolveWait`/`RejectWait`), and re-enqueue the change uniquely in the corresponding
   wait queue so a later promotion can pick it up. Preserve `global_invariants_hold()`.
2. **Origin-aware result handling.** In `handle_merge_result_with_tx`, when
   `merge_result.origin` is `ResolveWaitRetry` or `RejectWaitRetry` and the outcome is
   `Deferred(auto_resumable=true)` or `Err`, invoke the lane release through
   `shared_orchestrator_state` and resync scheduler-local wait sets. Manual
   (non-auto-resumable) deferrals keep today's reducer behavior (`MergeWait`, lane already
   released by the reducer's manual branch). Suppress the duplicate generic
   "Background merge failed" `ParallelEvent::Error` for retry origins whose retry body
   already emitted a specific failure event (`ResolveFailed` /
   `RejectionReviewFailed`); emit an operator-visible event for the
   workspace-lookup failure path that currently reports nothing.
3. **Backfilled regression tests** (all default-suite, each under 1 second, using
   controllable gates instead of sleeps):
   - Lock variant (prior Task 6d): with `global_merge_lock` held, a promoted ResolveWait
     retry defers with "Merge lane busy"; assert the lane is released, the change returns
     to ResolveWait, and the next dispatch promotes it again. Fails before the fix.
   - Deferred-then-retried convergence (prior Task 7): after the lock holder finishes and
     a merge-completion trigger fires, the deferred change is retried to completion
     without user action. Fails before the fix.
   - Gated-resolve scheduler test (prior Task 6a/6c): while a gated resolve runs, a
     dynamically queued change is ingested and `AnalysisStarted` fires within bounded
     scheduler ticks; the zero-recalculated-capacity variant suppresses apply dispatch
     with the capacity diagnostic.
   - Drain test (prior Task 5): with a spawned retry in flight and queue/in-flight empty,
     a finite-lifetime scheduler does not exit until the retry result arrives.

## Acceptance Criteria

- After a spawned base-lane retry reports an auto-resumable `Deferred` or an `Err`, the
  reducer base-mutating lane is unoccupied (`is_base_mutating_lane_occupied()` is false
  unless another occupant is legitimately active), the change is back in a retryable wait
  state, and a subsequent merge/resolve completion or queue notification re-promotes it.
- No change can remain in `Resolving`/`Rejecting` activity with no live base-lane task.
  `global_invariants_hold()` holds across promotion → deferral → release → re-promotion.
- A merge deferred by `try_lock` contention or the active-resolve precheck converges to
  `MergeCompleted` after the blocking work completes, without user action.
- While a gated (slow) resolve runs, a dynamically queued change reaches
  `AnalysisStarted` within normal debounce bounds, and apply dispatch stays
  capacity-gated exactly as today.
- A finite-lifetime scheduler does not exit or enter persistent idle while a spawned
  retry is in flight.
- Retry failures produce exactly one operator-visible failure report per failure (no
  duplicate generic "Background merge failed" on top of `ResolveFailed` /
  `RejectionReviewFailed`), and the workspace-lookup failure path emits an
  operator-visible event.
- Existing behavior is preserved for: manual (non-auto-resumable) deferral handling,
  blocked-only drain, resolve-wait/reject-wait ownership, terminal-error gates, and
  `pending_merge_count` accounting.

## Explicit Completion Conditions

- `src/orchestration/state.rs` exposes a lane-release method covering both
  `ResolveWait` and `RejectWait` origins with unit tests asserting activity reset,
  wait-state restoration, unique re-enqueue, and `global_invariants_hold()`.
- `src/parallel/queue_state.rs::handle_merge_result_with_tx` matches on
  `merge_result.origin` for `Deferred(auto)`/`Err` outcomes and calls the release; no
  code path leaves a promoted occupant `Resolving`/`Rejecting` after a non-Merged
  spawned-retry outcome.
- The four regression tests listed above exist in `src/parallel/tests/` (executor /
  auto_resolve / manual_resolve suites as appropriate); the lock-variant and convergence
  tests fail when the lane-release fix is reverted.
- Each new default-suite test completes in under 1 second (AGENTS.md rule), using
  controllable gates (file/oneshot) rather than long sleeps; any wide-window variant is
  gated behind the `heavy` feature.
- `cargo test` passes for `parallel::tests::executor`, `parallel::tests::auto_resolve`,
  `parallel::tests::manual_resolve`, and `orchestration::state` unit tests; `cargo fmt
  --check` and `cargo clippy` pass.

## Out of Scope

- Removing the `#[allow(dead_code)]` legacy inline retry paths
  (`maybe_dispatch_resolve_wait_retry`, `retry_deferred_base_lane_waiters`,
  `handle_merge_result`, `wait_for_persistent_idle_wake` wrappers) — separate cleanup.
- Changing base-mutating lane semantics (single occupant at a time stays).
- Changing `attempt_merge()` lock/counter ordering (already correct).
- TUI key bindings, status display taxonomy, serial mode (obsolete).
