# Graph Report - conflux  (2026-04-27)

## Corpus Check
- 228 files · ~1,239,275 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 4220 nodes · 11751 edges · 61 communities detected
- Extraction: 63% EXTRACTED · 37% INFERRED · 0% AMBIGUOUS · INFERRED: 4375 edges (avg confidence: 0.8)
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
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 67|Community 67]]
- [[_COMMUNITY_Community 68|Community 68]]
- [[_COMMUNITY_Community 69|Community 69]]
- [[_COMMUNITY_Community 92|Community 92]]
- [[_COMMUNITY_Community 108|Community 108]]
- [[_COMMUNITY_Community 109|Community 109]]
- [[_COMMUNITY_Community 110|Community 110]]
- [[_COMMUNITY_Community 111|Community 111]]
- [[_COMMUNITY_Community 112|Community 112]]
- [[_COMMUNITY_Community 113|Community 113]]
- [[_COMMUNITY_Community 114|Community 114]]
- [[_COMMUNITY_Community 115|Community 115]]
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

## God Nodes (most connected - your core abstractions)
1. `execute_acceptance_in_workspace()` - 62 edges
2. `run_orchestrator()` - 58 edges
3. `main()` - 56 edges
4. `run_tui_loop()` - 49 edges
5. `OrchestratorState` - 49 edges
6. `create_test_app()` - 48 edges
7. `render_buffer()` - 48 edges
8. `buffer_to_string()` - 44 edges
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
Cohesion: 0.02
Nodes (324): create_shared_active_commands(), check_task_progress(), archive_entry_exists(), ArchiveEventHandler, ArchiveLoopResult, ArchiveVerificationResult, build_archive_error_message(), delete_change_directory() (+316 more)

### Community 1 - "Community 1"
Cohesion: 0.02
Nodes (170): Colors, main(), OpenSpecManager, print_change_detail(), print_changes(), Extract change information from directory., Count completed and total tasks., Show detailed information about a change. (+162 more)

### Community 2 - "Community 2"
Cohesion: 0.02
Nodes (142): AcpClient, AcpMessage, create_worktree(), delete_worktree(), merge_worktree(), branch_delete(), checkout(), get_logs() (+134 more)

### Community 3 - "Community 3"
Cohesion: 0.02
Nodes (188): get_current_commit(), default_retry_patterns(), commit_workspace_change(), create_test_config(), create_test_config_with(), fix_scheduler_premature_exit_decrements_pending_merge_counter_on_merge_completion(), init_git_repo(), make_test_change() (+180 more)

### Community 4 - "Community 4"
Cohesion: 0.02
Nodes (152): main(), handle_tui_command(), AppState, formatValue(), logHelperState(), onInput(), onKeydown(), ParallelExecutor (+144 more)

### Community 5 - "Community 5"
Cohesion: 0.02
Nodes (136): build_apply_prompt(), expand_apply_command(), get_changed_files(), test_openspec_directory_structure(), execute_acceptance_in_workspace(), run_post_apply_cleanup_review(), escape_for_single_quoted_context(), escape_shell_value() (+128 more)

### Community 6 - "Community 6"
Cohesion: 0.02
Nodes (157): delete_acceptance_state(), AcpContent, AcpElicitationParams, AcpError, AcpEvent, AcpPromptBlock, AcpUpdateParams, dispatch_jsonrpc_response() (+149 more)

### Community 7 - "Community 7"
Cohesion: 0.02
Nodes (141): BaseHTTPRequestHandler, handle_start_processing_command(), TuiCommandContext, handle_cursor_movement(), handle_editor_launch(), handle_enter_key(), handle_f5_key(), handle_key_event() (+133 more)

### Community 8 - "Community 8"
Cohesion: 0.02
Nodes (102): branch_exists(), check_git_repo(), generate_unique_branch_name(), get_current_branch(), get_status(), has_uncommitted_changes(), is_head_empty_commit(), is_working_directory_clean() (+94 more)

