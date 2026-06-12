---
change_type: implementation
priority: high
dependencies:
  - fix-dynamic-queue-ingest-repo-root
references:
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
  - src/parallel/tests/executor.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-06-13-fix-spawned-retry-lane-release/proposal.md
---

# Fix Spawned Retry Give-Up Paths Leaving the Base-Mutating Lane Occupied

**Change Type**: implementation

## Problem/Context

`2026-06-13-fix-spawned-retry-lane-release` added
`release_base_mutating_lane_after_retry` and wired it into
`handle_merge_result_with_tx` for `Deferred(auto_resumable=true)` and `Err` retry
outcomes. However, several retry-body paths report `Ok(MergeTaskOutcome::Merged)`
without performing a merge and without any terminal reducer transition — "give-up"
paths that abandon the retry intent:

1. `retry_deferred_merges_for` already-merged-to-base skip
   (`src/parallel/queue_state.rs:1137`)
2. `retry_deferred_merges_for` workspace gone, `Ok(None)`
   (`src/parallel/queue_state.rs:1162`)
3. `retry_deferred_merges_for` stale workspace path
   (`src/parallel/queue_state.rs:1192`)
4. `retry_deferred_rejection_review_for` workspace gone, `Ok(None)`
   (`src/parallel/queue_state.rs:1374`)

For a promoted occupant (activity `Resolving`/`Rejecting`, `wait_state = None`),
`clear_resolve_wait_intent` / `clear_reject_wait_intent` only clear wait state and
queue membership — they do not reset `activity`. The `Merged` branch of
`handle_merge_result_with_tx` never calls the lane release, and `apply_observation`
early-returns for active entries, so nothing ever resets the occupant: it stays
`Resolving`/`Rejecting` forever, `is_base_mutating_lane_occupied()` stays true, and
every later `promote_next_base_mutating_lane_waiter()` returns `None`. All
ResolveWait/RejectWait waiters starve permanently — the same deadlock class the prior
change fixed for Deferred/Err outcomes.

This violates the canonical requirement "Non-blocking Merge in Scheduler Loop", in
particular the MUST NOT clause "lane 占有の解放漏れにより promotion が恒久的に不能と
なる状態（生存するタスクを伴わない Resolving / Rejecting の残留）を生じさせてはなら
ない" and the scenario "Retry failure without a failure event still releases the lane
（例: workspace が見つからない）". The backfilled tests
`resolve_retry_workspace_lookup_failure_is_operator_visible` /
`reject_retry_workspace_lookup_failure_is_operator_visible` only assert the Error
event and the `Ok(Merged)` outcome — not lane release — so they currently lock in the
broken behavior.

Tracked as beads issue `conflux-m8d`. Depends on
`fix-dynamic-queue-ingest-repo-root` because the default test suite is red on current
main until that change lands, which would block this change's quality gates.

## Proposed Solution

1. **Reducer-owned lane abandon.** Add an `OrchestratorState` method (e.g.
   `abandon_base_mutating_lane_occupant(change_id)`) that releases the lane for a
   non-terminal occupant whose `activity` is `Resolving`/`Rejecting` WITHOUT restoring
   a wait state and WITHOUT re-enqueueing: reset `activity` to `Idle`, leave
   `wait_state = None`, clear blocked metadata, and remove the change from both
   base-mutating wait queues. No-op for terminal entries and non-occupants. This is
   deliberately distinct from `release_base_mutating_lane_after_retry` (which restores
   the wait state for retryable outcomes): give-up means the intent is abandoned, so
   re-enqueueing would cause a give-up loop on every later merge trigger.
2. **Synchronous release at the give-up sites.** Each of the four give-up paths calls
   the abandon method through `shared_orchestrator_state` at the point where it clears
   the retry intent, before returning `Ok(Merged)`. Release happens in the retry body
   (synchronously with intent clearing), not by inference in the result handler's
   `Merged` branch — terminal transitions for some outcomes (e.g. rejection Confirm's
   `ChangeRejected`) travel asynchronously via the event channel, so occupant-state
   inference at result-delivery time would race.
3. **Test strengthening.** Extend the two workspace-lookup operator-visibility tests to
   promote a real occupant first and assert the lane is unoccupied afterwards and the
   next waiter can be promoted; both must fail before this fix. Add a give-up
   next-waiter convergence test.

## Acceptance Criteria

- After any of the four give-up paths runs for a promoted spawned-retry occupant,
  `is_base_mutating_lane_occupied()` is false (unless another occupant is legitimately
  active), the change is NOT re-enqueued in either base-mutating wait queue, and a
  queued next waiter can be promoted on the next trigger.
- No change can remain in `Resolving`/`Rejecting` activity with no live base-lane task
  after a give-up outcome. `global_invariants_hold()` holds across
  promotion → give-up → next-waiter promotion.
- Existing give-up observability is preserved: the workspace-gone paths still emit
  exactly one operator-visible `ParallelEvent::Error`, and the give-up outcome still
  acts as a `Merged`-equivalent trigger for next-waiter dispatch in
  `handle_merge_result_with_tx`.
- Existing behavior is preserved for: real merged outcomes (terminal transition via
  `mark_deferred_merge_completed_in_shared_state` / rejection events), auto-resumable
  Deferred and Err release-and-restore semantics from the prior change, manual
  deferral handling, and `pending_merge_count` accounting.

## Explicit Completion Conditions

- `src/orchestration/state.rs` exposes the abandon method with unit tests asserting:
  activity reset to `Idle`, `wait_state` stays `None` (not restored), no membership in
  `resolve_wait_queue`/`reject_wait_queue` afterwards, no-op on terminal entries and
  non-occupants, and `global_invariants_hold()` after abandon.
- All four give-up sites in `src/parallel/queue_state.rs` invoke the abandon method via
  `shared_orchestrator_state` before returning; `rg` shows the call adjacent to each
  `clear_resolve_wait_intent_for_outcome` / `clear_reject_wait_intent_for_success`
  give-up call.
- `resolve_retry_workspace_lookup_failure_is_operator_visible` and
  `reject_retry_workspace_lookup_failure_is_operator_visible` assert lane release and
  next-waiter promotability, and fail when the abandon wiring is reverted.
- A convergence test exists showing a second queued waiter is promoted after the first
  occupant's give-up, without user action.
- `cargo test --lib parallel::tests` and `cargo test --lib orchestration::state` pass;
  `cargo fmt --check` and
  `cargo clippy --locked --all-targets --all-features -- -D warnings` pass; each new
  default-suite test runs under 1 second (AGENTS.md rule).

## Out of Scope

- Changing what the give-up paths report as their `MergeTaskOutcome` (they keep
  returning `Ok(Merged)` as the next-waiter dispatch trigger; introducing a dedicated
  `GaveUp` outcome variant is a possible future refactor, not required for
  correctness).
- The `attempt_merge` Err path's TUI visibility regression (log-only failure report for
  retry origins) — observability-only follow-up, separate proposal if desired.
- The self-referential gated-resolve test fixture — handled by
  `fix-dynamic-queue-ingest-repo-root`.
- Removing the `#[allow(dead_code)]` legacy inline retry paths — separate cleanup.
