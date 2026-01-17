# Change: Run worktree setup on TUI-created worktrees

## Why
TUIの「+」操作で作成したworktreeで`.wt/setup`が実行されず、セットアップ処理（pre-commit hook生成など）が抜けるため、期待した作業環境が整わない。

## What Changes
- TUIのworktree作成フローで`.wt/setup`を実行する。
- `ROOT_WORKTREE_PATH` を設定してセットアップを実行する。
- セットアップ失敗時は作成処理を失敗として扱う。

## Impact
- Affected specs: `vcs-worktree-operations`
- Affected code: `src/tui/runner.rs`, `src/vcs/git/commands.rs`
