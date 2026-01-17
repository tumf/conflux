## MODIFIED Requirements

### Requirement: Archive Commit Completion via resolve_command
並列実行モードにおいて、archive 完了後のコミット作成は `resolve_command` に委譲し、作業ツリーがクリーンであり、かつ `openspec/changes/{change_id}` が存在しない場合にのみ archive コミット完了と判定しなければならない（SHALL）。

#### Scenario: Archive コミットが存在しても changes が残っている場合は未完了
- **GIVEN** `Archive: <change_id>` のコミットが存在する
- **AND** `openspec/changes/{change_id}` が存在している
- **WHEN** archive コミットの完了判定を行う
- **THEN** 未完了として扱う

### Requirement: Individual Merge on Archive Completion
並列実行モードにおいて、システムは merge 実行前に `verify_archive_completion` を再検証し、未アーカイブの場合は `MergeDeferred` を返して `MergeWait` に留めなければならない（SHALL）。

#### Scenario: Merge 直前に未アーカイブを検知した場合は MergeDeferred
- **GIVEN** 変更 A が archive 完了として処理された
- **AND** `verify_archive_completion` が未アーカイブを返す
- **WHEN** merge を開始する
- **THEN** `MergeDeferred` を返す
- **AND** 変更 A は `MergeWait` に留まる

### Requirement: Archive Resume Requires Archive Commit
archive コミットを確定する際、`ensure_archive_commit` は `openspec/changes/{change_id}` が存在する場合にエラーを返さなければならない（MUST）。

#### Scenario: changes 側が残っている場合は archive commit を作らない
- **GIVEN** `openspec/changes/{change_id}` が存在する
- **WHEN** `ensure_archive_commit` が archive コミットを作成しようとする
- **THEN** エラーを返す
