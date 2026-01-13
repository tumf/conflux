# parallel-execution Spec Delta

## MODIFIED Requirements
### Requirement: 並列モードのコミット起点対象判定
並列モードは、`HEAD` のコミットツリーに存在する change だけを実行対象として扱わなければならない（SHALL）。

並列実行の開始時、システムはコミットツリーから `openspec/changes/<change-id>/` を列挙し、対象外の change を除外しなければならない（SHALL）。

#### Scenario: 未コミット change を除外する
- **GIVEN** `HEAD` のコミットツリーに存在しない change がある
- **WHEN** 並列実行が開始される
- **THEN** その change は実行対象から除外される
- **AND** 除外された change ID が警告ログに記録される

### Requirement: 未コミット change の tasks 読み込みを行わない
並列モードは、実行対象の判定にコミットツリーを利用し、未コミット change の tasks 読み込みを試行してはならない（SHALL）。

#### Scenario: 未コミット change の tasks は読み込まれない
- **GIVEN** 未コミットの change が存在する
- **WHEN** 並列モードの対象判定が行われる
- **THEN** 未コミット change の `tasks.md` 読み込みは行われない
- **AND** 未コミット change は並列対象から外れる
