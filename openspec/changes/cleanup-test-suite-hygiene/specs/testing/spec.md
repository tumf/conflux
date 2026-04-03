## MODIFIED Requirements

### Requirement: 仕様ベーステスト

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。重複するテスト（同一シナリオを unit test と integration test の双方で検証）は、より低レベルかつ高速な側（unit test）を残し、integration 側を削除してよい（MAY）。環境依存で常時 skip される integration test は削除しなければならない（MUST）。さらに、リポジトリは test file の配置・命名・補助ヘルパにより test scope（unit / integration / contract / e2e）を明示しなければならない（SHALL）。real external boundary を使う検証は、その価値が contract/integration/e2e にある場合のみ残してよく（MAY）、pure unit coverage の主張としては扱ってはならない（MUST NOT）。`PATH`、`HOME`、current working directory などの process-global state を変更するテストは、共有 guard により直列化するか、競合しない設計へ書き換えなければならない（MUST）。

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

#### Scenario: Mixed-scope test file is reorganized

- **WHEN** a test file mixes pure unit checks, integration checks, and e2e-style flows in a way that obscures intent
- **THEN** the repository SHALL split or rename the tests so the dominant scope of each file is clear
- **AND** the resulting files or modules SHALL make it obvious which tests rely on real external boundaries

#### Scenario: Process-global test mutation is serialized

- **WHEN** a test changes `PATH`, `HOME`, or current working directory
- **THEN** it SHALL use a shared repository guard or equivalent serialization mechanism
- **AND** parallel test execution SHALL NOT introduce hidden races through unsynchronized process-global mutation

#### Scenario: Real external boundary test is retained as contract coverage

- **WHEN** a test exercises a real external boundary such as `git merge-tree`, worktree operations, OS process cleanup, or websocket protocol flow
- **THEN** the test MAY remain in the repository as integration, contract, or e2e coverage
- **AND** its placement, naming, or surrounding documentation SHALL NOT imply that it is pure unit coverage

#### Scenario: Duplicate unit-test execution across crate targets is removed

- **WHEN** the same internal test module can be reached through both library and binary crate targets
- **THEN** the repository SHALL consolidate test ownership so the same unit-test logic is not unintentionally executed twice
- **AND** test placement SHALL make it clear whether coverage belongs to the library crate, the binary crate, or an integration target

#### Scenario: Timing-sensitive tests avoid unnecessary wall-clock delay

- **WHEN** a test verifies debounce, retry, timeout, scheduler, or polling behavior that does not require real elapsed-time realism
- **THEN** the repository SHALL prefer deterministic time control, injected timing configuration, or equivalent test-only mechanisms over multi-second real-time sleeps
- **AND** the slowest tests SHALL NOT derive most of their runtime from avoidable wall-clock waiting
