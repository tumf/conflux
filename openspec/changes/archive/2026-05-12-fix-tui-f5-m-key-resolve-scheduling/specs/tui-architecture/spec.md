## MODIFIED Requirements

### Requirement: Queue State Synchronization

The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for scheduler-owned resolve retry work, and Space queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for normal queue operations.

However, for `ResolveWait`/`MergeWait` rows, the following SHALL be satisfied:
- Space operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.
- @ operation SHALL be ignored and MUST NOT modify any state.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is scheduler-owned retry work and not normal queue work.

In parallel mode, once the user explicitly queues a `NotQueued` change for execution (for example via `F5` after marking it), refresh-derived state reconciliation MUST preserve the queued display state until one of the following occurs:
- execution for that change actually starts,
- the backend explicitly rejects startup for that change, or
- the user explicitly dequeues the change.

Auto-refresh, reducer display synchronization, and eligibility reconciliation MUST NOT regress such a queued row back to `not queued` before backend analysis/dispatch begins.

`F5` SHALL be treated as app-level orchestration control and MUST NOT perform cursor-local merge resolve actions. A cursor row in `MergeWait` MUST NOT cause `F5` to emit `ResolveMerge` or transition that row to `resolve pending`.

<!-- Expected canonical result after archive: `tui-architecture` will remove the historical rule that F5 resolves cursor-local MergeWait rows and will define F5 as cursor-independent orchestration control. -->

#### Scenario: F5 on MergeWait does not resolve cursor row

- **GIVEN** the TUI cursor is on change `alpha`
- **AND** `alpha` is in `MergeWait`
- **AND** change `beta` is marked runnable work in `NotQueued`
- **WHEN** the user presses `F5`
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** `alpha` SHALL NOT transition to `resolve pending` because of `F5`
- **AND** normal orchestration start/resume/retry MAY proceed for marked runnable work such as `beta`

#### Scenario: F5 is not blocked by unrelated resolving

- **GIVEN** change `alpha` is resolving
- **AND** change `beta` is marked runnable work in `NotQueued`
- **WHEN** the user presses `F5`
- **THEN** normal orchestration for `beta` SHALL be allowed to start/resume/retry
- **AND** resolve serialization SHALL remain limited to merge resolve operations

### Requirement: MergeDeferred の待ち状態判定

TUI and scheduler-visible state SHALL classify merge deferrals by first evaluating active resolve/base-mutating lane occupancy and only then evaluating workspace/base dirty state.

If a `MergeDeferred` or merge retry classification occurs while another resolve/base-mutating operation is active, and the deferred change is not the currently resolving change itself, the change SHALL be represented as `ResolveWait` / `resolve pending` and SHALL remain scheduler-owned retry work. Dirty workspace/base evidence observed during the active resolve/base-mutating operation MUST NOT by itself cause manual `MergeWait`.

If no resolve/base-mutating operation is active, dirty workspace/base evidence SHALL be classified as manual `MergeWait` with scheduler-owned `ResolveWait` membership cleared until explicit retry intent is accepted.

If no resolve/base-mutating operation is active and retry preconditions are clean, a scheduler-owned retry MAY be promoted to `Resolving` by the scheduler.

<!-- Expected canonical result after archive: `tui-architecture` will require active resolve/base-mutating occupancy to be evaluated before dirty state when deciding between `resolve pending` and `merge wait`. -->

#### Scenario: Active resolve takes precedence over dirty evidence

- **GIVEN** change `alpha` is currently resolving or otherwise owns the base-mutating lane
- **AND** base/workspace state appears dirty because of `alpha`
- **AND** change `beta` receives merge deferral or retry classification
- **WHEN** the TUI/scheduler classifies `beta`
- **THEN** active resolve/base-mutating occupancy SHALL be evaluated before dirty state
- **AND** `beta` SHALL be represented as `ResolveWait` / `resolve pending`
- **AND** `beta` SHALL NOT be represented as manual `MergeWait` solely because the base/workspace appears dirty

#### Scenario: Dirty without active resolve becomes manual MergeWait

- **GIVEN** no resolve/base-mutating operation is active
- **AND** base/workspace state is dirty or manually blocked
- **AND** change `beta` receives merge retry classification
- **WHEN** the TUI/scheduler classifies `beta`
- **THEN** `beta` SHALL be represented as manual `MergeWait`
- **AND** scheduler-owned `ResolveWait(beta)` SHALL be cleared until explicit retry intent is accepted

#### Scenario: Clean pending retry can start resolving

- **GIVEN** no resolve/base-mutating operation is active
- **AND** change `beta` is scheduler-owned `ResolveWait`
- **AND** retry preconditions for `beta` are clean
- **WHEN** the scheduler evaluates pending base-mutating lane waiters
- **THEN** `beta` MAY transition from `resolve pending` to `resolving`
