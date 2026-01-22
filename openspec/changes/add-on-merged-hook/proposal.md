# Change: OnMerged フックの追加

## Why
現在の hooks には merge 完了後に実行されるフックが存在せず、ユーザーが「マージ完了」を契機に外部処理を実行できない。PostArchive は merge 前であり代替にならないため、明確に merge 後のタイミングを提供する必要がある。

## What Changes
- `on_merged` フックを追加し、change が base branch にマージされた直後に実行する。
- parallel 自動マージと TUI Worktree の手動マージの双方で on_merged を発火させる。
- フック種別一覧、実行順序、プレースホルダー表、テンプレート例を更新する。

## Impact
- Affected specs: hooks
- Affected code: `src/hooks.rs`, `src/parallel/mod.rs`, `src/tui/runner.rs`, `src/serial_run_service.rs`, `src/templates.rs`
