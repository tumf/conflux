# Change: エージェント子プロセスの stderr 出力を info レベルで表示する

**Change Type**: implementation

## Why

opencode などの AI エージェント CLI は、通常の動作ログ（思考過程、ツール実行結果、進捗など）を stderr に書き出し、stdout には最終結果のみを出力する。現在の `LogOutputHandler` は `on_stderr()` を一律 `warn!()` にマッピングしているため、エージェントの正常な出力がすべて WARN として表示され、ログが警告で埋まり視認性が著しく低下する。

## What Changes

- `OutputHandler` trait に `on_agent_stderr()` メソッドを追加し、エージェント子プロセスの stderr を `info` レベルで出力する
- 既存の `on_stderr()` は Conflux 内部の警告用途として `warn` レベルを維持する
- apply / archive / acceptance / resolve のストリーミング出力で、エージェント子プロセスの stderr を `on_agent_stderr()` 経由で送出する

## Impact

- Affected specs: observability
- Affected code: `src/orchestration/output.rs`, `src/orchestration/apply.rs`, `src/orchestration/archive.rs`, `src/orchestration/acceptance.rs`, `src/serial_run_service.rs`, `src/parallel/executor.rs`
