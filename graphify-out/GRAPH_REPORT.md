# Graph Report - conflux  (2026-04-27)

## Corpus Check
- 229 files · ~876,450 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 4198 nodes · 11744 edges · 42 communities detected
- Extraction: 63% EXTRACTED · 37% INFERRED · 0% AMBIGUOUS · INFERRED: 4359 edges (avg confidence: 0.8)
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
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 43|Community 43]]
- [[_COMMUNITY_Community 68|Community 68]]
- [[_COMMUNITY_Community 69|Community 69]]
- [[_COMMUNITY_Community 70|Community 70]]
- [[_COMMUNITY_Community 93|Community 93]]
- [[_COMMUNITY_Community 94|Community 94]]

## God Nodes (most connected - your core abstractions)
1. `execute_acceptance_in_workspace()` - 61 edges
2. `run_orchestrator()` - 58 edges
3. `main()` - 56 edges
4. `run_tui_loop()` - 49 edges
5. `OrchestratorState` - 49 edges
6. `OpenSpecManager` - 48 edges
7. `create_test_app()` - 48 edges
8. `render_buffer()` - 48 edges
9. `buffer_to_string()` - 44 edges
10. `GitWorkspaceManager` - 44 edges

## Surprising Connections (you probably didn't know these)
- `test_blocked_rejection_flow_end_to_end_creates_marker_and_removes_worktree()` --calls--> `execute_rejection_flow()`  [INFERRED]
  tests/e2e_git_worktree_tests.rs → src/orchestration/rejection.rs
- `proposal_session_create_and_list_use_frontend_contract_shape()` --calls--> `build_router()`  [INFERRED]
  tests/e2e_proposal_session.rs → src/server/api/mod.rs
- `proposal_session_prompt_injects_backend_managed_spec_guidance()` --calls--> `build_router()`  [INFERRED]
  tests/e2e_proposal_session.rs → src/server/api/mod.rs
- `proposal_session_create_does_not_inject_default_opencode_config_env()` --calls--> `build_router()`  [INFERRED]
  tests/e2e_proposal_session.rs → src/server/api/mod.rs
- `proposal_session_ws_accepts_frontend_message_aliases()` --calls--> `build_router()`  [INFERRED]
  tests/e2e_proposal_session.rs → src/server/api/mod.rs

## Communities

### Community 0 - "Community 0"
Cohesion: 0.01
Nodes (205): acceptance_test_streaming(), AiCommandRunner, OutputLine, test_inactivity_timeout_retry(), test_inactivity_timeout_streaming_pipeline(), test_post_completion_cleanup_on_cancellation(), test_post_completion_cleanup_on_failure(), test_post_completion_cleanup_on_success() (+197 more)

### Community 1 - "Community 1"
Cohesion: 0.02
Nodes (261): create_shared_active_commands(), create_worktree(), merge_worktree(), checkout(), main(), resolve_default_branch(), get_logs(), get_project_history() (+253 more)

### Community 2 - "Community 2"
Cohesion: 0.02
Nodes (265): check_task_progress(), archive_entry_exists(), ArchiveEventHandler, ArchiveLoopHookContext, ArchiveLoopResult, ArchiveVerificationResult, delete_change_directory(), ensure_archive_commit() (+257 more)

### Community 3 - "Community 3"
Cohesion: 0.02
Nodes (172): execute_worktree_command(), BaseHTTPRequestHandler, handle_start_processing_command(), handle_tui_command(), TuiCommandContext, AppState, ParallelExecutor, AppState (+164 more)

### Community 4 - "Community 4"
Cohesion: 0.02
Nodes (200): get_changed_files(), get_current_commit(), default_retry_patterns(), acceptance_verdict_grace_period(), commit_workspace_change(), create_test_config(), create_test_config_with(), execute_acceptance_in_workspace() (+192 more)

### Community 5 - "Community 5"
Cohesion: 0.02
Nodes (156): _append_evidence_issue(), Colors, _has_repository_evidence_hint(), _has_runnable_signal(), _has_verification_ownership(), _looks_like_artifact_only_task(), _looks_like_behavior_task(), main() (+148 more)

### Community 6 - "Community 6"
Cohesion: 0.02
Nodes (131): AcpClient, AcpMessage, branch_delete(), extract_tree_branch(), parse_project_url(), RemoteClient, resolve_project_url_and_branch(), test_add_project_no_auth_header() (+123 more)

