# tui-worktree-merge Specification

## Purpose
TBD - created by archiving change add-worktree-view-with-merge. Update Purpose after archive.
## Requirements
### Requirement: Merge Error Handling

マージ失敗時 SHALL provide clear error messages and recovery guidance.

#### Scenario: コンフリクト発生時の自動abort

- **GIVEN** マージ中にコンフリクトが発生した
- **WHEN** `git merge` がコンフリクトを報告する
- **THEN** `git merge --abort` が自動実行される
- **AND** "Merge conflict detected. Merge aborted. Manual resolution required." エラーが表示される
- **AND** コンフリクトファイル一覧が含まれる

#### Scenario: エラーポップアップの表示

- **GIVEN** マージが失敗した
- **WHEN** エラーイベントが受信される
- **THEN** エラー詳細がポップアップで表示される
- **AND** ポップアップは任意のキーで閉じられる

#### Scenario: マージ失敗後の状態

- **GIVEN** マージが失敗した
- **WHEN** エラー処理が完了する
- **THEN** base repositoryの状態は変更されていない
- **AND** worktreeは削除されていない
- **AND** ユーザーは再試行または手動解決を選択できる

