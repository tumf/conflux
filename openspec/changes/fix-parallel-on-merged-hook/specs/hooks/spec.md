## MODIFIED Requirements
### Requirement: on_merged hook
オーケストレーターはchangeがbase branchにマージされた直後に`on_merged`フックを実行しなければならない（SHALL）。

`on_merged`はマージ成功時のみ1回実行され、マージ失敗時には実行しない。

parallelモードでは、自動マージが成功した全ての経路で`on_merged`を実行しなければならない（SHALL）。

#### Scenario: Parallelモードで自動マージ完了
- **GIVEN** `hooks.on_merged`が`echo 'Merged {change_id}'`に設定されている
- **WHEN** parallelモードでchange`change-a`がbase branchにマージされ`MergeCompleted`が発行される
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: Parallelモードでarchive直後に即時マージ成功
- **GIVEN** `hooks.on_merged`が`echo 'Merged {change_id}'`に設定されている
- **AND** parallelモードでchange`change-a`がarchive完了後に即時マージされる
- **WHEN** マージが成功する
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: TUI Worktreeの手動マージ完了
- **GIVEN** `hooks.on_merged`が設定されている
- **AND** worktreeブランチ`change-a`をMキーでマージする
- **WHEN** `BranchMergeCompleted`が発行される
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: serial(run)でのマージ相当
- **GIVEN** runモード（非parallel）でchange`change-a`を処理している
- **WHEN** archiveが成功し、base branchに変更が反映済みと確認できる
- **THEN** `on_merged`が`{change_id}=change-a`で実行される
