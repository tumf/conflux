## MODIFIED Requirements

### Requirement: on_merged hook
オーケストレーターはchangeがbase branchにマージされた直後、mergedステータスへ遷移する直前に`on_merged`フックを実行しなければならない（SHALL）。

`on_merged`はマージ成功時のみ1回実行され、マージ失敗時には実行しない。

parallelモードでは、自動マージが成功した全ての経路で`on_merged`を実行しなければならない（SHALL）。

#### Scenario: on_merged executes before merged status transition
- **GIVEN** `hooks.on_merged`が`echo 'Merged {change_id}'`に設定されている
- **AND** change `change-a` のアーカイブが完了している
- **WHEN** base branchへのマージが成功する
- **THEN** `on_merged`が`{change_id}=change-a`で実行される
- **AND** `on_merged`の実行完了後にchangeのステータスがmergedに遷移する

#### Scenario: on_merged in parallel mode before MergeCompleted event
- **GIVEN** `hooks.on_merged`が`echo 'Merged {change_id}'`に設定されている
- **AND** parallelモードで`change-a`の自動マージが成功する
- **THEN** `on_merged`が実行された後に`MergeCompleted`イベントが送信される

#### Scenario: on_merged in TUI manual merge before BranchMergeCompleted event
- **GIVEN** `hooks.on_merged`が設定されている
- **AND** TUI Worktree viewで手動マージ（Mキー）が成功する
- **WHEN** マージが完了する
- **THEN** `on_merged`が実行された後に`BranchMergeCompleted`イベントが送信される

#### Scenario: on_merged failure does not block status transition
- **GIVEN** `hooks.on_merged`が設定されており実行時にエラーを返す
- **WHEN** マージが成功する
- **THEN** `on_merged`のエラーは警告ログに記録される
- **AND** changeのステータスはmergedに遷移する
