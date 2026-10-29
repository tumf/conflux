# Narrow post-classification cancellation fence

## Authority and causal sequence

At reviewed base `0444fd4c`, shared `RunControlService::cancel_stop` clears `RunSchedulerPort::set_graceful_stop(false)` and returns `StopCancelled`. The real TUI port stores to its `Arc<AtomicBool>` with `SeqCst` (`src/tui/run_supervisor.rs:447-449`); `ParallelExecutor::graceful_stop_requested` loads that flag with the same ordering. `OperatorApplication::apply_cancel_stop` projects the accepted result (`src/orchestration/operator_coordinator.rs:959-967`). The scheduler helper is not serialized with that command over the classifier's await.

Counterexample ordering:

1. Stop is pending; the helper's entry load sees true and the coherent reducer snapshot is complete.
2. Only inactive blocked candidates remain; an archived dependency makes `DependencyContext::is_blocked` call `effective_dependency_base`, which awaits `WorkspaceManager::ensure_original_branch_initialized` and `current_branch` before the real Git base-evidence probe.
3. Cancel-stop completes and stores false while that future is pending.
4. The classifier resumes, still correctly concludes blocked-only, and the current helper emits `Stopped` using its stale entry observation.

Ordinary missing dependencies, terminal-error rows, and held rows can classify without yielding. Holding the reducer lock after the snapshot is captured will not suspend this path. Neither is a valid race fixture. The archived-dependency route is essential.

## Decision and exact limit

Add a second `graceful_stop_requested()` check after either safe-boundary predicate has qualified and before successful-settlement logging / the existing `send_event(...Stopped).await`. False returns false, so the caller proceeds to its existing idle/admission logic rather than setting `graceful_stop_settled`. Keep the first check as the no-request fast path. Do not mutate the flag, compare-and-swap it, or add request identities.

This closes stale intent across the newly awaited classifier. The final flag load is the observation boundary for this fix, not an atomic global stop commit: `src/events.rs:1238-1243` can itself await channel capacity, and another thread may change the flag after the load. Existing event-send/dispatch races after the final load are not solved or claimed solved here. No new await is permitted between the final load and starting the existing send. A later true flag from a fresh Stop remains eligible.

## Deterministic regression fixture

Implement the following in the existing `blocked_idle_graceful_stop` test module, using production predicates rather than a classifier double:

- Temporary Git repository on `main`, with a committed valid queued proposal depending on `archived-upstream`. Assert the strict metadata parser returns that dependency. Create the archive entry `openspec/changes/archive/2026-01-01-archived-upstream/proposal.md` in the temporary checkout after the base commit, without adding it to `main`'s tree. There is no active `archived-upstream` proposal. Assert archive discovery includes the dependency while `is_merged_to_base(..., "main")` returns false. This deliberately models archived-on-disk but not integrated evidence, not a metadata parse error or nonexistent branch.
- Bind a complete reducer snapshot with ordinary queued intent, an empty JoinSet/in-flight set, and no lane/manual/merge work. Capture it before starting the boundary future.
- Inject a test-only `BlockingDependencyWorkspaceManager` through existing `ParallelExecutor::set_workspace_manager`. Its `ensure_original_branch_initialized` acknowledges entry via a one-shot channel, then awaits a release channel, then returns `main`; `current_branch` returns `Some("main")`. Keep `is_merged_to_base` real. Other unused mutating manager methods fail loudly if invoked. Synchronization is test-local only; no production delay, flag, or hook.
- Wire `RunControlService` and existing `run_control::testing::RecordingScheduler` to the same reducer/queue/marks. Bind `scheduler.graceful_stop_flag()` to the executor. Request Stop through `service.stop(OperatorMode::Running)` and assert `StopRequested` before polling the settlement future.
- Drive the actual helper future and the test control branch concurrently with `tokio::join!` or the equivalent scoped pinned future. Await the manager's entered acknowledgement before cancelling; assert the shared flag was true on entry. Await `service.cancel_stop(OperatorMode::Stopping)`, assert `StopCancelled` and flag false, then release classification. The helper must return false. Drain the real event channel and assert zero terminal/work-start events; no test injects a terminal event.
- For a later Stop on the same executor, make the manager's one-shot barrier pass through after its first release, call `service.stop(OperatorMode::Running)` again, and execute one further boundary evaluation. Assert true, exactly one emitted `Stopped` total, no `AllCompleted`, and no analyzer/worker/merge/push/cleanup started. This does not repeatedly settle the same request; the production loop exits on true.
- A second fresh fixture exercises the same entered/release barrier without withdrawing Stop. Assert no events while held, then true and exactly one `Stopped` after release. This rules out an always-false implementation.
- Use an event channel with known spare capacity (at least 16 and initially empty) so this test isolates classifier suspension, not send backpressure. Channel entry/release and helper completion establish all ordering. A 10-second timeout may only fail a hung await; timeout expiry never establishes a passing absence assertion. Do not use sleeps or a race lottery.

Required new test names:

1. `blocked_idle_graceful_stop_cancel_during_classifier_never_stops`
2. `blocked_idle_graceful_stop_pending_after_classifier_stops_once`

The first must fail on the unguarded helper at its false/no-Stopped assertions after a proven classifier barrier. The second and existing full-loop `blocked_idle_graceful_stop_settles_parked_dependency_blocked_scheduler` prevent disabling Stop as a fix. Existing `blocked_idle_graceful_stop_cancel_before_boundary_never_stops` remains a distinct pre-entry control.

## Verification map

| Contract | Ownership | Declared verification |
| --- | --- | --- |
| Cancel during classifier, genuine archived/unmerged dependency, later Stop, pending control | integration — real helper/classifier/reducer and temporary Git evidence, workspace suspension double | `idle-stop-cancel-race` |
| Exactly-once real loop, no work execution/success, incomplete/owned-work/dispatchable guards | integration — existing `blocked_idle_graceful_stop` tests, unchanged | `idle-stop-cancel-race` |
| Fully empty / no-request settlement and shared lifecycle semantics | integration — existing `idle_stopping_` adapter/boundary tests, unchanged | `idle-stop-existing-boundaries` |

Commands and authority are exclusively in proposal frontmatter; each is one bounded repository-local run, no full-suite repetition or external credential. Selected test names/counts and failures must be inspected, not inferred from Cargo exit zero alone. These are prospective acceptance conditions; proposal validation is not a test PASS.

## Rejected alternatives

- Set-clear before scheduler wake: useful existing control, but never crosses the entry-read/classifier interval.
- Hold the reducer write lock: the helper already has its snapshot; this does not reach the vulnerable await.
- Replace the classifier with a bool or insert a test-only sleep between flag read and send: bypasses the production cause and may pass without the intended fix.
- New cancellation generation, durable record, mutex spanning event delivery, retry timer, or frontend shortcut: unnecessary authority/behavior changes for this bounded race, and not justified by its non-destructive outcome.
