# Design: マージ完了後のGit worktreeクリーンアップ

## Goals
- マージ成功した変更に対応するGit worktreeを自動で削除する
- 再開（resume）や長時間運用でworktreeが蓄積しないようにする

## Non-Goals
- マージコンフリクト発生時にworktreeを削除する（コンフリクト解消のため保持が必要）
- apply失敗時にworktreeを削除する（デバッグ/再試行のため保持が必要）

## Decision
- 変更ごとのマージが成功した直後（MergeCompleted相当）に、その変更のworktreeをクリーンアップ対象とする
- worktreeの削除はbest-effortとし、失敗しても並列実行全体の失敗にはしない（warnログのみ）

## Worktree特定方法
- 可能な経路では、worktree作成時に「change_id → worktreeパス/ブランチ名」を保持し、それを用いて削除する
- パスが直接得られない/失われるケース（再開やプロセス再生成など）がある場合は、`git worktree list --porcelain` の結果から
  - `ws-<change_id>-<suffix>` 形式のブランチ名をキーに一致するworktreeを特定
  - 対象worktreeのパスを取得して `git worktree remove <path> --force` を実行
  - その後 `git branch -D <branch>` を実行

## Observability
- worktree削除の開始/成功/失敗をログに残す（特に失敗時のstderrを記録）
