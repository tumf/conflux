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

Canonical rule: `M` is **intent-only** (`ResolveWait` request in shared reducer state), scheduler loop is the **sole execution owner** for merge/resolve retry start, and reducer completion events (`ResolveCompleted`/`ResolveFailed`/`MergeDeferred`/`MergeCompleted`) are the **sole authority** for clearing or transitioning wait state.

<!-- Expected canonical result after archive: `parallel-execution` will require zero-change manual resolve scheduler startup to consume shared reducer lane-wait work before completion and to demote stale retry evidence visibly. -->

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

#### Scenario: dirty base demotes ResolveWait to MergeWait when no lane occupant exists

- **GIVEN** change `alpha` is in reducer-owned `ResolveWait`
- **AND** no other change is actively `Resolving` or `Rejecting`
- **AND** the base repository has uncommitted changes or a merge-in-progress marker requiring manual cleanup
- **WHEN** the scheduler evaluates pending base-mutating lane waiters for `alpha`
- **THEN** the scheduler emits a concrete manual deferral for `alpha`
- **AND** the reducer display status for `alpha` becomes `merge wait`
- **AND** `alpha` is not returned by `resolve_wait_change_ids()`

#### Scenario: dirty-to-clean base transition retries ResolveWait without another keypress

- **GIVEN** change `alpha` is in reducer-owned `ResolveWait`
- **AND** a prior scheduler evaluation observed the base repository as dirty
- **AND** the operator cleans the base repository without pressing `M` again
- **WHEN** the scheduler next evaluates pending base-mutating lane waiters with a clean base and free lane
- **THEN** `alpha` is promoted to `resolving` or successfully merged through scheduler-owned retry execution
- **AND** `alpha` does not remain indefinitely in `resolve pending`

#### Scenario: zero-change manual retry startup consumes ResolveWait

- **GIVEN** change `alpha` is archive-complete and visible as `merge wait`
- **AND** cflx has been restarted so no scheduler is currently running
- **WHEN** the user presses `M` for `alpha`
- **THEN** the TUI records reducer-owned `ResolveWait(alpha)` and starts a scheduler with zero normal queued changes
- **AND** the scheduler synchronizes `ResolveWait(alpha)` from the same shared reducer state
- **AND** the scheduler evaluates the retry instead of completing as `0 changes processed`
- **AND** `alpha` transitions to `resolving`, `merged`, `merge wait` with visible reason, or an explicit error/stalled state

#### Scenario: zero-change startup cannot report success while ResolveWait remains

- **GIVEN** a scheduler run has zero normal queued changes
- **AND** shared reducer state still contains `ResolveWait(alpha)` or `RejectWait(alpha)`
- **WHEN** the run reaches an idle/completion decision
- **THEN** it MUST NOT emit successful completion or `AllCompleted`
- **AND** it MUST continue retry evaluation or surface visible reducer evidence that clears the pending membership

#### Scenario: stale retry evidence clears resolve pending visibly

- **GIVEN** change `alpha` is in reducer-owned `ResolveWait`
- **AND** the archived workspace path required for retry is missing or stale
- **WHEN** the scheduler evaluates pending base-mutating lane waiters for `alpha`
- **THEN** scheduler-owned `ResolveWait(alpha)` is cleared
- **AND** `alpha` becomes visible as `merge wait` or explicit error/stalled state with a reason
- **AND** `alpha` does not remain indefinitely in `resolve pending`
