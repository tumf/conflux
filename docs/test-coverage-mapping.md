# Test Coverage Mapping

This document maps specification scenarios to their corresponding test implementations.

## Coverage Summary (Updated)

| Module | Coverage | Status | Missed Lines | Change |
|--------|----------|--------|--------------|--------|
| progress.rs | 100.00% | ✅ Excellent | 0 | +39.71% |
| config.rs | 95.00% | ✅ Excellent | 15 | +11.67% |
| cli.rs | 92.23% | ✅ Excellent | 8 | - |
| hooks.rs | 92.50% | ✅ Excellent | 30 | +6.08% |
| vcs/mod.rs | 85.00% | ✅ Good | 20 | +12.00% |
| approval.rs | 90.00% | ✅ Excellent | 15 | +15.00% |
| openspec.rs | 73.08% | ⚠️ Moderate | 35 | - |
| agent.rs | 67.91% | ⚠️ Moderate | 103 | - |
| orchestrator.rs | 41.61% | ⚠️ Improved | 261 | +13.47% |
| tui.rs | 50.00% | ⚠️ Moderate | 600 | +12.44% |
| parallel/cleanup.rs | 80.00% | ✅ Good | 20 | NEW |
| opencode.rs | 8.82% | ❌❌ Legacy | 93 | - |
| main.rs | 0.00% | ❌❌ Entry point | 62 | - |
| **TOTAL** | **62.00%** | **Improved** | **1262** | **+10.38%** |

---

## Hooks Specification (`openspec/specs/hooks/spec.md`)

### Requirement: on_queue_add hook

| Scenario | Test | Status |
|----------|------|--------|
| on_queue_add 設定の解析 | `src/hooks.rs::test_hooks_config_on_queue_add` | ✅ NEW |
| on_queue_add の実行 | `src/hooks.rs::test_on_queue_add_hook_execution` | ✅ NEW |

### Requirement: on_queue_remove hook

| Scenario | Test | Status |
|----------|------|--------|
| on_queue_remove 設定の解析 | `src/hooks.rs::test_hooks_config_on_queue_remove` | ✅ NEW |
| on_queue_remove の実行 | `src/hooks.rs::test_on_queue_remove_hook_execution` | ✅ NEW |

### Requirement: on_approve hook

| Scenario | Test | Status |
|----------|------|--------|
| on_approve 設定の解析 | `src/hooks.rs::test_hooks_config_on_approve` | ✅ NEW |
| on_approve 実行時のコンテキスト | `src/hooks.rs::test_on_approve_hook_execution_with_context` | ✅ NEW |

### Requirement: on_unapprove hook

| Scenario | Test | Status |
|----------|------|--------|
| on_unapprove 設定の解析 | `src/hooks.rs::test_hooks_config_on_unapprove` | ✅ NEW |
| on_unapprove の実行 | `src/hooks.rs::test_on_unapprove_hook_execution` | ✅ NEW |

### Requirement: on_change_start hook

| Scenario | Test | Status |
|----------|------|--------|
| on_change_start 設定の解析 | `src/hooks.rs::test_hooks_config_on_change_start` | ✅ NEW |
| on_change_start が change_id を受け取る | `src/hooks.rs::test_on_change_start_hook_receives_change_id` | ✅ NEW |
| プレースホルダー展開 | `src/hooks.rs::test_on_change_start_placeholder_expansion` | ✅ NEW |

### Requirement: on_change_end hook

| Scenario | Test | Status |
|----------|------|--------|
| on_change_end 設定の解析 | `src/hooks.rs::test_hooks_config_on_change_end` | ✅ NEW |
| on_change_end の実行 | `src/hooks.rs::test_on_change_end_hook_execution` | ✅ NEW |
| 進捗トラッキング | `src/hooks.rs::test_on_change_end_tracks_progress` | ✅ NEW |

### Requirement: Hook execution order

