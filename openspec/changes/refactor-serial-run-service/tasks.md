## 1. Implementation
- [x] 1.1 SerialRunService の骨格と共有インターフェースを追加する（`src/serial_run_service.rs` に CLI/TUI で共通化する run 関数を用意し、ParallelRunService と同様のイベント注入手段を持つことを確認）
- [x] 1.2 run 側の serial 実行を SerialRunService 経由に置き換える（`src/orchestrator.rs` の serial ループがサービス経由で呼ばれることを確認）
- [x] 1.3 TUI 側の serial 実行を SerialRunService 経由に置き換える（`src/tui/orchestrator.rs` がサービス経由でイベント送信を行うことを確認）
- [x] 1.4 共通フローで共有される apply/archive/acceptance の呼び出しを整理する（`src/orchestration/` の共有関数が両モードから使用されることを確認）
- [x] 1.5 出力差分（CLI ログ vs TUI チャンネル）を OutputHandler で吸収し、既存ログ形式を維持する（`LogOutputHandler` と `ChannelOutputHandler` の使用箇所が明確であることを確認）
- [x] 1.6 テストと検証を実施する（`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` を実行し、動作差分がないことを確認）
