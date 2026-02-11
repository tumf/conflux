## 1. Implementation
- [x] 1.1 hook実行開始時にコマンド文字列をLogEntryで発行する
  - 検証: TUIのLogs Viewに「Running <hook> hook: <command>」が表示されることを確認（`src/tui/state.rs` のログ追加経路）
- [x] 1.2 hookのstdout/stderrを取得し、Logs Viewへ出力する（サイズ制限あり）
  - 検証: `hooks`モジュールのテストで`echo`出力がLogEntryに反映されることを確認
- [x] 1.3 serial/parallelの双方でhookログが同一経路で表示されるようイベント送信を統一する
  - 検証: `ExecutionEvent`/`ParallelEvent` から `OrchestratorEvent::Log` が発行されることをコード上で確認
- [x] 1.4 仕様に合わせたメッセージ形式とログレベルを適用する
  - 検証: `info`相当でLogs Viewに表示され、`--logs`指定時にファイルにも出力されることを確認

## 2. Tests
- [x] 2.1 hook出力がLogs Viewに反映されることを確認する単体テストを追加する
  - 検証: `cargo test`で対象テストがパスする
