## MODIFIED Requirements

### Requirement: Queue State Synchronization

The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for resolve completion, and Space queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for queue operations.

For `ResolveWait`/`MergeWait` rows, both Space operation and bulk `x` operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.

During Running mode, bulk `x` operation SHALL apply only to non-active rows. Active rows (`Applying`, `Accepting`, `Archiving`, `Resolving`) MUST NOT have their queue_status changed and MUST NOT trigger stop requests through bulk toggle.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is not a target for queue operations.

In parallel mode, once the user explicitly queues a `NotQueued` change for execution (for example via `F5` after marking it), refresh-derived state reconciliation MUST preserve the queued display state until one of the following occurs:
- execution for that change actually starts,
- the backend explicitly rejects startup for that change, or
- the user explicitly dequeues the change.

Auto-refresh, reducer display synchronization, and eligibility reconciliation MUST NOT regress such a queued row back to `not queued` before backend analysis/dispatch begins.

#### Scenario: Remove from queue with Space key
- **WHEN** the user dequeues a [x] change with the Space key in Running mode
- **THEN** the status changes to `QueueStatus::NotQueued` and is removed from DynamicQueue

#### Scenario: Log removal operations
- **WHEN** a change is removed from DynamicQueue
- **THEN** the removal operation is logged

#### Scenario: Cannot change queue state during ResolveWait
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `ResolveWait`
- **WHEN** the user presses Space or `@`
- **THEN** the change status SHALL remain `ResolveWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space operation toggles only the execution mark

#### Scenario: Bulk toggle in Running mode excludes active changes
- **GIVEN** the TUI is in running mode
- **AND** one change is `resolving`
- **AND** another change is `not queued`
- **WHEN** the user presses `x`
- **THEN** the `resolving` change status SHALL remain unchanged
- **AND** no stop command SHALL be emitted for that `resolving` change
- **AND** the `not queued` change execution mark SHALL be toggled

#### Scenario: Bulk toggle preserves ResolveWait queue semantics
- **GIVEN** the TUI is in running mode
- **AND** a change is in `ResolveWait`
- **WHEN** the user presses `x`
- **THEN** `queue_status` SHALL remain `ResolveWait`
- **AND** DynamicQueue SHALL NOT be modified for that change
- **AND** only `selected` SHALL be toggled
