# Change: worktree削除時にブランチも削除

## Why
Worktreesビューからworktreeを削除した際に、対応するブランチが残り続けるため、ローカルブランチの整理が手動になってしまう。

## What Changes
- Worktreesビューの削除操作で、worktreeに紐づくローカルブランチも削除する
- ブランチが存在しない場合や削除に失敗した場合は、ログに警告を出して処理自体は継続する

## Impact
- Affected specs: vcs-worktree-operations
- Affected code: src/tui/runner.rs, src/vcs/git/mod.rs