### Community 7 - "Community 7"
Cohesion: 0.02
Nodes (73): delete_acceptance_state(), make_ai_runner(), test_archive_change_retries_until_verified(), get_conflict_files(), ParallelExecutor, AutoResolveGuard, build_conflict_resolve_prompt(), build_sequential_merge_resolve_prompt() (+65 more)

### Community 8 - "Community 8"
Cohesion: 0.02
Nodes (149): handle_merge_key(), dispatch_event(), post_archive_dispatch_event(), run_orchestrator(), test_archive_path_structure(), test_archive_verification_logic(), test_tui_archived_during_resolve(), test_tui_archived_no_active_resolve() (+141 more)

### Community 9 - "Community 9"
Cohesion: 0.02
Nodes (148): AcpContent, AcpElicitationParams, AcpError, AcpEvent, AcpPromptBlock, AcpUpdateParams, dispatch_jsonrpc_response(), dispatch_jsonrpc_response_ignores_non_u64_id_without_consuming_waiters() (+140 more)

### Community 10 - "Community 10"
Cohesion: 0.02
Nodes (85): apply_completion_check_interval(), apply_completion_grace_period(), ApplyBlockedHandoff, ApplyCompletionKind, ApplyConfig, ApplyEventHandler, ApplyIterationResult, ApplyLoopHookContext (+77 more)

### Community 11 - "Community 11"
Cohesion: 0.03
Nodes (114): ChangeStateSnapshot, configure_logging(), global_deduplicator(), LogDeduplicator, maybe_log_summary(), should_log_change_count(), should_log_task_progress(), test_config() (+106 more)

### Community 12 - "Community 12"
Cohesion: 0.03
Nodes (84): AnalysisResult, AnalyzePromptMetadata, create_test_analyzer(), create_test_change(), ParallelGroup, ParallelizationAnalyzer, test_build_prompt_all_selected(), test_build_prompt_includes_frontmatter_metadata_context() (+76 more)

### Community 13 - "Community 13"
Cohesion: 0.03
Nodes (74): delete_worktree(), test_cleanup_guard_all_preserved_does_nothing(), test_cleanup_guard_auto_backend_treated_as_git(), test_cleanup_guard_commit_enables_cleanup(), test_cleanup_guard_commit_on_success(), test_cleanup_guard_creation(), test_cleanup_guard_drop_with_empty_workspaces_does_nothing(), test_cleanup_guard_drop_without_commit_preserves_workspaces() (+66 more)

### Community 14 - "Community 14"
Cohesion: 0.05
Nodes (75): control_cancel_stop(), control_force_stop(), control_retry(), control_start(), control_stop(), ControlResponse, create_test_change(), CreateWorktreeRequest (+67 more)

### Community 15 - "Community 15"
Cohesion: 0.02
Nodes (31): check_git_available(), check_git_directory(), check_parallel_available(), CheckConflictsArgs, Cli, Commands, EvidenceMode, InitArgs (+23 more)

### Community 16 - "Community 16"
Cohesion: 0.03
Nodes (30): AcceptanceResult, build_acceptance_tail_findings(), canonical_verdict_kind(), detect_verdict_in_line(), parse_acceptance_output(), parse_findings(), parse_json_verdict(), strip_markdown_decorations() (+22 more)

### Community 17 - "Community 17"
Cohesion: 0.05
Nodes (61): extract_assistant_tool_summary(), extract_from_assistant(), extract_from_result(), extract_from_stream_event(), extract_text_from_stream_json(), extract_tool_result_summary(), extract_tool_summary_from_stream_json(), extract_tool_use_summary() (+53 more)

### Community 18 - "Community 18"
Cohesion: 0.04
Nodes (26): format_command_error(), test_vcs_backend_default_is_auto(), test_vcs_backend_deserialization(), test_vcs_error_constructors(), test_vcs_error_io(), test_workspace_creation(), test_workspace_status_failed_includes_message(), VcsBackend (+18 more)

### Community 19 - "Community 19"
Cohesion: 0.1
Nodes (7): confirmDeleteWorktree(), deleteWorktree(), escapeHtml(), fetchWorktrees(), mergeWorktree(), renderWorktrees(), WebMonitor

