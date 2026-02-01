## MODIFIED Requirements
### Requirement: Individual Merge on Archive Completion
並列実行モードにおいて、order-based再分析ループでもarchive完了後に個別mergeを実行しなければならない（SHALL）。

merge開始前に `is_archive_commit_complete` を使用してworktreeのarchive完了状態を検証しなければならない（MUST）。検証条件は以下の通り:
1. worktreeがclean（未コミットの変更がない）
2. `openspec/changes/<change_id>` が存在しない
3. archiveエントリ（`openspec/changes/archive/<date>-<change_id>` または `openspec/changes/archive/<change_id>`）が存在する

上記いずれかの条件を満たさない場合は `MergeDeferred` を返し、`MergeWait` に留めなければならない（MUST）。

#### Scenario: order-based実行でarchive後にMergeDeferredとなる（changesが残っている）
- **GIVEN** order-based再分析ループで変更Aのarchiveコマンドが完了している
- **AND** `openspec/changes/{change_id}` が存在している
- **WHEN** `attempt_merge()` がmerge前の検証を行う
- **THEN** `is_archive_commit_complete` は `false` を返す
- **AND** `attempt_merge()` は `MergeDeferred` を返す
- **AND** 変更Aは `MergeWait` に留まる

#### Scenario: worktreeがdirtyの場合はMergeDeferred
- **GIVEN** order-based再分析ループで変更Aのarchiveコマンドが完了している
- **AND** worktreeがdirty（未コミットの変更がある）
- **WHEN** `attempt_merge()` がmerge前の検証を行う
- **THEN** `is_archive_commit_complete` は `false` を返す
- **AND** `attempt_merge()` は `MergeDeferred` を返す
- **AND** 失敗理由に「archive未完了」の文脈が含まれる

#### Scenario: archiveエントリが存在しない場合はMergeDeferred
- **GIVEN** order-based再分析ループで変更Aのarchiveコマンドが完了している
- **AND** `openspec/changes/{change_id}` は存在しない
- **AND** archiveエントリも存在しない
- **WHEN** `attempt_merge()` がmerge前の検証を行う
- **THEN** `is_archive_commit_complete` は `false` を返す
- **AND** `attempt_merge()` は `MergeDeferred` を返す

#### Scenario: archive完了状態でmergeが実行される
- **GIVEN** worktreeがclean
- **AND** `openspec/changes/{change_id}` が存在しない
- **AND** archiveエントリが存在する
- **WHEN** `attempt_merge()` がmerge前の検証を行う
- **THEN** `is_archive_commit_complete` は `true` を返す
- **AND** mergeが実行される