### Community 9 - "Community 9"
Cohesion: 0.03
Nodes (59): make_ai_runner(), test_archive_change_retries_until_verified(), get_conflict_files(), ParallelExecutor, Template, AutoResolveGuard, build_conflict_resolve_prompt(), build_sequential_merge_resolve_prompt() (+51 more)

### Community 10 - "Community 10"
Cohesion: 0.03
Nodes (73): acceptance_test_streaming(), apply_change(), apply_change_streaming(), ApplyContext, ApplyResult, test_apply_context_new(), archive_change(), archive_change_streaming() (+65 more)

### Community 11 - "Community 11"
Cohesion: 0.03
Nodes (96): ArchiveResult, extract_tree_branch(), parse_project_url(), RemoteClient, resolve_default_branch(), resolve_project_url_and_branch(), test_add_project_no_auth_header(), test_authorization_header_sent_with_token() (+88 more)

### Community 12 - "Community 12"
Cohesion: 0.04
Nodes (84): dispatch_event(), post_archive_dispatch_event(), run_orchestrator(), run_orchestrator_parallel(), test_archive_path_structure(), test_archive_verification_logic(), test_tui_archived_during_resolve(), test_tui_archived_no_active_resolve() (+76 more)

### Community 13 - "Community 13"
Cohesion: 0.04
Nodes (103): ChangeStateSnapshot, configure_logging(), global_deduplicator(), LogDeduplicator, maybe_log_summary(), should_log_change_count(), should_log_task_progress(), test_config() (+95 more)

### Community 14 - "Community 14"
Cohesion: 0.04
Nodes (49): confirmDeleteWorktree(), deleteWorktree(), escapeHtml(), fetchWorktrees(), mergeWorktree(), renderWorktrees(), WebMonitor, ReanalysisReason (+41 more)

### Community 15 - "Community 15"
Cohesion: 0.05
Nodes (80): control_cancel_stop(), control_force_stop(), control_retry(), control_start(), control_stop(), ControlResponse, create_test_change(), CreateWorktreeRequest (+72 more)

### Community 16 - "Community 16"
Cohesion: 0.05
Nodes (56): AiCommandRunner, OutputLine, test_inactivity_timeout_retry(), test_inactivity_timeout_streaming_pipeline(), test_post_completion_cleanup_on_cancellation(), test_post_completion_cleanup_on_failure(), test_post_completion_cleanup_on_success(), test_shared_stagger_state() (+48 more)

### Community 17 - "Community 17"
Cohesion: 0.02
Nodes (31): check_git_available(), check_git_directory(), check_parallel_available(), CheckConflictsArgs, Cli, Commands, EvidenceMode, InitArgs (+23 more)

### Community 18 - "Community 18"
Cohesion: 0.04
Nodes (56): AnalysisResult, AnalyzePromptMetadata, create_test_analyzer(), create_test_change(), ParallelGroup, ParallelizationAnalyzer, test_build_prompt_all_selected(), test_build_prompt_includes_frontmatter_metadata_context() (+48 more)

### Community 19 - "Community 19"
Cohesion: 0.04
Nodes (54): apply_completion_check_interval(), apply_completion_grace_period(), ApplyBlockedHandoff, ApplyCompletionKind, ApplyConfig, ApplyEventHandler, ApplyIterationResult, ApplyLoopHookContext (+46 more)

### Community 20 - "Community 20"
Cohesion: 0.03
Nodes (30): AcceptanceResult, build_acceptance_tail_findings(), canonical_verdict_kind(), detect_verdict_in_line(), parse_acceptance_output(), parse_findings(), parse_json_verdict(), strip_markdown_decorations() (+22 more)

### Community 21 - "Community 21"
Cohesion: 0.05
Nodes (61): extract_assistant_tool_summary(), extract_from_assistant(), extract_from_result(), extract_from_stream_event(), extract_text_from_stream_json(), extract_tool_result_summary(), extract_tool_summary_from_stream_json(), extract_tool_use_summary() (+53 more)

