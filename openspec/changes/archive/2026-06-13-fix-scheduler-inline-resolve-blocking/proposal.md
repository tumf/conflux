---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/parallel/merge.rs
  - src/parallel/conflict.rs
  - src/orchestration/state.rs
  - src/parallel/tests/manual_resolve.rs
  - src/parallel/tests/auto_resolve.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-06-12-fix-resolve-queue-reanalysis/proposal.md
---

# Fix Scheduler Inline Resolve Blocking

**Change Type**: implementation

## Problem/Context

While a change is resolving in TUI parallel mode, a change newly queued with `x` stays
`queued` forever: dependency analysis never starts and apply never dispatches even though
run slots are free. Roughly fifteen prior changes (slot gating, debounce, classification,
reducer sync — most recently `2026-06-12-fix-resolve-queue-reanalysis`) did not fix the
user-visible bug because they all adjusted logic inside `perform_reanalysis_and_dispatch()`,
which is never reached while the symptom is active.

The actual root cause is that the scheduler loop task itself awaits base-lane resolve work
synchronously, in two variants:

1. **Inline resolve execution.** `execute_with_order_based_reanalysis()` awaits
   `maybe_dispatch_resolve_wait_retry()` at Step 2 of every iteration
   (`src/parallel/orchestration.rs:216`). That call chains through
   `retry_deferred_base_lane_waiters()` → `retry_deferred_merges_for()` →
   `attempt_merge()` → `merge_and_resolve()` → `resolve_conflicts_with_retry()`, which runs
   the AI resolve agent (minutes, with retries) inside the scheduler loop task. The same
   inline chain also runs from completion handlers (`src/parallel/queue_state.rs:669`,
   `:712`, `:805`), so consecutive resolve waiters chain back-to-back and starve analysis
   indefinitely. Manual resolve (`M` key) reaches the same inline path via reducer
   `ResolveWait` promotion.

2. **Inline lock wait.** A spawned post-archive merge task holds `global_merge_lock()`
   across its entire conflict-resolve agent run (`src/parallel/merge.rs:524` guard held
   through `merge_and_resolve` at `:610`). `attempt_merge()` checks the resolve-active
   counters only **after** acquiring the lock (`:526-538`). Whenever any
   resolve_wait/reject_wait entry exists and a queue notification arms
   `resolve_wait_retry_triggered` (`src/parallel/orchestration.rs:357` — pressing `x` does
   exactly this), the next loop iteration blocks on the lock until the active resolve
   finishes.

In both variants the loop never reaches dynamic-queue ingestion, queue reconciliation, or
`perform_reanalysis_and_dispatch()`, so the canonical requirements "Non-blocking Merge in
Scheduler Loop" and "Re-analysis triggers and non-blocking scheduler" are violated.
Existing regression tests mock the resolve command with instantly-completing commands, so
the starvation is invisible to the current test suite.

Tracking issue: beads `conflux-lti`.

## Proposed Solution

Make all base-mutating lane work (deferred merge retry including conflict resolve, and
rejection-review retry) execute outside the scheduler loop task, and make
`attempt_merge()` incapable of parking its caller on the global merge lock:

1. **Spawn resolve-wait retry execution.** `maybe_dispatch_resolve_wait_retry()` promotes
   the next base-mutating lane waiter (reducer-owned, unchanged) and then spawns the retry
   merge/resolve into a background tokio task — mirroring the existing `spawn_merge_task()`
   pattern — instead of awaiting it inline. The task reports its outcome back to the
   scheduler loop through the existing merge-result channel so completion handling,
   ResolveWait clearing, and next-waiter promotion stay scheduler-owned. The reducer's
   single-occupant base-mutating lane (`promote_next_base_mutating_lane_waiter()` returns
   `None` while occupied) remains the concurrency guard, so no second retry task can start
   while one is active.
2. **Apply the same non-blocking dispatch to completion handlers.** The inline
   `retry_deferred_base_lane_waiters().await` calls in workspace-completion, rejection, and
   merge-result handling delegate to the same spawn-based dispatch so back-to-back resolve
   waiters no longer monopolize the loop.
