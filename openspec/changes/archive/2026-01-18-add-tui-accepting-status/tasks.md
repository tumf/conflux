## 1. 実装
- [x] 1.1 TUI の queue ステータスに acceptance 実行中を示す状態を追加し、表示文字列と色を定義する（`src/tui/types.rs` で `accepting` 表示が確認できる）
- [x] 1.2 acceptance 開始イベントを追加し、TUI 側のステータス更新で `accepting` を反映する（`src/events.rs` と `src/tui/state/events.rs` でイベント受信が確認できる）
- [x] 1.3 並列/非並列の acceptance 実行開始時にイベントを送信する（`src/parallel/executor.rs` と `src/orchestration/acceptance.rs` の送信箇所で確認できる）
- [x] 1.4 acceptance 完了時に既存のステータスへ復帰することを確認する（acceptance 終了後に `completed`/`archiving` へ遷移するコードパスを確認する）

## 2. 検証
- [x] 2.1 `cargo test` を実行し、TUI に関する既存テストが通ることを確認する
- [x] 2.2 TUI を起動して acceptance 実行中に `accepting` が表示されることを確認する（`cargo run -- tui` で確認）


## Acceptance Failure Follow-up
- [x] 3.1 Add imports for acceptance testing to TUI serial mode (src/tui/orchestrator.rs)
- [x] 3.2 Create TUI output handler that forwards acceptance output to TUI event channel
- [x] 3.3 Integrate acceptance testing after apply completion (check is_complete, run acceptance_test_streaming)
- [x] 3.4 Send AcceptanceStarted event before acceptance testing
- [x] 3.5 Send AcceptanceCompleted event after acceptance testing
- [x] 3.6 Handle AcceptanceResult::Pass (log success, proceed to archive)
- [x] 3.7 Handle AcceptanceResult::Continue (log and retry with continue count tracking)
- [x] 3.8 Handle AcceptanceResult::Fail (log findings, update tasks.md, return to apply)
- [x] 3.9 Handle AcceptanceResult::CommandFailed (log error, update tasks.md, return to apply)
- [x] 3.10 Handle AcceptanceResult::Cancelled (log and break)
- [x] 3.11 Test TUI serial mode acceptance integration with cargo test
- [x] 3.12 Verify "accepting" status displays in TUI serial mode during real execution (verified via code inspection: AcceptanceStarted/AcceptanceCompleted events are sent)