### Community 22 - "Community 22"
Cohesion: 0.06
Nodes (53): acceptance_resume_ready_for_archive(), acceptance_state_can_be_deleted(), acceptance_state_is_not_created_under_worktree(), acceptance_state_path(), acceptance_state_root_dir(), acceptance_state_roundtrip(), AcceptanceState, AcceptanceStateStatus (+45 more)

### Community 23 - "Community 23"
Cohesion: 0.06
Nodes (31): append_recovery_task_section(), cleanup_worktree(), execute_rejection_flow(), extract_rejected_reason(), handle_resume_apply_from_rejecting(), has_rejection_proposal(), init_git_repo(), parse_rejection_review_output() (+23 more)

### Community 24 - "Community 24"
Cohesion: 0.09
Nodes (33): addProject(), APIError, controlRun(), controlStop(), createProposalSession(), createTerminalSession(), createWorktree(), deleteProject() (+25 more)

### Community 25 - "Community 25"
Cohesion: 0.07
Nodes (7): MockWorkspaceManager, test_detect_conflicts_no_conflicts(), test_detect_conflicts_with_conflicts(), test_get_vcs_log_for_revisions(), test_get_vcs_status(), test_resolve_merges_with_retry_args_clone(), test_resolve_merges_with_retry_args_struct()

### Community 26 - "Community 26"
Cohesion: 0.07
Nodes (14): test_workspace_status_failed_includes_message(), ExecutionContext, ExecutionContext<'a>, ExecutionResult, ProgressInfo, test_execution_context_is_parallel(), test_execution_context_new(), test_execution_context_working_dir() (+6 more)

### Community 27 - "Community 27"
Cohesion: 0.09
Nodes (17): default_server_data_dir(), AcceptancePromptMode, default_error_circuit_breaker_enabled(), default_error_circuit_breaker_threshold(), default_proposal_session_inactivity_timeout_secs(), default_proposal_transport_args(), default_proposal_transport_command(), default_server_bind() (+9 more)

### Community 28 - "Community 28"
Cohesion: 0.07
Nodes (1): ResumeTestManager

### Community 29 - "Community 29"
Cohesion: 0.19
Nodes (10): ActiveCommandGuard, ActiveCommandRegistry, make_key(), RootKind, test_acquire_and_release(), test_different_roots_independent(), test_double_acquire_fails(), test_guard_release_async() (+2 more)

### Community 30 - "Community 30"
Cohesion: 0.22
Nodes (12): CircuitBreakerConfig, ErrorHistory, normalize_error_message(), test_circuit_breaker_disabled(), test_clear_history(), test_detect_same_error_different_errors(), test_detect_same_error_with_threshold(), test_last_error() (+4 more)

