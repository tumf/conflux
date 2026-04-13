## MODIFIED Requirements

### Requirement: Queue State Synchronization
The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for resolve completion, and Space queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for queue operations.

However, for `ResolveWait`/`MergeWait` rows, the following SHALL be satisfied:
- Space operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.
- @ operation SHALL be ignored and MUST NOT modify any state.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is not a target for queue operations.

In parallel mode, once the user explicitly queues a `NotQueued` change for execution (for example via `F5` after marking it), refresh-derived state reconciliation MUST preserve the queued display state until one of the following occurs:
- execution for that change actually starts,
- the backend explicitly rejects startup for that change, or
- the user explicitly dequeues the change.

Auto-refresh, reducer display synchronization, and eligibility reconciliation MUST NOT regress such a queued row back to `not queued` before backend analysis/dispatch begins.

For active rows (`applying`, `accepting`, `archiving`, `resolving`), `Space` MUST remain reserved for queue/selection semantics and MUST NOT trigger force kill. A dedicated `K` key action MUST enter a confirmation mode for the current active change, and only an explicit `y` confirmation from that mode MAY request a force kill of the in-flight execution for that change. The TUI MUST keep the active display state until kill completion is confirmed. Only after successful kill completion MAY the row transition to `not queued` and clear `selected`. If force kill fails, the TUI MUST keep the row in its current execution state and surface the failure.

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

#### Scenario: Cannot change queue state during MergeWait
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait`
- **WHEN** the user presses Space or `@`
- **THEN** the change status SHALL remain `MergeWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space operation toggles only the execution mark

#### Scenario: Queued row is preserved before analysis starts
- **GIVEN** the TUI is in parallel mode
- **AND** a change is marked for execution from `NotQueued`
- **AND** the user presses `F5`
- **WHEN** the initial refresh-driven reducer display synchronization runs before backend analysis starts
- **THEN** the change status SHALL remain `Queued`
- **AND** the row SHALL NOT return to `not queued`

#### Scenario: Startup rejection can clear queued row before execution
- **GIVEN** the TUI is in parallel mode
- **AND** a change was explicitly queued by the user
- **WHEN** backend startup rejects that change before execution begins
- **THEN** the change status MAY return to `NotQueued`
- **AND** the rejection reason SHALL be logged

#### Scenario: Active change enters confirmation on K
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with status `applying`, `accepting`, `archiving`, or `resolving`
- **WHEN** the user presses `K`
- **THEN** the TUI SHALL enter a confirmation mode for that change
- **AND** the backend SHALL NOT issue a force-kill request yet
- **AND** the UI SHALL show confirmation hints for `y` and cancel

#### Scenario: Active change force-kills only after Y confirmation
- **GIVEN** the TUI is showing force-kill confirmation for an active change
- **WHEN** the user presses `y`
- **THEN** the backend SHALL issue a force kill for the in-flight execution of that change
- **AND** the row SHALL remain active until kill completion is confirmed
- **AND** after successful kill completion the row SHALL become `not queued`
- **AND** `selected` SHALL be cleared

#### Scenario: Active change kill confirmation can be canceled
- **GIVEN** the TUI is showing force-kill confirmation for an active change
- **WHEN** the user presses `n` or `Esc`
- **THEN** the confirmation mode SHALL close
- **AND** the backend SHALL NOT issue a force-kill request
- **AND** the row SHALL remain in its current active status

#### Scenario: Active change ignores Space for force-kill
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on an active change
- **WHEN** the user presses `Space`
- **THEN** the TUI SHALL NOT issue a force-kill request
- **AND** the row SHALL remain in its current active status unless another allowed queue/selection rule applies

#### Scenario: Active change stop failure preserves active state
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on an active change
- **WHEN** the force-kill request fails
- **THEN** the row SHALL remain in its current active status
- **AND** the UI SHALL surface a stop failure message
