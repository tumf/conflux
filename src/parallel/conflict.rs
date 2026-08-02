//! Conflict detection and resolution logic for parallel execution.

use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::history::{ResolveAttempt, ResolveContext};
use crate::vcs::{VcsBackend, WorkspaceManager};
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::events::{send_event, ParallelEvent};
use super::resolve_state::{
    self, BatchState, GitResolveEvidence, ResolveEvidence, SequentialMergeItem,
};

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
    let ai_runner = AiCommandRunner::from_orchestrator_config(config, shared_stagger_state.clone());

    // Build initial resolve command to send in ResolveStarted event (before retry loop)
    let initial_resolve_prompt = crate::agent::append_optional_prompt(
        build_conflict_resolve_prompt(
            config.get_resolve_skill(),
            vcs_prompt_prefix,
            &revisions.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            vcs_error,
            &vcs_status,
            &vcs_log,
            &conflict_files_str,
        ),
        config.get_resolve_append_prompt(),
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
        let mut resolve_prompt = build_conflict_resolve_prompt(
            config.get_resolve_skill(),
            vcs_prompt_prefix,
            &revisions.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            vcs_error,
            &vcs_status,
            &vcs_log,
            &conflict_files_str,
        );

        // Add context from previous attempts if any
        let continuation_context = resolve_context.format_continuation_context();
        if !continuation_context.is_empty() {
            resolve_prompt = format!("{}\n\n{}", resolve_prompt, continuation_context);
        }

        resolve_prompt = crate::agent::append_optional_prompt(
            resolve_prompt,
            config.get_resolve_append_prompt(),
        );

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
    /// Ordered batch admitted for sequential integration, including each
    /// change's archive worktree path.
    pub items: &'a [SequentialMergeItem],
    pub target_branch: &'a str,
    pub base_revision: &'a str,
    pub max_retries: u32,
    pub shared_stagger_state: crate::ai_command_runner::SharedStaggerState,
    pub auto_resolve_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// Upstream publication, not local integration, owns the change's terminal
    /// success for this cumulative merge.
    ///
    /// A per-change `ResolveCompleted` finalizes the reducer as terminal
    /// `merged`, which for an opted-in change is wrong twice over: the change is
    /// not yet published, and a terminal state swallows the later `PushFailed`
    /// that F5 retry needs. Publication emits its own change-scoped progress, so
    /// the finalizing event is suppressed here — mirroring the `MergeCompleted`
    /// suppression on the same path.
    pub publication_owns_completion: bool,
}

/// Emit the per-change resolve completion unless publication owns completion.
async fn send_resolve_completed(
    event_tx: &Option<mpsc::Sender<ParallelEvent>>,
    change_ids: &[String],
    publication_owns_completion: bool,
) {
    if publication_owns_completion {
        return;
    }
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
}

/// Render the worktree location block from validated Git identity.
///
/// Every admitted item is listed. A supplied path that cannot be validated is
/// reported with its concrete unsafe reason rather than collapsed to
/// `(unknown)`, so the agent never sees an anonymous location and the operator
/// can tell a stale path from a skipped one.
pub(crate) async fn render_worktree_locations(
    evidence: &dyn ResolveEvidence,
    items: &[SequentialMergeItem],
) -> String {
    let mut lines = Vec::with_capacity(items.len());
    for item in items {
        let identity = evidence
            .validate_worktree(&item.archive_path, &item.revision)
            .await;
        let location = match identity.resolved() {
            Some((path, tip)) => format!(
                "- {} => {} (change_id: {}, tip: {})",
                item.revision,
                path.display(),
                item.change_id,
                tip
            ),
            None => format!(
                "- {} => {} (change_id: {}, unvalidated: {})",
                item.revision,
                item.archive_path.display(),
                item.change_id,
                match &identity {
                    crate::vcs::git::commands::WorktreeIdentity::Unsafe { reason } =>
                        reason.as_str(),
                    _ => "unknown",
                }
            ),
        };
        lines.push(location);
    }
    lines.join("\n")
}

