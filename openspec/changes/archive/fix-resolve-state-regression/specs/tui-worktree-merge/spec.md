## MODIFIED Requirements

### Requirement: Merge Error Handling

マージ失敗時 SHALL provide clear error messages and recovery guidance.

Merged状態のchangeに対するResolveFailed/MergeWaitイベントは無視し、状態退行を防止しなければならない。

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

#### Scenario: ResolveFailed到着時にMerged状態が維持される

- **GIVEN** changeのqueue_statusがMergedである
- **WHEN** ResolveFailed イベントが到着する
- **THEN** queue_status SHALL remain Merged
- **AND** MergeWait への退行は発生しない
- **AND** 警告ポップアップは表示されない
- **AND** ログにはガード適用の旨が記録される

#### Scenario: auto-refreshでMerged状態がMergeWaitに退行しない

- **GIVEN** changeのqueue_statusがMergedである
- **AND** auto-refreshのmerge_wait_idsにそのchange_idが含まれる
- **WHEN** apply_merge_wait_statusが実行される
- **THEN** queue_status SHALL remain Merged
- **AND** MergeWait への遷移は発生しない
