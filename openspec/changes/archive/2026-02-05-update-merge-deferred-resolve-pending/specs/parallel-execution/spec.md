## MODIFIED Requirements
### Requirement: Parallel Execution Event Reporting
order-based再分析ループでもarchive完了後のmerge結果に応じてイベントを送信し、merge成功時にはcleanupイベントを送信しなければならない（SHALL）。
MergeDeferred が発生した場合は `MergeDeferred` イベントを送信し、待ち状態の表示は TUI 仕様に従って `MergeWait` または `ResolveWait` を判定しなければならない（SHALL）。

#### Scenario: order-based実行でmerge成功時にcleanupイベントを送信する
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **WHEN** mergeが成功する
- **THEN** `CleanupStarted` と `CleanupCompleted` が送信される
- **AND** worktreeが削除される

#### Scenario: MergeDeferred はイベントとして送信される
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **WHEN** mergeが `MergeDeferred` となる
- **THEN** `MergeDeferred` イベントが送信される