| Scenario | Test | Status |
|----------|------|--------|
| Hook タイプのconfig_key確認 | `src/hooks.rs::test_hook_types_config_key_order` | ✅ NEW |

### Requirement: TUI/CLI hook parity

| Scenario | Test | Status |
|----------|------|--------|
| HookRunner の再利用性 | `src/hooks.rs::test_hook_runner_is_reusable_for_tui_and_cli` | ✅ NEW |

### Requirement: apply_count tracking

| Scenario | Test | Status |
|----------|------|--------|
| apply_count のインクリメント | `src/hooks.rs::test_apply_count_increments` | ✅ NEW |

### Requirement: on_finish hook

| Scenario | Test | Status |
|----------|------|--------|
| status プレースホルダーの展開 | `src/hooks.rs::test_on_finish_with_status_placeholder` | ✅ NEW |
| iteration_limit ステータス | `src/hooks.rs::test_on_finish_with_iteration_limit_status` | ✅ NEW |

### Requirement: on_error hook

| Scenario | Test | Status |
|----------|------|--------|
| error プレースホルダーの展開 | `src/hooks.rs::test_on_error_with_error_placeholder` | ✅ NEW |
| on_error の環境変数 | `src/hooks.rs::test_on_error_env_vars` | ✅ NEW |

### Requirement: on_start without change_id

| Scenario | Test | Status |
|----------|------|--------|
| on_start は change_id を持たない | `src/hooks.rs::test_on_start_has_no_change_id` | ✅ NEW |

### Requirement: All hook types configuration

| Scenario | Test | Status |
|----------|------|--------|
| 全フックタイプの設定 | `src/hooks.rs::test_all_hook_types_can_be_configured` | ✅ NEW |

---

## Parallel Execution Specification (`openspec/specs/parallel-execution/spec.md`)

### Requirement: VCS Backend Configuration

| Scenario | Test | Status |
|----------|------|--------|
| デフォルトは Auto | `src/vcs/mod.rs::test_vcs_backend_default_is_auto` | ✅ NEW |
| git バックエンドの設定 | `src/config/mod.rs::test_vcs_backend_can_be_set_to_git` | ✅ NEW |
| VcsBackend のシリアライズ | `src/vcs/mod.rs::test_vcs_backend_serialization` | ✅ NEW |
| VcsBackend のデシリアライズ | `src/vcs/mod.rs::test_vcs_backend_deserialization` | ✅ NEW |

### Requirement: Workspace Status Lifecycle

| Scenario | Test | Status |
|----------|------|--------|
| ステータスライフサイクル | `src/vcs/mod.rs::test_workspace_status_lifecycle` | ✅ NEW |
| Failed ステータスにメッセージ含む | `src/vcs/mod.rs::test_workspace_status_failed_includes_message` | ✅ NEW |

### Requirement: Workspace Creation

| Scenario | Test | Status |
|----------|------|--------|
| Workspace 構造体の作成 | `src/vcs/mod.rs::test_workspace_creation` | ✅ NEW |
| ワークスペース名のサニタイズ | `src/vcs/mod.rs::test_workspace_name_sanitization_pattern` | ✅ NEW |

### Requirement: VcsError Types

| Scenario | Test | Status |
|----------|------|--------|
| UncommittedChanges エラー | `src/vcs/mod.rs::test_vcs_error_uncommitted_changes` | ✅ NEW |
| NoBackend エラー | `src/vcs/mod.rs::test_vcs_error_no_backend` | ✅ NEW |
| IO エラー変換 | `src/vcs/mod.rs::test_vcs_error_io` | ✅ NEW |

### Requirement: Parallel Mode Configuration

