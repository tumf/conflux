## ADDED Requirements

### Requirement: Reducer-Driven Change Status Consumption

TUI SHALL derive the displayed change status from the shared orchestration reducer state. TUI SHALL NOT own an independent lifecycle copy for queue, block, merge-wait, resolve-wait, or execution-stage transitions.

UI-local fields such as cursor position, selection mark, scroll position, and popup visibility MAY remain local to TUI state.

#### Scenario: TUI renders reducer-derived status

- **GIVEN** the shared orchestration state reports a change with queue intent queued and wait state merge-wait
- **WHEN** the TUI renders the change row
- **THEN** the displayed status is `merge wait`
- **AND** the TUI does not need to mutate a local `queue_status` copy to show it

#### Scenario: TUI keeps local UI state only

- **GIVEN** the user moves the cursor or toggles a selection mark
- **WHEN** the TUI updates local interaction state
- **THEN** cursor and selection state may change locally
- **AND** lifecycle status remains sourced from the shared reducer state

## MODIFIED Requirements

### Requirement: Event-Driven State Updates

The system MUST reconcile `MergeWait` and related waiting states during the 5-second auto-refresh by feeding workspace observations into the shared orchestration reducer.

The refresh path MUST NOT directly overwrite TUI-local queue status.

The system MUST release `MergeWait` back to `Queued` when either of the following conditions is true:

- The corresponding worktree does not exist
- The corresponding worktree exists and the worktree branch is not ahead of base

For auto-released changes that are no longer `MergeWait`, merge resolve operation hints and execution via `M` MUST NOT be performed.

Changes in `ResolveWait` SHALL remain `ResolveWait` during auto-refresh and MUST NOT be synthesized from workspace observation alone.

The system SHALL maintain a FIFO resolve wait queue for manual resolve operations triggered while another resolve is in progress.

When the user presses `M` on a `MergeWait` change while resolve is in progress, the reducer SHALL transition the change to `ResolveWait` and enqueue it (deduplicated).

When `ResolveCompleted` is received and the resolve wait queue is not empty, the reducer SHALL dequeue the next change and start its resolve immediately.

When `ResolveFailed` is received, the reducer SHALL NOT auto-start the next resolve; queued changes remain in `ResolveWait` until user action resumes.

Workspace observation of `WorkspaceState::Archived` MAY recover `MergeWait` when the worktree is still ahead of base, but MUST NOT reconstruct `ResolveWait`.

#### Scenario: Release MergeWait when worktree does not exist
- **GIVEN** a change is in `MergeWait`
- **AND** the corresponding worktree does not exist
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the shared reducer releases the change to queued intent
- **AND** the displayed status becomes `queued`

#### Scenario: Release MergeWait for worktree with no commits ahead
- **GIVEN** a change is in `MergeWait`
- **AND** the corresponding worktree exists
- **AND** the worktree branch is not ahead of base
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the shared reducer releases the change to queued intent
- **AND** the displayed status becomes `queued`

#### Scenario: Cannot use M for changes released from MergeWait
- **GIVEN** a change has returned from `MergeWait` to `Queued`
- **WHEN** the TUI key hints are rendered
- **THEN** the merge resolve hint via `M` is not displayed

#### Scenario: ResolveWait is retained during auto-refresh
- **GIVEN** a change is in `ResolveWait`
- **AND** resolve is in progress for another change
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the change status remains `ResolveWait`

#### Scenario: WorkspaceState Archived recovers MergeWait, not ResolveWait
- **GIVEN** a worktree exists and `detect_workspace_state` returns `WorkspaceState::Archived`
- **AND** the change is not merged because the worktree is still ahead of base
- **WHEN** the auto-refresh observation is reconciled
- **THEN** the displayed status is `merge wait`
- **AND** queue operations via Space or `@` are not accepted as direct queue mutations

#### Scenario: resolve 実行中の `M` は待ち行列へ追加される
- **GIVEN** a resolve operation is in progress
- **AND** the user presses `M` on a change in `MergeWait`
- **WHEN** the reducer processes the command
- **THEN** the change status transitions to `ResolveWait`
- **AND** the change_id is enqueued for resolve

#### Scenario: ResolveCompleted は次の待ち行列を開始する
- **GIVEN** the resolve wait queue has at least one change_id
- **AND** a resolve operation completes
- **WHEN** `ResolveCompleted` is processed
- **THEN** the next change_id is dequeued and its resolve starts

#### Scenario: ResolveFailed は自動開始しない
- **GIVEN** the resolve wait queue has at least one change_id
- **AND** a resolve operation fails
- **WHEN** `ResolveFailed` is processed
- **THEN** the next resolve does not start automatically
