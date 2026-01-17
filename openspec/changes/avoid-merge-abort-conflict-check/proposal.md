# Change: Worktree衝突チェックでmerge --abortを使わない

## Why
TUIのWorktreeビューで衝突チェックが定期実行される際、git merge --abortがtrackedなtasks.mdを復元し、アーカイブ処理を妨げる。作業ツリーを汚さない方法に切り替え、TUI起動中でもアーカイブが失敗しない状態を確保する必要がある。

## What Changes
- Worktree衝突チェックを作業ツリーに影響しないGit手法へ切り替える
- 変更はTUIの衝突チェック経路に限定し、Worktree一覧更新のタイミングは維持する

## Impact
- Affected specs: tui-worktree-view
- Affected code: src/vcs/git/commands.rs, src/tui/runner.rs
