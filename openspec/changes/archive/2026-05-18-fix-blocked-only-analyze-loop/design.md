# Design: Blocked-Only Parallel Scheduler Drain

## Current Failure Mode

The scheduler loop treats any non-empty local `queued` list as reason to consider re-analysis. When all remaining queued entries are unable to dispatch without explicit user action, repeated analysis cannot make progress. If `analyze_command` itself fails, the run can appear stuck in `running` while repeatedly emitting analyzer failures.

## Design Principles

- Keep workflow control workspace-local and reducer-derived; do not add durable external scheduler state.
- Classify work before calling expensive or failure-prone analysis.
- Preserve existing manual retry semantics for `MergeWait`.
- Make finite and persistent scheduler lifetimes differ only in their final wait/exit behavior.

## Scheduler Work Classes

The scheduler should derive a per-loop summary from local queued candidates, shared reducer state, in-flight set, lane waiters, pending merge tasks, and repository-visible workspace evidence.

Suggested classes:

- `DispatchableApply`: ordinary queued work that can be passed to dependency analysis and then selected for dispatch.
- `ManualMergeWait`: reducer-visible `MergeWait`; no ordinary apply dispatch until explicit `ResolveMerge`.
- `SchedulerLaneWait`: reducer-visible `ResolveWait` / `RejectWait`; consumed by scheduler retry paths, not ordinary analysis.
- `TerminalErrorRetryRequired`: recoverable terminal error that requires explicit retry before ordinary dispatch.
- `DependencyBlocked`: queued work blocked by unresolved dependencies, including terminal-error dependencies.
- `CandidateUnavailable`: reducer queue intent or workspace evidence exists, but no loadable ordinary candidate can be constructed.

The exact type shape can be minimal and private to `queue_state.rs`; it only needs enough detail to decide whether analysis is useful and to emit stable diagnostics.

## Finite Lifetime Behavior

Finite execution exits when all active scheduler-owned work is drained and the only remaining work is blocked/manual wait work. This does not mark those changes accepted, archived, or merged. It only stops the currently running scheduler loop because no automatic progress is possible.

## Persistent Lifetime Behavior

Persistent execution remains alive but must not poll. It should enter the same notification-driven idle wait used for fully drained persistent schedulers. Dynamic queue additions, explicit retry commands, resolve/reject retry notifications, merge results, or cancellation wake it.

## Analyzer Failure Dedupe

Analyzer failure dedupe is runtime-only observability state. A stable signature can include queued IDs, in-flight IDs, and normalized error class/message. It must not decide workflow routing and must be safe to lose on process restart.

## Risk and Mitigation

- Risk: treating a truly dispatchable candidate as blocked could delay work.
  - Mitigation: tests should prove ordinary queued changes still call analysis and dispatch when capacity exists.
- Risk: manual `MergeWait` becomes unretryable.
  - Mitigation: explicit `ResolveMerge` tests must cover promotion from `MergeWait` to `ResolveWait` after blocked-only drain.
- Risk: persistent scheduler misses wakeups.
  - Mitigation: reuse existing dynamic queue / scheduler retry notifications rather than introducing new channels.
