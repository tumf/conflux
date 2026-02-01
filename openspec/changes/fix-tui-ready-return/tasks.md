## 1. Implementation
- [ ] 1.1 正常完了時に `OrchestratorEvent::Stopped` を送らないようにし、停止要求時のみ `Stopped` を送る（確認: `src/tui/command_handlers.rs` で完了後の `Stopped` 送信が削除されている）
- [ ] 1.2 Ready復帰の回帰確認を追加する（確認: `src/tui/state/events/processing.rs` で `handle_all_completed` が Select へ遷移する前提をテスト、`cargo test` で該当テストが通る）
