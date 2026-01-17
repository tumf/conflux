## MODIFIED Requirements
### Requirement: Individual Merge on Archive Completion

並列実行モードにおいて、システムは merge 実行前に `verify_archive_completion` を再検証し、`openspec/changes/{change_id}` が存在する場合は未アーカイブとして `MergeDeferred` を返して `MergeWait` に留めなければならない（SHALL）。

#### Scenario: Merge 直前に changes が残っている場合は MergeDeferred
- **GIVEN** 変更 A が archive 完了として処理された
- **AND** `openspec/changes/{change_id}` が存在している
- **WHEN** merge を開始する
- **THEN** `verify_archive_completion` は未アーカイブを返す
- **AND** `MergeDeferred` を返す
- **AND** 変更 A は `MergeWait` に留まる
