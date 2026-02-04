## MODIFIED Requirements
### Requirement: 並列モードのコミット起点対象判定
並列モードは、`HEAD` のコミットツリーに存在し、かつ `openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルが存在しない change だけを実行対象として扱わなければならない（SHALL）。

並列実行の開始時、システムはコミットツリーから `openspec/changes/<change-id>/` を列挙し、対象外の change を除外しなければならない（SHALL）。

#### Scenario: 未コミット change を除外する
- **GIVEN** `HEAD` のコミットツリーに存在しない change がある
- **WHEN** 並列実行が開始される
- **THEN** その change は実行対象から除外される
- **AND** 除外された change ID が警告ログに記録される

#### Scenario: change 配下の未コミット差分がある場合は除外する
- **GIVEN** `HEAD` のコミットツリーに存在する change がある
- **AND** `openspec/changes/<change_id>/` 配下に未コミットまたは未追跡ファイルが存在する
- **WHEN** 並列実行が開始される
- **THEN** その change は実行対象から除外される
- **AND** 除外された change ID が警告ログに記録される
