# Test Coverage Mapping

This document maps specification scenarios to their corresponding test implementations.

## Coverage Summary (Updated)

| Module | Coverage | Status | Missed Lines | Change |
|--------|----------|--------|--------------|--------|
| progress.rs | 100.00% | ✅ Excellent | 0 | +39.71% |
| config.rs | 93.68% | ✅ Excellent | 18 | +10.35% |
| cli.rs | 92.23% | ✅ Excellent | 8 | - |
| hooks.rs | 86.42% | ✅ Good | 55 | - |
| openspec.rs | 73.08% | ⚠️ Moderate | 35 | - |
| agent.rs | 67.91% | ⚠️ Moderate | 103 | - |
| orchestrator.rs | 41.61% | ⚠️ Improved | 261 | +13.47% |
| tui.rs | 39.56% | ❌ Low | 767 | +2.67% |
| opencode.rs | 8.82% | ❌❌ Legacy | 93 | - |
| main.rs | 0.00% | ❌❌ Entry point | 62 | - |
| **TOTAL** | **56.77%** | **Improved** | **1402** | **+5.15%** |

## CLI Specification (`openspec/specs/cli/spec.md`)

### Requirement: サブコマンド構造

| Scenario | Test | Status |
|----------|------|--------|
| サブコマンドなしで実行 | `tests/ralph_compatibility.rs::test_mode_switching` | ✅ Covered |
| 不明なサブコマンドで実行 | - | ⚠️ CLI parsing error (implicit) |

### Requirement: run サブコマンド

| Scenario | Test | Status |
|----------|------|--------|
| run サブコマンドの基本実行 | `tests/ralph_compatibility.rs::test_execution_mode_separation` | ✅ Covered |
| 特定の変更を指定して実行 | `src/cli.rs::test_run_subcommand_change_option` | ✅ Covered |
| opencode パスのカスタマイズ | `tests/e2e_tests.rs::test_opencode_run_command_format` | ✅ Covered |
| openspec コマンドのカスタマイズ | `tests/e2e_tests.rs::test_openspec_apply_command_format` | ✅ Covered |

### Requirement: デフォルトTUI起動

| Scenario | Test | Status |
|----------|------|--------|
| サブコマンドなしでの起動 | `tests/ralph_compatibility.rs::test_mode_switching` | ✅ Covered |
| runサブコマンドでの起動（後方互換性） | `tests/ralph_compatibility.rs::test_execution_mode_separation` | ✅ Covered |

### Requirement: 変更選択モード

| Scenario | Test | Status |
|----------|------|--------|
| 変更一覧の初期表示 | `src/tui.rs::test_app_state_new` | ✅ Covered |
| カーソル移動 | `src/tui.rs::test_cursor_navigation` | ✅ Covered |
| 選択トグル | `src/tui.rs::test_toggle_selection` | ✅ Covered |
| 終了 | - | ⚠️ Implicit (TUI integration) |

### Requirement: 選択変更の実行開始

| Scenario | Test | Status |
|----------|------|--------|
| F5キーで実行開始 | `src/tui.rs::test_start_processing_with_selection` | ✅ Covered |
| 選択なしでF5キー | `src/tui.rs::test_start_processing_without_selection` | ✅ Covered |

### Requirement: 実行モードダッシュボード

| Scenario | Test | Status |
|----------|------|--------|
| 変更一覧の進捗表示 | `src/tui.rs::test_change_state_progress` | ✅ Covered |
| キュー状態の表示 | `src/tui.rs::test_queue_status_display` | ✅ Covered |
| 現在処理中の変更のハイライト | - | ⚠️ UI rendering (hard to unit test) |
| ログのリアルタイム表示 | - | ⚠️ UI rendering (hard to unit test) |
| 処理完了時の表示 | - | ⚠️ UI rendering (hard to unit test) |
| エラー発生時の表示 | `src/tui.rs::test_processing_error_transitions_to_error_mode` | ✅ Covered |

### Requirement: TUIレイアウト構成

