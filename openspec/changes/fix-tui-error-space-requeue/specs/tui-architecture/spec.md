## MODIFIED Requirements

### Requirement: Queue State Synchronization

The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for resolve completion, and Space queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for queue operations.

However, for `ResolveWait`/`MergeWait` rows, the following SHALL be satisfied:
- Space operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.
- @ operation SHALL be ignored and MUST NOT modify any state.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is not a target for queue operations.

When a change enters `Error` state in the TUI, the execution mark (`selected`) SHALL be cleared.

When a user marks an `Error` change again, that mark SHALL mean the change is intended for re-execution. In Running mode, the TUI SHALL convert that intent into re-queue behavior. In Stopped mode, the mark SHALL be preserved until resume and then restored to queued execution intent.

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

#### Scenario: Cannot change queue state during MergeWait
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait`
- **WHEN** the user presses Space or `@`
- **THEN** the change status SHALL remain `MergeWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space operation toggles only the execution mark

#### Scenario: Error clears mark in TUI
- **GIVEN** a change is currently execution-marked in the TUI
- **WHEN** that change transitions to `Error`
- **THEN** `selected` SHALL become `false`

#### Scenario: Re-marking an Error change restores requeue intent in Running mode
- **GIVEN** the TUI is in Running mode
- **AND** the cursor is on a change in `Error` state with `selected = false`
- **WHEN** the user marks the change again
- **THEN** the system treats it as requeue intent
- **AND** the change is returned to queued execution flow

#### Scenario: Re-marking an Error change is preserved until resume in Stopped mode
- **GIVEN** the TUI is in Stopped mode
- **AND** a change is in `Error` state with `selected = false`
- **WHEN** the user marks the change again
- **THEN** `selected` SHALL become `true`
- **AND** the change remains pending requeue until resume

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
