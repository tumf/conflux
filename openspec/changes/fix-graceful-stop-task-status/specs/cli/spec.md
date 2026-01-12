# Spec Delta: CLI - Graceful Stop状態管理の明確化

この変更では、Graceful stop完了時の変更状態管理を明確化します。

## MODIFIED Requirements

### Requirement: Escape Key Stop Behavior

TUI SHALL allow users to stop ongoing processing using the Escape key.

#### Scenario: Graceful stop completes naturally

- **WHEN** TUI is in Stopping mode
- **AND** the current agent process completes successfully
- **THEN** the TUI transitions to Stopped mode
- **AND** the completed change transitions to appropriate status (completed/archived)
- **AND** log displays "Stopped - processing halted"

#### Scenario: Graceful stop with incomplete processing (NEW)

- **WHEN** TUI is in Stopping mode
- **AND** the orchestrator stops without completing the current change
- **OR** the current change is still in Processing/Archiving state when stop completes
- **THEN** the TUI transitions to Stopped mode
- **AND** any changes in Processing or Archiving status SHALL transition to Queued status
- **AND** elapsed time for interrupted changes SHALL be recorded
- **AND** log displays "Processing stopped"
- **AND** interrupted changes can be resumed with F5 key

#### Scenario: Second Esc press forces immediate stop

- **WHEN** TUI is in Stopping mode
- **AND** user presses Escape key again
- **THEN** the current agent process is terminated immediately (SIGTERM)
- **AND** the TUI transitions to Stopped mode
- **AND** log displays "Force stopped - process terminated"
- **AND** the interrupted change status becomes "queued" (not error)
- **AND** elapsed time for interrupted changes SHALL be recorded

**Note**: Graceful stopとForce stopは、中断された変更の状態遷移において同じ動作をします（両方ともQueuedに戻す）。

---

## 実装上の注意

### 状態遷移の一貫性

以下の状況で、Processing/Archiving状態の変更をQueuedに戻す処理が必要：

1. **Force stop時**: `src/tui/runner.rs`のキーハンドリング
2. **Graceful stop完了時**: `src/tui/state/events.rs`の`OrchestratorEvent::Stopped`ハンドリング

### 対象となる状態

以下の状態の変更をQueuedに遷移させる：
- `QueueStatus::Processing`
- `QueueStatus::Archiving`

以下の状態は変更しない：
- `QueueStatus::Completed` - 正常完了
- `QueueStatus::Archived` - アーカイブ完了
- `QueueStatus::Error(_)` - エラー状態
- `QueueStatus::Queued` - すでにキュー内
- `QueueStatus::NotQueued` - キュー外

### 経過時間の記録

中断時に`started_at`から`elapsed_time`を計算して記録することで：
- ユーザーが処理時間を把握できる
- 再開時の参考情報になる