### Community 20 - "Community 20"
Cohesion: 0.09
Nodes (33): addProject(), APIError, controlRun(), controlStop(), createProposalSession(), createTerminalSession(), createWorktree(), deleteProject() (+25 more)

### Community 21 - "Community 21"
Cohesion: 0.11
Nodes (35): acceptance_resume_ready_for_archive(), acceptance_state_can_be_deleted(), acceptance_state_is_not_created_under_worktree(), acceptance_state_path(), acceptance_state_root_dir(), acceptance_state_roundtrip(), AcceptanceState, AcceptanceStateStatus (+27 more)

### Community 22 - "Community 22"
Cohesion: 0.16
Nodes (22): CommandQueue, CommandQueueConfig, StreamingOutputLine, test_config(), test_execute_with_retry_streaming_failure_no_retry(), test_execute_with_retry_streaming_success(), test_execute_with_retry_streaming_with_callback(), test_inactivity_timeout_error_message_format() (+14 more)

### Community 23 - "Community 23"
Cohesion: 0.09
Nodes (16): AcceptancePromptMode, default_error_circuit_breaker_enabled(), default_error_circuit_breaker_threshold(), default_proposal_session_inactivity_timeout_secs(), default_proposal_transport_args(), default_proposal_transport_command(), default_server_bind(), default_server_data_dir() (+8 more)

### Community 24 - "Community 24"
Cohesion: 0.07
Nodes (1): ResumeTestManager

### Community 25 - "Community 25"
Cohesion: 0.13
Nodes (14): shutdown_signal(), spawn_server(), spawn_server_with_url(), start_server(), test_web_config_auto_assign_port(), test_web_config_default(), test_web_config_enabled(), WebConfig (+6 more)

### Community 26 - "Community 26"
Cohesion: 0.19
Nodes (10): ActiveCommandGuard, ActiveCommandRegistry, make_key(), RootKind, test_acquire_and_release(), test_different_roots_independent(), test_double_acquire_fails(), test_guard_release_async() (+2 more)

### Community 27 - "Community 27"
Cohesion: 0.28
Nodes (14): Template, create_test_change(), ProgressDisplay, test_progress_archive_change(), test_progress_archive_without_current(), test_progress_complete_all(), test_progress_complete_change(), test_progress_complete_without_current() (+6 more)

### Community 28 - "Community 28"
Cohesion: 0.22
Nodes (12): CircuitBreakerConfig, ErrorHistory, normalize_error_message(), test_circuit_breaker_disabled(), test_clear_history(), test_detect_same_error_different_errors(), test_detect_same_error_with_threshold(), test_last_error() (+4 more)

### Community 29 - "Community 29"
Cohesion: 0.22
Nodes (9): FailedChangeTracker, MergeResult, test_failed_tracker_new(), test_mark_failed(), test_should_skip_no_dependencies(), test_should_skip_no_failed_dependency(), test_should_skip_with_failed_dependency(), test_should_skip_with_multiple_dependencies() (+1 more)

