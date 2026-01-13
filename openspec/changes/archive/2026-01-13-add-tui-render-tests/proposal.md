# Change: TUI描画テストの追加

## Why
TUIの描画ロジックは実端末依存だと回帰を検知しづらいため、TestBackendで安定して検証できるテスト方針が必要です。

## What Changes
- `ratatui::backend::TestBackend` を使った描画テストの導入指針を明確化する
- TUI描画シナリオをバッファ検査またはスナップショットで検証するテストを追加する

## Impact
- Affected specs: testing
- Affected code: `src/tui/render.rs`, `src/tui/runner.rs`, `tests/`（新規テスト）
