# Graph Report - exclude-generated-dashboard-assets-from-fixers  (2026-04-29)

## Corpus Check
- 228 files · ~1,255,225 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 4254 nodes · 11882 edges · 69 communities detected
- Extraction: 63% EXTRACTED · 37% INFERRED · 0% AMBIGUOUS · INFERRED: 4410 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 39|Community 39]]
- [[_COMMUNITY_Community 40|Community 40]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 44|Community 44]]
- [[_COMMUNITY_Community 45|Community 45]]
- [[_COMMUNITY_Community 50|Community 50]]
- [[_COMMUNITY_Community 75|Community 75]]
- [[_COMMUNITY_Community 76|Community 76]]
- [[_COMMUNITY_Community 77|Community 77]]
- [[_COMMUNITY_Community 100|Community 100]]
- [[_COMMUNITY_Community 116|Community 116]]
- [[_COMMUNITY_Community 117|Community 117]]
- [[_COMMUNITY_Community 118|Community 118]]
- [[_COMMUNITY_Community 119|Community 119]]
- [[_COMMUNITY_Community 120|Community 120]]
- [[_COMMUNITY_Community 121|Community 121]]
- [[_COMMUNITY_Community 122|Community 122]]
- [[_COMMUNITY_Community 123|Community 123]]
- [[_COMMUNITY_Community 124|Community 124]]
- [[_COMMUNITY_Community 125|Community 125]]
- [[_COMMUNITY_Community 126|Community 126]]
- [[_COMMUNITY_Community 127|Community 127]]
- [[_COMMUNITY_Community 128|Community 128]]
- [[_COMMUNITY_Community 129|Community 129]]
- [[_COMMUNITY_Community 130|Community 130]]
- [[_COMMUNITY_Community 131|Community 131]]
- [[_COMMUNITY_Community 132|Community 132]]
- [[_COMMUNITY_Community 133|Community 133]]
- [[_COMMUNITY_Community 134|Community 134]]
- [[_COMMUNITY_Community 135|Community 135]]
- [[_COMMUNITY_Community 136|Community 136]]

## God Nodes (most connected - your core abstractions)
1. `execute_acceptance_in_workspace()` - 62 edges
2. `run_orchestrator()` - 58 edges
3. `main()` - 56 edges
4. `create_test_app()` - 50 edges
5. `render_buffer()` - 50 edges
6. `run_tui_loop()` - 49 edges
7. `OrchestratorState` - 49 edges
8. `buffer_to_string()` - 46 edges
9. `GitWorkspaceManager` - 44 edges
10. `run_git()` - 44 edges

## Surprising Connections (you probably didn't know these)
- `test_blocked_rejection_flow_end_to_end_creates_marker_and_removes_worktree()` --calls--> `execute_rejection_flow()`  [INFERRED]
  tests/e2e_git_worktree_tests.rs → src/orchestration/rejection.rs
- `Handler` --calls--> `with_deduplicator()`  [INFERRED]
  tests/fixtures/mock_opencode_server.py → src/tui/log_deduplicator.rs
- `fetchAPI()` --calls--> `parse()`  [INFERRED]
  dashboard/src/api/restClient.ts → src/config/jsonc.rs
- `_make_manager()` --calls--> `OpenSpecManager`  [INFERRED]
  skills/tests/test_spec_only_acceptance.py → src/openspec_cmd.rs
- `test_list_changes_ignores_invalid_dir()` --calls--> `list_changes()`  [INFERRED]
  skills/tests/test_cflx_list_ignores_invalid_change_dirs.py → src/web/api.rs

## Communities

### Community 0 - "Community 0"
Cohesion: 0.01
Nodes (318): AcpClient, create_shared_active_commands(), archive_entry_exists(), ArchiveEventHandler, ArchiveLoopResult, ArchiveVerificationResult, delete_change_directory(), ensure_archive_commit() (+310 more)