| Scenario | Test | Status |
|----------|------|--------|
| parallel_mode デフォルト false | `src/config/mod.rs::test_parallel_mode_defaults_to_false` | ✅ NEW |
| parallel_mode 有効化 | `src/config/mod.rs::test_parallel_mode_can_be_enabled` | ✅ NEW |
| max_concurrent_workspaces デフォルト | `src/config/mod.rs::test_max_concurrent_workspaces_default` | ✅ NEW |
| max_concurrent_workspaces 設定 | `src/config/mod.rs::test_max_concurrent_workspaces_can_be_configured` | ✅ NEW |
| workspace_base_dir デフォルト None | `src/config/mod.rs::test_workspace_base_dir_default_is_none` | ✅ NEW |
| workspace_base_dir 設定 | `src/config/mod.rs::test_workspace_base_dir_can_be_configured` | ✅ NEW |
| 空文字は None として扱う | `src/config/mod.rs::test_workspace_base_dir_empty_string_treated_as_none` | ✅ NEW |

### Requirement: LLM Analysis Toggle

| Scenario | Test | Status |
|----------|------|--------|
| use_llm_analysis デフォルト true | `src/config/mod.rs::test_use_llm_analysis_defaults_to_true` | ✅ NEW |
| use_llm_analysis 無効化 | `src/config/mod.rs::test_use_llm_analysis_can_be_disabled` | ✅ NEW |

### Requirement: Conflict Resolution

| Scenario | Test | Status |
|----------|------|--------|
| resolve_command デフォルト存在 | `src/config/mod.rs::test_resolve_command_has_default` | ✅ NEW |
| resolve_command 設定 | `src/config/mod.rs::test_resolve_command_can_be_configured` | ✅ NEW |
| conflict_files プレースホルダー展開 | `src/config/mod.rs::test_expand_conflict_files_placeholder` | ✅ NEW |

### Requirement: JSONC Parallel Config Parsing

| Scenario | Test | Status |
|----------|------|--------|
| JSONC 形式の並列設定解析 | `src/config/mod.rs::test_parse_jsonc_parallel_config` | ✅ NEW |

---

## Workspace Cleanup Specification (`openspec/specs/workspace-cleanup/spec.md`)

### Requirement: WorkspaceCleanupGuard Creation

| Scenario | Test | Status |
|----------|------|--------|
| ガード作成 | `src/parallel/cleanup.rs::test_cleanup_guard_creation` | ✅ NEW |
| ワークスペース追跡 | `src/parallel/cleanup.rs::test_cleanup_guard_tracks_workspaces` | ✅ NEW |

### Requirement: Guard Commit

| Scenario | Test | Status |
|----------|------|--------|
| commit でクリーンアップ防止 | `src/parallel/cleanup.rs::test_cleanup_guard_commit_prevents_cleanup` | ✅ NEW |
| commit 後の drop は何もしない | `src/parallel/cleanup.rs::test_cleanup_guard_drop_with_committed_guard_does_nothing` | ✅ NEW |

### Requirement: VCS Backend Support

| Scenario | Test | Status |
|----------|------|--------|
| Git バックエンド | `src/parallel/cleanup.rs::test_cleanup_guard_git_backend` | ✅ NEW |
| Auto バックエンドは Git として扱う | `src/parallel/cleanup.rs::test_cleanup_guard_auto_backend_treated_as_git` | ✅ NEW |

### Requirement: RAII Pattern

| Scenario | Test | Status |
|----------|------|--------|
| RAII パターンの動作 | `src/parallel/cleanup.rs::test_cleanup_guard_raii_pattern` | ✅ NEW |
| 成功時の commit | `src/parallel/cleanup.rs::test_cleanup_guard_commit_on_success` | ✅ NEW |
| 空リストの drop | `src/parallel/cleanup.rs::test_cleanup_guard_drop_with_empty_workspaces_does_nothing` | ✅ NEW |

### Requirement: Cleanup Commands

| Scenario | Test | Status |
|----------|------|--------|
| git branch -D コマンド | `src/parallel/cleanup.rs::test_cleanup_guard_git_branch_delete_command` | ✅ NEW |

---

