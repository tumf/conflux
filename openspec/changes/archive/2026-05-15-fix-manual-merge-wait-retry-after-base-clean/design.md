# Design: Manual Merge-Wait Retry After Base Clean

## Background

The post-archive merge lane is reducer-owned. TUI display state, executor-local retry sets, and scheduler notifications are caches or delivery mechanisms, not workflow-control authority.

The failing sequence is a handoff problem rather than a new workflow concept:

1. Merge readiness detects dirty base and emits `MergeDeferred(auto_resumable=false)`.
2. The reducer correctly records manual `MergeWait` and removes scheduler-owned resolve membership.
3. The user resolves the manual blocker outside Conflux by cleaning the base repository.
4. The user presses `M`, which should create explicit reducer-owned `ResolveWait` intent.
5. The scheduler must consume that intent even when no ordinary queued apply work remains.

## Design Principles

- Keep workflow routing derived from workspace/git/base tree evidence; do not add durable external retry markers.
- Treat the shared reducer as the source of truth for manual retry membership.
- Treat executor-local sets as synchronized caches that must not outlive reducer truth.
- Make scheduler wakeups level-triggered by reducer lane-wait membership where possible, not dependent only on ordinary queue candidates.
- Prefer observable diagnostics over silent no-op behavior for accepted retry intent that cannot dispatch.

## Implementation Strategy

### Reducer acceptance

`ReducerCommand::ResolveMerge` should continue to accept manual retry for non-terminal `MergeWait` and for repository-visible archived-but-not-merged states that require retry. It must continue to reject final `Merged` or `Rejected` states.

### Scheduler wake path

When TUI handles `TuiCommand::ResolveMerge` and the scheduler is already running, `notify_scheduler()` must be sufficient to make the scheduler re-check reducer-owned `ResolveWait` / `RejectWait` membership. The scheduler should not require ordinary queued changes to exist before evaluating base-lane waiters.

When the scheduler is stopped, `run_orchestrator_parallel(Vec::new(), ...)` must preserve the caller-provided shared reducer and execute the existing empty-queue lane-wait path.

### Retry dedupe

`last_dispatched_resolve_wait_changes` and base-dirty tracking are useful for preventing duplicate dispatch loops, but explicit user retry after manual deferral is a new intent boundary. The implementation should clear or invalidate stale dispatch dedupe when reducer membership is removed by manual deferral or when explicit retry is accepted.

### Diagnostics

Accepted retry intent that cannot reach `attempt_merge()` should emit enough evidence to distinguish:

- no preserved workspace found;
- stale workspace path;
- base still dirty;
- another base-mutating lane is active;
- reducer rejected stale/final state;
- scheduler did not observe shared reducer membership.

## Risks and Trade-offs

- Over-waking the scheduler can cause duplicate retry attempts. Existing reducer-owned membership and base-lane single-dispatch limits should remain the guardrail.
- Clearing too much executor-local dedupe can reintroduce repeated retries while a blocker persists. The reset should be tied to explicit retry intent or manual-deferral state transitions.
- TUI-local `is_resolving` must not become the authority for whether scheduler retry exists.

## Verification Approach

Use unit and integration-style Rust tests that construct reducer, TUI command handler, and parallel executor states directly. Avoid relying on live interactive TUI reproduction as the only acceptance path.