| Scenario | Test | Status |
|----------|------|--------|
| 選択モードのレイアウト | - | ⚠️ UI rendering |
| 実行モードのレイアウト | - | ⚠️ UI rendering |
| 最小ターミナルサイズ | - | ⚠️ UI rendering (complex to test) |

### Requirement: 自動更新機能

| Scenario | Test | Status |
|----------|------|--------|
| 定期的な自動更新 | `src/tui.rs::test_should_refresh_after_interval` | ✅ **NEW** |
| 自動更新インジケーター | - | ⚠️ UI rendering |
| 更新中の表示継続 | - | ⚠️ UI rendering |

### Requirement: 新規変更検出

| Scenario | Test | Status |
|----------|------|--------|
| 新規変更の検出 | `src/tui.rs::test_update_changes_detects_new` | ✅ Covered |
| 新規変更のデフォルト状態 | `src/tui.rs::test_update_changes_marks_new_changes_correctly` | ✅ **NEW** |
| NEWバッジの表示 | `src/tui.rs::test_new_badge_state_tracking` | ✅ **NEW** |
| 新規変更件数の追跡 | `src/tui.rs::test_new_change_count_tracking` | ✅ **NEW** |

### Requirement: 動的実行キュー

| Scenario | Test | Status |
|----------|------|--------|
| 実行中のキュー追加 | `src/tui.rs::test_toggle_selection_adds_to_queue_after_removal_in_running_mode` | ✅ Covered |
| キュー待機中の変更を解除 | `src/tui.rs::test_toggle_selection_removes_from_queue_in_running_mode` | ✅ Covered |
| キュー追加後の処理順序 | - | ⚠️ Implicit in queue logic |
| 処理中の変更は変更不可 | `src/tui.rs::test_toggle_selection_does_nothing_for_processing_status` | ✅ Covered |
| アーカイブ中の変更は変更不可 | `src/tui.rs::test_toggle_selection_does_nothing_for_archived_status` | ✅ Covered |

### Requirement: エラー状態の表示

| Scenario | Test | Status |
|----------|------|--------|
| エラー発生時のモード遷移 | `src/tui.rs::test_processing_error_transitions_to_error_mode` | ✅ Covered |
| ステータスパネルのエラー表示 | - | ⚠️ UI rendering |
| エラー状態でのChange表示 | `src/tui.rs::test_toggle_selection_does_nothing_for_error_status` | ✅ Covered |

### Requirement: F5キーでのエラーリトライ

| Scenario | Test | Status |
|----------|------|--------|
| F5キーでリトライ開始 | `src/tui.rs::test_retry_error_changes_from_error_mode` | ✅ Covered |
| リトライ時のログ表示 | `src/tui.rs::test_retry_logs_retrying_message` | ✅ Covered |
| リトライ成功後の状態 | - | ⚠️ Integration test needed |

---

## Configuration Specification (`openspec/specs/configuration/spec.md`)

### Requirement: Environment Variable Configuration for OpenSpec Command

| Scenario | Test | Status |
|----------|------|--------|
| 環境変数のみ設定 | `src/cli.rs::test_env_var_openspec_cmd` | ✅ Covered |
| CLI 引数が環境変数より優先 | `src/cli.rs::test_cli_arg_overrides_env_var` | ✅ Covered |
| どちらも未設定時はデフォルト値を使用 | `src/cli.rs::test_default_openspec_cmd` | ✅ Covered |

### Requirement: エージェントコマンドの設定ファイル

| Scenario | Test | Status |
|----------|------|--------|
| プロジェクト設定ファイルが存在する場合 | `src/config.rs::test_get_commands_with_custom_values` | ✅ Covered |
| 設定ファイルが存在しない場合のフォールバック | `src/config.rs::test_get_commands_with_defaults` | ✅ Covered |
| 部分的な設定のフォールバック | `src/config.rs::test_partial_config_with_fallback` | ✅ Covered |

### Requirement: 設定ファイルの優先順位

