## MODIFIED Requirements

### Requirement: Parallel Execution Event Reporting

order-based再分析ループでもarchive完了後のmerge結果に応じてイベントを送信し、merge成功時にはcleanupイベントを送信しなければならない（SHALL）。

MergeDeferred が発生した場合は `MergeDeferred` イベントを送信し、shared orchestration reducer が待ち状態を `MergeWait` または `ResolveWait` として評価しなければならない（SHALL）。TUI は reducer から導出された表示状態を使わなければならない（SHALL）。

#### Scenario: order-based実行でmerge成功時にcleanupイベントを送信する
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **WHEN** mergeが成功する
- **THEN** `CleanupStarted` と `CleanupCompleted` が送信される
- **AND** worktreeが削除される

#### Scenario: MergeDeferred はイベントとして送信される
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **WHEN** mergeが `MergeDeferred` となる
- **THEN** `MergeDeferred` イベントが送信される

#### Scenario: MergeDeferred は reducer が merge wait を導出する
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **AND** resolve は現在実行されていない
- **WHEN** `MergeDeferred` が reducer に適用される
- **THEN** change A の待ち理由は `MergeWait` になる
- **AND** TUI の表示状態は `merge wait` になる

#### Scenario: resolve 実行中の MergeDeferred は reducer が resolve wait を導出する
- **GIVEN** order-based再分析ループで別 change の resolve が実行中である
- **AND** 変更Aに対して `MergeDeferred` が発生する
- **WHEN** reducer が waiting state を評価する
- **THEN** change A は必要に応じて `ResolveWait` に遷移できる
- **AND** TUI は reducer 由来の表示状態を使う