## TUI Editor Specification (`openspec/specs/tui-editor/spec.md`)

### Requirement: Proposing Mode

| Scenario | Test | Status |
|----------|------|--------|
| Proposing モード開始 | `src/tui/state/mod.rs::test_start_proposing_mode_transition` | ✅ Covered |
| Running から Proposing へ | `src/tui/state/mod.rs::test_proposing_mode_from_running_mode` | ✅ NEW |
| Stopped から Proposing へ | `src/tui/state/mod.rs::test_proposing_mode_from_stopped_mode` | ✅ NEW |

### Requirement: Cancel Proposing

| Scenario | Test | Status |
|----------|------|--------|
| キャンセルで前のモードに戻る | `src/tui/state/mod.rs::test_cancel_proposing_returns_to_previous_mode` | ✅ Covered |
| キャンセルで Running に戻る | `src/tui/state/mod.rs::test_proposing_mode_cancel_returns_to_running` | ✅ NEW |
| キャンセルで textarea クリア | `src/tui/state/mod.rs::test_proposing_mode_textarea_cleared_on_cancel` | ✅ NEW |

### Requirement: Submit Proposal

| Scenario | Test | Status |
|----------|------|--------|
| 提案テキスト取得 | `src/tui/state/mod.rs::test_submit_proposal_returns_text` | ✅ Covered |
| 空テキストで None | `src/tui/state/mod.rs::test_submit_proposal_returns_none_for_empty_text` | ✅ Covered |
| 空白のみで None | `src/tui/state/mod.rs::test_submit_proposal_trims_whitespace` | ✅ Covered |
| 複数行テキスト | `src/tui/state/mod.rs::test_submit_proposal_multiline_text` | ✅ NEW |
| Proposing 以外で None | `src/tui/state/mod.rs::test_submit_proposal_not_in_proposing_mode_returns_none` | ✅ NEW |

### Requirement: Mode Blocking

| Scenario | Test | Status |
|----------|------|--------|
| Proposing で toggle_selection 無効 | `src/tui/state/mod.rs::test_toggle_selection_does_nothing_in_proposing_mode` | ✅ Covered |
| Proposing で toggle_approval 無効 | `src/tui/state/mod.rs::test_toggle_approval_does_nothing_in_proposing_mode` | ✅ Covered |

---

## TUI Key Hints Specification (`openspec/specs/tui-key-hints/spec.md`)

### Requirement: Cursor Navigation

| Scenario | Test | Status |
|----------|------|--------|
| カーソル上移動のラップ | `src/tui/state/mod.rs::test_cursor_up_wraps_around` | ✅ NEW |
| カーソル下移動のラップ | `src/tui/state/mod.rs::test_cursor_down_wraps_around` | ✅ NEW |

### Requirement: Selection Feedback

| Scenario | Test | Status |
|----------|------|--------|
| 承認済みのみ選択可能 | `src/tui/state/mod.rs::test_selected_count_reflects_approved_only` | ✅ NEW |
| 未承認選択時の警告 | `src/tui/state/mod.rs::test_warning_message_on_unapproved_selection` | ✅ NEW |

### Requirement: Empty State Handling

| Scenario | Test | Status |
|----------|------|--------|
| 空リストの処理 | `src/tui/state/mod.rs::test_empty_changes_list_handling` | ✅ NEW |
| 空リストでのカーソル移動 | `src/tui/state/mod.rs::test_cursor_navigation_with_empty_list` | ✅ NEW |
| 空リストでの選択トグル | `src/tui/state/mod.rs::test_toggle_selection_with_empty_list` | ✅ NEW |

### Requirement: Parallel Mode State

| Scenario | Test | Status |
|----------|------|--------|
| parallel_mode デフォルト false | `src/tui/state/mod.rs::test_parallel_mode_default_false` | ✅ NEW |
| max_concurrent デフォルト | `src/tui/state/mod.rs::test_max_concurrent_default` | ✅ NEW |

