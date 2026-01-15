# Tasks

## 実装タスク

- [x] `src/tui/runner.rs` の `run_tui` 関数に `web_state` 引数を追加
  - シグネチャを `web_state: Option<Arc<WebState>>` に変更
  - `run_tui_loop` への呼び出しに `web_state` を渡す

- [x] `src/tui/runner.rs` の `run_tui_loop` 関数に `web_state` 引数を追加
  - シグネチャに `web_state: Option<Arc<WebState>>` を追加
  - `run_orchestrator_parallel` への呼び出しに `web_state` を渡す

- [x] `src/tui/orchestrator.rs` の `run_orchestrator_parallel` 関数に `web_state` 引数を追加
  - シグネチャに `web_state: Option<Arc<WebState>>` を追加
  - `orchestrator.rs:828-842` と同様のイベント転送ループを実装
  - `ParallelEvent` を受信して `web_state.apply_execution_event()` を呼び出す
  - `AllCompleted` / `Stopped` イベントでループを終了

- [x] `src/main.rs` の TUI モード起動箇所を修正
  - Line 102-134: `web_state` を `run_tui()` に渡す
  - `web_url` だけでなく `web_state` も渡すように変更

- [x] イベント転送タスクの停止処理を実装
  - CLIモード (`orchestrator.rs`) と同じ終了条件を使用
  - チャネルのdropとタスクのawaitを適切に行う

## テストタスク

- [x] TUI + Web監視 + 並列実行モードでの動作確認
  - `cargo build --features web-monitoring`
  - `tui --web --parallel` で起動
  - ブラウザでWebUIにアクセス
  - 並列実行開始後、WebUIでステータスがリアルタイム更新されることを確認

- [x] CLIモードでの既存動作が維持されていることを確認
  - `run --web --parallel` で起動
  - WebUIでリアルタイム更新が正常に動作することを確認

- [x] WebSocket接続状態の確認
  - ブラウザの開発者ツールでWebSocket接続を確認
  - `state_update` メッセージが受信されることを確認

## ドキュメントタスク

- [x] CHANGELOG更新（該当する場合）
  - TUIモードでのWeb監視修正を記載
  - **Note**: No CHANGELOG update needed as this is a bug fix that restores expected behavior
