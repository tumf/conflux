# Change: AGENTS.md のプロジェクト構造を最新化

## Why

AGENTS.md の「Project Structure」セクションが古く、現在のモジュール構成と一致していない。AI エージェントがコードベースを正確に理解するためには、最新のファイル構造が必要。

## What Changes

- Project Structure セクションを現在の src/ ディレクトリ構造に更新
- 存在しないモジュール（`opencode.rs`, `state.rs`）を削除
- 新しいモジュール（`analyzer.rs`, `config.rs`, `hooks.rs`, `vcs_backend.rs`, `jj_commands.rs`, `git_commands.rs`, `history.rs`, `task_parser.rs`, `jj_workspace.rs`, `git_workspace.rs`, `parallel_run_service.rs`, `agent.rs`, `approval.rs`, `parallel_executor.rs`, `templates.rs`, `tui/`）を追加
- Key Dependencies 表に `async-trait` を追加
- 各モジュールの説明を更新

## Impact

- Affected specs: documentation
- Affected code: AGENTS.md のみ（コード変更なし）
