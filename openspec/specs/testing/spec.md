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

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。workflow split によって fixed guidance の authoritative source が command template / dedicated skill / Rust runtime enforcement に分割される場合、少なくとも output contract と ownership boundary を検証する drift-detection tests が存在しなければならない（SHALL）。

#### Scenario: Workflow split prompt contracts are regression-tested

- **WHEN** operation-specific prompt surfaces are split across command templates, dedicated skills, and Rust prompt builders
- **THEN** the repository contains targeted tests that detect authoritative-source drift for fixed guidance and machine-readable output contracts
- **AND** acceptance-related tests cover production-style formatting drift cases that would otherwise trigger an unintended `CONTINUE` fallback
- **AND** the tests fail when documented prompt or parser contracts are changed in only one surface without corresponding updates to the others

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

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。workflow split によって fixed guidance の authoritative source が command template / dedicated skill / Rust runtime enforcement に分割される場合、少なくとも output contract と ownership boundary を検証する drift-detection tests が存在しなければならない（SHALL）。

#### Scenario: Workflow split prompt contracts are regression-tested

- **WHEN** operation-specific prompt surfaces are split across command templates, dedicated skills, and Rust prompt builders
- **THEN** the repository contains targeted tests that detect authoritative-source drift for fixed guidance and machine-readable output contracts
- **AND** acceptance-related tests cover production-style formatting drift cases that would otherwise trigger an unintended `CONTINUE` fallback
- **AND** the tests fail when documented prompt or parser contracts are changed in only one surface without corresponding updates to the others

### Requirement: 仕様ベーステスト

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。workflow split によって fixed guidance の authoritative source が command template / dedicated skill / Rust runtime enforcement に分割される場合、少なくとも output contract と ownership boundary を検証する drift-detection tests が存在しなければならない（SHALL）。

#### Scenario: Workflow split prompt contracts are regression-tested

- **WHEN** operation-specific prompt surfaces are split across command templates, dedicated skills, and Rust prompt builders
- **THEN** the repository contains targeted tests that detect authoritative-source drift for fixed guidance and machine-readable output contracts
- **AND** acceptance-related tests cover production-style formatting drift cases that would otherwise trigger an unintended `CONTINUE` fallback
- **AND** the tests fail when documented prompt or parser contracts are changed in only one surface without corresponding updates to the others

### Requirement: 仕様ベーステスト

全ての仕様シナリオに対応するテストが存在しなければならない（SHALL）。workflow split によって fixed guidance の authoritative source が command template / dedicated skill / Rust runtime enforcement に分割される場合、少なくとも output contract と ownership boundary を検証する drift-detection tests が存在しなければならない（SHALL）。

#### Scenario: Workflow split prompt contracts are regression-tested

- **WHEN** operation-specific prompt surfaces are split across command templates, dedicated skills, and Rust prompt builders
- **THEN** the repository contains targeted tests that detect authoritative-source drift for fixed guidance and machine-readable output contracts
- **AND** acceptance-related tests cover production-style formatting drift cases that would otherwise trigger an unintended `CONTINUE` fallback
- **AND** the tests fail when documented prompt or parser contracts are changed in only one surface without corresponding updates to the others

### Requirement: Spec Test Annotations Parsing

`spec_test_annotations` モジュールは仕様ファイルからシナリオを正しくパースしなければならない (SHALL)。

LazyLock 移行後も、パース結果が従来と同一であることをテストスイートで担保しなければならない (MUST)。

#### Scenario: LazyLock 移行後もパース結果が同一

- **GIVEN** `src/spec_test_annotations.rs` の正規表現が `LazyLock<Regex>` に移行済みである
- **WHEN** 既存の spec テストアノテーションパーサーテストを実行する
- **THEN** すべてのテストが移行前と同一の結果を返す

### Requirement: git_sync 回帰テストは共通フィクスチャを利用する

システムは `git/sync` の主要回帰シナリオについて、公開レスポンス契約を固定した characterization test を維持しなければならない（SHALL）。

これらのテストは、router / state / repository divergence の共通フィクスチャを再利用し、公開 API の期待結果を意図中心に検証できる構造でなければならない（MUST）。

#### Scenario: 同期済みシナリオの公開レスポンスが維持される

- **GIVEN** ローカルとリモートが既に一致しているプロジェクトがある
- **WHEN** `POST /api/v1/projects/{id}/git/sync` を呼び出す
- **THEN** 応答の `status` は既存どおりである
- **AND** `resolve_command_ran`、`resolve_exit_code`、`push.status`、`skipped_reason` の契約は変わらない

#### Scenario: 差分ありシナリオでも helper 抽出後に回帰しない

- **GIVEN** local/remote divergence または remote ahead の状態を作る共通 fixture がある
- **WHEN** `git/sync` 回帰テストを実行する
- **THEN** 既存どおりのステータスコードと JSON 契約が検証される
- **AND** テスト本体は重複した準備コードではなく期待挙動の assertion を中心に記述される

### Requirement: Deterministic completion for the malformed shadow-judge fixture

