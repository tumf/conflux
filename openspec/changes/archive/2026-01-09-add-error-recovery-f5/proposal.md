# Change: エラー発生時のOrchestratorステータス表示とF5リトライ機能

## Why

現在、opencode実行がエラー（LLMエラー、料金不足など）で失敗した場合、Changeのステータスは正しく `[error]` と表示されますが、Orchestratorのステータスパネルは `Waiting...` のまま止まってしまいます。これによりユーザーは処理が継続中なのかエラーで停止したのか判断できず、リカバリー操作も行えません。

## What Changes

- エラー発生時、Orchestratorのステータスパネルに「Error」状態を明示的に表示
- 新しい `AppMode::Error` を追加し、エラー状態を管理
- エラー状態でF5キーを押すとエラー状態のChangeをリトライ可能に
- リトライ時、エラー状態のChangeをキューに再追加して処理を再開

## Impact

- Affected specs: `cli/spec.md` (実行モードダッシュボード関連)
- Affected code:
  - `src/tui.rs` - AppMode、render_status、F5キーハンドリング
  - `src/tui.rs` - OrchestratorEvent処理、run_orchestrator関数
