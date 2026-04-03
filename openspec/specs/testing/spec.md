# testing Specification

## Purpose
Defines testing strategy, coverage requirements, and test organization.
## Requirements
### Requirement: カバレッジ測定コマンド

プロジェクトはカバレッジ測定のための標準化されたコマンドを提供しなければならない（SHALL）。

#### Scenario: HTMLレポート生成

- **WHEN** 開発者が `cargo llvm-cov --all-features --workspace --html` を実行する
- **THEN** `target/llvm-cov/html/index.html` にHTMLカバレッジレポートが生成される
- **AND** レポートにはモジュール別の詳細なカバレッジ情報が含まれる

#### Scenario: サマリー表示

- **WHEN** 開発者が `cargo llvm-cov --all-features --workspace --summary-only` を実行する
- **THEN** 標準出力にカバレッジサマリーが表示される
- **AND** モジュール別のカバレッジパーセンテージが含まれる

#### Scenario: LCOV形式の生成

- **WHEN** 開発者が `cargo llvm-cov --all-features --workspace --lcov --output-path target/coverage.lcov` を実行する
- **THEN** `target/coverage.lcov` にLCOV形式のカバレッジデータが生成される
- **AND** CI/CDツールで利用可能な形式である

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

### Requirement: カバレッジギャップ分析

カバレッジ測定は仕様・テスト・実装の整合性を確認するために使用されなければならない（SHALL）。

#### Scenario: 仕様ギャップの検出

- **WHEN** 仕様に記載されている振る舞いに対応するテストがない
- **THEN** その振る舞いがカバレッジギャップとして特定される
- **AND** テスト追加が必要とマークされる

#### Scenario: 実装ギャップの検出

- **WHEN** 実装されているコードに対応する仕様が存在しない
- **AND** そのコードのカバレッジが低い（<70%）
- **THEN** そのコードが実装ギャップとして特定される
- **AND** 仕様化または削除が必要とマークされる

#### Scenario: カバレッジ分析レポート

- **WHEN** カバレッジギャップ分析が実行される
- **THEN** 以下の情報を含むレポートが生成される:
  - 仕様ギャップリスト（仕様あり・テストなし）
  - 実装ギャップリスト（実装あり・仕様なし）
  - 優先度付け（P0-P3）

### Requirement: テストの優先度付け

テストの追加と改善は優先度に基づいて行われなければならない（SHALL）。

#### Scenario: P0 - 仕様シナリオのテストなし

- **WHEN** 仕様シナリオに対応するテストが存在しない
- **THEN** そのテストはP0（最優先）とマークされる
- **AND** 即座に対応が必要とされる

#### Scenario: P1 - 低カバレッジコアモジュール

- **WHEN** コアモジュール（orchestrator, agent, opencode）のカバレッジが30%未満である
- **THEN** そのモジュールはP1（高優先度）とマークされる
- **AND** 分析と仕様化またはテスト追加が必要とされる

#### Scenario: P2 - 中カバレッジモジュール

- **WHEN** モジュールのカバレッジが30-70%である
- **THEN** そのモジュールはP2（中優先度）とマークされる
- **AND** 段階的なテスト追加が推奨される

#### Scenario: P3 - 高カバレッジモジュール

- **WHEN** モジュールのカバレッジが70%以上である
- **THEN** そのモジュールはP3（低優先度）とマークされる
- **AND** 現状維持、新機能追加時に改善が推奨される

### Requirement: Configuration仕様のテストギャップ解消

Configuration仕様の全シナリオがテストでカバーされなければならない（SHALL）。

#### Scenario: 環境変数OPENSPEC_CMDの優先順位テスト

- **WHEN** 環境変数 `OPENSPEC_CMD` が設定されている
- **AND** CLI引数 `--openspec-cmd` が指定されている
- **THEN** CLI引数の値が優先されることをテストで検証する
- **Test**: `test_cli_arg_overrides_env_var_openspec_cmd` in `src/config.rs`

#### Scenario: プロジェクト設定の優先順位テスト

- **WHEN** プロジェクト設定 `.cflx.jsonc` が存在する
- **AND** グローバル設定 `~/.config/cflx/config.jsonc` が存在する
- **THEN** プロジェクト設定が優先されることをテストで検証する
- **Test**: `test_project_config_overrides_global_config` in `src/config.rs`

### Requirement: CLI仕様のテストギャップ解消

CLI仕様の全シナリオがテストでカバーされなければならない（SHALL）。

#### Scenario: TUI自動更新機能のテスト

- **WHEN** TUIが表示されている
- **AND** 5秒が経過する
- **THEN** `openspec list` が自動実行されることをテストで検証する
- **Test**: `test_tui_auto_refresh_interval` in `src/tui.rs`

#### Scenario: NEWバッジ表示ロジックのテスト

- **WHEN** 新しい変更が検出される
- **THEN** その変更に「NEW」バッジが表示されることをテストで検証する
- **Test**: `test_new_badge_display_for_new_changes` in `src/tui.rs`

### Requirement: 低カバレッジモジュールの分析

カバレッジが30%未満のモジュールは詳細に分析されなければならない（SHALL）。

#### Scenario: opencode.rsの分析

- **WHEN** `opencode.rs` のカバレッジが8.82%である
- **THEN** 未テストの93行が以下のように分類される:
  - 仕様化されている振る舞い → テスト追加
  - 仕様化されていない振る舞い → 仕様化または削除
- **AND** 分析結果が文書化される

#### Scenario: orchestrator.rsの分析

- **WHEN** `orchestrator.rs` のカバレッジが28.14%である
- **THEN** 未テストの286行が以下のように分類される:
  - 仕様化されている振る舞い → テスト追加
  - 仕様化されていない振る舞い → 仕様化または削除
- **AND** 分析結果が文書化される

### Requirement: 継続的カバレッジ分析

カバレッジ分析は継続的に実施されなければならない（SHALL）。

#### Scenario: 新機能追加時の分析

- **WHEN** 新機能が追加される
- **THEN** 以下のプロセスが実行される:
  1. 仕様にシナリオ追加
  2. シナリオに対応するテスト作成
  3. 実装
  4. カバレッジ測定
  5. ギャップ確認

#### Scenario: 定期レビュー

- **WHEN** 月次レビューが実施される
- **THEN** 以下が実行される:
  1. カバレッジレポート生成
  2. マッピングドキュメント更新
  3. 新しいギャップの特定
  4. 優先度付けと対応計画作成


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


### Requirement: Spec Test Annotations Parsing

`spec_test_annotations` モジュールは仕様ファイルからシナリオを正しくパースしなければならない (SHALL)。

LazyLock 移行後も、パース結果が従来と同一であることをテストスイートで担保しなければならない (MUST)。

#### Scenario: LazyLock 移行後もパース結果が同一

- **GIVEN** `src/spec_test_annotations.rs` の正規表現が `LazyLock<Regex>` に移行済みである
- **WHEN** 既存の spec テストアノテーションパーサーテストを実行する
- **THEN** すべてのテストが移行前と同一の結果を返す
