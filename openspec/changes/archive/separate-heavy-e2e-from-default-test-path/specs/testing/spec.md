## MODIFIED Requirements

### Requirement: 仕様ベーステスト

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。重複するテスト（同一シナリオを unit test と integration test の双方で検証）は、より低レベルかつ高速な側（unit test）を残し、integration 側を削除してよい（MAY）。環境依存で常時 skip される integration test は削除しなければならない（MUST）。さらに、リポジトリはデフォルトの開発ループで走らせる高速テスト群と、実 `git`・実 process・実 socket・実 filesystem・避けられる wall-clock 待機などを含む重い real-boundary テスト群を明示的に分離しなければならない（SHALL）。重い test 群は明示実行の opt-in tier として保持してよい（MAY）が、通常の default-path test として暗黙実行されてはならない（MUST NOT）。

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

#### Scenario: Heavy real-boundary tests are excluded from default path
- **GIVEN** a Rust test suite requires real `git`, worktree operations, OS process execution, socket/websocket interaction, or avoidable wall-clock waiting
- **WHEN** the repository defines its default developer test path
- **THEN** that suite SHALL be placed in the heavy opt-in tier rather than the default path
- **AND** developers SHALL have an explicit documented command or mechanism to run it intentionally

#### Scenario: Default developer validation stays on fast path
- **GIVEN** a developer runs the repository's ordinary local validation loop
- **WHEN** they use the default-path test command
- **THEN** only the fast default-path tier is required to run by default
- **AND** heavy opt-in tests are not silently included unless explicitly requested

#### Scenario: Heavy tier remains available for explicit validation
- **GIVEN** a maintainer wants full real-boundary confidence
- **WHEN** they invoke the repository's documented heavy-tier command or equivalent mechanism
- **THEN** the heavy E2E/contract suites execute
- **AND** the separation from the default path does not remove that coverage from the repository
