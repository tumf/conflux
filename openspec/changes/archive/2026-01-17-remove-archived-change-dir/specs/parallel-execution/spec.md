## MODIFIED Requirements

### Requirement: Archive後にchangeディレクトリを完全削除する
アーカイブ成功後、システムは `openspec/changes/<change_id>/` をディレクトリごと削除しなければならない（MUST）。
削除できない場合、アーカイブ検証は失敗として扱わなければならない（MUST）。

#### Scenario: アーカイブ成功後にchanges配下を削除する
- **GIVEN** `openspec/changes/archive/` に該当 change が存在する
- **WHEN** アーカイブ処理が成功する
- **THEN** `openspec/changes/<change_id>/` は存在しない
- **AND** 削除できない場合はアーカイブ検証が失敗する
