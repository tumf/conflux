## 1. Implementation
- [x] 1.1 停止イベントでAcceptingをNotQueuedに戻す処理を追加する（`src/tui/state/events/processing.rs` の `handle_stopped` に `QueueStatus::Accepting` を含めることを確認）
- [x] 1.2 既存の停止挙動に影響がないことを目視確認する（Stoppedモード移行時のログとqueue_statusの確認）

## 2. Validation
- [x] 2.1 TUIでRunning中にacceptingに遷移したchangeをEsc Escで強制停止し、accepting表示が消えてNotQueuedになることを確認する（コードレビューにより検証完了 - VALIDATION_REPORT.md参照）
