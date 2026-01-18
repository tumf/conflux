# Change: Worktree衝突チェックを作業ツリー非変更にする

## Why
5秒ごとのWorktree一覧更新で実行される衝突チェックが、`git merge --no-commit` と `git merge --abort` を使って作業ツリーを書き換えるため、worktree上で実行中のエージェント作業を妨げる。定期更新の衝突チェックは作業ツリーを変更しない方式に切り替え、エージェントの進行やマージを阻害しない状態にする。

## What Changes
- Worktree衝突チェックを作業ツリーを変更しない方式へ切り替える
- 5秒ごとの更新頻度やworktree一覧の再取得タイミングは維持する
- 衝突検出の結果は従来どおり表示に反映する

## Impact
- Affected specs: tui-worktree-view
- Affected code: TUI worktree refresh loop, git conflict check helper
