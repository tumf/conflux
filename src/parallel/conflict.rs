//! Conflict detection and resolution logic for parallel execution.

use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::history::{ResolveAttempt, ResolveContext};
use crate::vcs::git::commands as git_commands;
use crate::vcs::{VcsBackend, WorkspaceManager};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::events::{send_event, ParallelEvent};

/// RAII guard that decrements auto_resolve_count on drop.
/// This ensures the counter is decremented on all exit paths (success, error, early return).
struct AutoResolveGuard {
    counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl AutoResolveGuard {
    fn new(counter: std::sync::Arc<std::sync::atomic::AtomicUsize>) -> Self {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self { counter }
    }
}

impl Drop for AutoResolveGuard {
    fn drop(&mut self) {
        self.counter
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Detect conflicted files using the workspace manager.
pub async fn detect_conflicts(workspace_manager: &dyn WorkspaceManager) -> Result<Vec<String>> {
    workspace_manager
        .detect_conflicts()
        .await
        .map_err(OrchestratorError::from)
}

/// Get VCS status output for context.
pub async fn get_vcs_status(workspace_manager: &dyn WorkspaceManager) -> Result<String> {
    workspace_manager
        .get_status()
        .await
        .map_err(OrchestratorError::from)
}

/// Get VCS log for specific revisions.
pub async fn get_vcs_log_for_revisions(
    workspace_manager: &dyn WorkspaceManager,
    revisions: &[String],
) -> Result<String> {
    workspace_manager
        .get_log_for_revisions(revisions)
        .await
        .map_err(OrchestratorError::from)
}

/// Attempt to resolve conflicts with retries using the configured resolve command.
#[allow(clippy::too_many_arguments)]
pub async fn resolve_conflicts_with_retry(
    workspace_manager: &dyn WorkspaceManager,
    config: &OrchestratorConfig,
    event_tx: &Option<mpsc::Sender<ParallelEvent>>,
    revisions: &[String],
    change_ids: &[String],
    vcs_error: &str,
    max_retries: u32,
    shared_stagger_state: crate::ai_command_runner::SharedStaggerState,
    auto_resolve_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) -> Result<()> {
    // Create RAII guard to ensure counter is decremented on all exit paths
    let _guard = AutoResolveGuard::new(auto_resolve_count);

    send_event(event_tx, ParallelEvent::ConflictResolutionStarted).await;

    // Get conflict files for the resolve command
    let conflict_files = detect_conflicts(workspace_manager).await?;
    let conflict_files_str = conflict_files.join(", ");

    // Get VCS status for context
    let vcs_status = get_vcs_status(workspace_manager).await.unwrap_or_default();

    // Get VCS log for the conflicting revisions
    let vcs_log = get_vcs_log_for_revisions(workspace_manager, revisions)
        .await
        .unwrap_or_default();

    // Get the VCS-specific conflict resolution prompt prefix
    let vcs_prompt_prefix = workspace_manager.conflict_resolution_prompt();

    // Create resolve context for tracking attempts
    let mut resolve_context = ResolveContext::new(max_retries);

    // Create a combined change_id for logging (join multiple IDs if present)
    let combined_change_id = change_ids.join("+");

    // Create AiCommandRunner for resolve command execution
    use crate::ai_command_runner::AiCommandRunner;
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::*;
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: config
            .command_queue_stagger_delay_ms
            .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
        max_retries: config
            .command_queue_max_retries
            .unwrap_or(DEFAULT_MAX_RETRIES),
        retry_delay_ms: config
            .command_queue_retry_delay_ms
            .unwrap_or(DEFAULT_RETRY_DELAY_MS),
        retry_error_patterns: config
            .command_queue_retry_patterns
            .clone()
            .unwrap_or_else(default_retry_patterns),
        retry_if_duration_under_secs: config
            .command_queue_retry_if_duration_under_secs
            .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
        inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
        inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
    };
    let stream_json_textify = config.get_stream_json_textify();
    let mut ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());
    ai_runner.set_stream_json_textify(stream_json_textify);

