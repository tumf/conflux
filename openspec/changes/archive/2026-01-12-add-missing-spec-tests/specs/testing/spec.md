## MODIFIED Requirements

### Requirement: 仕様とテストのマッピングドキュメント

プロジェクトは仕様シナリオとテストケースの対応関係を文書化しなければならない（SHALL）。

#### Scenario: マッピングドキュメントの構造

- **WHEN** 開発者が `docs/test-coverage-mapping.md` を参照する
- **THEN** 全ての仕様要件とシナリオがリスト化されている
- **AND** 各シナリオに対応するテスト関数名とファイルパスが記載されている
- **AND** テストが存在しないシナリオが明示されている

#### Scenario: マッピングの更新

- **WHEN** 新しい仕様シナリオが追加される
- **OR** 新しいテストが追加される
- **THEN** マッピングドキュメントが更新される

#### Scenario: Full spec coverage in mapping

- **WHEN** the mapping document is reviewed
- **THEN** all specs are included:
  - cli (47 requirements)
  - configuration (24 requirements)
  - hooks (13 requirements)
  - parallel-execution (9 requirements)
  - testing (9 requirements)
  - tui-editor (9 requirements)
  - tui-key-hints (4 requirements)
  - tui-architecture (3 requirements)
  - workspace-cleanup (3 requirements)
  - code-maintenance (3 requirements)
  - documentation (2 requirements)
- **AND** each requirement has at least one scenario mapped to a test or marked as UI-only

### Requirement: 仕様ベーステスト

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。

#### Scenario: 仕様シナリオのテストカバー

- **WHEN** 仕様に新しいシナリオが追加される
- **THEN** そのシナリオをテストするテストケースが作成される
- **AND** テスト関数名にシナリオの内容が反映される（例: `test_env_var_overrides_default`）

#### Scenario: テストなしシナリオの検出

- **WHEN** カバレッジ分析が実行される
- **THEN** 対応するテストがない仕様シナリオがレポートされる

#### Scenario: UI-only scenarios exclusion

- **WHEN** a scenario describes pure UI rendering behavior
- **THEN** it MAY be marked as "UI-only" in the mapping
- **AND** snapshot tests or manual testing is recommended instead
- **AND** such scenarios are not counted as test gaps
