## MODIFIED Requirements

### Requirement: TUI Stopped Mode
TUIはStoppedモードを提供し、実行中のみqueuedを保持する方針で変更状態を管理しなければならない（SHALL）。Stopped遷移時はqueued状態をNotQueuedへ戻し、実行マーク（[x]）は保持する。Stoppedモード中のSpace操作は実行マークの付与/解除のみを行い、queue_statusはNotQueuedのまま維持する。F5再開時は実行マークの付いたchangeをqueuedに復元して処理を再開する。Stopped中のタスク進捗更新はqueued化を行わない。

#### Scenario: Stopped mode display
- **WHEN** TUI is in Stopped mode
- **THEN** header status displays "Stopped" in gray color
- **AND** the change list remains visible with current statuses
- **AND** execution-marked changes show "[x]" while their queue_status remains not queued

#### Scenario: Queue management in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on an execution-marked change
- **THEN** the execution mark is removed and queue_status remains not queued

#### Scenario: Queue addition in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-marked change
- **THEN** the execution mark is added and queue_status remains not queued

#### Scenario: Task completion in Stopped mode does not auto-queue
- **WHEN** TUI is in Stopped mode
- **AND** a change's tasks are updated (e.g., all tasks marked complete)
- **THEN** the change queue_status SHALL remain not queued
- **AND** the change SHALL NOT be automatically added to the queue

#### Scenario: Resume processing from Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** one or more changes are execution-marked
- **AND** user presses F5
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes after converting execution-marked changes to queued
- **AND** log displays "Resuming processing..."

#### Scenario: Resume with empty queue shows warning
- **WHEN** TUI is in Stopped mode
- **AND** no changes are execution-marked
- **AND** user presses F5
- **THEN** a warning message is displayed
- **AND** the TUI remains in Stopped mode

### Requirement: Interrupted Change Handling
停止によって中断されたchangeは、queuedを実行中のみ保持する方針に従って扱われなければならない（SHALL）。強制停止時はqueue_statusをNotQueuedへ戻し、実行マークは保持する。再開時は実行マークの付いたchangeが再度queuedになり、再処理できる。

#### Scenario: Force-stopped change returns to not queued
- **WHEN** a change is being processed
- **AND** user force stops with second Esc press
- **THEN** the change status becomes not queued (not error)
- **AND** the execution mark remains set
- **AND** the change can be re-processed on resume

#### Scenario: Partial progress preserved
- **WHEN** a change had some tasks completed before force stop
- **THEN** the completed tasks remain completed
- **AND** the tasks.md file reflects actual progress
- **AND** resuming continues from the partial state
