## Context

One scheduler pass currently consults reducer-owned work through several independent non-blocking reads:

1. `admit_dynamic_queue_hint` pops and refuses a dynamic hint when `try_read` fails.
2. `sync_resolve_wait_from_shared_state_nonblocking` leaves scheduler-local lane waits unchanged when `try_read` fails.
3. `reconcile_queued_candidates_from_shared_state` returns an empty outcome when `try_read` fails.
4. `DependencyContext::from_executor` and `classify_queued_work` each perform another `try_read` for eligibility, wait, and hold facts.

A concurrent reducer write can make any read fail or let one pass combine different reducer revisions. Each operation fails closed locally, but an empty scheduler-local queue can then satisfy `work_drained` before classification runs. Persistent lifetime enters an event-driven wait with no timer; finite lifetime can terminate successfully or as blocked/stalled. Temporary unreadability is therefore not a stable scheduler state and must not be converted into one.

## Goals

- Detect all reducer-owned scheduler work in one evaluation from one coherent reducer view.
- Preserve consumed queue hints and suspend safely through short reducer writes, then continue automatically after release.
- Prevent analysis and dispatch on incomplete evidence.
- Preserve event-driven persistent idle for stable states.
- Keep all workflow control workspace-local plus process-lifetime reducer state, as required by the constitution.

## Non-Goals

- Polling the reducer, repository, or worktrees while stably idle.
- Adding a durable snapshot cache.
- Broad scheduler-loop refactoring.
- Changing dependency or blocker semantics.

## Decision: Await and Copy One Snapshot

Use the native Tokio `RwLock` read future instead of translating `try_read` contention into queue state. Near the start of each reducer-dependent scheduler evaluation:

1. drain dynamic notifications without permanently rejecting a hint whose authorization cannot yet be read;
2. await one shared reducer read guard before reconciliation or any drain/idle decision;
3. copy the minimal immutable facts needed for queue admission, reducer-intent reconciliation, lane waits, drain/idle detection, and queue/dependency classification;
4. drop the guard explicitly;
5. reconcile the scheduler-local queue and construct `DependencyContext` from the copied facts;
6. continue repository checks and dependency analysis without any reducer guard.

This is suspension, not thread blocking. It requires no timer, no extra notification channel, and no new durable state machine. Once the current writer releases, Tokio wakes the waiting scheduler evaluation directly. If ingestion cannot share the snapshot without broad restructuring, it may retain unreadable hints for the same pass, but it must not pop-and-discard the only wake edge.

The captured view should include a reducer revision if available for diagnostics and tests, but the revision must remain process-local and must not become a completion or resume input.

## Why Not a Persistent-Idle Timer

Adding a timer branch to `wait_for_persistent_idle_wake` would fix the observed stall but undo the explicit no-polling design. It would repeatedly run reconciliation and classification for genuinely blocked work and risks reintroducing analyzer churn.

If lock ordering makes one early awaited snapshot unsafe, the fallback design is an explicit typed `SnapshotUnavailable` flow decision with a one-shot scheduler-local retry retained before both finite termination and persistent idle. It must preserve consumed hints and must not reuse empty reconciliation, `candidate_unavailable`, drained, or stable blocked-only handling. That extra state is unnecessary while a single awaited snapshot is safe.

## Lock Ordering and Cancellation

No caller may hold the reducer write guard while awaiting the scheduler work snapshot or `classify_queued_work`. The implementation must avoid acquiring dynamic-queue, last-change, lane, or other async locks before the reducer read when those paths can acquire locks in reverse order. Existing tests that hold a write guard and invoke the read synchronously or in the same task must move the read/scheduler call to a separately spawned task before awaiting release.

The scheduler's cancellation token must be selectable while awaiting the snapshot, or the scheduler future must be dropped promptly when cancelled. Tests must pin this behavior rather than assuming it.

## Verification Strategy

Use deterministic Tokio coordination rather than timing races:

- start with an empty scheduler-local queue and one reducer-visible queued intent;
- acquire the reducer write guard in the test;
- start the scheduler in a separate task and assert reconciliation, analyzer, and dispatch counters remain zero;
- release the guard;
- require reconciliation and dispatch under a short timeout without another queue notification;
- run the same contention against finite lifetime and prove it does not exit as drained or blocked/stalled before release;
- repeat with a real external or Acceptance hold and assert stable held classification;
- use an analyzer fake that attempts a reducer write to prove the read guard was released before analyzer execution;
- replace existing same-task lock-unavailable tests with coordinated contention tests that cannot self-deadlock;
- retain existing persistent-idle tests proving no worktree scan occurs without a wake.

At least one test must traverse the full scheduler loop from dynamic admission/reconciliation through termination/idle checks. A direct `classify_queued_work` or `perform_reanalysis_and_dispatch` test alone cannot prove that false drain and persistent idle are avoided.

## Risks and Mitigations

- **Read waits behind a long writer:** reducer writes are in-memory event reductions and must remain short; cancellation coverage prevents shutdown from hanging.
- **Deadlock through lock inversion:** acquire one reducer read before other classification locks, copy facts, and release it before every repository or analyzer await.
- **Stale snapshot during later dispatch:** existing dispatch admission gates remain authoritative and recheck current state before workspace preparation; a later reducer mutation supplies its own queue edge.
- **Consumed hint is lost before snapshot acquisition:** unreadable admission retains the hint or defers its disposition within the same evaluation; it never converts it into a final refusal.
- **Analyzer polling returns:** no persistent-idle timer is added; unchanged-input suppression and existing idle tests remain intact.
- **Mixed reducer revisions:** one copied snapshot replaces the two independent `try_read` operations.
