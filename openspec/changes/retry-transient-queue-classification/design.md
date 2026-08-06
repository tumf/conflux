## Context

`classify_queued_work` currently derives reducer facts twice:

1. `DependencyContext::from_executor` performs a synchronous `try_read` for terminal, resolving, wait, and queue-eligibility facts.
2. `classify_queued_work` performs another `try_read` for reducer queue IDs and wait/hold sets.

A concurrent reducer write can make either read fail or let the two reads observe different revisions. The fail-closed result is correct only as a momentary dispatch gate. It is not a stable scheduler state.

Persistent idle deliberately removes the ordinary timer to avoid repository scan and analyzer spam. Once temporary unavailability is misclassified as blocked-only, nothing guarantees another pass.

## Goals

- Classify each pass from one coherent reducer view.
- Suspend safely through short reducer writes and continue automatically after release.
- Prevent analysis and dispatch on incomplete evidence.
- Preserve event-driven persistent idle for stable states.
- Keep all workflow control workspace-local plus process-lifetime reducer state, as required by the constitution.

## Non-Goals

- Polling the reducer, repository, or worktrees while stably idle.
- Adding a durable snapshot cache.
- Broad scheduler-loop refactoring.
- Changing dependency or blocker semantics.

## Decision: Await and Copy One Snapshot

Use the native Tokio `RwLock` read future instead of translating `try_read` contention into a queue classification. At the start of reducer-dependent classification:

1. await one shared reducer read guard;
2. copy the minimal immutable facts needed by queue and dependency classification;
3. drop the guard explicitly;
4. construct `DependencyContext` from the copied facts;
5. continue repository checks and dependency analysis without any reducer guard.

This is suspension, not thread blocking. It requires no timer, no notification channel, and no new state machine. Once the current writer releases, Tokio wakes the waiting classification future directly.

The captured view should include a reducer revision if available for diagnostics and tests, but the revision must remain process-local and must not become a completion or resume input.

## Why Not a Persistent-Idle Timer

Adding a timer branch to `wait_for_persistent_idle_wake` would fix the observed stall but undo the explicit no-polling design. It would repeatedly run reconciliation and classification for genuinely blocked work and risks reintroducing analyzer churn.

If future lock ownership makes awaited acquisition unsafe, the fallback design is an explicit typed `SnapshotUnavailable` flow decision with a one-shot scheduler-local retry. It must not reuse `candidate_unavailable` or stable blocked-only handling. That extra state is unnecessary while a single awaited snapshot is safe.

## Lock Ordering and Cancellation

No caller may hold the reducer write guard while awaiting `classify_queued_work`. The implementation must avoid acquiring other async locks before the reducer read when those paths can acquire locks in reverse order.

The scheduler's cancellation token must be selectable while awaiting the snapshot, or the classification future must be dropped promptly when the enclosing scheduler is cancelled. Tests must pin this behavior rather than assuming it.

## Verification Strategy

Use deterministic Tokio coordination rather than timing races:

- acquire the reducer write guard in the test;
- start a scheduler/classification future and assert analyzer and dispatch counters remain zero;
- release the guard;
- require completion under a short timeout without queue notification;
- repeat with a real external or Acceptance hold and assert stable held classification;
- use an analyzer fake that attempts a reducer write to prove the read guard was released before analyzer execution;
- retain existing persistent-idle tests proving no worktree scan occurs without a wake.

At least one test must traverse `perform_reanalysis_and_dispatch` or the scheduler loop. A direct `classify_queued_work` test alone cannot prove that persistent idle is avoided.

## Risks and Mitigations

- **Read waits behind a long writer:** reducer writes are in-memory event reductions and must remain short; cancellation coverage prevents shutdown from hanging.
- **Deadlock through lock inversion:** acquire one reducer read before other classification locks, copy facts, and release it before every repository or analyzer await.
- **Stale snapshot during later dispatch:** existing dispatch admission gates remain authoritative and recheck current state before workspace preparation.
- **Analyzer polling returns:** no persistent-idle timer is added; unchanged-input suppression and existing idle tests remain intact.
- **Mixed reducer revisions:** one copied snapshot replaces the two independent `try_read` operations.
