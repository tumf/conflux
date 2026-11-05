## MODIFIED Requirements

### Requirement: Merge Error Handling

マージ失敗時 SHALL provide clear error messages and recovery guidance. When a merge conflicts, the TUI merge path SHALL run `git merge --abort` through the shared typed aborting merge, SHALL verify restoration from repository evidence (`HEAD` equals the pre-merge commit, no `MERGE_HEAD`, clean base working directory), and SHALL report the verified outcome: the message SHALL state that the merge was aborted and the base restored only when restoration was confirmed, and SHALL state that the abort failed or restoration is unconfirmed, with manual recovery required, otherwise. The TUI SHALL NOT emit "Merge aborted" when restoration was not confirmed.

#### Scenario: コンフリクト発生時の自動abort

- **GIVEN** マージ中にコンフリクトが発生した
- **WHEN** `git merge` がコンフリクトを報告する
- **THEN** `git merge --abort` が自動実行され、その終了状態と復元結果が検証される
- **AND** 復元が確認された場合、"Merge conflict detected. Merge aborted; base restored to <commit>." エラーが表示される
- **AND** コンフリクトファイル一覧が含まれる

#### Scenario: abort失敗時は復元を主張しない

- **GIVEN** マージ中にコンフリクトが発生した
- **WHEN** `git merge --abort` が非ゼロで終了する、または `HEAD`、`MERGE_HEAD`、clean状態の検証に失敗する
- **THEN** "Merge aborted" ではなく、abort失敗または復元未確認と手動回復の必要性を示すエラーが表示される
- **AND** base repositoryは追加の補償操作を受けない

#### Scenario: エラーポップアップの表示

- **GIVEN** マージが失敗した
- **WHEN** エラーイベントが受信される
- **THEN** エラー詳細がポップアップで表示される
- **AND** ポップアップは任意のキーで閉じられる

#### Scenario: マージ失敗後の状態

- **GIVEN** マージが失敗し、復元が確認された
- **WHEN** エラー処理が完了する
- **THEN** base repositoryの状態は変更されていない
- **AND** worktreeは削除されていない
- **AND** ユーザーは再試行または手動解決を選択できる