### Community 1 - "Community 1"
Cohesion: 0.02
Nodes (246): execute_worktree_command(), main(), handle_start_processing_command(), handle_tui_command(), TuiCommandContext, AppState, ParallelExecutor, AppState (+238 more)

### Community 2 - "Community 2"
Cohesion: 0.02
Nodes (168): Colors, main(), OpenSpecManager, print_change_detail(), print_changes(), Extract change information from directory., Count completed and total tasks., Show detailed information about a change. (+160 more)

### Community 3 - "Community 3"
Cohesion: 0.02
Nodes (131): AcpMessage, create_worktree(), delete_worktree(), merge_worktree(), BaseHTTPRequestHandler, branch_delete(), checkout(), get_logs() (+123 more)

### Community 4 - "Community 4"
Cohesion: 0.02
Nodes (163): AcpContent, AcpElicitationParams, AcpError, AcpEvent, AcpPromptBlock, AcpUpdateParams, dispatch_jsonrpc_response(), dispatch_jsonrpc_response_ignores_non_u64_id_without_consuming_waiters() (+155 more)

### Community 5 - "Community 5"
Cohesion: 0.02
Nodes (126): acceptance_test_streaming(), apply_change(), apply_change_streaming(), ApplyContext, ApplyResult, test_apply_context_new(), archive_change(), archive_change_streaming() (+118 more)

### Community 6 - "Community 6"
Cohesion: 0.03
Nodes (107): delete_acceptance_state(), get_changed_files(), get_current_commit(), default_retry_patterns(), ReanalysisReason, should_reanalyze_queue(), send_event(), commit_workspace_change() (+99 more)

### Community 7 - "Community 7"
Cohesion: 0.02
Nodes (162): default_server_data_dir(), OrchestratorConfig, merge(), env_test_lock(), get_global_config_path(), get_global_config_paths(), get_platform_config_path(), get_xdg_config_path() (+154 more)

### Community 8 - "Community 8"
Cohesion: 0.02
Nodes (132): build_apply_prompt(), expand_apply_command(), test_openspec_directory_structure(), escape_for_single_quoted_context(), escape_shell_value(), expand_change_id(), expand_conflict_files(), expand_placeholder() (+124 more)

### Community 9 - "Community 9"
Cohesion: 0.04
Nodes (44): test_archive_change_retries_until_verified(), ParallelExecutor, AutoResolveGuard, build_conflict_resolve_prompt(), build_sequential_merge_resolve_prompt(), detect_conflicts(), get_vcs_log_for_revisions(), get_vcs_status() (+36 more)

### Community 10 - "Community 10"
Cohesion: 0.04
Nodes (105): ChangeStateSnapshot, configure_logging(), global_deduplicator(), LogDeduplicator, maybe_log_summary(), should_log_change_count(), should_log_task_progress(), test_config() (+97 more)

### Community 11 - "Community 11"
Cohesion: 0.02
Nodes (45): check_git_available(), check_git_directory(), check_parallel_available(), CheckConflictsArgs, Cli, Commands, EvidenceMode, InitArgs (+37 more)

### Community 12 - "Community 12"
Cohesion: 0.03
Nodes (94): ArchiveResult, extract_tree_branch(), parse_project_url(), RemoteClient, resolve_default_branch(), resolve_project_url_and_branch(), test_add_project_no_auth_header(), test_authorization_header_sent_with_token() (+86 more)

### Community 13 - "Community 13"
Cohesion: 0.04
Nodes (72): dispatch_event(), post_archive_dispatch_event(), run_orchestrator(), test_archive_path_structure(), test_archive_verification_logic(), test_tui_archived_during_resolve(), test_tui_archived_no_active_resolve(), test_tui_shared_state_pending_changes_decrease_when_cleared() (+64 more)

### Community 14 - "Community 14"
Cohesion: 0.04
Nodes (63): append_recovery_task_section(), cleanup_worktree(), execute_rejection_flow(), extract_rejected_reason(), handle_resume_apply_from_rejecting(), has_rejection_proposal(), init_git_repo(), parse_rejection_review_output() (+55 more)