| Scenario | Test | Status |
|----------|------|--------|
| プロジェクト設定がグローバル設定より優先される | `src/config.rs::test_load_project_config_takes_priority` | ✅ **NEW** |
| プロジェクト設定がない場合はグローバル設定を使用 | `src/config.rs::test_load_returns_default_when_no_config_exists` | ✅ **NEW** |
| カスタムパスからの設定読み込み | `src/config.rs::test_load_from_custom_path` | ✅ **NEW** |

### Requirement: プレースホルダーの展開

| Scenario | Test | Status |
|----------|------|--------|
| {change_id} プレースホルダーの正常な展開 | `src/config.rs::test_expand_change_id` | ✅ Covered |
| 複数の {change_id} プレースホルダー | `src/config.rs::test_expand_change_id_multiple` | ✅ Covered |
| {prompt} プレースホルダーの展開 | `src/config.rs::test_expand_prompt` | ✅ Covered |

### Requirement: 依存関係分析コマンドの設定

| Scenario | Test | Status |
|----------|------|--------|
| カスタム分析コマンドの使用 | `src/agent.rs::test_analyze_dependencies_echo_command` | ✅ Covered |
| 分析コマンド未設定時のフォールバック | `src/config.rs::test_get_commands_with_defaults` | ✅ Covered |

### Requirement: JSONC 形式のサポート

| Scenario | Test | Status |
|----------|------|--------|
| コメント付き設定ファイルの解析 | `src/config.rs::test_parse_jsonc_with_single_line_comments` | ✅ Covered |
| | `src/config.rs::test_parse_jsonc_with_multi_line_comments` | ✅ Covered |
| 末尾カンマの許容 | `src/config.rs::test_parse_jsonc_with_trailing_comma` | ✅ Covered |

---

## Test Statistics

- **Total Tests**: 117 (101 unit + 13 e2e + 3 compatibility)
- **Covered Scenarios**: ~42 out of ~50 specification scenarios
- **Coverage Percentage**: ~84% of specification scenarios have tests
- **Line Coverage**: 56.77% (+5.15% from baseline)

---

## New Tests Added in This Analysis

### config.rs (+3 tests)
- `test_load_from_custom_path` - Custom config path loading
- `test_load_returns_default_when_no_config_exists` - Default fallback
- `test_load_project_config_takes_priority` - Project vs global priority

### tui.rs (+5 tests)
- `test_should_refresh_after_interval` - Auto-refresh timing
- `test_new_badge_state_tracking` - NEW badge state management
- `test_update_changes_marks_new_changes_correctly` - New change detection
- `test_new_change_count_tracking` - New change count
- `test_change_state_is_new_default_false` - Default is_new state

### orchestrator.rs (+5 tests)
- `test_build_analysis_prompt_format` - Prompt format verification
- `test_build_analysis_prompt_with_empty_changes` - Empty changes handling
- `test_build_analysis_prompt_with_single_change` - Single change handling
- `test_orchestrator_creation` - Orchestrator initialization
- `test_orchestrator_with_target_change` - Target change setting

### progress.rs (+9 tests)
- `test_progress_complete_change` - Change completion
- `test_progress_archive_change` - Archive operation
- `test_progress_error` - Error display
- `test_progress_complete_all` - Complete all
- `test_progress_set_message` - Message setting
- `test_progress_multiple_updates` - Multiple updates
- `test_progress_complete_without_current` - Complete edge case
- `test_progress_archive_without_current` - Archive edge case
- `test_progress_error_without_current` - Error edge case

---

## Remaining Gaps

### UI Rendering Tests
UI rendering code in `tui.rs` is difficult to unit test due to ratatui dependencies.
Consider:
- Snapshot testing for UI components
- Integration tests with mock terminal
- Manual testing for visual verification

### Async Process Tests
`opencode.rs` and parts of `orchestrator.rs` spawn external processes.
Consider:
- Integration tests with mock scripts
- Contract tests for command interfaces
- Manual testing for end-to-end scenarios

### Legacy Code
`opencode.rs` is marked as legacy and is being replaced by `agent.rs`.
No additional tests recommended for legacy code.
