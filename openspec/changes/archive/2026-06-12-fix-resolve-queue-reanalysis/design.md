# Design: Resolve Queue Re-analysis

## Current Behavior

The order-based scheduler loop performs an outer capacity check before calling `perform_reanalysis_and_dispatch()`:

- queued work exists;
- available slots are calculated from max parallelism, in-flight tasks, active manual resolves, and active auto resolves;
- if available slots are zero, the scheduler emits a no-available-slots diagnostic and does not enter the re-analysis path.

This means active resolve work can suppress queue reconciliation and dependency analysis for unrelated queued changes.

## Desired Behavior

Re-analysis and dispatch capacity are separate concerns:

- Re-analysis eligibility answers: should queued work be classified, analyzed, and made ready for future dispatch?
- Dispatch capacity answers: can selected work start now without exceeding configured concurrency?

The scheduler should allow re-analysis while capacity is zero, then suppress only the final dispatch step when capacity remains zero.

## Implementation Approach

1. Move the authoritative capacity gate into the dispatch phase inside `perform_reanalysis_and_dispatch()`.
2. Let the outer scheduler loop call `perform_reanalysis_and_dispatch()` whenever local/reconciled queued work exists.
3. Within `perform_reanalysis_and_dispatch()`:
   - classify queued work;
   - preserve blocked-only short-circuiting;
   - run debounce logic with resolve-completion and slot-recovery bypasses;
   - run dependency analysis for dispatchable queued candidates;
   - update dependency tracker state from analysis results;
   - recalculate slots immediately before dispatch;
   - if slots are zero, emit a capacity-gated diagnostic and return without dispatching.
4. Preserve existing reducer/shared-state ownership rules for resolve wait, reject wait, terminal errors, and blocked-only drain.

## Invariants

- The scheduler must not exceed `max_concurrent_workspaces` once active resolve work is included in capacity accounting.
- Resolve and rejection review remain base-mutating lane operations and must not be bypassed by ordinary apply dispatch.
- Repository-visible evidence remains the source of workflow truth; no new durable workflow-control state is introduced.
- Diagnostics may be emitted for observability, but must not become workflow-control inputs.

## Risks and Mitigations

- Risk: running analysis at zero capacity could increase repeated analysis churn.
  - Mitigation: retain debounce behavior except for explicit slot recovery, resolve completion, and repair-candidate paths.
- Risk: changing the guard could accidentally dispatch while capacity is zero.
  - Mitigation: add regression tests that assert analysis callback execution without `ApplyStarted` while resolve consumes all slots.
- Risk: blocked-only idle behavior could regress.
  - Mitigation: keep blocked-only classification before expensive analysis and run existing blocked-only tests.