### Community 15 - "Community 15"
Cohesion: 0.05
Nodes (75): control_cancel_stop(), control_force_stop(), control_retry(), control_start(), control_stop(), ControlResponse, create_test_change(), CreateWorktreeRequest (+67 more)

### Community 16 - "Community 16"
Cohesion: 0.04
Nodes (57): apply_completion_check_interval(), apply_completion_grace_period(), ApplyBlockedHandoff, ApplyCompletionKind, ApplyConfig, ApplyEventHandler, ApplyIterationResult, ApplyLoopHookContext (+49 more)

### Community 17 - "Community 17"
Cohesion: 0.05
Nodes (55): AnalysisResult, AnalyzePromptMetadata, create_test_analyzer(), create_test_change(), ParallelGroup, ParallelizationAnalyzer, test_build_prompt_all_selected(), test_build_prompt_includes_frontmatter_metadata_context() (+47 more)

### Community 18 - "Community 18"
Cohesion: 0.03
Nodes (30): AcceptanceResult, build_acceptance_tail_findings(), canonical_verdict_kind(), detect_verdict_in_line(), parse_acceptance_output(), parse_findings(), parse_json_verdict(), strip_markdown_decorations() (+22 more)

### Community 19 - "Community 19"
Cohesion: 0.06
Nodes (49): AiCommandRunner, OutputLine, test_inactivity_timeout_retry(), test_inactivity_timeout_streaming_pipeline(), test_post_completion_cleanup_on_cancellation(), test_post_completion_cleanup_on_failure(), test_post_completion_cleanup_on_success(), test_shared_stagger_state() (+41 more)

### Community 20 - "Community 20"
Cohesion: 0.05
Nodes (63): extract_assistant_tool_summary(), extract_from_assistant(), extract_from_result(), extract_from_stream_event(), extract_text_from_stream_json(), extract_tool_result_summary(), extract_tool_summary_from_stream_json(), extract_tool_use_summary() (+55 more)

### Community 21 - "Community 21"
Cohesion: 0.06
Nodes (53): acceptance_resume_ready_for_archive(), acceptance_state_can_be_deleted(), acceptance_state_is_not_created_under_worktree(), acceptance_state_path(), acceptance_state_root_dir(), acceptance_state_roundtrip(), AcceptanceState, AcceptanceStateStatus (+45 more)

### Community 22 - "Community 22"
Cohesion: 0.07
Nodes (34): commit(), detect_workspace_state(), get_latest_wip_snapshot(), has_apply_commit(), has_archive_files(), init_git_repo(), is_merged_to_base(), test_detect_workspace_state_applied() (+26 more)

### Community 23 - "Community 23"
Cohesion: 0.09
Nodes (23): create_test_config(), test_auto_resolve_counter_is_thread_safe(), test_auto_resolve_counter_reduces_available_slots(), test_combined_manual_and_auto_resolve_slots(), test_multiple_auto_resolves_consume_multiple_slots(), create_test_config(), test_manual_resolve_completion_notifies_scheduler(), test_manual_resolve_counter_reduces_available_slots() (+15 more)

### Community 24 - "Community 24"
Cohesion: 0.1
Nodes (7): confirmDeleteWorktree(), deleteWorktree(), escapeHtml(), fetchWorktrees(), mergeWorktree(), renderWorktrees(), WebMonitor

### Community 25 - "Community 25"
Cohesion: 0.09
Nodes (33): addProject(), APIError, controlRun(), controlStop(), createProposalSession(), createTerminalSession(), createWorktree(), deleteProject() (+25 more)

### Community 26 - "Community 26"
Cohesion: 0.07
Nodes (7): MockWorkspaceManager, test_detect_conflicts_no_conflicts(), test_detect_conflicts_with_conflicts(), test_get_vcs_log_for_revisions(), test_get_vcs_status(), test_resolve_merges_with_retry_args_clone(), test_resolve_merges_with_retry_args_struct()