### Community 30 - "Community 30"
Cohesion: 0.2
Nodes (15): _blocks_equal(), delta_to_canonical(), merge_spec_delta(), parse_delta_sections(), Shared spec promotion engine for Conflux archive workflow.  Provides requirement, Split spec content into (preamble, [(normalized_key, full_block), ...]).      fu, Simulate spec promotion without writing any files.      Returns (result_content,, Convert a delta-format spec to canonical format for brand-new specs. (+7 more)

### Community 31 - "Community 31"
Cohesion: 0.21
Nodes (6): get_cflx_embedded_skills(), test_embedded_skills_count(), test_embedded_skills_have_auxiliary_files(), test_embedded_skills_names(), test_rust_prompt_builder_does_not_contain_acceptance_checklist(), get_archive_readiness_context()

### Community 32 - "Community 32"
Cohesion: 0.24
Nodes (1): WebSocketClient

### Community 34 - "Community 34"
Cohesion: 0.29
Nodes (6): AppMode, MergeConflictInfo, StopMode, ViewMode, WorktreeAction, WorktreeInfo

### Community 35 - "Community 35"
Cohesion: 0.7
Nodes (4): formatValue(), logHelperState(), onInput(), onKeydown()

### Community 38 - "Community 38"
Cohesion: 0.67
Nodes (3): create_skill_structure(), main(), Create the skill directory structure with template files.

### Community 43 - "Community 43"
Cohesion: 0.67
Nodes (1): ApiDoc

### Community 68 - "Community 68"
Cohesion: 1.0
Nodes (1): Configure sys.path so shared modules are importable from tests.

### Community 69 - "Community 69"
Cohesion: 1.0
Nodes (1): Result<T, E>

### Community 70 - "Community 70"
Cohesion: 1.0
Nodes (1): OutputLine

### Community 93 - "Community 93"
Cohesion: 1.0
Nodes (1): Emit warning for invalid change directory.

### Community 94 - "Community 94"
Cohesion: 1.0
Nodes (1): Emit warning for invalid change directory.

## Knowledge Gaps
- **260 isolated node(s):** `Create the skill directory structure with template files.`, `Validate YAML frontmatter in SKILL.md.`, `Validate skill directory structure.`, `Package skill into a .skill file (zip with .skill extension).`, `Configure sys.path so shared modules are importable from tests.` (+255 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 24`** (28 nodes): `ResumeTestManager`, `.backend_type()`, `.check_available()`, `.cleanup_all()`, `.cleanup_workspace()`, `.conflict_resolution_prompt()`, `.create_iteration_snapshot()`, `.create_workspace()`, `.detect_conflicts()`, `.ensure_original_branch_initialized()`, `.find_existing_workspace()`, `.forget_workspace_sync()`, `.get_current_revision()`, `.get_log_for_revisions()`, `.get_revision_in_workspace()`, `.get_status()`, `.list_worktree_change_ids()`, `.max_concurrent()`, `.merge_workspaces()`, `.original_branch()`, `.prepare_for_parallel()`, `.repo_root()`, `.reuse_workspace()`, `.set_commit_message()`, `.snapshot_working_copy()`, `.squash_wip_commits()`, `.update_workspace_status()`, `.workspaces()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 32`** (10 nodes): `wsClient.ts`, `WebSocketClient`, `.attemptReconnect()`, `.connect()`, `.constructor()`, `.disconnect()`, `.isConnected()`, `.notifyConnectionChange()`, `.on()`, `.startPingTimer()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 43`** (3 nodes): `ApiDoc`, `main()`, `openapi_gen.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 68`** (2 nodes): `Configure sys.path so shared modules are importable from tests.`, `conftest.py`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 69`** (2 nodes): `Result<T, E>`, `.or_fail()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 70`** (2 nodes): `OutputLine`, `output.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 93`** (1 nodes): `Emit warning for invalid change directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 94`** (1 nodes): `Emit warning for invalid change directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `merge_spec_delta()` connect `Community 5` to `Community 0`, `Community 1`, `Community 6`, `Community 7`, `Community 9`, `Community 12`?**
  _High betweenness centrality (0.032) - this node is a cross-community bridge._
- **Why does `run_orchestrator()` connect `Community 8` to `Community 0`, `Community 1`, `Community 2`, `Community 3`, `Community 4`, `Community 6`, `Community 7`, `Community 9`, `Community 10`, `Community 11`, `Community 12`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
- **Why does `run_tui_loop()` connect `Community 3` to `Community 0`, `Community 1`, `Community 2`, `Community 6`, `Community 7`, `Community 8`, `Community 13`, `Community 14`, `Community 15`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
- **Are the 56 inferred relationships involving `execute_acceptance_in_workspace()` (e.g. with `.dispatch_change_to_workspace()` and `.is_cancelled()`) actually correct?**
  _`execute_acceptance_in_workspace()` has 56 INFERRED edges - model-reasoned connections that need verification._
- **Are the 55 inferred relationships involving `run_orchestrator()` (e.g. with `handle_start_processing_command()` and `.with_event_tx()`) actually correct?**
  _`run_orchestrator()` has 55 INFERRED edges - model-reasoned connections that need verification._
- **Are the 49 inferred relationships involving `main()` (e.g. with `parse()` and `.load()`) actually correct?**
  _`main()` has 49 INFERRED edges - model-reasoned connections that need verification._
- **Are the 47 inferred relationships involving `run_tui_loop()` (e.g. with `.new()` and `list_changes_in_head()`) actually correct?**
  _`run_tui_loop()` has 47 INFERRED edges - model-reasoned connections that need verification._
