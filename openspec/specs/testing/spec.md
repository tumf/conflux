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
