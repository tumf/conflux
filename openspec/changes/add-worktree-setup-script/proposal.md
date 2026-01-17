# Change: Worktree作成時のセットアップスクリプト自動実行

## Why
worktree作成後にpre-commitなどのセットアップを自動化したいが、プロジェクトごとに手順が異なるため、任意のスクリプトを実行できる仕組みが必要になる。

## What Changes
- `.wt/setup` が存在する場合、worktree作成時に実行するルールを追加する
- 実行時に `ROOT_WORKTREE_PATH` としてベースリポジトリのパスを渡す
- `.wt/setup` がない場合は何もしない（従来どおり）

## Impact
- Affected specs: vcs-worktree-operations
- Affected code: src/vcs/git/mod.rs, src/vcs/git/commands.rs