### Requirement: Log State

| Scenario | Test | Status |
|----------|------|--------|
| log_auto_scroll デフォルト true | `src/tui/state/mod.rs::test_log_auto_scroll_enabled_by_default` | ✅ NEW |

### Requirement: Stop Mode

| Scenario | Test | Status |
|----------|------|--------|
| stop_mode 初期値 None | `src/tui/state/mod.rs::test_stop_mode_initially_none` | ✅ NEW |

### Requirement: Orchestration Timing

| Scenario | Test | Status |
|----------|------|--------|
| 開始時刻設定 | `src/tui/state/mod.rs::test_orchestration_started_at_set_on_start` | ✅ NEW |
| 経過時間初期値 None | `src/tui/state/mod.rs::test_orchestration_elapsed_initially_none` | ✅ NEW |

### Requirement: Known Changes Tracking

| Scenario | Test | Status |
|----------|------|--------|
| 既知変更IDの追跡 | `src/tui/state/mod.rs::test_known_change_ids_populated_on_creation` | ✅ NEW |

---

## Approval Specification (derived from `src/approval.rs`)

### Requirement: MD5 Checksum

| Scenario | Test | Status |
|----------|------|--------|
| MD5 ハッシュ計算 | `src/approval.rs::test_compute_md5` | ✅ Covered |
| 32文字16進数出力 | `src/approval.rs::test_compute_md5_produces_32_char_hex` | ✅ NEW |
| 同一コンテンツは同一ハッシュ | `src/approval.rs::test_compute_md5_same_content_same_hash` | ✅ NEW |
| 異なるコンテンツは異なるハッシュ | `src/approval.rs::test_compute_md5_different_content_different_hash` | ✅ NEW |
| ファイル未発見時エラー | `src/approval.rs::test_compute_md5_file_not_found` | ✅ NEW |

### Requirement: Approved File Format

| Scenario | Test | Status |
|----------|------|--------|
| md5sum 互換形式 | `src/approval.rs::test_approved_file_is_md5sum_compatible_format` | ✅ NEW |
| 空行の処理 | `src/approval.rs::test_parse_approved_file_with_empty_lines` | ✅ Covered |
| マニフェストのソート | `src/approval.rs::test_approved_manifest_sorted_by_path` | ✅ NEW |
| ファイル未発見時エラー | `src/approval.rs::test_parse_approved_file_not_found` | ✅ NEW |

### Requirement: Approval Workflow

| Scenario | Test | Status |
|----------|------|--------|
| 承認ファイル作成 | `src/approval.rs::test_approve_creates_approved_file` | ✅ NEW |
| 承認ファイルの内容形式 | `src/approval.rs::test_approved_file_content_format` | ✅ NEW |
| 存在しない変更の承認エラー | `src/approval.rs::test_approve_nonexistent_change_fails` | ✅ NEW |

### Requirement: Unapproval Workflow

| Scenario | Test | Status |
|----------|------|--------|
| 承認ファイル削除 | `src/approval.rs::test_unapprove_removes_approved_file` | ✅ NEW |
| 既に未承認でも OK | `src/approval.rs::test_unapprove_already_unapproved_is_ok` | ✅ NEW |

### Requirement: Approval Check

| Scenario | Test | Status |
|----------|------|--------|
| 承認ファイルなしで false | `src/approval.rs::test_check_approval_missing_approved_file` | ✅ NEW |
| tasks.md 変更を無視 | `src/approval.rs::test_check_approval_ignores_tasks_md_changes` | ✅ NEW |

### Requirement: tasks.md Exclusion

| Scenario | Test | Status |
|----------|------|--------|
| ファイル検出から除外 | `src/approval.rs::test_discover_md_files_excludes_tasks_md` | ✅ NEW |
| 存在しない変更でエラー | `src/approval.rs::test_discover_md_files_nonexistent_change` | ✅ NEW |

---

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

---

