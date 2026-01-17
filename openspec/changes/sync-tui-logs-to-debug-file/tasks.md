# Tasks: sync-tui-logs-to-debug-file

## タスク一覧

- [x] 1. `LogLevel` enumを`src/events.rs`に追加
  - `Info`, `Success`, `Warn`, `Error`の4レベル
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`を適用

- [x] 2. `LogEntry`に`level`フィールドを追加
  - `pub level: LogLevel`
  - 既存の`color`フィールドは維持（TUI表示用）

- [x] 3. `LogEntry`のコンストラクタを更新
  - `info()` → `LogLevel::Info`
  - `success()` → `LogLevel::Success`
  - `warn()` → `LogLevel::Warn`
  - `error()` → `LogLevel::Error`

- [x] 4. `src/tui/state/logs.rs`に`tracing`のインポートを追加

- [x] 5. `add_log()`メソッドでレベルに応じた`tracing`出力を追加
  - `Info`/`Success` → `tracing::info!(target: "tui_log", ...)`
  - `Warn` → `tracing::warn!(target: "tui_log", ...)`
  - `Error` → `tracing::error!(target: "tui_log", ...)`

- [x] 6. ユニットテストを追加
  - `LogLevel`の各レベルが正しく設定されることを確認
  - `LogEntry::info()`, `error()`等のコンストラクタテスト

- [x] 7. `cargo fmt && cargo clippy && cargo test`で検証
