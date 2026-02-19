## ADDED Requirements
### Requirement: Archive Test Fixture Helpers
アーカイブ関連テストは、変更/アーカイブのディレクトリ構造を作成する共通フィクスチャヘルパーを使用しなければならない (MUST)。

#### Scenario: 既存と同等のディレクトリ構造
- **WHEN** テストがフィクスチャヘルパーを呼び出す
- **THEN** `openspec/changes` と `openspec/changes/archive` が作成される
- **AND** 既存テストと同じ前提で検証が実行できる
