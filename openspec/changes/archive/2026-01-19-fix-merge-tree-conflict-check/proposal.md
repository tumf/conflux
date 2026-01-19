# Change: git merge-tree 競合チェック失敗の修正

## Why
TUI Worktree View の競合チェックが `git merge-tree` の使用方法エラーで失敗し、衝突判定が常に失敗として扱われるため。

## What Changes
- `git merge-tree --write-tree` の実行引数を正しい形式に修正する。
- 競合検出を stderr 依存から stdout を優先する形に整理し、競合時の挙動を明確にする。
- 競合判定失敗時のログに診断情報（stdout/stderr/exit code）を含める。

## Impact
- Affected specs: `openspec/specs/tui-worktree-view/spec.md`
- Affected code: `src/vcs/git/commands.rs`, `src/tui/runner.rs`