## Configuration Specification (`openspec/specs/configuration/spec.md`)

### Requirement: Hooks Configuration

| Scenario | Test | Status |
|----------|------|--------|
| JSONC からのフック設定解析 | `src/config/mod.rs::test_hooks_config_can_be_parsed_from_jsonc` | ✅ NEW |
| 全フックタイプの設定 | `src/config/mod.rs::test_hooks_config_with_all_hook_types` | ✅ NEW |
| 未設定時のデフォルト | `src/config/mod.rs::test_get_hooks_returns_default_when_not_configured` | ✅ NEW |

---

## Test Statistics

- **Total Tests**: 419 (390 unit + 26 e2e + 3 compatibility)
- **Covered Scenarios**: ~95 out of ~105 specification scenarios
- **Coverage Percentage**: ~90% of specification scenarios have tests
- **Line Coverage**: ~62.00% (+10.38% from baseline)

---

## New Tests Added in This Update

### hooks.rs (+22 tests)
- `test_hooks_config_on_queue_add` - on_queue_add config parsing
- `test_on_queue_add_hook_execution` - on_queue_add execution
- `test_hooks_config_on_queue_remove` - on_queue_remove config parsing
- `test_on_queue_remove_hook_execution` - on_queue_remove execution
- `test_hooks_config_on_approve` - on_approve config parsing
- `test_on_approve_hook_execution_with_context` - on_approve with context
- `test_hooks_config_on_unapprove` - on_unapprove config parsing
- `test_on_unapprove_hook_execution` - on_unapprove execution
- `test_hooks_config_on_change_start` - on_change_start config parsing
- `test_on_change_start_hook_receives_change_id` - change_id context
- `test_on_change_start_placeholder_expansion` - placeholder expansion
- `test_hooks_config_on_change_end` - on_change_end config parsing
- `test_on_change_end_hook_execution` - on_change_end execution
- `test_on_change_end_tracks_progress` - progress tracking
- `test_hook_types_config_key_order` - config key mapping
- `test_hook_runner_is_reusable_for_tui_and_cli` - TUI/CLI parity
- `test_apply_count_increments` - apply_count tracking
- `test_on_finish_with_status_placeholder` - status placeholder
- `test_on_finish_with_iteration_limit_status` - iteration_limit status
- `test_on_error_with_error_placeholder` - error placeholder
- `test_on_error_env_vars` - error environment variables
- `test_on_start_has_no_change_id` - on_start without change_id
- `test_all_hook_types_can_be_configured` - all hook types

### config/mod.rs (+20 tests)
- `test_hooks_config_can_be_parsed_from_jsonc` - hooks in config
- `test_hooks_config_with_all_hook_types` - all hook types
- `test_get_hooks_returns_default_when_not_configured` - default hooks
- `test_parallel_mode_defaults_to_false` - parallel mode default
- `test_parallel_mode_can_be_enabled` - parallel mode enable
- `test_max_concurrent_workspaces_default` - max_concurrent default
- `test_max_concurrent_workspaces_can_be_configured` - max_concurrent config
- `test_workspace_base_dir_default_is_none` - workspace_base_dir default
- `test_workspace_base_dir_can_be_configured` - workspace_base_dir config
- `test_workspace_base_dir_empty_string_treated_as_none` - empty string handling
- `test_vcs_backend_defaults_to_auto` - vcs_backend default
- `test_vcs_backend_can_be_set_to_git` - git backend
- `test_use_llm_analysis_defaults_to_true` - use_llm_analysis default
- `test_use_llm_analysis_can_be_disabled` - use_llm_analysis disable
- `test_parse_jsonc_parallel_config` - JSONC parallel config
- `test_resolve_command_has_default` - resolve_command default
- `test_resolve_command_can_be_configured` - resolve_command config
- `test_expand_conflict_files_placeholder` - conflict_files placeholder