### Community 27 - "Community 27"
Cohesion: 0.07
Nodes (14): test_workspace_status_failed_includes_message(), ExecutionContext, ExecutionContext<'a>, ExecutionResult, ProgressInfo, test_execution_context_is_parallel(), test_execution_context_new(), test_execution_context_working_dir() (+6 more)

### Community 28 - "Community 28"
Cohesion: 0.21
Nodes (17): test_cleanup_guard_all_preserved_does_nothing(), test_cleanup_guard_auto_backend_treated_as_git(), test_cleanup_guard_commit_enables_cleanup(), test_cleanup_guard_commit_on_success(), test_cleanup_guard_creation(), test_cleanup_guard_drop_with_empty_workspaces_does_nothing(), test_cleanup_guard_drop_without_commit_preserves_workspaces(), test_cleanup_guard_git_backend() (+9 more)

### Community 29 - "Community 29"
Cohesion: 0.17
Nodes (20): test_japanese_log_preview_truncation_no_panic(), restore_terminal(), clear_screen(), EditorTarget, find_change_dir(), get_version_string(), launch_editor_for_change(), make_temp_dir() (+12 more)

### Community 30 - "Community 30"
Cohesion: 0.19
Nodes (10): ActiveCommandGuard, ActiveCommandRegistry, make_key(), RootKind, test_acquire_and_release(), test_different_roots_independent(), test_double_acquire_fails(), test_guard_release_async() (+2 more)

### Community 31 - "Community 31"
Cohesion: 0.22
Nodes (12): CircuitBreakerConfig, ErrorHistory, normalize_error_message(), test_circuit_breaker_disabled(), test_clear_history(), test_detect_same_error_different_errors(), test_detect_same_error_with_threshold(), test_last_error() (+4 more)

### Community 32 - "Community 32"
Cohesion: 0.13
Nodes (2): MockWebSocket, WebSocketClient

