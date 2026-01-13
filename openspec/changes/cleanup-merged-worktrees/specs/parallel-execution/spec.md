## MODIFIED Requirements
### Requirement: Git Worktree Workspace Management
Git バックエンド使用時、システムは `git worktree` を使用してワークスペースを管理しなければならない（SHALL）。

さらに、マージ成功した変更に対応するworktreeは、マージ完了後にクリーンアップしなければならない（SHALL）。クリーンアップは `git worktree remove <path>` と関連ブランチの削除（例: `git branch -D <branch>`）を含む。

クリーンアップに失敗した場合、システムは警告ログを出力し、処理全体は継続しなければならない（SHALL）。

#### Scenario: マージ成功後にworktreeが削除される
- **GIVEN** Git backend による並列実行を行っている
- **AND** 変更 `change_id` に対応するworktreeとブランチが作成されている
- **WHEN** `change_id` のマージが成功し、マージ完了として扱われる
- **THEN** システムは `git worktree remove <path>` を実行する
- **AND** システムは関連ブランチを削除する（例: `git branch -D <branch>`）
- **AND** worktree削除の失敗は警告として記録され、並列実行は継続する
