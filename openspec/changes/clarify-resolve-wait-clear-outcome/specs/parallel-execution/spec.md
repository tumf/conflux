## MODIFIED Requirements

### Requirement: State-Driven Reanalysis Scheduling

The parallel scheduler SHALL treat `MergeWait` retry requests and queued change dispatch as scheduler-owned state transitions derived from observable reducer / scheduler state, rather than as direct TUI execution side effects.

A user pressing `M` for a `MergeWait` change MUST register retry intent that becomes visible to the scheduler, but MUST NOT by itself execute `resolve_deferred_merge(...)` or any equivalent merge / resolve operation outside the scheduler loop.

When execution slots remain available, queued changes and retry-eligible `MergeWait` changes MUST be evaluated within the same scheduler loop. A retry intent for one change MUST NOT suppress dependency re-analysis or dispatch of another queued change when the normal re-analysis conditions are satisfied.

Completion of a scheduler-owned merge / resolve retry MUST feed back into the same completion semantics used for ordinary scheduler progress, so that re-analysis and dispatch resume from scheduler state rather than from a TUI-only notify side effect.

When a user registers manual retry intent while other apply/archive/resolve work is in flight, the scheduler MUST preserve the reducer-owned `ResolveWait`, continue unrelated apply/archive progress, and retry the pending merge after the in-flight work releases scheduler/base-lane capacity. The pending change MUST NOT remain indefinitely in `ResolveWait` solely because unrelated apply/archive work was active at the time of the `M` keypress.

When the scheduler is running, no resolve/base-mutating operation is active, and one or more reducer-owned `ResolveWait` changes are retry-clean, the scheduler SHALL promote exactly one eligible pending retry to `resolving` during a scheduling evaluation. Other pending retries SHALL remain pending until the base-mutating lane clears again.

When a reducer-owned `ResolveWait` retry is evaluated while the base repository is dirty and no other non-terminal change is actively `Resolving` or `Rejecting`, the scheduler SHALL classify the retry as manual-intervention merge wait and feed a concrete manual deferral back into the reducer. The change MUST transition from `resolve pending` to `merge wait`, and it MUST be removed from reducer-owned resolve-wait queues.

When a reducer-owned `ResolveWait` retry is evaluated after the base repository becomes clean and the base-mutating lane is free, the scheduler SHALL retry or promote the pending merge without requiring another `M` keypress. The change MUST NOT remain indefinitely in `resolve pending` solely because the previous evaluation observed a dirty base repository.

When a scheduler is started with zero normal queued changes solely to consume reducer-owned manual merge retry intent, it MUST treat existing `ResolveWait` / `RejectWait` membership as scheduler work. It MUST synchronize that membership from the caller-owned shared reducer state before idle or completion decisions, evaluate at least one eligible lane-wait retry, and MUST NOT complete as a zero-change success while shared lane-wait membership remains pending or active.

If retry evaluation observes stale, missing, or manually blocked retry prerequisites, the scheduler MUST feed visible reducer evidence that clears scheduler-owned pending membership and transitions the change to `merge wait` or an explicit error/stalled state with a reason. It MUST NOT leave a change indefinitely visible as `resolve pending` when no scheduler-consumable retry work remains.

Internal helper names or comments used by this retry-clearing path SHOULD describe stale, missing, already-merged, and success outcomes neutrally. They MUST NOT make stale or missing workspace cleanup look like successful merge completion.

Canonical rule: `M` is **intent-only** (`ResolveWait` request in shared reducer state), scheduler loop is the **sole execution owner** for merge/resolve retry start, and reducer completion events (`ResolveCompleted`/`ResolveFailed`/`MergeDeferred`/`MergeCompleted`) are the **sole authority** for clearing or transitioning wait state.

<!-- Expected canonical result after archive: `parallel-execution` will clarify that retry intent clearing helpers should describe outcome semantics rather than success-only semantics. -->

#### Scenario: M key registers retry intent instead of direct execution

- **GIVEN** change `alpha` is in `MergeWait`
- **WHEN** the user presses `M`
- **THEN** the system records scheduler-visible retry intent for `alpha`
- **AND** the TUI command path does not directly execute `resolve_deferred_merge(...)`

#### Scenario: stale retry evidence clears resolve pending visibly

- **GIVEN** change `alpha` is in reducer-owned `ResolveWait`
- **AND** the archived workspace path required for retry is missing or stale
- **WHEN** the scheduler evaluates pending base-mutating lane waiters for `alpha`
- **THEN** scheduler-owned `ResolveWait(alpha)` is cleared
- **AND** `alpha` becomes visible as `merge wait` or explicit error/stalled state with a reason
- **AND** `alpha` does not remain indefinitely in `resolve pending`

#### Scenario: retry-clearing helper wording is outcome-neutral

- **GIVEN** the scheduler clears `ResolveWait` for an already-merged, missing-workspace, stale-workspace, or successful-merge outcome
- **WHEN** a maintainer reads the helper name or comments in the retry-clearing path
- **THEN** the wording indicates a terminal/no-longer-retryable outcome
- **AND** it does not describe stale or missing workspace cleanup as success