### Community 31 - "Community 31"
Cohesion: 0.2
Nodes (15): _blocks_equal(), delta_to_canonical(), merge_spec_delta(), parse_delta_sections(), Shared spec promotion engine for Conflux archive workflow.  Provides requirement, Split spec content into (preamble, [(normalized_key, full_block), ...]).      fu, Simulate spec promotion without writing any files.      Returns (result_content,, Convert a delta-format spec to canonical format for brand-new specs. (+7 more)

### Community 32 - "Community 32"
Cohesion: 0.21
Nodes (6): get_cflx_embedded_skills(), test_embedded_skills_count(), test_embedded_skills_have_auxiliary_files(), test_embedded_skills_names(), test_rust_prompt_builder_does_not_contain_acceptance_checklist(), get_archive_readiness_context()

### Community 34 - "Community 34"
Cohesion: 0.29
Nodes (6): AppMode, MergeConflictInfo, StopMode, ViewMode, WorktreeAction, WorktreeInfo

### Community 37 - "Community 37"
Cohesion: 0.67
Nodes (3): create_skill_structure(), main(), Create the skill directory structure with template files.

### Community 42 - "Community 42"
Cohesion: 0.67
Nodes (1): ApiDoc

### Community 67 - "Community 67"
Cohesion: 1.0
Nodes (1): Configure sys.path so shared modules are importable from tests.

### Community 68 - "Community 68"
Cohesion: 1.0
Nodes (1): Result<T, E>

### Community 69 - "Community 69"
Cohesion: 1.0
Nodes (1): OutputLine

### Community 92 - "Community 92"
Cohesion: 1.0
Nodes (1): Emit warning for invalid change directory.

### Community 108 - "Community 108"
Cohesion: 1.0
Nodes (1): ANSI color codes for terminal output.

### Community 109 - "Community 109"
Cohesion: 1.0
Nodes (1): Manage OpenSpec changes and specifications.

### Community 110 - "Community 110"
Cohesion: 1.0
Nodes (1): Warn about obsolete OpenSpec artifacts that should be removed.

### Community 111 - "Community 111"
Cohesion: 1.0
Nodes (1): Return True when directory is a valid change (has proposal.md).

### Community 112 - "Community 112"
Cohesion: 1.0
Nodes (1): Emit warning for invalid change directory.

### Community 113 - "Community 113"
Cohesion: 1.0
Nodes (1): List all changes or specs.

### Community 114 - "Community 114"
Cohesion: 1.0
Nodes (1): Extract change information from directory.

### Community 115 - "Community 115"
Cohesion: 1.0
Nodes (1): Count completed and total tasks.

### Community 116 - "Community 116"
Cohesion: 1.0
Nodes (1): Show detailed information about a change.

### Community 117 - "Community 117"
Cohesion: 1.0
Nodes (1): Find the directory for a given change ID.

### Community 118 - "Community 118"
Cohesion: 1.0
Nodes (1): Validate a change or all changes.

### Community 119 - "Community 119"
Cohesion: 1.0
Nodes (1): Extract Change Type value from proposal.md content.

### Community 120 - "Community 120"
Cohesion: 1.0
Nodes (1): Validate a single change directory.

### Community 121 - "Community 121"
Cohesion: 1.0
Nodes (1): Validate tasks.md file format.

### Community 122 - "Community 122"
Cohesion: 1.0
Nodes (1): Validate spec delta files.

### Community 123 - "Community 123"
Cohesion: 1.0
Nodes (1): Emit archive-risk warnings for spec-only proposals with only MODIFIED/REMOVED de

### Community 124 - "Community 124"
Cohesion: 1.0
Nodes (1): Archive a deployed change.

### Community 125 - "Community 125"
Cohesion: 1.0
Nodes (1): Simulate spec promotion and return errors without writing any files.

### Community 126 - "Community 126"
Cohesion: 1.0
Nodes (1): Update canonical specs from change deltas (simulation already passed).

### Community 127 - "Community 127"
Cohesion: 1.0
Nodes (1): Print changes or specs in a formatted way.

### Community 128 - "Community 128"
Cohesion: 1.0
Nodes (1): Print detailed change information.

## Knowledge Gaps
- **292 isolated node(s):** `Create the skill directory structure with template files.`, `Validate YAML frontmatter in SKILL.md.`, `Validate skill directory structure.`, `Package skill into a .skill file (zip with .skill extension).`, `Configure sys.path so shared modules are importable from tests.` (+287 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 28`** (28 nodes): `ResumeTestManager`, `.backend_type()`, `.check_available()`, `.cleanup_all()`, `.cleanup_workspace()`, `.conflict_resolution_prompt()`, `.create_iteration_snapshot()`, `.create_workspace()`, `.detect_conflicts()`, `.ensure_original_branch_initialized()`, `.find_existing_workspace()`, `.forget_workspace_sync()`, `.get_current_revision()`, `.get_log_for_revisions()`, `.get_revision_in_workspace()`, `.get_status()`, `.list_worktree_change_ids()`, `.max_concurrent()`, `.merge_workspaces()`, `.original_branch()`, `.prepare_for_parallel()`, `.repo_root()`, `.reuse_workspace()`, `.set_commit_message()`, `.snapshot_working_copy()`, `.squash_wip_commits()`, `.update_workspace_status()`, `.workspaces()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 42`** (3 nodes): `ApiDoc`, `main()`, `openapi_gen.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 67`** (2 nodes): `Configure sys.path so shared modules are importable from tests.`, `conftest.py`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 68`** (2 nodes): `Result<T, E>`, `.or_fail()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 69`** (2 nodes): `OutputLine`, `output.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 92`** (1 nodes): `Emit warning for invalid change directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 108`** (1 nodes): `ANSI color codes for terminal output.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 109`** (1 nodes): `Manage OpenSpec changes and specifications.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 110`** (1 nodes): `Warn about obsolete OpenSpec artifacts that should be removed.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 111`** (1 nodes): `Return True when directory is a valid change (has proposal.md).`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 112`** (1 nodes): `Emit warning for invalid change directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 113`** (1 nodes): `List all changes or specs.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 114`** (1 nodes): `Extract change information from directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 115`** (1 nodes): `Count completed and total tasks.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 116`** (1 nodes): `Show detailed information about a change.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 117`** (1 nodes): `Find the directory for a given change ID.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 118`** (1 nodes): `Validate a change or all changes.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 119`** (1 nodes): `Extract Change Type value from proposal.md content.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 120`** (1 nodes): `Validate a single change directory.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 121`** (1 nodes): `Validate tasks.md file format.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 122`** (1 nodes): `Validate spec delta files.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 123`** (1 nodes): `Emit archive-risk warnings for spec-only proposals with only MODIFIED/REMOVED de`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 124`** (1 nodes): `Archive a deployed change.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 125`** (1 nodes): `Simulate spec promotion and return errors without writing any files.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 126`** (1 nodes): `Update canonical specs from change deltas (simulation already passed).`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 127`** (1 nodes): `Print changes or specs in a formatted way.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 128`** (1 nodes): `Print detailed change information.`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `main()` connect `Community 11` to `Community 0`, `Community 1`, `Community 3`, `Community 4`, `Community 7`, `Community 8`, `Community 9`, `Community 12`, `Community 13`, `Community 14`, `Community 16`, `Community 17`, `Community 18`?**
  _High betweenness centrality (0.028) - this node is a cross-community bridge._
- **Why does `execute_acceptance_in_workspace()` connect `Community 5` to `Community 0`, `Community 3`, `Community 4`, `Community 7`, `Community 10`, `Community 11`, `Community 12`, `Community 16`, `Community 20`, `Community 22`, `Community 23`?**
  _High betweenness centrality (0.027) - this node is a cross-community bridge._
- **Why does `parse_findings()` connect `Community 20` to `Community 0`, `Community 10`, `Community 7`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Are the 57 inferred relationships involving `execute_acceptance_in_workspace()` (e.g. with `.dispatch_change_to_workspace()` and `.is_cancelled()`) actually correct?**
  _`execute_acceptance_in_workspace()` has 57 INFERRED edges - model-reasoned connections that need verification._
- **Are the 55 inferred relationships involving `run_orchestrator()` (e.g. with `handle_start_processing_command()` and `.with_event_tx()`) actually correct?**
  _`run_orchestrator()` has 55 INFERRED edges - model-reasoned connections that need verification._
- **Are the 49 inferred relationships involving `main()` (e.g. with `parse()` and `.load()`) actually correct?**
  _`main()` has 49 INFERRED edges - model-reasoned connections that need verification._
- **Are the 47 inferred relationships involving `run_tui_loop()` (e.g. with `.new()` and `list_changes_in_head()`) actually correct?**
  _`run_tui_loop()` has 47 INFERRED edges - model-reasoned connections that need verification._
