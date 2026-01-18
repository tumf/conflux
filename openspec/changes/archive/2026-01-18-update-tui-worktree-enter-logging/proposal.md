# Change: TUI Worktrees Enterの無反応理由をログで可視化

## Why
TUIのWorktreesビューでEnterキーが無反応に見える事象があり、原因が設定不足なのか状態条件なのか判別できません。現状はログが出ず調査に手戻りが発生するため、無視される理由をTUIログに残して可観測性を高めます。

## What Changes
- WorktreesビューでEnterが無視される条件ごとに警告ログを追加する
- 設定未指定や選択無しなどの理由を明示してユーザーに示す

## Impact
- Affected specs: tui-worktree-view, observability
- Affected code: src/tui/runner.rs
