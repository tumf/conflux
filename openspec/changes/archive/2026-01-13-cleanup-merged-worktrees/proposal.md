# 変更提案: マージ完了後のGit worktreeを削除してクリーンアップ

## Why
並列実行（Git worktree）で変更を処理したあと、マージが完了したworktreeが残り続けると、ディスク使用量の増加や `git worktree list` のノイズになり、再開（resume）時の判定や運用（手動の `git worktree prune` など）にも負担が出ます。
このため「マージが終わったworktreeは自動で削除する」という期待値を仕様として明確化します。

## What Changes
- マージ成功した変更に対応するGit worktree（ディレクトリ）を削除する
- 関連する一時ブランチも削除する
- worktreeパスが直接分からない場合でも、`git worktree list --porcelain` から同一changeのworktreeを特定して削除できるようにする（best-effort）
- 削除に失敗した場合は警告ログを残し、処理全体は継続する

## Impact
- Affected specs: `parallel-execution`
- Affected code: `src/vcs/git/mod.rs`, `src/parallel/cleanup.rs`, `src/parallel/executor.rs`（想定）