/// Attempt to resolve merges with retries using the configured resolve command.
pub async fn resolve_merges_with_retry(args: ResolveMergesWithRetryArgs<'_>) -> Result<()> {
    let ResolveMergesWithRetryArgs {
        workspace_manager,
        config,
        event_tx,
        items,
        target_branch,
        base_revision,
        max_retries,
        shared_stagger_state,
        auto_resolve_count,
        publication_owns_completion,
    } = args;

    // Create RAII guard to ensure counter is decremented on all exit paths
    let _guard = AutoResolveGuard::new(auto_resolve_count);

    send_event(event_tx, ParallelEvent::ConflictResolutionStarted).await;

    let revisions = SequentialMergeItem::revisions(items);
    let revisions = revisions.as_slice();
    let change_ids = SequentialMergeItem::change_ids(items);
    let change_ids = change_ids.as_slice();

    let is_git = matches!(
        workspace_manager.backend_type(),
        VcsBackend::Git | VcsBackend::Auto
    );
    let evidence = GitResolveEvidence::new(workspace_manager.repo_root());

    // Repository evidence decides entry, not process memory. A conflict-free
    // `MERGE_HEAD` no longer short-circuits into a blind commit: it enters the
    // classifier like every other state and reaches the agent as identity-proven
    // continuation guidance.
    let mut batch_state = if is_git {
        Some(resolve_state::classify_batch(&evidence, items, base_revision).await)
    } else {
        None
    };

    if batch_state.as_ref().is_some_and(BatchState::is_complete) {
        info!(
            "Sequential merge batch is already complete for revisions: {}",
            revisions.join(", ")
        );
        send_event(event_tx, ParallelEvent::ConflictResolutionCompleted).await;
        send_resolve_completed(event_tx, change_ids, publication_owns_completion).await;
        return Ok(());
    }

    let conflict_files = detect_conflicts(workspace_manager).await?;
    let conflict_files_str = conflict_files.join(", ");

    let vcs_status = get_vcs_status(workspace_manager).await.unwrap_or_default();
    let vcs_log = get_vcs_log_for_revisions(workspace_manager, revisions)
        .await
        .unwrap_or_default();

    let vcs_prompt_prefix = workspace_manager.conflict_resolution_prompt();

    let merge_plan = items
        .iter()
        .map(|item| format!("- {} => {}", item.revision, item.change_id))
        .collect::<Vec<_>>()
        .join("\n");

    let worktree_locations = if is_git {
        render_worktree_locations(&evidence, items).await
    } else {
        items
            .iter()
            .map(|item| {
                format!(
                    "- {} => {} (change_id: {})",
                    item.revision,
                    item.archive_path.display(),
                    item.change_id
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let phase_diagnosis = batch_state
        .as_ref()
        .map(BatchState::diagnosis)
        .unwrap_or_default();

    // Create resolve context for tracking attempts
    let mut resolve_context = ResolveContext::new(max_retries);

    // Create a combined change_id for logging (join multiple IDs if present)
    let combined_change_id = change_ids.join("+");

    // Create AiCommandRunner for resolve command execution
    use crate::ai_command_runner::AiCommandRunner;
    let ai_runner = AiCommandRunner::from_orchestrator_config(config, shared_stagger_state.clone());

    // Build initial resolve command to send in ResolveStarted event (before retry loop)
    let initial_resolve_prompt = crate::agent::append_optional_prompt(
        build_sequential_merge_resolve_prompt(
            config.get_resolve_skill(),
            vcs_prompt_prefix,
            target_branch,
            base_revision,
            &merge_plan,
            &worktree_locations,
            &vcs_status,
            &vcs_log,
            &conflict_files_str,
            &phase_diagnosis,
        ),
        config.get_resolve_append_prompt(),
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

        // The newest classification is the authoritative continuation guidance
        // for this attempt.
        let phase_diagnosis = batch_state
            .as_ref()
            .map(BatchState::diagnosis)
            .unwrap_or_default();

        let mut resolve_prompt = build_sequential_merge_resolve_prompt(
            config.get_resolve_skill(),
            vcs_prompt_prefix,
            target_branch,
            base_revision,
            &merge_plan,
            &worktree_locations,
            &vcs_status,
            &vcs_log,
            &conflict_files_str,
            &phase_diagnosis,
        );

        // Add context from previous attempts if any
        let continuation_context = resolve_context.format_continuation_context();
        if !continuation_context.is_empty() {
            resolve_prompt = format!("{}\n\n{}", resolve_prompt, continuation_context);
        }

        resolve_prompt = crate::agent::append_optional_prompt(
            resolve_prompt,
            config.get_resolve_append_prompt(),
        );

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
            if is_git {
                // Repository evidence, not the agent's exit status or prose,
                // decides whether the batch progressed. Every unfinished state
                // returns as one structured phase diagnosis instead of the old
                // generic "missing merge commits" fallback.
                let state = resolve_state::classify_batch(&evidence, items, base_revision).await;
                let complete = state.is_complete();
                let reason = state.diagnosis();
                batch_state = Some(state);

                if !complete {
                    warn!(
                        "Sequential merge batch incomplete after resolve attempt {}/{}: {}",
                        attempt,
                        max_retries,
                        reason.replace('\n', " | ")
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
            send_resolve_completed(event_tx, change_ids, publication_owns_completion).await;

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

/// Build prompt for conflict resolution (variable context only; fixed guidance lives in cflx-resolve).
fn build_conflict_resolve_prompt(
    resolve_skill: &str,
    vcs_prompt_prefix: &str,
    revisions: &[&str],
    vcs_error: &str,
    vcs_status: &str,
    vcs_log: &str,
    conflict_files_str: &str,
) -> String {
    format!(
        "{}\n\n\
         {}\n\n\
         Conflicting revisions: {}\n\n\
         VCS error output:\n\
         {}\n\n\
         Current VCS status:\n\
         {}\n\n\
         VCS log for conflicting changes:\n\
         {}\n\n\
         Conflicting files: {}",
        crate::agent::prompt::skill_prelude(resolve_skill),
        vcs_prompt_prefix,
        revisions.join(", "),
        vcs_error,
        vcs_status,
        vcs_log,
        conflict_files_str
    )
}

/// Build prompt for sequential merge resolution (variable context only; fixed guidance lives in cflx-resolve).
#[allow(clippy::too_many_arguments)]
fn build_sequential_merge_resolve_prompt(
    resolve_skill: &str,
    vcs_prompt_prefix: &str,
    target_branch: &str,
    base_revision: &str,
    merge_plan: &str,
    worktree_locations: &str,
    vcs_status: &str,
    vcs_log: &str,
    conflict_files_str: &str,
    phase_diagnosis: &str,
) -> String {
    let diagnosis_block = if phase_diagnosis.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n\nRepository-derived phase diagnosis:\n{}",
            phase_diagnosis
        )
    };
    format!(
        "{}\n\n\
         {}\n\n\
         Operation: sequential merge\n\n\
         Target branch: {}\n\
         Base revision before merges: {}\n\
         Merge plan (branch => change_id):\n{}\n\n\
         Worktree directories (branch => path):\n{}\n\n\
         Current VCS status:\n{}\n\n\
         VCS log for branches:\n{}\n\n\
         Conflicting files (repo root, if any): {}{}",
        crate::agent::prompt::skill_prelude(resolve_skill),
        vcs_prompt_prefix,
        target_branch,
        base_revision,
        merge_plan,
        worktree_locations,
        vcs_status,
        vcs_log,
        conflict_files_str,
        diagnosis_block
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_resolve_prompt_has_skill_prelude() {
        let prompt = build_conflict_resolve_prompt(
            crate::config::defaults::DEFAULT_RESOLVE_SKILL,
            "Git conflict resolution:",
            &["branch-a", "branch-b"],
            "merge failed",
            "UU file.rs",
            "commit log here",
            "file.rs",
        );
        assert!(prompt.contains("$cflx-resolve"));
        assert!(prompt.contains("load skills: cflx-resolve"));
        // Variable context present
        assert!(prompt.contains("branch-a, branch-b"));
        assert!(prompt.contains("file.rs"));
    }

    #[test]
    fn test_conflict_resolve_prompt_uses_custom_skill_prelude() {
        let prompt = build_conflict_resolve_prompt(
            "team-resolve",
            "prefix",
            &["rev1"],
            "err",
            "status",
            "log",
            "files",
        );
        assert!(prompt.contains("$team-resolve"));
        assert!(prompt.contains("load skills: team-resolve"));
        assert!(!prompt.contains("$cflx-resolve"));
    }

    #[test]
    fn test_conflict_resolve_prompt_no_fixed_guidance() {
        let prompt = build_conflict_resolve_prompt(
            crate::config::defaults::DEFAULT_RESOLVE_SKILL,
            "prefix",
            &["rev1"],
            "err",
            "status",
            "log",
            "files",
        );
        // Fixed guidance must NOT appear (owned by cflx-resolve skill)
        assert!(
            !prompt.contains("Please resolve the merge conflicts"),
            "Resolution instruction must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("Safety Constraints"),
            "Safety constraints must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("--no-verify"),
            "Safety rules must not be in Rust prompt"
        );
    }

    #[test]
    fn resolve_append_prompt_is_applied_to_conflict_resolve_command_tail() {
        let prompt = crate::agent::append_optional_prompt(
            build_conflict_resolve_prompt(
                crate::config::defaults::DEFAULT_RESOLVE_SKILL,
                "Git conflict resolution:",
                &["branch-a", "branch-b"],
                "merge failed",
                "UU file.rs",
                "commit log here",
                "file.rs",
            ),
            Some("resolve tail {change_id}"),
        );
        let command =
            crate::config::OrchestratorConfig::expand_prompt("agent --prompt '{prompt}'", &prompt);

        assert_eq!(command.matches("resolve tail {change_id}").count(), 1);
        assert!(command.ends_with("resolve tail {change_id}'"));
        assert!(!command.contains("branch-a resolve tail"));
    }

    #[test]
    fn test_sequential_merge_prompt_has_skill_prelude() {
        let prompt = build_sequential_merge_resolve_prompt(
            crate::config::defaults::DEFAULT_RESOLVE_SKILL,
            "Git conflict resolution:",
            "main",
            "abc123",
            "- branch-a => change-1",
            "- branch-a => /path (change_id: change-1)",
            "clean",
            "log entries",
            "(none)",
            "phase: final_merge_missing",
        );
        assert!(prompt.contains("$cflx-resolve"));
        assert!(prompt.contains("load skills: cflx-resolve"));
        assert!(prompt.contains("Operation: sequential merge"));
        // Variable context present
        assert!(prompt.contains("Target branch: main"));
        assert!(prompt.contains("abc123"));
        assert!(prompt.contains("change-1"));
    }

    #[test]
    fn test_sequential_merge_prompt_uses_custom_skill_prelude() {
        let prompt = build_sequential_merge_resolve_prompt(
            "team-resolve",
            "prefix",
            "main",
            "base",
            "plan",
            "locations",
            "status",
            "log",
            "files",
            "",
        );
        assert!(prompt.contains("$team-resolve"));
        assert!(prompt.contains("load skills: team-resolve"));
        assert!(!prompt.contains("$cflx-resolve"));
    }

    #[test]
    fn test_sequential_merge_prompt_no_fixed_guidance() {
        let prompt = build_sequential_merge_resolve_prompt(
            crate::config::defaults::DEFAULT_RESOLVE_SKILL,
            "prefix",
            "main",
            "base",
            "plan",
            "locations",
            "status",
            "log",
            "files",
            "",
        );
        // Fixed guidance must NOT appear (owned by cflx-resolve skill)
        assert!(
            !prompt.contains("Requirements:"),
            "Requirements section must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("Instructions (repeat"),
            "Step-by-step instructions must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("Pre-sync base into"),
            "Commit convention must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("Merge change:"),
            "Merge commit convention must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("git merge --no-ff"),
            "Git commands must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("git rm -rf"),
            "Git cleanup commands must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("--no-verify"),
            "Safety rules must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("Complete the merges"),
            "Completion instruction must not be in Rust prompt"
        );
    }
}
