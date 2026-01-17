# Change: apply の WIP スナップショットで --no-verify を許可する

## Why
apply の WIP スナップショットが pre-commit フックによって失敗すると、進捗スナップショットが残らずスタール検知や再開判断に支障が出るため、WIP 記録の信頼性を優先する。

## What Changes
- apply プロンプトから `--no-verify` の一律禁止を削除し、WIP スナップショット用途ではフックを回避できるようにする
- Git バックエンドの WIP コミット作成は `git commit --no-verify --allow-empty` 相当で実行する
- apply 通常コミットや merge/resolve の挙動は変更せず、影響を WIP スナップショットに限定する

## Impact
- Affected specs: agent-prompts, parallel-execution, cli
- Affected code: `src/agent.rs`, `src/vcs/git/mod.rs`