    // Build initial resolve command to send in ResolveStarted event (before retry loop)
    let initial_resolve_prompt = format!(
        "{}\n\n\
         A merge conflict occurred while trying to merge the following revisions:\n\
         {}\n\n\
         VCS error output:\n\
         {}\n\n\
         Current VCS status:\n\
         {}\n\n\
         VCS log for conflicting changes:\n\
         {}\n\n\
         Conflicting files: {}\n\n\
         Please resolve the merge conflicts in the listed files.\n\n\
         IMPORTANT:\n\
         - Do NOT use --no-verify flag when committing. Always run pre-commit hooks.\n\
         - Do NOT break existing functionality unrelated to the conflicting changes.\n\
         - When resolving conflicts, preserve both sides' intent where possible.\n\
         - If shared code is modified, ensure all existing callers still work correctly.\n\
         - Do NOT remove or alter existing functionality that is not part of the conflicting changes.",
        vcs_prompt_prefix,
        revisions.join(", "),
        vcs_error,
        vcs_status,
        vcs_log,
        conflict_files_str
    );
    let template = config.get_resolve_command()?;
    let initial_command =
        crate::config::OrchestratorConfig::expand_prompt(template, &initial_resolve_prompt);
    // Expand {conflict_files} placeholder if present in the command template
    let initial_command =
        crate::config::expand::expand_conflict_files(&initial_command, &conflict_files_str);

    // Send ResolveStarted event for each change_id with the command string
    for change_id in change_ids {
        send_event(
            event_tx,
            ParallelEvent::ResolveStarted {
                change_id: change_id.to_string(),
                command: initial_command.clone(),
            },
        )
        .await;
    }

    for attempt in 1..=max_retries {
        let start = Instant::now();
        info!(
            "Conflict resolution attempt {}/{} for files: {}",
            attempt, max_retries, conflict_files_str
        );

        // Build the resolve prompt with VCS-specific context
        let mut resolve_prompt = format!(
            "{}\n\n\
             A merge conflict occurred while trying to merge the following revisions:\n\
             {}\n\n\
             VCS error output:\n\
             {}\n\n\
             Current VCS status:\n\
             {}\n\n\
             VCS log for conflicting changes:\n\
             {}\n\n\
             Conflicting files: {}\n\n\
             Please resolve the merge conflicts in the listed files.\n\n\
             IMPORTANT:\n\
             - Do NOT use --no-verify flag when committing. Always run pre-commit hooks.\n\
             - Do NOT break existing functionality unrelated to the conflicting changes.\n\
             - When resolving conflicts, preserve both sides' intent where possible.\n\
             - If shared code is modified, ensure all existing callers still work correctly.\n\
             - Do NOT remove or alter existing functionality that is not part of the conflicting changes.",
            vcs_prompt_prefix,
            revisions.join(", "),
            vcs_error,
            vcs_status,
            vcs_log,
            conflict_files_str
        );

        // Add context from previous attempts if any
        let continuation_context = resolve_context.format_continuation_context();
        if !continuation_context.is_empty() {
            resolve_prompt = format!("{}\n\n{}", resolve_prompt, continuation_context);
        }

        // Use AiCommandRunner for streaming resolve command execution
        let template = config.get_resolve_command()?;
        let command = crate::config::OrchestratorConfig::expand_prompt(template, &resolve_prompt);
        let (mut child, mut rx) = ai_runner
            .execute_streaming_with_retry(
                &command,
                Some(workspace_manager.repo_root()),
                Some("resolve"),
                None,
            )
            .await?;

        // Create output collector for history
        let mut output_collector = crate::history::OutputCollector::new();

        // Stream output to events
        while let Some(line) = rx.recv().await {
            let text = match &line {
                crate::ai_command_runner::OutputLine::Stdout(s) => {
                    output_collector.add_stdout(s);
                    s.clone()
                }
                crate::ai_command_runner::OutputLine::Stderr(s) => {
                    output_collector.add_stderr(s);
                    s.clone()
                }
            };
            send_event(
                event_tx,
                ParallelEvent::ResolveOutput {
                    change_id: combined_change_id.clone(),
                    output: text.clone(),
                    iteration: Some(attempt),
                },
            )
            .await;
        }

        // Wait for process to complete
        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Resolve command failed in workspace '{}' (attempt {}): {}",
                workspace_manager.repo_root().display(),
                attempt,
                e
            ))
        })?;
        let status_success = status.success();

        // Verify resolution regardless of exit code
        let remaining_conflicts = detect_conflicts(workspace_manager).await?;
        let duration = start.elapsed();

        if remaining_conflicts.is_empty() {
            if !status_success {
                warn!(
                    "Resolve command exited non-zero but conflicts cleared (attempt {}/{})",
                    attempt, max_retries
                );
            }
            // Record successful resolution
            resolve_context.record(ResolveAttempt {
                attempt,
                command_success: status_success,
                verification_success: true,
                duration,
                continuation_reason: None,
                exit_code: status.code(),
                stdout_tail: output_collector.stdout_tail(),
                stderr_tail: output_collector.stderr_tail(),
            });
            send_event(event_tx, ParallelEvent::ConflictResolutionCompleted).await;
            // Guard will decrement auto resolve counter on drop
            return Ok(());
        }

        // Record failed attempt with continuation reason
        let continuation_reason = if status_success {
            let reason = format!(
                "Conflicts still present after resolution attempt: {}",
                remaining_conflicts.join(", ")
            );
            warn!("{}", reason);
            Some(reason)
        } else {
            let reason = format!(
                "Resolution command failed with exit code: {:?}",
                status.code()
            );
            warn!(
                "Resolution attempt {} failed with exit code: {:?}",
                attempt,
                status.code()
            );
            Some(reason)
        };

        resolve_context.record(ResolveAttempt {
            attempt,
            command_success: status_success,
            verification_success: false,
            duration,
            continuation_reason,
            exit_code: status.code(),
            stdout_tail: output_collector.stdout_tail(),
            stderr_tail: output_collector.stderr_tail(),
        });
    }

    let error_msg = format!("Failed to resolve conflicts after {} attempts", max_retries);
    send_event(
        event_tx,
        ParallelEvent::ConflictResolutionFailed {
            error: error_msg.clone(),
        },
    )
    .await;

    // Guard will decrement auto resolve counter on drop

    // Return VCS-specific error
    match workspace_manager.backend_type() {
        VcsBackend::Git | VcsBackend::Auto => Err(OrchestratorError::GitConflict(error_msg)),
    }
}

