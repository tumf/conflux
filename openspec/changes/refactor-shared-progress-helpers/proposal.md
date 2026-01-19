# Change: 進捗取得とAPIエラー応答の共通化

## Why
TUI と Web で change の進捗取得ロジックが重複しており、修正のたびに複数箇所を更新する必要があるため保守コストが高い。Web API の Not Found 応答も重複実装されているため、形式の不整合が起きるリスクがある。

## What Changes
- change の進捗取得を共通ヘルパーに集約し、worktree → archive → base のフォールバック順序を維持する。
- Web API の Not Found 応答生成を共通ヘルパーに集約し、既存のエラーメッセージ形式を維持する。
- 既存挙動は変更せず、既存テストと追加テストで同一性を確認する。

## Impact
- Affected specs: code-maintenance
- Affected code: src/task_parser.rs, src/tui/runner.rs, src/tui/state/events.rs, src/web/state.rs, src/web/api.rs, src/web/error.rs