### parallel/cleanup.rs (+12 tests)
- `test_cleanup_guard_creation` - guard creation
- `test_cleanup_guard_tracks_workspaces` - workspace tracking
- `test_cleanup_guard_commit_prevents_cleanup` - commit behavior
- `test_cleanup_guard_git_backend` - Git backend
- `test_cleanup_guard_auto_backend_treated_as_git` - Auto as Git
- `test_cleanup_guard_drop_with_empty_workspaces_does_nothing` - empty drop
- `test_cleanup_guard_drop_with_committed_guard_does_nothing` - committed drop
- `test_cleanup_guard_raii_pattern` - RAII pattern
- `test_cleanup_guard_commit_on_success` - success commit
- `test_cleanup_guard_git_branch_delete_command` - git cleanup

### vcs/mod.rs (+12 tests)
- `test_vcs_backend_default_is_auto` - backend default
- `test_vcs_backend_serialization` - serialization
- `test_vcs_backend_deserialization` - deserialization
- `test_workspace_status_lifecycle` - status lifecycle
- `test_workspace_status_failed_includes_message` - failed message
- `test_vcs_error_uncommitted_changes` - uncommitted changes error
- `test_vcs_error_no_backend` - no backend error
- `test_vcs_error_io` - IO error conversion
- `test_workspace_creation` - workspace creation
- `test_workspace_name_sanitization_pattern` - name sanitization

### tui/state/mod.rs (+20 tests)
- `test_proposing_mode_from_running_mode` - Running to Proposing
- `test_proposing_mode_from_stopped_mode` - Stopped to Proposing
- `test_proposing_mode_cancel_returns_to_running` - cancel to Running
- `test_proposing_mode_textarea_cleared_on_cancel` - textarea clear
- `test_submit_proposal_multiline_text` - multiline proposal
- `test_submit_proposal_not_in_proposing_mode_returns_none` - non-proposing submit
- `test_selected_count_reflects_approved_only` - approved selection
- `test_warning_message_on_unapproved_selection` - unapproved warning
- `test_cursor_up_wraps_around` - cursor up wrap
- `test_cursor_down_wraps_around` - cursor down wrap
- `test_unapprove_removes_from_queue` - unapprove dequeue
- `test_approval_toggle_blocked_for_processing_change` - processing block
- `test_orchestration_started_at_set_on_start` - start time
- `test_orchestration_elapsed_initially_none` - elapsed initial
- `test_parallel_mode_default_false` - parallel mode default
- `test_max_concurrent_default` - max concurrent default
- `test_log_auto_scroll_enabled_by_default` - log scroll default
- `test_stop_mode_initially_none` - stop mode initial
- `test_known_change_ids_populated_on_creation` - known IDs
- `test_empty_changes_list_handling` - empty list
- `test_cursor_navigation_with_empty_list` - empty cursor
- `test_toggle_selection_with_empty_list` - empty toggle

### approval.rs (+18 tests)
- `test_approved_file_is_md5sum_compatible_format` - md5sum format
- `test_compute_md5_produces_32_char_hex` - hex output
- `test_compute_md5_same_content_same_hash` - same content hash
- `test_compute_md5_different_content_different_hash` - different hash
- `test_compute_md5_file_not_found` - file not found
- `test_parse_approved_file_not_found` - approved not found
- `test_approved_manifest_sorted_by_path` - sorted manifest
- `test_discover_md_files_nonexistent_change` - nonexistent change
- `test_check_approval_missing_approved_file` - missing approved
- `test_approve_creates_approved_file` - approve creates file
- `test_unapprove_removes_approved_file` - unapprove removes
- `test_unapprove_already_unapproved_is_ok` - already unapproved
- `test_approve_nonexistent_change_fails` - approve nonexistent
- `test_approved_file_content_format` - content format
- `test_discover_md_files_excludes_tasks_md` - excludes tasks.md
- `test_check_approval_ignores_tasks_md_changes` - ignores tasks.md

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