The malformed shadow-judge runtime fixture SHALL establish completion of the actual judge task before it invokes the existing collection path. A request file or child-written output marker MUST NOT be treated as task completion. Ordering support MUST be test-only, per-analyzer-instance and opt-in solely for this fixture; it MUST NOT change non-test waiting, cancellation, cleanup, parsing or shadow authority.

#### Scenario: Completed malformed child produces one bounded diagnostic

- **GIVEN** the fixture runs the real malformed judge child with the proposal secret and the real conventional analyzer
- **WHEN** its test-only barrier observes the actual handle is finished before collection
- **THEN** the original test emits exactly one WARN failure-category record with `outcome="invalid_json"` and `expected_answers=2`, not `cancelled`
- **AND** raw secret output and unvalidated model identity are absent, and the authoritative order/dependencies remain unchanged
- **AND** neither sleep nor a substituted result/category establishes that outcome

#### Scenario: Completion never arrives

- **GIVEN** the opt-in fixture has no shadow or its actual task does not finish
- **WHEN** the barrier precondition or generous hang safeguard fails
- **THEN** the test fails explicitly rather than skipping or claiming completion
- **AND** the existing unfinished-judge runtime controls remain opt-out and continue to exercise production nonblocking cancellation unchanged

### Requirement: Shared merge-lane fixture isolation without suite serialization

The manual resolve-wait retry fixture and its lane-entering competitors SHALL coordinate through the existing process-global `merge_lock_test_mutex` at the actual lane boundary. Guard ownership MUST cover actual completion of spawned lane work; no new repository/change key or blanket suite serialization may substitute for the mutex protecting the existing process-global lane. Production lock scope, try-lock behavior, retry routing and manual versus automatic deferral semantics MUST remain unchanged.

#### Scenario: Unrelated apply completion causes a manual retry outcome

- **GIVEN** `test_manual_resolve_wait_retries_after_in_flight_apply_completes` holds its existing test guard and prepares an incomplete archived workspace
- **WHEN** unrelated in-flight Apply completes and the scheduler dispatches its real retry
- **THEN** the original dispatch assertion and `MergeDeferred(auto_resumable=false)` assertion both hold under default parallel test scheduling
- **AND** the post-archive pipeline fixture, dirty-to-clean retry fixture, and converging spawned-retry fixture use the same test guard across their actual lane operations, not their unrelated setup

#### Scenario: Intentional contention still exercises the busy branch

- **GIVEN** a contention control owns the outer test guard and intentionally holds the production merge lock
- **WHEN** it calls the existing `attempt_merge`
- **THEN** the original automatic busy-lane deferral is observed without waiting
- **AND** tests that do not touch the lane remain parallel and are not guarded merely to make the suite green

### Requirement: Conflict fixture identity is local and conflict setup is explicit

The conflicted-status fixture SHALL initialize author identity with repository-local `user.name` and `user.email` solely in its own temporary repository. Before production status assertions, it MUST establish a real content-conflicted index rather than accept any nonzero merge status. Global Git configuration, test-process identity environment, shared helpers and intentional missing-identity controls MUST remain unchanged.

#### Scenario: Status classifies a genuinely conflicted index

- **GIVEN** the temporary conflict repository has local identity and diverging edits to `conflict.txt`
- **WHEN** the fixture runs its real merge with global/system configuration disabled
- **THEN** setup asserts merge exit code 1, `MERGE_HEAD`, and unmerged stages 1, 2 and 3 for `conflict.txt`
- **AND** the existing production status paths still report `UU conflict.txt` and a dirty working directory
- **AND** identity/configuration failure is a setup failure, not evidence of a conflict

#### Scenario: Missing-identity negative controls are not repaired away

- **GIVEN** the final Apply and WIP lock-retry negative fixtures intentionally blank their own local identity
- **WHEN** they run unchanged alongside the locally prepared conflict fixture
- **THEN** identity failures remain terminal, produce no commit and are not retried
- **AND** both exact controls and the default-parallel full-lib gates pass without suite-wide identity overrides

### Requirement: Fixture repair is accepted with default-parallel full-lib evidence

This fixture-only repair SHALL retain all three original test identities and substantive assertions and SHALL pass its declared focused selections plus both default and explicit `web-monitoring` full-lib commands under ordinary default parallel scheduling. No new ignore/skip/feature exclusion, assertion weakening, suite serialization or process-global environment change may count as a repair.

#### Scenario: Focused success alone is insufficient

- **GIVEN** all exact fixture tests pass in isolation
- **WHEN** either declared full-lib command fails, selects no tests or has no completed result
- **THEN** the change is not complete and no full-suite PASS is claimed
- **AND** unrelated failures are reported without expanding this test-only scope

#### Scenario: Candidate has complete repository-local evidence

- **GIVEN** all focused gates select their expected nonzero tests and both full-lib commands complete with exit 0 and default parallel scheduling
- **WHEN** Conflux reviews the candidate diff and captured test counts/logs
- **THEN** only the specified fixture bodies and explicitly test-only support have changed, with product semantics and negative controls preserved
- **AND** logs remain external observability outputs, not target-worktree artifacts or workflow authority
