## MODIFIED Requirements

### Requirement: 仕様ベーステスト

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。重複するテスト（同一シナリオを unit test と integration test の双方で検証）は、より低レベルかつ高速な側（unit test）を残し、integration 側を削除してよい（MAY）。環境依存で常時 skip される integration test は削除しなければならない（MUST）。

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

#### Scenario: Redundant integration test removal

- **WHEN** an integration test duplicates a unit test that covers the same spec scenario
- **THEN** the integration test MAY be removed
- **AND** the unit test MUST be retained

#### Scenario: Environment-dependent test removal

- **WHEN** an integration test depends on user-local files that may not exist
- **AND** the test unconditionally skips when those files are absent
- **THEN** the test MUST be removed or rewritten to use repository-local fixtures