### Community 33 - "Community 33"
Cohesion: 0.2
Nodes (15): _blocks_equal(), delta_to_canonical(), merge_spec_delta(), parse_delta_sections(), Shared spec promotion engine for Conflux archive workflow.  Provides requirement, Split spec content into (preamble, [(normalized_key, full_block), ...]).      fu, Simulate spec promotion without writing any files.      Returns (result_content,, Convert a delta-format spec to canonical format for brand-new specs. (+7 more)

### Community 34 - "Community 34"
Cohesion: 0.21
Nodes (6): get_cflx_embedded_skills(), test_embedded_skills_count(), test_embedded_skills_have_auxiliary_files(), test_embedded_skills_names(), test_rust_prompt_builder_does_not_contain_acceptance_checklist(), get_archive_readiness_context()

### Community 36 - "Community 36"
Cohesion: 0.29
Nodes (6): AppMode, MergeConflictInfo, StopMode, ViewMode, WorktreeAction, WorktreeInfo

### Community 37 - "Community 37"
Cohesion: 0.33
Nodes (2): TuiCommand, TuiEventSink

### Community 38 - "Community 38"
Cohesion: 0.6
Nodes (5): generate_qr_string(), test_generate_qr_string_empty_url(), test_generate_qr_string_short_url(), test_generate_qr_string_valid_url(), test_generate_qr_string_without_feature()

### Community 39 - "Community 39"
Cohesion: 0.33
Nodes (1): NoOpArchiveEventHandler

### Community 40 - "Community 40"
Cohesion: 0.7
Nodes (4): formatValue(), logHelperState(), onInput(), onKeydown()

### Community 42 - "Community 42"
Cohesion: 0.4
Nodes (1): dashboard_assets()

### Community 44 - "Community 44"
Cohesion: 0.67
Nodes (3): create_skill_structure(), main(), Create the skill directory structure with template files.

### Community 45 - "Community 45"
Cohesion: 0.83
Nodes (3): make_change_state(), parallel_mode_excludes_uncommitted_rows_from_bulk_toggle(), running_mode_excludes_active_rows_from_bulk_toggle()

### Community 50 - "Community 50"
Cohesion: 0.67
Nodes (1): ApiDoc

### Community 75 - "Community 75"
Cohesion: 1.0
Nodes (1): Configure sys.path so shared modules are importable from tests.

### Community 76 - "Community 76"
Cohesion: 1.0
Nodes (1): Result<T, E>

### Community 77 - "Community 77"
Cohesion: 1.0
Nodes (1): OutputLine

### Community 100 - "Community 100"
Cohesion: 1.0
Nodes (1): Emit warning for invalid change directory.

### Community 116 - "Community 116"
Cohesion: 1.0
Nodes (1): ANSI color codes for terminal output.

### Community 117 - "Community 117"
Cohesion: 1.0
Nodes (1): Manage OpenSpec changes and specifications.

### Community 118 - "Community 118"
Cohesion: 1.0
Nodes (1): Warn about obsolete OpenSpec artifacts that should be removed.

### Community 119 - "Community 119"
Cohesion: 1.0
Nodes (1): Return True when directory is a valid change (has proposal.md).

### Community 120 - "Community 120"
Cohesion: 1.0
Nodes (1): Emit warning for invalid change directory.

### Community 121 - "Community 121"
Cohesion: 1.0
Nodes (1): List all changes or specs.

### Community 122 - "Community 122"
Cohesion: 1.0
Nodes (1): Extract change information from directory.

### Community 123 - "Community 123"
Cohesion: 1.0
Nodes (1): Count completed and total tasks.

### Community 124 - "Community 124"
Cohesion: 1.0
Nodes (1): Show detailed information about a change.

### Community 125 - "Community 125"
Cohesion: 1.0
Nodes (1): Find the directory for a given change ID.

### Community 126 - "Community 126"
Cohesion: 1.0
Nodes (1): Validate a change or all changes.

### Community 127 - "Community 127"
Cohesion: 1.0
Nodes (1): Extract Change Type value from proposal.md content.

### Community 128 - "Community 128"
Cohesion: 1.0
Nodes (1): Validate a single change directory.

### Community 129 - "Community 129"
Cohesion: 1.0
Nodes (1): Validate tasks.md file format.

### Community 130 - "Community 130"
Cohesion: 1.0
Nodes (1): Validate spec delta files.

### Community 131 - "Community 131"
Cohesion: 1.0
Nodes (1): Emit archive-risk warnings for spec-only proposals with only MODIFIED/REMOVED de

### Community 132 - "Community 132"
Cohesion: 1.0
Nodes (1): Archive a deployed change.

### Community 133 - "Community 133"
Cohesion: 1.0
Nodes (1): Simulate spec promotion and return errors without writing any files.

### Community 134 - "Community 134"
Cohesion: 1.0
Nodes (1): Update canonical specs from change deltas (simulation already passed).

### Community 135 - "Community 135"
Cohesion: 1.0
Nodes (1): Print changes or specs in a formatted way.

### Community 136 - "Community 136"
Cohesion: 1.0
Nodes (1): Print detailed change information.

## Knowledge Gaps
- **292 isolated node(s):** `Create the skill directory structure with template files.`, `Validate YAML frontmatter in SKILL.md.`, `Validate skill directory structure.`, `Package skill into a .skill file (zip with .skill extension).`, `Configure sys.path so shared modules are importable from tests.` (+287 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 32`** (17 nodes): `wsClient.ts`, `useProposalChat.test.ts`, `MockWebSocket`, `.close()`, `.constructor()`, `.emitClose()`, `.emitMessage()`, `.emitOpen()`, `WebSocketClient`, `.attemptReconnect()`, `.connect()`, `.constructor()`, `.disconnect()`, `.isConnected()`, `.notifyConnectionChange()`, `.on()`, `.startPingTimer()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 37`** (6 nodes): `TuiCommand`, `TuiEventSink`, `.new()`, `.on_event()`, `.on_state_changed()`, `events.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 39`** (6 nodes): `NoOpArchiveEventHandler`, `.on_archive_output()`, `.on_archive_started()`, `.on_hook_completed()`, `.on_hook_failed()`, `.on_hook_started()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 42`** (5 nodes): `dashboard_assets()`, `dashboard_favicon()`, `dashboard_icons()`, `dashboard_index()`, `dashboard.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 50`** (3 nodes): `ApiDoc`, `main()`, `openapi_gen.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 75`** (2 nodes): `Configure sys.path so shared modules are importable from tests.`, `conftest.py`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 76`** (2 nodes): `Result<T, E>`, `.or_fail()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 77`** (2 nodes): `OutputLine`, `output.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 100`** (1 nodes): `Emit warning for invalid change directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 116`** (1 nodes): `ANSI color codes for terminal output.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 117`** (1 nodes): `Manage OpenSpec changes and specifications.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 118`** (1 nodes): `Warn about obsolete OpenSpec artifacts that should be removed.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 119`** (1 nodes): `Return True when directory is a valid change (has proposal.md).`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 120`** (1 nodes): `Emit warning for invalid change directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 121`** (1 nodes): `List all changes or specs.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 122`** (1 nodes): `Extract change information from directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 123`** (1 nodes): `Count completed and total tasks.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 124`** (1 nodes): `Show detailed information about a change.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 125`** (1 nodes): `Find the directory for a given change ID.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 126`** (1 nodes): `Validate a change or all changes.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 127`** (1 nodes): `Extract Change Type value from proposal.md content.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 128`** (1 nodes): `Validate a single change directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 129`** (1 nodes): `Validate tasks.md file format.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 130`** (1 nodes): `Validate spec delta files.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 131`** (1 nodes): `Emit archive-risk warnings for spec-only proposals with only MODIFIED/REMOVED de`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 132`** (1 nodes): `Archive a deployed change.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 133`** (1 nodes): `Simulate spec promotion and return errors without writing any files.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 134`** (1 nodes): `Update canonical specs from change deltas (simulation already passed).`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 135`** (1 nodes): `Print changes or specs in a formatted way.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 136`** (1 nodes): `Print detailed change information.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `main()` connect `Community 12` to `Community 0`, `Community 1`, `Community 2`, `Community 6`, `Community 7`, `Community 9`, `Community 10`, `Community 11`, `Community 13`, `Community 17`, `Community 19`?**
  _High betweenness centrality (0.033) - this node is a cross-community bridge._
- **Why does `run_server()` connect `Community 0` to `Community 3`, `Community 6`, `Community 7`, `Community 12`, `Community 17`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Why does `run_tui_loop()` connect `Community 9` to `Community 0`, `Community 1`, `Community 3`, `Community 4`, `Community 5`, `Community 6`, `Community 8`, `Community 11`, `Community 12`, `Community 13`, `Community 15`?**
  _High betweenness centrality (0.024) - this node is a cross-community bridge._
- **Are the 57 inferred relationships involving `execute_acceptance_in_workspace()` (e.g. with `.dispatch_change_to_workspace()` and `.is_cancelled()`) actually correct?**
  _`execute_acceptance_in_workspace()` has 57 INFERRED edges - model-reasoned connections that need verification._
- **Are the 55 inferred relationships involving `run_orchestrator()` (e.g. with `handle_start_processing_command()` and `.with_event_tx()`) actually correct?**
  _`run_orchestrator()` has 55 INFERRED edges - model-reasoned connections that need verification._
- **Are the 49 inferred relationships involving `main()` (e.g. with `parse()` and `.load()`) actually correct?**
  _`main()` has 49 INFERRED edges - model-reasoned connections that need verification._
- **Are the 2 inferred relationships involving `create_test_app()` (e.g. with `.new()` and `.clear()`) actually correct?**
  _`create_test_app()` has 2 INFERRED edges - model-reasoned connections that need verification._
