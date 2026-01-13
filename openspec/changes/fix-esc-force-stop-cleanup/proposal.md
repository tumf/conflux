# 変更提案: Esc二度押しで子プロセスを確実に終了

## Why
TUIでEsc二回による強制停止後も子プロセスが残留し、並列実行や再開の妨げになるため、強制停止時の後始末を明確化します。

## What Changes
- 強制停止時に現在のエージェントプロセスと子プロセスを確実に終了する
- 強制停止時のログと状態遷移の期待値を明確化する

## Impact
- Affected specs: `cli`
- Affected code: `src/tui/runner.rs`, `src/agent.rs`, `src/process_manager.rs`, `src/parallel/executor.rs`
