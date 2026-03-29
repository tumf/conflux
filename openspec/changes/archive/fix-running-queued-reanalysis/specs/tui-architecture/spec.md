## MODIFIED Requirements

### Requirement: Event-Driven State Updates

The TUI MUST evaluate `MergeWait` in the 5-second auto-refresh and return it to `Queued` if any of the following conditions are met:

- The corresponding worktree does not exist
- The corresponding worktree exists and the worktree branch is not ahead of base

For auto-released changes that are no longer `MergeWait`, merge resolve operation hints and execution via `M` MUST NOT be performed.

Furthermore, changes that are serialized and in a waiting state for resolve SHALL be retained as `ResolveWait` and MUST NOT be returned to `NotQueued` by auto-refresh.

The TUI SHALL maintain a FIFO resolve wait queue for manual resolve operations triggered while another resolve is in progress.

When the user presses `M` on a `MergeWait` change while resolve is in progress, the change SHALL transition to `ResolveWait` and be enqueued (deduplicated).

When `ResolveCompleted` is received and the resolve wait queue is not empty, the TUI SHALL dequeue the next change and start its resolve immediately.

When `ResolveFailed` is received, the TUI SHALL NOT auto-start the next resolve; queued changes remain in `ResolveWait` until user action resumes.

Manual resolve completion MUST update the shared reducer-owned display state before the next refresh-derived status synchronization so that successful merge completion is not overwritten by stale `ResolveWait`.

When manual resolve completion releases a parallel execution slot while additional changes remain explicitly queued, the TUI/event pipeline MUST trigger scheduler re-evaluation so the queued rows do not remain apparently idle in `Running` mode (MUST).

#### Scenario: Release MergeWait when worktree does not exist
- **GIVEN** a change is in `MergeWait`
- **AND** the corresponding worktree does not exist
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the change status returns to `Queued`

#### Scenario: Release MergeWait for worktree with no commits ahead
- **GIVEN** a change is in `MergeWait`
- **AND** the corresponding worktree exists
- **AND** the worktree branch is not ahead of base
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the change status returns to `Queued`

#### Scenario: Cannot use M for changes released from MergeWait
- **GIVEN** a change has returned from `MergeWait` to `Queued`
- **WHEN** the TUI key hints are rendered
- **THEN** the merge resolve hint via `M` is not displayed

#### Scenario: ResolveWait is retained during auto-refresh
- **GIVEN** a change is in `ResolveWait`
- **AND** resolve is in progress for another change
- **WHEN** the 5-second polling auto-refresh is executed
- **THEN** the change status remains `ResolveWait`

#### Scenario: Changes with WorkspaceState::Archived are identified as ResolveWait
- **GIVEN** a worktree exists and `detect_workspace_state` returns `WorkspaceState::Archived`
- **AND** the change is not merged (ahead of base)
- **WHEN** the TUI auto-refresh is executed
- **THEN** the change status is displayed as `ResolveWait`
- **AND** queue operations via Space/@ keys are not accepted

#### Scenario: resolve 実行中の `M` は待ち行列へ追加される
- **GIVEN** a resolve operation is in progress
- **AND** the user presses `M` on a change in `MergeWait`
- **WHEN** the TUI processes the key event
- **THEN** the change status SHALL transition to `ResolveWait`
- **AND** the change_id SHALL be enqueued for resolve

#### Scenario: ResolveCompleted は次の待ち行列を開始する
- **GIVEN** the resolve wait queue has at least one change_id
- **AND** a resolve operation completes
- **WHEN** `ResolveCompleted` is processed
- **THEN** the next change_id SHALL be dequeued and its resolve started

#### Scenario: ResolveFailed は自動開始しない
- **GIVEN** the resolve wait queue has at least one change_id
- **AND** a resolve operation fails
- **WHEN** `ResolveFailed` is processed
- **THEN** the next resolve SHALL NOT start automatically

#### Scenario: Successful manual resolve is not regressed by refresh
- **GIVEN** a change was previously shown as `ResolveWait`
- **AND** manual resolve completes successfully and the change becomes merged
- **WHEN** the next TUI auto-refresh synchronizes reducer-derived display status
- **THEN** the row remains `Merged` or is removed according to retention rules
- **AND** the row does not return to `resolve pending`

#### Scenario: Running mode queued rows resume after manual resolve frees a slot
- **GIVEN** the TUI is in `Running` mode
- **AND** one manual resolve is active and consuming a parallel slot
- **AND** additional changes are explicitly queued for execution
- **WHEN** the manual resolve completes successfully
- **THEN** the scheduler is triggered to re-evaluate queued changes
- **AND** queued rows do not remain indefinitely idle waiting for an unrelated completion event
