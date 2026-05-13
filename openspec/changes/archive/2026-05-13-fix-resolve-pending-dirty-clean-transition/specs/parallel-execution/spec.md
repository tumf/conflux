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

Canonical rule: `M` is **intent-only** (`ResolveWait` request in shared reducer state), scheduler loop is the **sole execution owner** for merge/resolve retry start, and reducer completion events (`ResolveCompleted`/`ResolveFailed`/`MergeDeferred`/`MergeCompleted`) are the **sole authority** for clearing or transitioning wait state.

<!-- Expected canonical result after archive: `parallel-execution` will require dirty-base `ResolveWait` retries to demote to `MergeWait`, and dirty-to-clean base transitions to trigger scheduler-owned retry promotion without another keypress. -->

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

### Requirement: post-archive-merge-dispatch

If `on_merged` fails because the root repository is not safe for repo-mutating hook execution, such as root `.git/index.lock` contention, Conflux SHALL treat that as a hook failure that blocks merged transition when `continue_on_failure=false`.

A deferred merge caused by another active non-terminal change in `Resolving` or `Rejecting` SHALL advance into reducer-owned auto-resumable merge/resolve handling (`ResolveWait` or immediate resolving when promoted). Active `Rejecting` is included because rejection review can touch and dirty base state.

A deferred merge caused by active `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, dirty base without an active base-mutating lane occupant, or other manual intervention requirement SHALL NOT be classified as automatic `ResolveWait` solely because that state exists. Dirty base and manual intervention deferrals SHALL remain in manual merge wait handling (`MergeWait`).

The implementation MUST NOT infer auto-resumable versus manual-wait behavior by parsing a human-readable deferred reason string.

A change already in reducer-owned `ResolveWait` MUST follow the same classification rules when its retry is evaluated: active `Resolving` or `Rejecting` by another change remains auto-resumable, while dirty base without an active base-mutating lane occupant demotes to manual `MergeWait`.

<!-- Expected canonical result after archive: `parallel-execution` will explicitly require retry-time `ResolveWait` classification to match post-archive merge deferral classification. -->

#### Scenario: active resolving deferred archive promotes to resolve wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because another change is actively `Resolving`
**When**: the deferred merge result is processed
**Then**: the archived change enters auto-resumable deferred handling (`ResolveWait` or equivalent queued resolve intent)
**And**: this decision does not depend on parsing a free-form reason string

#### Scenario: active rejecting deferred archive promotes to resolve wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because another change is actively `Rejecting`
**When**: the deferred merge result is processed
**Then**: the archived change enters auto-resumable deferred handling (`ResolveWait` or equivalent queued resolve intent)
**And**: rejection review completion or failure triggers retry of deferred merge work

#### Scenario: dirty-base deferred archive stays merge wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because the base branch is dirty while no other change is actively `Resolving` or `Rejecting`
**When**: the deferred merge result is processed
**Then**: the change remains in manual merge wait handling (`MergeWait`)
**And**: it is not classified as auto-resumable

#### Scenario: dirty-base ResolveWait retry demotes to merge wait

**Given**: change `alpha` is already in reducer-owned `ResolveWait`
**And**: no other change is actively `Resolving` or `Rejecting`
**And**: merge retry is deferred because the base branch is dirty
**When**: the deferred retry result is processed
**Then**: `alpha` transitions to manual merge wait handling (`MergeWait`)
**And**: `alpha` is no longer treated as auto-resumable retry work

#### Scenario: root index lock contention blocks merged transition

**Given**: change `alpha` is repository-visible merged
**And**: `hooks.on_merged` runs a repo-mutating command such as `make bump-patch`
**And**: root `.git/index.lock` contention causes the hook to exit non-zero
**When**: the scheduler handles hook completion
**Then**: `alpha` does not transition to terminal `Merged`
**And**: `MergeCompleted` is not emitted for `alpha`
**And**: the operator-visible failure context includes the hook failure details