3. **Never block on `global_merge_lock` from the scheduler-owned dispatch path.** In
   `attempt_merge()`, evaluate the resolve-active counters before lock acquisition and use
   non-blocking lock acquisition (`try_lock`), returning the existing auto-resumable
   `MergeAttempt::Deferred` ("merge lane busy" class reason) when the lock is held. Deferred
   outcomes re-enter the existing ResolveWait retry flow on the next merge/resolve
   completion, which already exists today.
4. **Regression coverage with a slow resolve.** Add tests in which the resolve command is
   slow/blocked (controllable mock, e.g. long-running command or a gated future) and assert
   that, while it runs, a newly queued ordinary change is ingested, analyzed
   (`AnalysisStarted`), capacity-gate logged when applicable, and dispatched once capacity
   allows — without waiting for the resolve to finish.

Variants 1 and 2 ship in one change because the user-visible acceptance criterion
("a change queued during resolve is analyzed promptly") only holds when both blocking
variants are removed; fixing either alone leaves the same observable freeze reproducible
through the other path. They also touch the same functions and share regression tests.

## Acceptance Criteria

- While a resolve (manual `M`-key, deferred-merge retry, or post-archive auto resolve) is
  running, a change newly queued via the dynamic queue (`x` key) is ingested into the
  scheduler queue and dependency analysis starts within normal debounce bounds — it must
  not wait for the resolve to complete.
- Ordinary apply dispatch of the analyzed change still respects recalculated slot capacity
  (resolve consumes capacity exactly as before; no dispatch when recalculated capacity is
  zero).
- The scheduler loop task never awaits the resolve agent, the deferred merge retry, the
  rejection-review retry, or `global_merge_lock` acquisition directly; the loop keeps
  cycling (dynamic-queue checks, reconciliation, re-analysis, diagnostics) during active
  resolve work.
- At most one base-mutating lane operation (resolve or rejection review) executes at a
  time, preserved by the reducer lane occupancy — no change in lane semantics.
- Resolve/merge outcomes (merged, deferred, failed) are still delivered to the scheduler
  loop and still trigger ResolveWait clearing, next-waiter promotion, and reanalysis
  (`ResolveCompletion` reason) exactly as today.
- Existing behavior is preserved for: blocked-only drain, persistent-idle wait,
  finite-lifetime exit, resolve-wait/reject-wait ownership, terminal-error stop gates, and
  `is_fully_drained` accounting (an in-flight spawned retry must keep the scheduler from
  exiting or entering idle, via the lane occupancy and/or pending counters).

## Explicit Completion Conditions

- `src/parallel/queue_state.rs`: `maybe_dispatch_resolve_wait_retry()` (and the completion
  handlers at the current `:669`, `:712`, `:805` call sites) no longer transitively await
  `resolve_conflicts_with_retry()` or `merge_and_resolve()` in the scheduler loop task;
  retry execution is spawned and reports through the merge-result channel.
- `src/parallel/merge.rs`: `attempt_merge()` checks resolve-active counters before lock
  acquisition and acquires `global_merge_lock()` non-blockingly, returning an
  auto-resumable `Deferred` when the lane is busy.
- New regression tests exist that use a slow/gated resolve and assert
  ingestion + `AnalysisStarted` (and dispatch when capacity exists) for a change queued
  during the resolve, and they fail against the current inline implementation.
- Drain/idle/exit accounting tests cover an in-flight spawned retry task (scheduler does
  not exit while a spawned resolve retry is active).
- `cflx openspec validate fix-scheduler-inline-resolve-blocking --strict` passes.
- `cargo test` for the parallel scheduler suites (`parallel::tests::manual_resolve`,
  `parallel::tests::auto_resolve`, `parallel::tests::executor`) passes.

## Out of Scope

- Changing global merge/resolve lane semantics (single base-mutating operation at a time
  stays).
- Allowing resolve plus apply to exceed configured slot capacity.
- Replacing or re-tuning dependency analysis, debounce windows, or dispatch ordering.
- TUI key bindings and status display taxonomy.
- Serial mode (obsolete).
