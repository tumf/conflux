//! Conflict detection and resolution logic for parallel execution.

use crate::agent::AgentRunner;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::vcs::git::commands as git_commands;
use crate::vcs::{VcsBackend, WorkspaceManager};
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::events::{send_event, ParallelEvent};

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
pub async fn resolve_conflicts_with_retry(
    workspace_manager: &dyn WorkspaceManager,
    config: &OrchestratorConfig,
    event_tx: &Option<mpsc::Sender<ParallelEvent>>,
    revisions: &[String],
    vcs_error: &str,
    max_retries: u32,
) -> Result<()> {
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

    for attempt in 1..=max_retries {
        info!(
            "Conflict resolution attempt {}/{} for files: {}",
            attempt, max_retries, conflict_files_str
        );

        // Build the resolve prompt with VCS-specific context
        let resolve_prompt = format!(
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
             Please resolve the merge conflicts in the listed files.",
            vcs_prompt_prefix,
            revisions.join(", "),
            vcs_error,
            vcs_status,
            vcs_log,
            conflict_files_str
        );

        // Use AgentRunner for streaming resolve command execution
        let agent = AgentRunner::new(config.clone());
        let (mut child, mut rx) = agent
            .run_resolve_streaming_in_dir(&resolve_prompt, workspace_manager.repo_root())
            .await?;

        // Stream output to events
        while let Some(line) = rx.recv().await {
            let text = match &line {
                crate::agent::OutputLine::Stdout(s) | crate::agent::OutputLine::Stderr(s) => {
                    s.clone()
                }
            };
            send_event(
                event_tx,
                ParallelEvent::ResolveOutput {
                    output: text.clone(),
                },
            )
            .await;
        }

        // Wait for process to complete
        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!("Resolve command failed: {}", e))
        })?;

        if status.success() {
            // Verify resolution
            let remaining_conflicts = detect_conflicts(workspace_manager).await?;
            if remaining_conflicts.is_empty() {
                send_event(event_tx, ParallelEvent::ConflictResolutionCompleted).await;
                return Ok(());
            }
            warn!(
                "Conflicts still present after resolution attempt: {:?}",
                remaining_conflicts
            );
        } else {
            warn!(
                "Resolution attempt {} failed with exit code: {:?}",
                attempt,
                status.code()
            );
        }
    }

    let error_msg = format!("Failed to resolve conflicts after {} attempts", max_retries);
    send_event(
        event_tx,
        ParallelEvent::ConflictResolutionFailed {
            error: error_msg.clone(),
        },
    )
    .await;

    // Return VCS-specific error
    match workspace_manager.backend_type() {
        VcsBackend::Git | VcsBackend::Auto => Err(OrchestratorError::GitConflict(error_msg)),
    }
}

#[derive(Clone, Copy)]
pub struct ResolveMergesWithRetryArgs<'a> {
    pub workspace_manager: &'a dyn WorkspaceManager,
    pub config: &'a OrchestratorConfig,
    pub event_tx: &'a Option<mpsc::Sender<ParallelEvent>>,
    pub revisions: &'a [String],
    pub change_ids: &'a [String],
    pub target_branch: &'a str,
    pub base_revision: &'a str,
    pub max_retries: u32,
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
    } = args;

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

    for attempt in 1..=max_retries {
        info!(
            "Merge resolution attempt {}/{} for branches: {}",
            attempt,
            max_retries,
            revisions.join(", ")
        );

        let resolve_prompt = format!(
            "{}\n\n\
             You must complete sequential Git merges into the target branch.\n\n\
             Target branch: {}\n\
             Base revision before merges: {}\n\
             Merge plan (branch => change_id):\n{}
\n\
             Instructions:\n\
             1) Ensure you are on the target branch.\n\
             2) For each branch in order, run a merge that always creates a merge commit, even if fast-forward is possible.\n\
             3) The merge commit message MUST include the change_id (use: \"Merge change: <change_id>\").\n\
             4) If a conflict occurs, resolve it, run git add, and complete the merge commit.\n\
             5) If a pre-commit hook modifies files and stops the commit, re-stage and re-run git commit with the same message.\n\
             6) Do not use destructive commands like reset --hard.\n\n\
             Current VCS status:\n{}\n\n\
             VCS log for branches:\n{}\n\n\
             Conflicting files (if any): {}\n\n\
             Complete the merges so that the target branch has merge commits for every change_id.",
            vcs_prompt_prefix,
            target_branch,
            base_revision,
            merge_plan,
            vcs_status,
            vcs_log,
            conflict_files_str
        );

        let agent = AgentRunner::new(config.clone());
        let (mut child, mut rx) = agent
            .run_resolve_streaming_in_dir(&resolve_prompt, workspace_manager.repo_root())
            .await?;

        while let Some(line) = rx.recv().await {
            let text = match &line {
                crate::agent::OutputLine::Stdout(s) | crate::agent::OutputLine::Stderr(s) => {
                    s.clone()
                }
            };
            send_event(
                event_tx,
                ParallelEvent::ResolveOutput {
                    output: text.clone(),
                },
            )
            .await;
        }

        let status = child.wait().await.map_err(|e| {
            OrchestratorError::AgentCommand(format!("Resolve command failed: {}", e))
        })?;

        if status.success() {
            let remaining_conflicts = detect_conflicts(workspace_manager).await?;
            if remaining_conflicts.is_empty() {
                if matches!(
                    workspace_manager.backend_type(),
                    VcsBackend::Git | VcsBackend::Auto
                ) {
                    let merge_in_progress =
                        git_commands::is_merge_in_progress(workspace_manager.repo_root())
                            .await
                            .map_err(OrchestratorError::from)?;

                    if merge_in_progress {
                        warn!(
                            "Merge still in progress after resolve attempt {}/{}",
                            attempt, max_retries
                        );
                        send_event(
                            event_tx,
                            ParallelEvent::ResolveOutput {
                                output:
                                    "Merge still in progress (MERGE_HEAD exists); retrying resolve"
                                        .to_string(),
                            },
                        )
                        .await;
                        continue;
                    }
                }

                send_event(event_tx, ParallelEvent::ConflictResolutionCompleted).await;
                return Ok(());
            }
            warn!(
                "Conflicts still present after merge resolution attempt: {:?}",
                remaining_conflicts
            );
        } else {
            warn!(
                "Merge resolution attempt {} failed with exit code: {:?}",
                attempt,
                status.code()
            );
        }
    }

    let error_msg = format!("Failed to resolve merges after {} attempts", max_retries);
    send_event(
        event_tx,
        ParallelEvent::ConflictResolutionFailed {
            error: error_msg.clone(),
        },
    )
    .await;

    match workspace_manager.backend_type() {
        VcsBackend::Git | VcsBackend::Auto => Err(OrchestratorError::GitConflict(error_msg)),
    }
}