#[derive(Clone)]
pub struct ResolveMergesWithRetryArgs<'a> {
    pub workspace_manager: &'a dyn WorkspaceManager,
    pub config: &'a OrchestratorConfig,
    pub event_tx: &'a Option<mpsc::Sender<ParallelEvent>>,
    pub revisions: &'a [String],
    pub change_ids: &'a [String],
    pub target_branch: &'a str,
    pub base_revision: &'a str,
    pub max_retries: u32,
    pub shared_stagger_state: crate::ai_command_runner::SharedStaggerState,
    pub auto_resolve_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

/// Attempt to resolve merges with retries using the configured resolve command.
pub async fn resolve_merges_with_retry(args: ResolveMergesWithRetryArgs<'_>) -> Result<()> {
    let ResolveMergesWithRetryArgs {
        workspace_manager,
        config,
        event_tx,
        revisions,
        change_ids,
        target_branch,
        base_revision,
        max_retries,
        shared_stagger_state,
        auto_resolve_count,
    } = args;

    // Create RAII guard to ensure counter is decremented on all exit paths
    let _guard = AutoResolveGuard::new(auto_resolve_count);

    send_event(event_tx, ParallelEvent::ConflictResolutionStarted).await;

    let conflict_files = detect_conflicts(workspace_manager).await?;
    let conflict_files_str = if conflict_files.is_empty() {
        "(none)".to_string()
    } else {
        conflict_files.join(", ")
    };

    let vcs_status = get_vcs_status(workspace_manager).await.unwrap_or_default();
    let vcs_log = get_vcs_log_for_revisions(workspace_manager, revisions)
        .await
        .unwrap_or_default();

    let vcs_prompt_prefix = workspace_manager.conflict_resolution_prompt();

    let merge_plan = revisions
        .iter()
        .zip(change_ids.iter())
        .map(|(rev, change_id)| format!("- {} => {}", rev, change_id))
        .collect::<Vec<_>>()
        .join("\n");

    let workspaces = workspace_manager.workspaces();

    let workspace_paths: HashMap<String, PathBuf> = workspaces
        .iter()
        .map(|workspace| (workspace.name.clone(), workspace.path.clone()))
        .collect();

    let workspace_base_revisions: HashMap<String, String> = workspaces
        .into_iter()
        .map(|workspace| (workspace.name, workspace.base_revision))
        .collect();

    let worktree_locations = revisions
        .iter()
        .zip(change_ids.iter())
        .map(|(rev, change_id)| {
            let path = workspace_paths
                .get(rev)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "(unknown)".to_string());
            format!("- {} => {} (change_id: {})", rev, path, change_id)
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Create resolve context for tracking attempts
    let mut resolve_context = ResolveContext::new(max_retries);

    // Create a combined change_id for logging (join multiple IDs if present)
    let combined_change_id = change_ids.join("+");

    // Create AiCommandRunner for resolve command execution
    use crate::ai_command_runner::AiCommandRunner;
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::*;
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: config
            .command_queue_stagger_delay_ms
            .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
        max_retries: config
            .command_queue_max_retries
            .unwrap_or(DEFAULT_MAX_RETRIES),
        retry_delay_ms: config
            .command_queue_retry_delay_ms
            .unwrap_or(DEFAULT_RETRY_DELAY_MS),
        retry_error_patterns: config
            .command_queue_retry_patterns
            .clone()
            .unwrap_or_else(default_retry_patterns),
        retry_if_duration_under_secs: config
            .command_queue_retry_if_duration_under_secs
            .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
        inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
        inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
    };
    let stream_json_textify = config.get_stream_json_textify();
    let mut ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());
    ai_runner.set_stream_json_textify(stream_json_textify);

    // Build initial resolve command to send in ResolveStarted event (before retry loop)
    let initial_resolve_prompt = format!(
        "{}\n\n\
         You must complete sequential Git merges into the target branch.\n\n\
         Target branch: {}\n\
         Base revision before merges: {}\n\
         Merge plan (branch => change_id):\n{}\n\n\
         Worktree directories (branch => path):\n{}\n\n\
         Requirements:\n\
         - Before merging each branch into the target branch, you MUST pre-sync base into that worktree branch (base -> worktree) from inside the worktree directory.\n\
         - If a pre-sync merge commit is created, its subject MUST be exactly: \"Pre-sync base into <change_id>\".\n\
         - The final merge into the target branch MUST create a merge commit with subject exactly: \"Merge change: <change_id>\".\n\
         - Do NOT use --no-verify flag when committing. Always run pre-commit hooks.\n\
         - Do NOT break existing functionality unrelated to the changes being merged.\n\
         - When resolving conflicts, preserve both sides' intent where possible.\n\
         - If shared code is modified, ensure all existing callers still work correctly.\n\
         - Do NOT remove or alter existing functionality that is not part of the changes being merged.\n\n\
         Instructions (repeat for each branch in order):\n\
         1) Pre-sync in the worktree directory:\n\
            - cd <worktree_path>\n\
            - git checkout <branch>\n\
            - git merge --no-ff -m \"Pre-sync base into <change_id>\" <target_branch>\n\
            - If a conflict occurs, resolve it, git add, then git commit -m \"Pre-sync base into <change_id>\" to complete the merge.\n\
            - If the merge commit message is wrong, fix it with: git commit --amend -m \"Pre-sync base into <change_id>\".\n\
             2) Final merge into the target branch (in the repo root):\n\
                 - cd <repo_root>\n\
                 - git checkout <target_branch>\n\
                 - git merge --no-ff --no-commit <branch>\n\
                 - If a conflict occurs, resolve it and git add the resolved files.\n\
                 - BEFORE creating the merge commit:\n\
                   * If `openspec/changes/<change_id>/proposal.md` exists AND `openspec/changes/archive/` contains the same <change_id>, remove `openspec/changes/<change_id>` (the directory was resurrected by the merge and must be deleted).\n\
                   * Use `git rm -rf openspec/changes/<change_id>` to remove the resurrected directory.\n\
                 - Finally, run `git commit -m \"Merge change: <change_id>\"` to complete the merge.\n\
         3) If a pre-commit hook modifies files and stops the commit, re-stage and re-run git commit with the same message.\n\
         4) Do not use destructive commands like reset --hard.\n\n\
         Current VCS status:\n{}\n\n\
         VCS log for branches:\n{}\n\n\
         Conflicting files (repo root, if any): {}\n\n\
         Complete the merges so that the target branch has merge commits for every change_id.",
        vcs_prompt_prefix,
        target_branch,
        base_revision,
        merge_plan,
        worktree_locations,
        vcs_status,
        vcs_log,
        conflict_files_str
    );
    let template = config.get_resolve_command()?;
    let initial_command =
        crate::config::OrchestratorConfig::expand_prompt(template, &initial_resolve_prompt);
    // Expand {conflict_files} placeholder if present in the command template
    let initial_command =
        crate::config::expand::expand_conflict_files(&initial_command, &conflict_files_str);

    // Send ResolveStarted for each change_id to update TUI status with command string
    for change_id in change_ids {
        send_event(
            event_tx,
            ParallelEvent::ResolveStarted {
                change_id: change_id.to_string(),
                command: initial_command.clone(),
            },
        )
        .await;
    }

    for attempt in 1..=max_retries {
        let start = Instant::now();
        info!(
            "Merge resolution attempt {}/{} for branches: {}",
            attempt,
            max_retries,
            revisions.join(", ")
        );

        let mut resolve_prompt = format!(
            "{}\n\n\
             You must complete sequential Git merges into the target branch.\n\n\
             Target branch: {}\n\
             Base revision before merges: {}\n\
             Merge plan (branch => change_id):\n{}\n\n\
             Worktree directories (branch => path):\n{}\n\n\
             Requirements:\n\
             - Before merging each branch into the target branch, you MUST pre-sync base into that worktree branch (base -> worktree) from inside the worktree directory.\n\
             - If a pre-sync merge commit is created, its subject MUST be exactly: \"Pre-sync base into <change_id>\".\n\
             - The final merge into the target branch MUST create a merge commit with subject exactly: \"Merge change: <change_id>\".\n\
             - Do NOT use --no-verify flag when committing. Always run pre-commit hooks.\n\
             - Do NOT break existing functionality unrelated to the changes being merged.\n\
             - When resolving conflicts, preserve both sides' intent where possible.\n\
             - If shared code is modified, ensure all existing callers still work correctly.\n\
             - Do NOT remove or alter existing functionality that is not part of the changes being merged.\n\n\
             Instructions (repeat for each branch in order):\n\
             1) Pre-sync in the worktree directory:\n\
                - cd <worktree_path>\n\
                - git checkout <branch>\n\
                - git merge --no-ff -m \"Pre-sync base into <change_id>\" <target_branch>\n\
                - If a conflict occurs, resolve it, git add, then git commit -m \"Pre-sync base into <change_id>\" to complete the merge.\n\
                - If the merge commit message is wrong, fix it with: git commit --amend -m \"Pre-sync base into <change_id>\".\n\
             2) Final merge into the target branch (in the repo root):\n\
                 - cd <repo_root>\n\
                 - git checkout <target_branch>\n\
                 - git merge --no-ff --no-commit <branch>\n\
                 - If a conflict occurs, resolve it and git add the resolved files.\n\
                 - BEFORE creating the merge commit:\n\
                   * If `openspec/changes/<change_id>/proposal.md` exists AND `openspec/changes/archive/` contains the same <change_id>, remove `openspec/changes/<change_id>` (the directory was resurrected by the merge and must be deleted).\n\
                   * Use `git rm -rf openspec/changes/<change_id>` to remove the resurrected directory.\n\
                 - Finally, run `git commit -m \"Merge change: <change_id>\"` to complete the merge.\n\
             3) If a pre-commit hook modifies files and stops the commit, re-stage and re-run git commit with the same message.\n\
             4) Do not use destructive commands like reset --hard.\n\n\
             Current VCS status:\n{}\n\n\
             VCS log for branches:\n{}\n\n\
             Conflicting files (repo root, if any): {}\n\n\
             Complete the merges so that the target branch has merge commits for every change_id.",
            vcs_prompt_prefix,
            target_branch,
            base_revision,
            merge_plan,
            worktree_locations,
            vcs_status,
             vcs_log,
            conflict_files_str
        );

        // Add context from previous attempts if any
        let continuation_context = resolve_context.format_continuation_context();
        if !continuation_context.is_empty() {
            resolve_prompt = format!("{}\n\n{}", resolve_prompt, continuation_context);
        }

        // Use AiCommandRunner for streaming resolve command execution
        let template = config.get_resolve_command()?;
        let command = crate::config::OrchestratorConfig::expand_prompt(template, &resolve_prompt);
        let (mut child, mut rx) = ai_runner
            .execute_streaming_with_retry(
                &command,
                Some(workspace_manager.repo_root()),
                Some("resolve"),
                None,
            )
            .await?;

        // Create output collector for history
        let mut output_collector = crate::history::OutputCollector::new();

        while let Some(line) = rx.recv().await {
            let text = match &line {
                crate::ai_command_runner::OutputLine::Stdout(s) => {
                    output_collector.add_stdout(s);
                    s.clone()
                }
                crate::ai_command_runner::OutputLine::Stderr(s) => {
                    output_collector.add_stderr(s);
                    s.clone()
                }
            };
            send_event(
                event_tx,
                ParallelEvent::ResolveOutput {
                    change_id: combined_change_id.clone(),
                    output: text.clone(),
                    iteration: Some(attempt),
                },
            )
            .await;
        }

        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Resolve command failed in workspace '{}' (attempt {}): {}",
                workspace_manager.repo_root().display(),
                attempt,
                e
            ))
        })?;
        let status_success = status.success();
        let duration = start.elapsed();

        let remaining_conflicts = detect_conflicts(workspace_manager).await?;
        if remaining_conflicts.is_empty() {
            if matches!(
                workspace_manager.backend_type(),
                VcsBackend::Git | VcsBackend::Auto
            ) {
                let repo_root = workspace_manager.repo_root();
                let merge_in_progress = git_commands::is_merge_in_progress(repo_root)
                    .await
                    .map_err(OrchestratorError::from)?;

                if merge_in_progress {
                    let reason =
                        "Merge still in progress (MERGE_HEAD exists); retrying resolve".to_string();
                    warn!(
                        "Merge still in progress after resolve attempt {}/{}",
                        attempt, max_retries
                    );
                    send_event(
                        event_tx,
                        ParallelEvent::ResolveOutput {
                            change_id: combined_change_id.clone(),
                            output: reason.clone(),
                            iteration: Some(attempt),
                        },
                    )
                    .await;
                    resolve_context.record(ResolveAttempt {
                        attempt,
                        command_success: status_success,
                        verification_success: false,
                        duration,
                        continuation_reason: Some(reason),
                        exit_code: status.code(),
                        stdout_tail: output_collector.stdout_tail(),
                        stderr_tail: output_collector.stderr_tail(),
                    });
                    continue;
                }

                // Ensure there is no unfinished pre-sync merge left in any worktree.
                let mut retry_reason: Option<String> = None;
                for revision in revisions {
                    if let Some(worktree_path) = workspace_paths.get(revision) {
                        let worktree_merge_in_progress =
                            git_commands::is_merge_in_progress(worktree_path)
                                .await
                                .map_err(OrchestratorError::from)?;
                        if worktree_merge_in_progress {
                            retry_reason = Some(format!(
                                "Worktree merge still in progress for '{}' (MERGE_HEAD exists); retrying resolve",
                                revision
                            ));
                            break;
                        }

                        let worktree_conflicts = git_commands::get_conflict_files(worktree_path)
                            .await
                            .map_err(OrchestratorError::from)?;
                        if !worktree_conflicts.is_empty() {
                            retry_reason = Some(format!(
                                "Worktree conflicts still present for '{}' ({}); retrying resolve",
                                revision,
                                worktree_conflicts.join(", ")
                            ));
                            break;
                        }
                    }
                }

                if let Some(reason) = retry_reason {
                    warn!("{}", reason);
                    send_event(
                        event_tx,
                        ParallelEvent::ResolveOutput {
                            change_id: combined_change_id.clone(),
                            output: reason.clone(),
                            iteration: Some(attempt),
                        },
                    )
                    .await;
                    resolve_context.record(ResolveAttempt {
                        attempt,
                        command_success: status_success,
                        verification_success: false,
                        duration,
                        continuation_reason: Some(reason),
                        exit_code: status.code(),
                        stdout_tail: output_collector.stdout_tail(),
                        stderr_tail: output_collector.stderr_tail(),
                    });
                    continue;
                }

                // Validate pre-sync merge commit subject convention inside each worktree.
                // (If no pre-sync merge commit was created, this check is a no-op.)
                let mut presync_retry_reason: Option<String> = None;
                for (revision, change_id) in revisions.iter().zip(change_ids.iter()) {
                    let Some(worktree_path) = workspace_paths.get(revision) else {
                        continue;
                    };

                    let mismatches = git_commands::presync_merge_subject_mismatches_since(
                        worktree_path,
                        base_revision,
                        change_id,
                    )
                    .await
                    .map_err(OrchestratorError::from)?;

                    if !mismatches.is_empty() {
                        presync_retry_reason = Some(format!(
                            "Invalid pre-sync merge commit subject in worktree '{}' (expected: 'Pre-sync base into {}', got: {}); retrying resolve",
                            revision,
                            change_id,
                            mismatches.join("; ")
                        ));
                        break;
                    }
                }

                if let Some(reason) = presync_retry_reason {
                    warn!("{}", reason);
                    send_event(
                        event_tx,
                        ParallelEvent::ResolveOutput {
                            change_id: combined_change_id.clone(),
                            output: reason.clone(),
                            iteration: Some(attempt),
                        },
                    )
                    .await;
                    resolve_context.record(ResolveAttempt {
                        attempt,
                        command_success: status_success,
                        verification_success: false,
                        duration,
                        continuation_reason: Some(reason),
                        exit_code: status.code(),
                        stdout_tail: output_collector.stdout_tail(),
                        stderr_tail: output_collector.stderr_tail(),
                    });
                    continue;
                }

                let missing_commits =
                    git_commands::missing_merge_commits_since(repo_root, base_revision, change_ids)
                        .await
                        .map_err(OrchestratorError::from)?;

                if !missing_commits.is_empty() {
                    let reason = format!(
                        "Missing merge commits for change_ids ({}); retrying resolve",
                        missing_commits.join(", ")
                    );
                    warn!(
                        "Missing merge commits after resolve attempt {}/{}: {:?}",
                        attempt, max_retries, missing_commits
                    );
                    send_event(
                        event_tx,
                        ParallelEvent::ResolveOutput {
                            change_id: combined_change_id.clone(),
                            output: reason.clone(),
                            iteration: Some(attempt),
                        },
                    )
                    .await;
                    resolve_context.record(ResolveAttempt {
                        attempt,
                        command_success: status_success,
                        verification_success: false,
                        duration,
                        continuation_reason: Some(reason),
                        exit_code: status.code(),
                        stdout_tail: output_collector.stdout_tail(),
                        stderr_tail: output_collector.stderr_tail(),
                    });
                    continue;
                }

                // Enforce that each worktree branch was pre-synced with the target branch state
                // that existed immediately before its final merge commit.
                let mut presync_missing_reason: Option<String> = None;
                for (revision, change_id) in revisions.iter().zip(change_ids.iter()) {
                    let expected_subject = format!("Merge change: {}", change_id);
                    let merge_commit = git_commands::merge_commit_hash_by_subject_since(
                        repo_root,
                        base_revision,
                        expected_subject.as_str(),
                    )
                    .await
                    .map_err(OrchestratorError::from)?;

                    let Some(merge_commit) = merge_commit else {
                        continue;
                    };

                    let pre_merge_base =
                        git_commands::first_parent_of(repo_root, merge_commit.trim())
                            .await
                            .map_err(OrchestratorError::from)?;
                    let pre_merge_base = pre_merge_base.trim();

                    let includes_presync_base =
                        git_commands::is_ancestor(repo_root, pre_merge_base, revision)
                            .await
                            .map_err(OrchestratorError::from)?;

                    if !includes_presync_base {
                        let short = &pre_merge_base[..8.min(pre_merge_base.len())];
                        presync_missing_reason = Some(format!(
                            "Worktree branch '{}' does not include pre-merge base '{}' for change_id '{}' (pre-sync may have been skipped); retrying resolve",
                            revision, short, change_id
                        ));
                        break;
                    }

                    let (Some(worktree_path), Some(worktree_base_revision)) = (
                        workspace_paths.get(revision),
                        workspace_base_revisions.get(revision),
                    ) else {
                        continue;
                    };

                    // If the worktree was created from an older base revision, require that a
                    // pre-sync merge commit exists with the standard subject.
                    if worktree_base_revision.trim() != pre_merge_base {
                        let expected_presync_subject = format!("Pre-sync base into {}", change_id);
                        let presync_commit = git_commands::merge_commit_hash_by_subject_since(
                            worktree_path,
                            worktree_base_revision.trim(),
                            expected_presync_subject.as_str(),
                        )
                        .await
                        .map_err(OrchestratorError::from)?;

                        if presync_commit.is_none() {
                            presync_missing_reason = Some(format!(
                                "Missing pre-sync merge commit in worktree '{}' (expected subject: 'Pre-sync base into {}'); retrying resolve",
                                revision, change_id
                            ));
                            break;
                        }
                    }
                }

                if let Some(reason) = presync_missing_reason {
                    warn!("{}", reason);
                    send_event(
                        event_tx,
                        ParallelEvent::ResolveOutput {
                            change_id: combined_change_id.clone(),
                            output: reason.clone(),
                            iteration: Some(attempt),
                        },
                    )
                    .await;
                    resolve_context.record(ResolveAttempt {
                        attempt,
                        command_success: status_success,
                        verification_success: false,
                        duration,
                        continuation_reason: Some(reason),
                        exit_code: status.code(),
                        stdout_tail: output_collector.stdout_tail(),
                        stderr_tail: output_collector.stderr_tail(),
                    });
                    continue;
                }
            }

            if !status_success {
                warn!(
                    "Resolve command exited non-zero but goals met (attempt {}/{})",
                    attempt, max_retries
                );
            }
            // Record successful resolution
            resolve_context.record(ResolveAttempt {
                attempt,
                command_success: status_success,
                verification_success: true,
                duration,
                continuation_reason: None,
                exit_code: status.code(),
                stdout_tail: output_collector.stdout_tail(),
                stderr_tail: output_collector.stderr_tail(),
            });
            send_event(event_tx, ParallelEvent::ConflictResolutionCompleted).await;

            // Send ResolveCompleted for each change_id to update TUI status
            for change_id in change_ids {
                send_event(
                    event_tx,
                    ParallelEvent::ResolveCompleted {
                        change_id: change_id.to_string(),
                        worktree_change_ids: None,
                    },
                )
                .await;
            }

            // Guard will decrement auto resolve counter on drop
            return Ok(());
        }

        // Record failed attempt with continuation reason
        let continuation_reason = if status_success {
            let reason = format!(
                "Conflicts still present after merge resolution attempt: {}",
                remaining_conflicts.join(", ")
            );
            warn!("{}", reason);
            Some(reason)
        } else {
            let reason = format!(
                "Merge resolution command failed with exit code: {:?}",
                status.code()
            );
            warn!(
                "Merge resolution attempt {} failed with exit code: {:?}",
                attempt,
                status.code()
            );
            Some(reason)
        };

        resolve_context.record(ResolveAttempt {
            attempt,
            command_success: status_success,
            verification_success: false,
            duration,
            continuation_reason,
            exit_code: status.code(),
            stdout_tail: output_collector.stdout_tail(),
            stderr_tail: output_collector.stderr_tail(),
        });
    }

    let error_msg = format!("Failed to resolve merges after {} attempts", max_retries);
    send_event(
        event_tx,
        ParallelEvent::ConflictResolutionFailed {
            error: error_msg.clone(),
        },
    )
    .await;

    // Send ResolveFailed for each change_id to update TUI status
    for change_id in change_ids {
        send_event(
            event_tx,
            ParallelEvent::ResolveFailed {
                change_id: change_id.to_string(),
                error: error_msg.clone(),
            },
        )
        .await;
    }

    // Guard will decrement auto resolve counter on drop

    match workspace_manager.backend_type() {
        VcsBackend::Git | VcsBackend::Auto => Err(OrchestratorError::GitConflict(error_msg)),
    }
}
