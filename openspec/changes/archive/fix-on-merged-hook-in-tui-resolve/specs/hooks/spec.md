## MODIFIED Requirements

### Requirement: on_merged hook

オーケストレーターはchangeがbase branchにマージされた直後、mergedステータスへ遷移する直前に`on_merged`フックを実行しなければならない（SHALL）。

`on_merged`はマージ成功時のみ1回実行され、マージ失敗時には実行しない。

parallelモードでは、自動マージが成功した全ての経路で`on_merged`を実行しなければならない（SHALL）。

TUI の ResolveMerge（遅延マージ解決）成功時にも `on_merged` を実行しなければならない（SHALL）。`ResolveCompleted` イベント送信前にフックを実行し、フック完了後にステータスを merged に遷移させる。

`on_merged` フック実行前に、`.git/index.lock` ファイルの解放を待機しなければならない（SHALL）。最大待機時間は `hooks.index_lock_wait_secs`（デフォルト 10 秒）で設定可能。

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

#### Scenario: TUI ResolveMerge完了
- **GIVEN** `hooks.on_merged`が設定されている
- **AND** change`change-a`の遅延マージ解決（ResolveMerge）が成功する
- **WHEN** `ResolveCompleted`が発行される前
- **THEN** `on_merged`が`{change_id}=change-a`で実行される
- **AND** `on_merged`の実行完了後に`ResolveCompleted`イベントが送信される

#### Scenario: serial(run)でのマージ相当
- **GIVEN** runモード（非parallel）でchange`change-a`を処理している
- **WHEN** archiveが成功し、base branchに変更が反映済みと確認できる
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: index.lock 待機後にフック実行

- **GIVEN** `hooks.on_merged` が設定されている
- **AND** `.git/index.lock` ファイルが存在する
- **WHEN** `on_merged` フックが実行される
- **THEN** オーケストレーターは `.git/index.lock` の解放を最大 `index_lock_wait_secs` 秒（デフォルト 10）待機する
- **AND** 解放後にフックコマンドを実行する

#### Scenario: index.lock 待機タイムアウト

- **GIVEN** `hooks.on_merged` が設定されている
- **AND** `.git/index.lock` ファイルが `index_lock_wait_secs` 秒を超えて存在し続ける
- **WHEN** `on_merged` フックが実行される
- **THEN** オーケストレーターは警告ログを出力してフックコマンドの実行を試行する
