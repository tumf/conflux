## MODIFIED Requirements
### Requirement: Individual Merge on Archive Completion
並列実行モードにおいて、order-based再分析ループでもarchive完了後に個別mergeを実行し、`verify_archive_completion` が未アーカイブを返す場合は `MergeDeferred` として `MergeWait` に留めなければならない（SHALL）。

#### Scenario: order-based実行でarchive後にMergeDeferredとなる
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **AND** `openspec/changes/{change_id}` が存在している
- **WHEN** archive後のmergeを開始する
- **THEN** `verify_archive_completion` は未アーカイブを返す
- **AND** `MergeDeferred` を返す
- **AND** 変更Aは `MergeWait` に留まる

### Requirement: Parallel Execution Event Reporting
order-based再分析ループでもarchive完了後のmerge結果に応じてイベントを送信し、merge成功時にはcleanupイベント、MergeDeferred時にはMergeWait遷移を送信しなければならない（SHALL）。

#### Scenario: order-based実行でmerge成功時にcleanupイベントを送信する
- **GIVEN** order-based再分析ループで変更Aのarchiveが完了している
- **WHEN** mergeが成功する
- **THEN** `CleanupStarted` と `CleanupCompleted` が送信される
- **AND** worktreeが削除される
