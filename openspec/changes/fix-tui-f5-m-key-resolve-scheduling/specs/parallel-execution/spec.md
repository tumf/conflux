## MODIFIED Requirements

### Requirement: State-Driven Reanalysis Scheduling

The parallel scheduler SHALL treat `MergeWait` retry requests and queued change dispatch as scheduler-owned state transitions derived from observable reducer / scheduler state, rather than as direct TUI execution side effects.

A user pressing `M` for a `MergeWait` change MUST register retry intent that becomes visible to the scheduler, but MUST NOT by itself execute `resolve_deferred_merge(...)` or any equivalent merge / resolve operation outside the scheduler loop.

When execution slots remain available, queued changes and retry-eligible `MergeWait` changes MUST be evaluated within the same scheduler loop. A retry intent for one change MUST NOT suppress dependency re-analysis or dispatch of another queued change when the normal re-analysis conditions are satisfied.

Completion of a scheduler-owned merge / resolve retry MUST feed back into the same completion semantics used for ordinary scheduler progress, so that re-analysis and dispatch resume from scheduler state rather than from a TUI-only notify side effect.

When a user registers manual retry intent while other apply/archive/resolve work is in flight, the scheduler MUST preserve the reducer-owned `ResolveWait`, continue unrelated apply/archive progress, and retry the pending merge after the in-flight work releases scheduler/base-lane capacity. The pending change MUST NOT remain indefinitely in `ResolveWait` solely because unrelated apply/archive work was active at the time of the `M` keypress.

When the scheduler is running, no resolve/base-mutating operation is active, and one or more reducer-owned `ResolveWait` changes are retry-clean, the scheduler SHALL promote exactly one eligible pending retry to `resolving` during a scheduling evaluation. Other pending retries SHALL remain pending until the base-mutating lane clears again.

Canonical rule: `M` is **intent-only** (`ResolveWait` request in shared reducer state), scheduler loop is the **sole execution owner** for merge/resolve retry start, and reducer completion events (`ResolveCompleted`/`ResolveFailed`/`MergeDeferred`/`MergeCompleted`) are the **sole authority** for clearing or transitioning wait state.

<!-- Expected canonical result after archive: `parallel-execution` will require a running scheduler to promote one clean ResolveWait retry when the base-mutating lane is free. -->

#### Scenario: M key registers retry intent instead of direct execution

- **GIVEN** change `alpha` is in `MergeWait`
- **WHEN** the user presses `M`
- **THEN** the system records scheduler-visible retry intent for `alpha`
- **AND** the TUI command path does not directly execute `resolve_deferred_merge(...)`

#### Scenario: queued change still dispatches while another change is resolving

- **GIVEN** change `alpha` is already in `Resolving` and consumes one execution slot
- **AND** `max_parallelism` is greater than one so at least one slot remains available
- **AND** change `beta` is newly queued
- **AND** change `gamma` has scheduler-visible retry intent from `MergeWait`
- **WHEN** the scheduler evaluates re-analysis and dispatch from observable state
- **THEN** the scheduler may dispatch `beta` using the remaining available slot
- **AND** retry intent for `gamma` does not by itself suppress `beta` analysis or dispatch

#### Scenario: running scheduler promotes one clean ResolveWait retry

- **GIVEN** the scheduler is running
- **AND** no resolve/base-mutating operation is active
- **AND** changes `alpha` and `beta` are in reducer-owned `ResolveWait`
- **AND** retry preconditions for both are clean
- **WHEN** the scheduler evaluates pending base-mutating lane waiters
- **THEN** exactly one of `alpha` or `beta` SHALL start resolving
- **AND** the other SHALL remain `resolve pending`

#### Scenario: manual resolve intent progresses after unrelated apply completes

- **GIVEN** change `alpha` is in `MergeWait`
- **AND** change `beta` is applying or archiving in the same scheduler run
- **WHEN** the user presses `M` for `alpha`
- **THEN** the reducer records `alpha` in `ResolveWait`
- **AND** the scheduler continues `beta` apply/archive progress
- **WHEN** `beta` completes and the base-mutating lane is free
- **THEN** the scheduler retries the preserved merge for `alpha` without requiring another `M` keypress
- **AND** `alpha` does not remain indefinitely in `resolve pending` solely because `beta` was active when retry intent was registered
