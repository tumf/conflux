# Change: TUI 状態管理のガードロジック分割

## Why
`AppState` の `request_merge_worktree_branch` や `toggle_selection` が長く、条件分岐が複雑です。ガード判定とアクション構築を分離して見通しを改善します。

## What Changes
- ガード判定や状態遷移の構築をサブモジュール／ヘルパーに抽出する
- 既存の UI 挙動と制約条件は維持する

## Impact
- Affected specs: `code-maintenance`
- Affected code: `src/tui/state/mod.rs`（必要に応じて `src/tui/state/` 配下）
