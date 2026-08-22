//! Workspace execution logic for apply and archive operations.

use crate::agent::{AgentRunner, CleanupReviewDiagnostic, CleanupReviewFailureKind, OutputLine};
use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::execution::apply as common_apply;
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::parallel::output_bridge::ParallelApplyEventHandler;

use super::archive_state::delete_archive_state;
use super::events::ParallelEvent;
use crate::orchestration::build_acceptance_tail_findings;
use crate::stall::StallDetector;
use crate::vcs::git::commands as git_commands;
use crate::vcs::git::commands::has_uncommitted_changes;
use crate::vcs::git::GitWorkspaceManager;
use crate::vcs::VcsBackend;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

async fn wait_for_streaming_child_with_cancel(
    child: &mut crate::process_manager::StreamingChildHandle,
    cancel_token: Option<&CancellationToken>,
    operation: &str,
    change_id: &str,
    workspace_path: &Path,
    attempt: Option<u32>,
) -> Result<std::process::ExitStatus> {
    if let Some(token) = cancel_token {
        tokio::select! {
            _ = token.cancelled() => {
                warn!(
                    operation = operation,
                    change_id = change_id,
                    workspace = %workspace_path.display(),
                    attempt = attempt,
                    "Cancellation observed while waiting for child status; terminating child"
                );
                let _ = child.terminate();
                // Typed intentional stop: the run boundary must be able to tell
                // cancellation apart from an agent-command crash without
                // matching on message text.
                Err(OrchestratorError::cancelled(operation, change_id, workspace_path))
            }
            status = child.wait() => status.map_err(|e| {
                let attempt_context = attempt
                    .map(|attempt| format!(" (attempt {})", attempt))
                    .unwrap_or_default();
                OrchestratorError::AgentCommand(format!(
                    "Failed to wait for {} command for '{}' in workspace '{}'{}: {}",
                    operation,
                    change_id,
                    workspace_path.display(),
                    attempt_context,
                    e
                ))
            }),
        }
    } else {
        child.wait().await.map_err(|e| {
            let attempt_context = attempt
                .map(|attempt| format!(" (attempt {})", attempt))
                .unwrap_or_default();
            OrchestratorError::AgentCommand(format!(
                "Failed to wait for {} command for '{}' in workspace '{}'{}: {}",
                operation,
                change_id,
                workspace_path.display(),
                attempt_context,
                e
            ))
        })
    }
}

/// Parallel execution context for hooks
#[derive(Debug, Clone, Default)]
pub struct ParallelHookContext {
    /// Workspace path (set as OPENSPEC_WORKSPACE_PATH env var)
    pub workspace_path: String,
    /// Group index (set as OPENSPEC_GROUP_INDEX env var)
    pub group_index: Option<u32>,
    /// Total changes being processed in this group
    #[allow(dead_code)] // Available for future use in hook context
    pub total_changes_in_group: usize,
    /// Total changes in the run
    pub total_changes: usize,
    /// Changes processed so far
    pub changes_processed: usize,
}

/// Build a HookContext with managed-workspace environment variables.
fn build_parallel_hook_context(
    change_id: &str,
    completed_tasks: u32,
    total_tasks: u32,
    apply_count: u32,
    parallel_ctx: Option<&ParallelHookContext>,
) -> HookContext {
    let (changes_processed, total_changes, remaining_changes) = match parallel_ctx {
        Some(ctx) => (
            ctx.changes_processed,
            ctx.total_changes,
            ctx.total_changes.saturating_sub(ctx.changes_processed),
        ),
        None => (0, 0, 0),
    };

    let mut ctx = HookContext::new(changes_processed, total_changes, remaining_changes, false)
        .with_change(change_id, completed_tasks, total_tasks)
        .with_apply_count(apply_count);

    // Add parallel-specific environment variables
    if let Some(parallel_ctx) = parallel_ctx {
        // These will be added to env_vars through a custom method
        ctx = ctx.with_parallel_context(&parallel_ctx.workspace_path, parallel_ctx.group_index);
    }

    ctx
}

/// Maximum corrective cleanup-review attempts after the initial one.
///
/// Fixed protocol safety bound rather than configuration: the initial attempt
/// plus these corrections gives three operation attempts, each of which contains
/// its own command-queue transport retries. Active-run memory only, so a restart
/// re-derives cleanup-review from workspace and Git evidence with a fresh budget.
const MAX_CLEANUP_REVIEW_RETRIES: u32 = 2;

/// Outcome of one cleanup-review operation attempt.
enum CleanupReviewAttempt {
    /// Command completed, exactly one standalone marker was emitted, and a fresh
    /// repository query proved the worktree clean.
    Success,
    /// An ordinary failure that may consume corrective budget.
    Failure(CleanupReviewDiagnostic),
    /// Classified permission/tool-policy denial. It enters the existing
    /// non-terminal hold immediately, starts no corrective attempt, and consumes
    /// no generic cleanup failure budget.
    PermissionDenied(crate::permission::PermissionDenial),
}

/// Post-apply cleanup-review with bounded operation-level correction.
///
/// The command queue already retries one cleanup-review command at transport
/// level. Above it, this owns at most three operation attempts. Every attempt is
/// validated in a fixed order — command ownership, then exactly one standalone
/// marker, then a fresh repository query — and each corrective prompt carries
/// only the latest bounded diagnostic.
///
/// Cancellation and classified permission denial are owned by their existing
/// routing: neither starts another attempt, and permission denial does not
/// consume the generic failure budget. All retry state lives in this future, so
/// nothing durable is written and a restart recomputes cleanup-review from the
/// workspace alone.
async fn run_post_apply_cleanup_review(
    change_id: &str,
    workspace_path: &Path,
    config: &OrchestratorConfig,
    ai_runner: &AiCommandRunner,
    cancel_token: Option<&CancellationToken>,
    event_tx: Option<&mpsc::Sender<ParallelEvent>>,
) -> Result<()> {
    let max_attempts = MAX_CLEANUP_REVIEW_RETRIES.saturating_add(1);
    let mut latest: Option<CleanupReviewDiagnostic> = None;

    for attempt in 1..=max_attempts {
        // Explicit cancellation never starts another attempt. It is a typed
        // intentional stop, not a cleanup failure, so the run boundary reports
        // it as a stop for both global cancellation and a per-change queue stop.
        if cancel_token.is_some_and(|token| token.is_cancelled()) {
            return Err(OrchestratorError::cancelled(
                "cleanup-review",
                change_id,
                workspace_path,
            ));
        }

        match run_cleanup_review_attempt(
            change_id,
            workspace_path,
            config,
            ai_runner,
            cancel_token,
            attempt,
            max_attempts,
            latest.as_ref(),
        )
        .await?
        {
            CleanupReviewAttempt::Success => {
                info!(
                    change_id = %change_id,
                    workspace = %workspace_path.display(),
                    attempt = attempt,
                    "Post-apply cleanup review succeeded and worktree is clean"
                );
                return Ok(());
            }
            CleanupReviewAttempt::PermissionDenied(denial) => {
                warn!(
                    change_id = %change_id,
                    category = denial.category.as_str(),
                    denied_target = %denial.denied_target,
                    "Cleanup-review blocked by permission/tool policy denial; entering non-terminal hold without a corrective attempt"
                );
                if let Some(tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ExecutionBlocked {
                            change_id: change_id.to_string(),
                            blocker: crate::events::StalledBlocker::permission_denial(
                                "cleanup-review",
                                &denial,
                            ),
                        })
                        .await;
                    let _ = tx
                        .send(ParallelEvent::WorkspaceStatusUpdated {
                            change_id: change_id.to_string(),
                            workspace_name: workspace_path
                                .file_name()
                                .map(|name| name.to_string_lossy().to_string())
                                .unwrap_or_else(|| workspace_path.display().to_string()),
                            status: crate::vcs::WorkspaceStatus::Blocked,
                        })
                        .await;
                }
                return Err(OrchestratorError::PermissionStalled {
                    denied_path: denial.denied_target.clone(),
                    guidance: denial.format_guidance(),
                });
            }
            CleanupReviewAttempt::Failure(diagnostic) => {
                warn!(
                    change_id = %change_id,
                    workspace = %workspace_path.display(),
                    attempt = attempt,
                    max_attempts = max_attempts,
                    failure_kind = diagnostic.kind.label(),
                    marker_count = diagnostic.marker_count,
                    "Cleanup-review attempt did not produce a handoff-ready worktree"
                );
                if let Some(tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::Log(
                            crate::events::LogEntry::warn(format!(
                                "Cleanup-review attempt {}/{} failed ({})",
                                attempt,
                                max_attempts,
                                diagnostic.kind.label()
                            ))
                            .with_change_id(change_id)
                            .with_operation("cleanup-review"),
                        ))
                        .await;
                }
                latest = Some(diagnostic);
            }
        }
    }

    // Exhaustion leaves the managed worktree exactly as the cleanup agent left
    // it, so an explicit operator retry re-derives cleanup from that evidence.
    let diagnostic = latest.expect("an exhausted cleanup-review loop recorded a failure");
    Err(OrchestratorError::AgentCommand(format!(
        "Cleanup-review failed on {} operation attempts for change '{}' in workspace '{}': {}",
        max_attempts,
        change_id,
        workspace_path.display(),
        format_cleanup_review_diagnostic(&diagnostic)
    )))
}

/// One-line bounded rendering of the latest cleanup-review diagnosis.
fn format_cleanup_review_diagnostic(diagnostic: &CleanupReviewDiagnostic) -> String {
    const MAX_TAIL_CHARS: usize = 400;

    fn condense(tail: &str) -> String {
        let single_line = tail.split_whitespace().collect::<Vec<_>>().join(" ");
        match single_line.char_indices().nth(MAX_TAIL_CHARS) {
            Some((idx, _)) => format!("{}...", &single_line[..idx]),
            None => single_line,
        }
    }

    let mut parts = vec![format!("failure_kind: {}", diagnostic.kind.label())];
    if let Some(code) = diagnostic.exit_code {
        parts.push(format!("exit_code: {}", code));
    }
    parts.push(format!(
        "standalone_clean_marker_count: {}",
        diagnostic.marker_count
    ));
    if let Some(status) = diagnostic
        .status_tail
        .as_deref()
        .filter(|tail| !tail.trim().is_empty())
    {
        parts.push(format!("status: {}", condense(status)));
    }
    // Reported independently of the primary kind: a command or marker failure
    // that also lost status evidence must say so in the terminal diagnostic.
    if let Some(status_error) = diagnostic
        .status_error
        .as_deref()
        .filter(|tail| !tail.trim().is_empty())
    {
        parts.push(format!("status_error: {}", condense(status_error)));
    }
    // stdout and stderr are bounded separately, so both are reported: an exit
    // code with only one stream is an incomplete picture of the failure.
    if let Some(stdout) = diagnostic
        .stdout_tail
        .as_deref()
        .filter(|tail| !tail.trim().is_empty())
    {
        parts.push(format!("stdout: {}", condense(stdout)));
    }
    if let Some(stderr) = diagnostic
        .stderr_tail
        .as_deref()
        .filter(|tail| !tail.trim().is_empty())
    {
        parts.push(format!("stderr: {}", condense(stderr)));
    }
    parts.join(" | ")
}

/// Run and validate exactly one cleanup-review operation attempt.
#[allow(clippy::too_many_arguments)]
async fn run_cleanup_review_attempt(
    change_id: &str,
    workspace_path: &Path,
    config: &OrchestratorConfig,
    ai_runner: &AiCommandRunner,
    cancel_token: Option<&CancellationToken>,
    attempt: u32,
    max_attempts: u32,
    previous: Option<&CleanupReviewDiagnostic>,
) -> Result<CleanupReviewAttempt> {
    let user_template = config.get_acceptance_command()?;
    let prompt = crate::agent::build_cleanup_review_prompt_with_skill(
        config.get_cleanup_review_skill(),
        change_id,
        previous,
    );
    let command = OrchestratorConfig::expand_prompt(
        &OrchestratorConfig::expand_change_id(user_template, change_id),
        &prompt,
    );

    info!(
        change_id = %change_id,
        workspace = %workspace_path.display(),
        attempt = attempt,
        max_attempts = max_attempts,
        "Starting post-apply cleanup review for dirty managed worktree"
    );

    let (mut child, mut output_rx) = ai_runner
        .execute_streaming_with_retry(
            &command,
            Some(workspace_path),
            Some("cleanup-review"),
            Some(change_id),
        )
        .await?;

    // Bounded tails for diagnostics; the marker contract is evaluated with a
    // fixed-size streaming scanner so an attempt that floods stdout cannot grow
    // this frame's memory with retained output.
    let mut output_collector = crate::history::OutputCollector::new();
    let mut marker_scanner = crate::agent::CleanupMarkerScanner::new();
    loop {
        let line = if let Some(token) = cancel_token {
            tokio::select! {
                _ = token.cancelled() => {
                    warn!(
                        change_id = %change_id,
                        workspace = %workspace_path.display(),
                        attempt = attempt,
                        "Cancellation observed while streaming cleanup-review output; terminating child"
                    );
                    // Terminate through the managed handle, then drain and close
                    // the owned channel so no producer is left writing into it.
                    let _ = child.terminate();
                    output_rx.close();
                    while output_rx.recv().await.is_some() {}
                    return Err(OrchestratorError::cancelled(
                        "cleanup-review",
                        change_id,
                        workspace_path,
                    ));
                }
                line = output_rx.recv() => line,
            }
        } else {
            output_rx.recv().await
        };

        let Some(line) = line else { break };
        match line {
            crate::ai_command_runner::OutputLine::Stdout(s) => {
                output_collector.add_stdout(&s);
                marker_scanner.observe(&s);
            }
            crate::ai_command_runner::OutputLine::Stderr(s) => {
                output_collector.add_stderr(&s);
            }
        }
    }

    let status = wait_for_streaming_child_with_cancel(
        &mut child,
        cancel_token,
        "cleanup-review",
        change_id,
        workspace_path,
        Some(attempt),
    )
    .await?;

    let stdout_tail = output_collector.stdout_tail();
    let stderr_tail = output_collector.stderr_tail();
    let marker_count = marker_scanner.count();

    // Step 1 — command ownership. A classified permission/tool-policy denial is
    // distinguished before generic command failure so it can enter the existing
    // non-terminal hold without consuming corrective budget.
    if let Some(denial) = crate::permission::classify_permission_denial(&[
        stdout_tail.as_deref(),
        stderr_tail.as_deref(),
    ]) {
        return Ok(CleanupReviewAttempt::PermissionDenied(denial));
    }

    if !status.success() {
        // A status query that failed at the same time is kept as its own field:
        // dropping it would make "no status evidence" indistinguishable from a
        // clean worktree in the corrective prompt.
        let (status_tail, status_error) = split_status_inspection(workspace_path).await;
        return Ok(CleanupReviewAttempt::Failure(CleanupReviewDiagnostic {
            kind: CleanupReviewFailureKind::CommandFailed,
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
            marker_count,
            status_tail,
            status_error,
        }));
    }

    // Step 2 — protocol marker. Missing and duplicate markers both fail.
    if marker_count != 1 {
        let (status_tail, status_error) = split_status_inspection(workspace_path).await;
        let kind = if marker_count == 0 {
            CleanupReviewFailureKind::MarkerMissing
        } else {
            CleanupReviewFailureKind::MarkerDuplicate
        };
        return Ok(CleanupReviewAttempt::Failure(CleanupReviewDiagnostic {
            kind,
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
            marker_count,
            status_tail,
            status_error,
        }));
    }

    // Step 3 — repository truth. A marker never overrides Git state, and a
    // failed status query is never clean.
    match has_uncommitted_changes(workspace_path).await {
        Ok((false, _)) => Ok(CleanupReviewAttempt::Success),
        Ok((true, dirty_status)) => Ok(CleanupReviewAttempt::Failure(CleanupReviewDiagnostic {
            kind: CleanupReviewFailureKind::DirtyRemains,
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
            marker_count,
            status_tail: Some(dirty_status),
            status_error: None,
        })),
        Err(e) => Ok(CleanupReviewAttempt::Failure(CleanupReviewDiagnostic {
            kind: CleanupReviewFailureKind::StatusInspectionFailed,
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
            marker_count,
            status_tail: None,
            status_error: Some(format!("status inspection failed: {}", e)),
        })),
    }
}

/// Fresh porcelain evidence for a corrective prompt, split into the observed
/// status and the inspection error.
///
/// Exactly one side is `Some`: a failed query is reported as an error rather
/// than silently omitted, so the next attempt can tell "unproven" apart from
/// "nothing reported".
async fn split_status_inspection(workspace_path: &Path) -> (Option<String>, Option<String>) {
    match current_porcelain_status(workspace_path).await {
        Ok(status) => (Some(status), None),
        Err(e) => (None, Some(e.to_string())),
    }
}

/// Fresh porcelain evidence for a corrective prompt.
///
/// A failed query is reported as such rather than silently omitted, so the next
/// attempt knows cleanliness is unproven.
async fn current_porcelain_status(workspace_path: &Path) -> Result<String> {
    match has_uncommitted_changes(workspace_path).await {
        Ok((true, status)) => Ok(status),
        Ok((false, _)) => Ok("clean".to_string()),
        Err(e) => Err(OrchestratorError::GitCommand(format!(
            "status inspection failed: {}",
            e
        ))),
    }
}

async fn mark_acceptance_context_injected(
    agent: &AgentRunner,
    change_id: &str,
    acceptance_tail_injected: &Arc<Mutex<std::collections::HashMap<String, bool>>>,
) {
    if agent.acceptance_context_was_injected(change_id) {
        acceptance_tail_injected
            .lock()
            .await
            .insert(change_id.to_string(), true);
    }
}

async fn prepare_acceptance_context_for_apply(
    agent: &mut AgentRunner,
    change_id: &str,
    acceptance_history: &Arc<Mutex<crate::history::AcceptanceHistory>>,
    acceptance_tail_injected: &Arc<Mutex<std::collections::HashMap<String, bool>>>,
) {
    agent.seed_acceptance_history(acceptance_history.lock().await.clone());
    let already_injected = acceptance_tail_injected
        .lock()
        .await
        .get(change_id)
        .copied()
        .unwrap_or(false);
    if already_injected {
        let _ = agent.get_acceptance_tail_context_for_apply(change_id);
    }
}

/// Execute apply command in a single workspace, repeating until tasks are 100% complete.
///
/// Returns (revision, final_iteration_count) on success.
#[allow(clippy::too_many_arguments)]
pub async fn execute_apply_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    _apply_cmd_template: &str,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
    hooks: Option<&HookRunner>,
    parallel_ctx: Option<&ParallelHookContext>,
    cancel_token: Option<&CancellationToken>,
    ai_runner: &AiCommandRunner,
    repo_root: &Path,
    _apply_history: &Arc<Mutex<crate::history::ApplyHistory>>,
    acceptance_history: &Arc<Mutex<crate::history::AcceptanceHistory>>,
    acceptance_tail_injected: &Arc<Mutex<std::collections::HashMap<String, bool>>>,
    // The sole per-change `max_iterations` owner for this process run. Replaces
    // the previous `_initial_iteration` plumbing: parallel no longer carries a
    // per-cycle starting count, because the shared owner already holds the
    // cumulative per-change total across every Apply entry.
    apply_budget: &common_apply::ApplyBudget,
) -> Result<(
    String,
    u32,
    Option<crate::execution::apply::ApplyBlockedHandoff>,
    Option<crate::execution::apply::ApplyRejectedHandoff>,
)> {
    // Validate that workspace_path is a worktree, not the base repository
    match git_commands::is_worktree(repo_root, workspace_path).await {
        Ok(true) => {
            info!(
                "Workspace path validation passed: {} is a valid worktree",
                workspace_path.display()
            );
        }
        Ok(false) => {
            let error_msg = format!(
                "Parallel apply execution guard: workspace_path is NOT a worktree (executing in base repository is forbidden)
\
                 change_id: {}
\
                 workspace_path: {}
\
                 repo_root: {}",
                change_id,
                workspace_path.display(),
                repo_root.display()
            );
            return Err(OrchestratorError::GitCommand(error_msg));
        }
        Err(e) => {
            let error_msg = format!(
                "Failed to validate worktree status for parallel apply
\
                 change_id: {}
\
                 workspace_path: {}
\
                 repo_root: {}
\
                 validation_error: {}",
                change_id,
                workspace_path.display(),
                repo_root.display(),
                e
            );
            return Err(OrchestratorError::GitCommand(error_msg));
        }
    }

    // Create AgentRunner for execute_apply_loop
    let mut agent = AgentRunner::new(config.clone());
    prepare_acceptance_context_for_apply(
        &mut agent,
        change_id,
        acceptance_history,
        acceptance_tail_injected,
    )
    .await;

    // Clone event_tx for permission error handling before moving it to event_handler
    let event_tx_for_permission = event_tx.clone();

    // Create event handler for apply loop
    let event_handler = ParallelApplyEventHandler::new(change_id.to_string(), event_tx);

    // Create hook context for apply loop
    // The managed workspace is always known here, so a change-level hook always
    // carries workspace and group identity — with or without run-level counts.
    let hook_ctx = match parallel_ctx {
        Some(ctx) => common_apply::ApplyLoopHookContext::new(
            ctx.changes_processed,
            ctx.total_changes,
            ctx.total_changes.saturating_sub(ctx.changes_processed),
            workspace_path.to_string_lossy().to_string(),
            ctx.group_index.unwrap_or(0) as usize,
        ),
        None => common_apply::ApplyLoopHookContext::new(
            0,
            0,
            0,
            workspace_path.to_string_lossy().to_string(),
            0,
        ),
    };

    // Create workspace manager for WIP commit/stall detection
    // The workspace (Git worktree) is already created, so we just need a manager
    // for commit operations
    let workspace_manager = GitWorkspaceManager::new(
        workspace_path.parent().unwrap_or(repo_root).to_path_buf(),
        repo_root.to_path_buf(),
        1, // max_concurrent (not used for existing worktree)
        config.clone(),
    );

    // Execute apply loop using common implementation
    let apply_result = match common_apply::execute_apply_loop(
        change_id,
        workspace_path,
        config,
        &mut agent,
        vcs_backend,
        Some(&workspace_manager), // Pass workspace_manager for WIP commits and stall detection
        hooks,
        &hook_ctx,
        &event_handler,
        cancel_token,
        ai_runner,
        apply_budget,
        |_line| async move {
            // Output is handled by event_handler
        },
    )
    .await
    {
        Ok(result) => result,
        Err(crate::error::OrchestratorError::PermissionStalled {
            denied_path,
            guidance,
        }) => {
            warn!(
                "Repeated unresolved permission/tool policy denial for {} in workspace {}: {}",
                change_id,
                workspace_path.display(),
                denied_path
            );

            let denial = crate::permission::PermissionDenial {
                category: crate::permission::PermissionDenialCategory::CommandPolicy,
                denied_target: denied_path.clone(),
                evidence: guidance.clone(),
            };
            if let Some(ref tx) = event_tx_for_permission {
                let _ = tx
                    .send(ParallelEvent::ExecutionBlocked {
                        change_id: change_id.to_string(),
                        blocker: crate::events::StalledBlocker::permission_denial("apply", &denial),
                    })
                    .await;
                let _ = tx
                    .send(ParallelEvent::WorkspaceStatusUpdated {
                        change_id: change_id.to_string(),
                        workspace_name: workspace_path
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_else(|| workspace_path.display().to_string()),
                        status: crate::vcs::WorkspaceStatus::Blocked,
                    })
                    .await;
            }

            return Err(crate::error::OrchestratorError::PermissionStalled {
                denied_path,
                guidance,
            });
        }
        Err(crate::error::OrchestratorError::PermissionBlocked {
            denied_path,
            guidance,
        }) => {
            // Permission blocked - emit event and return error with guidance
            use tracing::warn;
            warn!(
                "Permission auto-rejected for {} in workspace {}: {}",
                change_id,
                workspace_path.display(),
                denied_path
            );

            // Send event if event_tx is available
            if let Some(ref tx) = event_tx_for_permission {
                use crate::parallel::ParallelEvent;
                let _ = tx
                    .send(ParallelEvent::ApplyFailed {
                        change_id: change_id.to_string(),
                        error: format!("Permission auto-rejected: {}\n{}", denied_path, guidance),
                    })
                    .await;
            }

            return Err(crate::error::OrchestratorError::PermissionBlocked {
                denied_path,
                guidance,
            });
        }
        Err(e) => return Err(e),
    };

    if let Some((attempt, findings)) = agent.get_acceptance_follow_up(change_id) {
        acceptance_history
            .lock()
            .await
            .set_follow_up_findings(change_id, attempt, findings);
    }
    mark_acceptance_context_injected(&agent, change_id, acceptance_tail_injected).await;

    if apply_result.blocked_handoff.is_none() && apply_result.rejected_handoff.is_none() {
        let (is_dirty, dirty_status) =
            has_uncommitted_changes(workspace_path).await.map_err(|e| {
                OrchestratorError::AgentCommand(format!(
                    "Failed to inspect worktree dirty state after apply completion for '{}': {}",
                    change_id, e
                ))
            })?;

        if is_dirty {
            warn!(
                change_id = %change_id,
                workspace = %workspace_path.display(),
                dirty_status = %dirty_status,
                "Managed worktree is dirty after apply completion; running post-apply cleanup review before acceptance handoff"
            );
            run_post_apply_cleanup_review(
                change_id,
                workspace_path,
                config,
                ai_runner,
                cancel_token,
                event_tx_for_permission.as_ref(),
            )
            .await?;
        }
    }

    info!(
        "Apply completed for {} (revision={})",
        change_id, apply_result.revision
    );

    // Return revision, iteration count, and apply-blocked handoff metadata
    Ok((
        apply_result.revision,
        apply_result.iterations,
        apply_result.blocked_handoff,
        apply_result.rejected_handoff,
    ))
}

/// Execute archive command in a workspace with streaming output
#[allow(clippy::too_many_arguments)]
pub async fn execute_archive_finalization_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
    ai_runner: &AiCommandRunner,
    shared_stagger_state: &Arc<Mutex<Option<std::time::Instant>>>,
) -> Result<String> {
    use crate::execution::archive::{ensure_archive_commit, verify_archive_completion};

    let verification = verify_archive_completion(change_id, Some(workspace_path));
    if !verification.is_success() {
        return Err(OrchestratorError::AgentCommand(format!(
            "Cannot resume archive finalization for '{}': archive move has regressed; active change directory is present or archive files are missing in '{}'",
            change_id,
            workspace_path.display()
        )));
    }

    if let Some(ref tx) = event_tx {
        let _ = tx
            .send(ParallelEvent::ArchiveResumed {
                change_id: change_id.to_string(),
                reason: Some("archive_commit_incomplete".to_string()),
                summary: Some(
                    "Archive move is already complete; resuming commit finalization only"
                        .to_string(),
                ),
            })
            .await;
    }

    let resolve_agent =
        AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());
    let change_id_owned = change_id.to_string();
    let event_tx_clone = event_tx.clone();
    ensure_archive_commit(
        change_id,
        workspace_path,
        &resolve_agent,
        ai_runner,
        vcs_backend,
        move |line| {
            let event_tx = event_tx_clone.clone();
            let change_id = change_id_owned.clone();
            async move {
                let text = match line {
                    OutputLine::Stdout(text) | OutputLine::Stderr(text) => text,
                };
                if let Some(ref tx) = event_tx {
                    if text.contains("Archive commit finalization retry scheduled") {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                crate::events::LogEntry::warn(text.clone())
                                    .with_change_id(&change_id)
                                    .with_operation("archive-finalization"),
                            ))
                            .await;
                    }
                    let _ = tx
                        .send(ParallelEvent::ArchiveOutput {
                            change_id,
                            output: text,
                            iteration: 1,
                        })
                        .await;
                }
            }
        },
    )
    .await?;

    if let Err(err) = delete_archive_state(workspace_path) {
        warn!(
            "Failed to delete archive state for {} after archive finalization resume: {}",
            change_id, err
        );
    }

    match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            let revision = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!(
                        "Failed to get revision after archive finalization resume: {}",
                        e
                    ))
                })?;
            if revision.status.success() {
                Ok(String::from_utf8_lossy(&revision.stdout).trim().to_string())
            } else {
                Err(OrchestratorError::GitCommand(format!(
                    "Failed to get revision after archive finalization resume: {}",
                    String::from_utf8_lossy(&revision.stderr).trim()
                )))
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_archive_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    archive_cmd_template: &str,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
    hooks: Option<&HookRunner>,
    parallel_ctx: Option<&ParallelHookContext>,
    cancel_token: Option<&CancellationToken>,
    ai_runner: &AiCommandRunner,
    archive_history: &Arc<Mutex<crate::history::ArchiveHistory>>,
    apply_history: &Arc<Mutex<crate::history::ApplyHistory>>,
    shared_stagger_state: &Arc<Mutex<Option<std::time::Instant>>>,
) -> Result<String> {
    if cancel_token.is_some_and(|token| token.is_cancelled()) {
        return Err(OrchestratorError::AgentCommand(format!(
            "Cancelled archive for '{}' in workspace '{}'",
            change_id,
            workspace_path.display()
        )));
    }

    // Verify task completion before archiving using common function
    use crate::execution::archive::get_task_progress;

    let progress = match get_task_progress(change_id, Some(workspace_path)) {
        Ok(Some(progress)) => {
            if progress.total == 0 {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Cannot archive '{}' in workspace '{}': tasks.md exists but contains no tasks (0 tasks found)",
                    change_id,
                    workspace_path.display()
                )));
            }
            if progress.completed < progress.total {
                return Err(OrchestratorError::AgentCommand(format!(
                    "Cannot archive '{}' in workspace '{}': tasks not complete ({}/{} tasks completed)",
                    change_id,
                    workspace_path.display(),
                    progress.completed,
                    progress.total
                )));
            }
            info!(
                "Task verification passed for {}: {}/{} tasks completed",
                change_id, progress.completed, progress.total
            );
            progress
        }
        Ok(None) => {
            return Err(OrchestratorError::AgentCommand(format!(
                "Cannot archive '{}' in workspace '{}': tasks.md not found at {} or in archive directory",
                change_id,
                workspace_path.display(),
                workspace_path
                    .join("openspec/changes")
                    .join(change_id)
                    .join("tasks.md")
                    .display()
            )));
        }
        Err(e) => {
            return Err(OrchestratorError::AgentCommand(format!(
                "Cannot archive '{}' in workspace '{}': failed to parse tasks.md: {}",
                change_id,
                workspace_path.display(),
                e
            )));
        }
    };

    crate::vcs::git::commands::get_current_commit(workspace_path)
        .await
        .map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Cannot archive '{}' in workspace '{}': failed to resolve current revision: {}",
                change_id,
                workspace_path.display(),
                e
            ))
        })?;

    let stall_detector = StallDetector::new(config.get_stall_detection());

    // Run pre_archive hook
    if let Some(hook_runner) = hooks {
        let hook_ctx = build_parallel_hook_context(
            change_id,
            progress.completed,
            progress.total,
            0, // apply_count not relevant for archive
            parallel_ctx,
        );

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(ParallelEvent::HookStarted {
                    change_id: change_id.to_string(),
                    hook_type: "pre_archive".to_string(),
                })
                .await;
        }

        match hook_runner.run_hook(HookType::PreArchive, &hook_ctx).await {
            Ok(()) => {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookCompleted {
                            change_id: change_id.to_string(),
                            hook_type: "pre_archive".to_string(),
                        })
                        .await;
                }
            }
            Err(e) => {
                error!("pre_archive hook failed for {}: {}", change_id, e);
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookFailed {
                            change_id: change_id.to_string(),
                            hook_type: "pre_archive".to_string(),
                            error: e.to_string(),
                        })
                        .await;
                }
                return Err(e);
            }
        }
    }

    // Build prompt with history context
    let user_prompt = config.get_archive_prompt();
    let history_context = {
        let history = archive_history.lock().await;
        history.format_context(change_id)
    };
    let full_prompt = crate::agent::append_optional_prompt(
        crate::agent::build_archive_prompt_with_skill(
            config.get_archive_skill(),
            change_id,
            user_prompt,
            &history_context,
        ),
        config.get_archive_append_prompt(),
    );

    // Expand change_id and prompt in archive command
    let command = OrchestratorConfig::expand_change_id(archive_cmd_template, change_id);
    let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);

    debug!("Archive command in workspace: {}", command);

    // Send ArchiveStarted event with expanded command
    if let Some(ref tx) = event_tx {
        let _ = tx
            .send(ParallelEvent::ArchiveStarted {
                change_id: change_id.to_string(),
                command: command.clone(),
            })
            .await;
    }

    use crate::execution::archive::{
        build_archive_error_message, ensure_archive_commit, extract_archive_runtime_blocker,
        verify_archive_completion, ARCHIVE_COMMAND_MAX_RETRIES,
    };
    use crate::history::ArchivePrimaryReason;

    let max_attempts = ARCHIVE_COMMAND_MAX_RETRIES.saturating_add(1);
    let mut attempt: u32 = 0;
    let is_git_repo = if matches!(vcs_backend, VcsBackend::Git | VcsBackend::Auto) {
        match git_commands::check_git_repo(workspace_path).await {
            Ok(is_repo) => is_repo,
            Err(e) => {
                warn!(
                    "Failed to check Git repository status for {}: {}",
                    change_id, e
                );
                false
            }
        }
    } else {
        false
    };
    let mut empty_commit_streak = 0u32;

    loop {
        attempt += 1;
        let start = std::time::Instant::now();

        // Execute command via AiCommandRunner (with stagger and retry)
        // Execute in workspace directory (cwd parameter)
        debug!(
            module = module_path!(),
            "Executing shell command via AiCommandRunner with retry: {} (cwd: {:?})",
            command,
            workspace_path
        );
        let (mut child, mut output_rx) = ai_runner
            .execute_streaming_with_retry(
                &command,
                Some(workspace_path),
                Some("archive"),
                Some(change_id),
            )
            .await?;

        // Create output collector for history
        let mut output_collector = crate::history::OutputCollector::new();

        // Forward output to event channel while observing cancellation even if
        // the archive command stays quiet after startup.
        use crate::ai_command_runner::OutputLine as AiOutputLine;
        let change_id_clone = change_id.to_string();
        let event_tx_clone = event_tx.clone();
        loop {
            let line = if let Some(token) = cancel_token {
                tokio::select! {
                    _ = token.cancelled() => {
                        warn!(
                            change_id = %change_id,
                            workspace = %workspace_path.display(),
                            attempt = attempt,
                            "Archive cancellation observed while waiting for streaming output; terminating child"
                        );
                        let _ = child.terminate();
                        return Err(OrchestratorError::AgentCommand(format!(
                            "Cancelled archive for '{}' in workspace '{}'",
                            change_id,
                            workspace_path.display()
                        )));
                    }
                    line = output_rx.recv() => line,
                }
            } else {
                output_rx.recv().await
            };

            let Some(line) = line else { break };

            // Collect output for history
            match &line {
                AiOutputLine::Stdout(s) => output_collector.add_stdout(s),
                AiOutputLine::Stderr(s) => output_collector.add_stderr(s),
            }

            if let Some(ref tx) = event_tx_clone {
                let output_text = match line {
                    AiOutputLine::Stdout(s) | AiOutputLine::Stderr(s) => s,
                };
                let _ = tx
                    .send(ParallelEvent::ArchiveOutput {
                        change_id: change_id_clone.clone(),
                        output: output_text,
                        iteration: attempt,
                    })
                    .await;
            }
        }

        // Wait for process to complete. Keep this cancellation-aware because
        // output pipes may close before the child process status is available.
        let status = wait_for_streaming_child_with_cancel(
            &mut child,
            cancel_token,
            "archive",
            change_id,
            workspace_path,
            Some(attempt),
        )
        .await?;

        if !status.success() {
            return Err(OrchestratorError::AgentCommand(format!(
                "Archive command failed for change '{}' in workspace '{}' (attempt {}) with exit code: {:?}",
                change_id,
                workspace_path.display(),
                attempt,
                status.code()
            )));
        }

        if is_git_repo {
            if let Err(e) =
                git_commands::create_archive_wip_commit(workspace_path, change_id, attempt).await
            {
                warn!(
                    "Failed to create WIP(archive) commit for {} (attempt {}): {}",
                    change_id, attempt, e
                );
            } else if stall_detector.config().enabled {
                match git_commands::is_head_empty_commit(workspace_path).await {
                    Ok(is_empty) => {
                        if is_empty {
                            empty_commit_streak = empty_commit_streak.saturating_add(1);
                        } else {
                            empty_commit_streak = 0;
                        }
                        if empty_commit_streak >= stall_detector.config().threshold {
                            let message = format!(
                                "Stall detected for {} after {} empty WIP commits (archive)",
                                change_id, empty_commit_streak
                            );
                            warn!(
                                "{} (threshold {})",
                                message,
                                stall_detector.config().threshold
                            );
                            return Err(OrchestratorError::AgentCommand(message));
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to check WIP(archive) commit for {} (attempt {}): {}",
                            change_id, attempt, e
                        );
                    }
                }
            }
        }

        let verification = verify_archive_completion(change_id, Some(workspace_path));

        // Record archive attempt in history
        {
            let mut history = archive_history.lock().await;
            let verification_result = if verification.is_success() {
                None
            } else {
                Some(format!(
                    "Change still exists at openspec/changes/{}",
                    change_id
                ))
            };
            let attempt_record = crate::history::ArchiveAttempt {
                attempt: history.count(change_id) + 1,
                success: status.success() && verification.is_success(),
                duration: start.elapsed(),
                error: if status.success() && verification.is_success() {
                    None
                } else if !status.success() {
                    Some(format!("Exit code: {:?}", status.code()))
                } else {
                    Some("Archive command succeeded but verification failed".to_string())
                },
                primary_reason: if status.success() && verification.is_success() {
                    None
                } else if !status.success() {
                    Some(ArchivePrimaryReason::CommandFailed)
                } else {
                    Some(ArchivePrimaryReason::VerificationFailed)
                },
                verification_result,
                exit_code: status.code(),
                stdout_tail: output_collector.stdout_tail(),
                stderr_tail: output_collector.stderr_tail(),
            };
            history.record(change_id, attempt_record);
        }

        if verification.is_success() {
            break;
        }

        if attempt <= ARCHIVE_COMMAND_MAX_RETRIES {
            let retry_summary =
                "archive verification failed; change directory still exists".to_string();
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ArchiveRetryScheduled {
                        change_id: change_id.to_string(),
                        attempt,
                        max_attempts,
                        reason: Some(
                            ArchivePrimaryReason::VerificationFailed
                                .as_str()
                                .to_string(),
                        ),
                        summary: Some(retry_summary),
                    })
                    .await;
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(format!(
                            "Archive verification failed for {} (attempt {}/{}); retrying archive command",
                            change_id, attempt, max_attempts
                        ))
                        .with_change_id(change_id)
                        .with_operation("archive")
                        .with_iteration(attempt),
                    ))
                    .await;
            }
            warn!(
                change_id = %change_id,
                attempt = attempt,
                max_attempts = max_attempts,
                "Archive verification failed; retrying archive command"
            );
            continue;
        }

        let runtime_blocker = extract_archive_runtime_blocker(
            output_collector.stdout_tail().as_deref(),
            output_collector.stderr_tail().as_deref(),
        );
        let final_error = build_archive_error_message(
            change_id,
            Some(workspace_path),
            runtime_blocker.as_deref(),
        );
        return Err(OrchestratorError::AgentCommand(final_error));
    }

    info!(
        "Archive verification passed for {}: change moved to archive",
        change_id
    );

    if is_git_repo {
        if let Err(e) = git_commands::squash_archive_wip_commits(workspace_path, change_id).await {
            warn!(
                "Failed to squash WIP(archive) commits for {}: {}",
                change_id, e
            );
        }
    }

    let resolve_agent =
        AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());
    let change_id_owned = change_id.to_string();
    let event_tx_clone = event_tx.clone();
    let final_attempt = attempt;
    ensure_archive_commit(
        change_id,
        workspace_path,
        &resolve_agent,
        ai_runner,
        vcs_backend,
        move |line| {
            let event_tx = event_tx_clone.clone();
            let change_id = change_id_owned.clone();
            let iteration = final_attempt;
            async move {
                let text = match line {
                    OutputLine::Stdout(text) | OutputLine::Stderr(text) => text,
                };
                if let Some(ref tx) = event_tx {
                    if text.contains("Archive commit finalization retry scheduled") {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                crate::events::LogEntry::warn(text.clone())
                                    .with_change_id(&change_id)
                                    .with_operation("archive-finalization")
                                    .with_iteration(iteration),
                            ))
                            .await;
                    }
                    let _ = tx
                        .send(ParallelEvent::ArchiveOutput {
                            change_id,
                            output: text,
                            iteration,
                        })
                        .await;
                }
            }
        },
    )
    .await?;

    // Get the current revision after archive
    // Note: The worktree may have been deleted by the archive command (e.g., /conflux:archive),
    // so we need to handle the case where the Git repository is no longer accessible.
    let revision = match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            debug!(
                module = module_path!(),
                "Executing git command: git rev-parse HEAD (cwd: {:?})", workspace_path
            );

            // Check if the workspace path still exists and is a Git repository
            if !workspace_path.exists() {
                warn!(
                    "Workspace path {:?} no longer exists after archive (likely deleted by archive command), using placeholder revision",
                    workspace_path
                );
                "archived".to_string()
            } else {
                match Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .current_dir(workspace_path)
                    .output()
                    .await
                {
                    Ok(revision_output) if revision_output.status.success() => {
                        String::from_utf8_lossy(&revision_output.stdout)
                            .trim()
                            .to_string()
                    }
                    Ok(revision_output) => {
                        let stderr = String::from_utf8_lossy(&revision_output.stderr);
                        warn!(
                            "Failed to get revision from workspace {:?} after archive: {} (likely deleted by archive command), using placeholder",
                            workspace_path, stderr
                        );
                        "archived".to_string()
                    }
                    Err(e) => {
                        warn!(
                            "Failed to execute git rev-parse in workspace {:?} after archive: {} (likely deleted by archive command), using placeholder",
                            workspace_path, e
                        );
                        "archived".to_string()
                    }
                }
            }
        }
    };

    // Run post_archive hook
    if let Some(hook_runner) = hooks {
        let hook_ctx = build_parallel_hook_context(
            change_id,
            progress.completed,
            progress.total,
            0, // apply_count not relevant for archive
            parallel_ctx,
        );

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(ParallelEvent::HookStarted {
                    change_id: change_id.to_string(),
                    hook_type: "post_archive".to_string(),
                })
                .await;
        }

        match hook_runner.run_hook(HookType::PostArchive, &hook_ctx).await {
            Ok(()) => {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookCompleted {
                            change_id: change_id.to_string(),
                            hook_type: "post_archive".to_string(),
                        })
                        .await;
                }
            }
            Err(e) => {
                error!("post_archive hook failed for {}: {}", change_id, e);
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::HookFailed {
                            change_id: change_id.to_string(),
                            hook_type: "post_archive".to_string(),
                            error: e.to_string(),
                        })
                        .await;
                }
                return Err(e);
            }
        }
    }

    // Clear history after successful archive
    {
        let mut apply_hist = apply_history.lock().await;
        apply_hist.clear(change_id);
        let mut archive_hist = archive_history.lock().await;
        archive_hist.clear(change_id);
    }

    if let Err(err) = delete_archive_state(workspace_path) {
        warn!(
            "Failed to delete archive state for {} after archive completion: {}",
            change_id, err
        );
    }

    Ok(revision)
}

/// Execute acceptance test in a workspace with streaming output
fn format_acceptance_failure_log_message(
    findings: &[crate::acceptance::AcceptanceFinding],
) -> String {
    let finding_count = findings.len();
    let blocking_gate_context = findings
        .first()
        .map(|finding| finding.text().to_string())
        .unwrap_or_else(|| "no acceptance findings captured".to_string());

    format!(
        "Acceptance failed ({} findings), blocking gate context: {}",
        finding_count, blocking_gate_context
    )
}

fn resolve_acceptance_state_revision(start_revision: &str, end_revision: Option<String>) -> String {
    end_revision.unwrap_or_else(|| start_revision.to_string())
}

fn revision_to_history_commit_hash(revision: &str) -> Option<String> {
    if revision == "unknown" {
        None
    } else {
        Some(revision.to_string())
    }
}

const ACCEPTANCE_VERDICT_GRACE_DEFAULT_SECS: u64 = 30;

tokio::task_local! {
    /// Test-only task-local override for the verdict grace period. Scoped to
    /// the calling task via [`scoped_verdict_grace_secs_for_test`] so it does
    /// not leak across concurrent tests the way an env var or static atomic
    /// would.
    pub(crate) static VERDICT_GRACE_OVERRIDE_SECS: u64;
}

/// Returns the grace period applied after detecting a canonical standalone
/// acceptance verdict. Defaults to 30 seconds. Tests may override the value
/// for the current task via [`scoped_verdict_grace_secs_for_test`].
pub(crate) fn acceptance_verdict_grace_period() -> std::time::Duration {
    let secs = VERDICT_GRACE_OVERRIDE_SECS
        .try_with(|secs| *secs)
        .ok()
        .filter(|secs| *secs > 0)
        .unwrap_or(ACCEPTANCE_VERDICT_GRACE_DEFAULT_SECS);
    std::time::Duration::from_secs(secs)
}

/// Run `fut` with the verdict grace period overridden to `secs`. The override
/// is scoped to the current task only; concurrent tasks (including parallel
/// tests) keep seeing the default. Test-only helper.
#[cfg(test)]
pub(crate) async fn scoped_verdict_grace_secs_for_test<F, R>(secs: u64, fut: F) -> R
where
    F: std::future::Future<Output = R>,
{
    VERDICT_GRACE_OVERRIDE_SECS.scope(secs, fut).await
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_acceptance_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    agent: &mut AgentRunner,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    cancel_token: Option<&CancellationToken>,
    ai_runner: &AiCommandRunner,
    config: &OrchestratorConfig,
    acceptance_tail_injected: &Arc<Mutex<std::collections::HashMap<String, bool>>>,
    acceptance_history: &Arc<Mutex<crate::history::AcceptanceHistory>>,
    base_branch: Option<&str>,
    protocol_retry: Option<crate::orchestration::acceptance::AcceptanceProtocolRetry>,
) -> Result<(crate::orchestration::AcceptanceResult, u32)> {
    use crate::acceptance::{parse_acceptance_output, AcceptanceResult as ParseResult};

    if cancel_token.is_some_and(|token| token.is_cancelled()) {
        return Ok((crate::orchestration::AcceptanceResult::Cancelled, 0));
    }

    info!("Running acceptance test for {} in workspace", change_id);

    // Capture current commit hash for diff tracking
    let commit_hash = crate::vcs::git::commands::get_current_commit(workspace_path)
        .await
        .ok(); // Allow to fail silently (non-git repos)

    // Get the acceptance iteration number (attempt number that will be used)
    let acceptance_iteration = agent.next_acceptance_attempt_number(change_id);

    // Build prompt with system instructions and history context
    let user_prompt = config.get_acceptance_prompt();
    let history_context = agent.format_acceptance_history(change_id);

    // Build diff context for all acceptance attempts
    let diff_context = {
        // Get current commit hash
        let current_commit = crate::vcs::git::commands::get_current_commit(workspace_path)
            .await
            .ok();

        // Determine base commit for diff
        let base_commit = {
            let acc_history = acceptance_history.lock().await;
            if acc_history.count(change_id) == 0 {
                // First acceptance: use base branch
                base_branch.map(|b| b.to_string())
            } else {
                // 2nd+ acceptance: use last acceptance commit
                acc_history.last_commit_hash(change_id)
            }
        };

        // Get changed files if we have both base and current commits
        if let (Some(base), Some(current)) = (base_commit.as_ref(), current_commit.as_ref()) {
            match crate::vcs::git::commands::get_changed_files(workspace_path, Some(base), current)
                .await
            {
                Ok(files) => {
                    // Get previous findings for 2nd+ attempts
                    let previous_findings = {
                        let acc_history = acceptance_history.lock().await;
                        if acc_history.count(change_id) > 0 {
                            acc_history
                                .last_findings(change_id)
                                .map(|findings| crate::acceptance::finding_texts(&findings))
                        } else {
                            None
                        }
                    };

                    // Build diff context if we have files or findings
                    if !files.is_empty() || previous_findings.is_some() {
                        crate::agent::build_acceptance_diff_context(
                            &files,
                            previous_findings.as_deref(),
                        )
                    } else {
                        String::new()
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to get changed files for acceptance diff context: {}",
                        e
                    );
                    String::new()
                }
            }
        } else {
            String::new()
        }
    };

    // Build last acceptance output context for 2nd+ attempts
    let stdout_tail = agent.get_last_acceptance_stdout_tail(change_id);
    let stderr_tail = agent.get_last_acceptance_stderr_tail(change_id);
    let previous_findings_text = agent
        .get_last_acceptance_finding_texts(change_id)
        .map(|findings| findings.join("\n"));
    // A denial observed by an invocation that never completed records no
    // canonical attempt, so its evidence is not in the history tails above. The
    // latest-only command-recovery diagnostic is where that classification
    // survives: without it, an unchanged repeated denial would be retried as an
    // ordinary command failure until the recovery budget is spent instead of
    // entering the existing non-terminal hold on the second observation.
    let previous_acceptance_denial = agent
        .acceptance_command_recovery(change_id)
        .and_then(crate::orchestration::acceptance::classify_acceptance_command_denial)
        .or_else(|| {
            crate::permission::classify_permission_denial(&[
                stdout_tail.as_deref(),
                stderr_tail.as_deref(),
                previous_findings_text.as_deref(),
            ])
        });
    let last_output_context = crate::agent::build_last_acceptance_output_context(
        stdout_tail.as_deref(),
        stderr_tail.as_deref(),
    );
    // Only a missing-verdict protocol retry carries the corrective continuation
    // block; ordinary acceptance invocations leave it empty.
    let protocol_retry_context = protocol_retry.map_or_else(String::new, |retry| {
        crate::agent::build_missing_verdict_continuation_context(
            retry,
            stdout_tail.as_deref(),
            stderr_tail.as_deref(),
            agent
                .get_last_acceptance_finding_texts(change_id)
                .as_deref(),
        )
    });

    // Latest-only command-recovery evidence for an Acceptance retry after a
    // command failure. Empty for every other invocation, and stored apart from
    // canonical acceptance history so it can never become a verdict.
    let command_recovery_context = crate::agent::build_acceptance_command_recovery_context(
        agent.acceptance_command_recovery(change_id),
    );

    // Build prompt injected into `{prompt}`
    let full_prompt = crate::agent::append_optional_prompt(
        match config.get_acceptance_prompt_mode() {
            crate::config::AcceptancePromptMode::Full => {
                crate::agent::build_acceptance_prompt_with_skill(
                    config.get_accept_skill(),
                    change_id,
                    user_prompt,
                    &history_context,
                    &last_output_context,
                    &diff_context,
                    &protocol_retry_context,
                    &command_recovery_context,
                )
            }
            crate::config::AcceptancePromptMode::ContextOnly => {
                crate::agent::build_acceptance_prompt_context_only_with_skill(
                    config.get_accept_skill(),
                    change_id,
                    user_prompt,
                    &history_context,
                    &last_output_context,
                    &diff_context,
                    &protocol_retry_context,
                    &command_recovery_context,
                )
            }
        },
        config.get_acceptance_append_prompt(),
    );

    // Expand change_id and prompt in command
    let template = config.get_acceptance_command()?;
    let command = OrchestratorConfig::expand_change_id(template, change_id);
    let command = OrchestratorConfig::expand_prompt(&command, &full_prompt);

    debug!(
        module = module_path!(),
        command = %crate::events::command_log_summary(&command),
        cwd = ?workspace_path,
        "Executing acceptance command via AiCommandRunner"
    );

    let start_revision = commit_hash.clone().unwrap_or_else(|| "unknown".to_string());

    // Send AcceptanceStarted event with command
    if let Some(ref tx) = event_tx {
        let _ = tx
            .send(ParallelEvent::AcceptanceStarted {
                change_id: change_id.to_string(),
                command: command.clone(),
            })
            .await;
    }

    // Capture start time for history recording
    let start_time = std::time::Instant::now();

    // Acceptance reviews an implementation rather than producing one, so it runs
    // under its own dedicated absolute deadline instead of Apply's much larger
    // `command_max_runtime_secs`. A positive common limit still caps it; a
    // disabled (`0`) common limit does not unbound it.
    let acceptance_runtime_limit_secs = config.get_acceptance_runtime_limit_secs();
    let acceptance_runner = ai_runner.with_max_runtime_secs(acceptance_runtime_limit_secs);

    // Execute command via AiCommandRunner (with stagger and retry)
    let (mut child, mut output_rx) = acceptance_runner
        .execute_streaming_with_retry(
            &command,
            Some(workspace_path),
            Some("acceptance"),
            Some(change_id),
        )
        .await?;

    // Create output collector for history
    let mut output_collector = crate::history::OutputCollector::new();
    let mut full_stdout = String::new();

    // Grace period after detecting a canonical standalone verdict before
    // terminating the acceptance child process. This handles the case where
    // the agent process (or its child processes) does not exit
    // promptly after emitting the verdict but keeps stdout/stderr pipes open,
    // which previously left acceptance to wait for inactivity timeout retry.
    let verdict_grace_period = acceptance_verdict_grace_period();

    let mut verdict_detected = false;
    let mut verdict_stream_detector = crate::acceptance::VerdictStreamDetector::default();
    let mut verdict_deadline: Option<tokio::time::Instant> = None;
    let mut early_terminated = false;

    // Stream output until channel closes or verdict grace period expires.
    use crate::ai_command_runner::OutputLine as AiOutputLine;
    loop {
        let line = if let Some(deadline) = verdict_deadline {
            let recv_future = output_rx.recv();
            let recv_with_deadline = tokio::time::timeout_at(deadline, recv_future);
            let recv_result = if let Some(token) = cancel_token {
                tokio::select! {
                    _ = token.cancelled() => {
                        warn!(
                            change_id = %change_id,
                            workspace = %workspace_path.display(),
                            iteration = acceptance_iteration,
                            "Acceptance cancellation observed while waiting for streaming output; terminating child"
                        );
                        let _ = child.terminate();
                        return Ok((crate::orchestration::AcceptanceResult::Cancelled, 0));
                    }
                    result = recv_with_deadline => result,
                }
            } else {
                recv_with_deadline.await
            };

            match recv_result {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(_) => {
                    info!(
                        "Acceptance verdict grace period ({}s) expired for {}, terminating child process",
                        verdict_grace_period.as_secs(),
                        change_id
                    );
                    let _ = child.terminate();
                    early_terminated = true;
                    break;
                }
            }
        } else if let Some(token) = cancel_token {
            tokio::select! {
                _ = token.cancelled() => {
                    warn!(
                        change_id = %change_id,
                        workspace = %workspace_path.display(),
                        iteration = acceptance_iteration,
                        "Acceptance cancellation observed while waiting for streaming output; terminating child"
                    );
                    let _ = child.terminate();
                    return Ok((crate::orchestration::AcceptanceResult::Cancelled, 0));
                }
                line = output_rx.recv() => match line {
                    Some(line) => line,
                    None => break,
                },
            }
        } else {
            match output_rx.recv().await {
                Some(line) => line,
                None => break,
            }
        };

        // Check for cancellation after receiving a line as well, so cancellation
        // wins before parsing or recording any later output.
        if cancel_token.is_some_and(|token| token.is_cancelled()) {
            warn!("Acceptance test cancelled for: {}", change_id);
            let _ = child.terminate();
            return Ok((crate::orchestration::AcceptanceResult::Cancelled, 0));
        }

        match line {
            AiOutputLine::Stdout(s) => {
                output_collector.add_stdout(&s);
                full_stdout.push_str(&s);
                full_stdout.push('\n');

                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::Log(
                            crate::events::LogEntry::info(&s)
                                .with_change_id(change_id)
                                .with_operation("acceptance")
                                .with_iteration(acceptance_iteration),
                        ))
                        .await;
                }

                // Detect canonical verdict and start grace period.
                // Primary: strict JSON verdict object (standalone or wrapped in
                // a supported agent JSONL event). Fallback: strict plain-text
                // standalone canonical marker. Trailing-text concatenation on
                // the plain-text marker does NOT count.
                if !verdict_detected && verdict_stream_detector.detect(&s).is_some() {
                    verdict_detected = true;
                    verdict_deadline = Some(tokio::time::Instant::now() + verdict_grace_period);
                    info!(
                        "Acceptance canonical verdict detected for {}, starting {}s grace period",
                        change_id,
                        verdict_grace_period.as_secs()
                    );
                }
            }
            AiOutputLine::Stderr(s) => {
                output_collector.add_stderr(&s);

                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::Log(
                            crate::events::LogEntry::info(&s)
                                .with_change_id(change_id)
                                .with_operation("acceptance")
                                .with_iteration(acceptance_iteration),
                        ))
                        .await;
                }
            }
        }
    }

    // Wait for child process to complete. After early verdict-driven
    // termination this returns the terminated exit status, which we treat as
    // success-equivalent for verdict-based result classification below. Keep
    // status waiting cancellation-aware because output pipes can close before
    // the child process is reaped.
    let status = match wait_for_streaming_child_with_cancel(
        &mut child,
        cancel_token,
        "acceptance",
        change_id,
        workspace_path,
        Some(acceptance_iteration),
    )
    .await
    {
        Ok(status) => status,
        Err(err) if cancel_token.is_some_and(|token| token.is_cancelled()) => {
            warn!(
                change_id = %change_id,
                workspace = %workspace_path.display(),
                "Acceptance cancelled while waiting for child status: {}",
                err
            );
            return Ok((crate::orchestration::AcceptanceResult::Cancelled, 0));
        }
        Err(err) => return Err(err),
    };

    // Absolute Acceptance runtime limit. This gate runs before the exit-status
    // and verdict branches below because a SIGKILLed agent reports the same
    // non-zero status a crash does: the reason is read from the runner's typed
    // termination, never inferred. Expiry closes retry admission for this
    // invocation, and the process-group cleanup evidence travels with it so an
    // unproven group is reported rather than silently acknowledged.
    {
        let termination = child.termination().await;
        if termination.is_runtime_limit() {
            let cleanup = child.process_group_cleanup().await;
            let limit = crate::orchestration::acceptance::classify_acceptance_runtime_limit(
                termination,
                acceptance_runtime_limit_secs,
                &cleanup,
            )
            .expect("runtime-limit termination always classifies as a runtime limit");
            warn!(
                change_id = %change_id,
                workspace = %workspace_path.display(),
                iteration = acceptance_iteration,
                limit_secs = limit.limit_secs,
                cleanup_confirmed = limit.cleanup_confirmed,
                "Acceptance exceeded its absolute runtime limit: {}",
                limit.cleanup_diagnostics
            );
            // No canonical attempt was produced, so nothing is recorded in
            // acceptance history and no attempt number is consumed: a terminated
            // invocation is not a verdict and must never be replayed as one.
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::error(limit.summary(change_id))
                            .with_change_id(change_id)
                            .with_operation("acceptance")
                            .with_iteration(acceptance_iteration),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }
            return Ok((
                crate::orchestration::AcceptanceResult::RuntimeLimit { limit },
                0,
            ));
        }
    }

    // When the runtime terminated the child after a canonical verdict, the
    // terminated exit code is not a real failure — the acceptance result is
    // already determined by the verdict. Skip the command-failure branch in
    // that case so the verdict drives the final result.
    let verdict_finalized_run = early_terminated && verdict_detected;

    let end_revision = resolve_acceptance_state_revision(
        &start_revision,
        crate::vcs::git::commands::get_current_commit(workspace_path)
            .await
            .ok(),
    );
    if end_revision != start_revision {
        warn!(
            module = module_path!(),
            change_id = %change_id,
            start_revision = %start_revision,
            end_revision = %end_revision,
            workspace = %workspace_path.display(),
            "Acceptance updated HEAD during execution; durable acceptance state will use end revision"
        );
    }

    // Record attempt
    let stdout_tail = output_collector.stdout_tail();
    let stderr_tail = output_collector.stderr_tail();

    // Parse acceptance output
    let parse_result = parse_acceptance_output(&full_stdout);
    let tail_findings = build_acceptance_tail_findings(stdout_tail.clone(), stderr_tail.clone());

    // Check if command failed. A verdict-finalized run was terminated by us
    // after the canonical verdict, so the non-zero exit is expected and the
    // verdict drives the final result.
    if !status.success() && !verdict_finalized_run {
        let current_denial = crate::permission::classify_permission_denial(&[
            stdout_tail.as_deref(),
            stderr_tail.as_deref(),
        ]);
        if let Some(denial) = &current_denial {
            let repeated_unresolved = previous_acceptance_denial
                .as_ref()
                .is_some_and(|previous| previous.signature() == denial.signature())
                && end_revision == start_revision;
            if repeated_unresolved {
                let attempt_number = agent.next_acceptance_attempt_number(change_id);
                let attempt = crate::history::AcceptanceAttempt {
                    attempt: attempt_number,
                    passed: false,
                    duration: start_time.elapsed(),
                    findings: Some(crate::acceptance::legacy_findings(tail_findings.clone())),
                    exit_code: status.code(),
                    stdout_tail: stdout_tail.clone(),
                    stderr_tail: stderr_tail.clone(),
                    commit_hash: revision_to_history_commit_hash(&end_revision),
                };
                agent.record_acceptance_attempt(change_id, attempt.clone());
                acceptance_history.lock().await.record(change_id, attempt);
                acceptance_tail_injected.lock().await.remove(change_id);

                let blocker =
                    crate::events::StalledBlocker::permission_denial("acceptance", denial);
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ExecutionBlocked {
                            change_id: change_id.to_string(),
                            blocker: blocker.clone(),
                        })
                        .await;
                    let _ = tx
                        .send(ParallelEvent::AcceptanceCompleted {
                            change_id: change_id.to_string(),
                        })
                        .await;
                }
                return Ok((
                    crate::orchestration::AcceptanceResult::PermissionStalled { blocker },
                    attempt_number,
                ));
            }
        }

        let error_msg = format!(
            "Acceptance command failed with exit code: {:?}",
            status.code()
        );

        // A command that never completed produced no verdict, so it records no
        // canonical `AcceptanceAttempt` in either the agent-local or the shared
        // Acceptance history: transport evidence must not be replayed as
        // previous findings, and it must not consume attempt numbering. The
        // dedicated latest-only command-recovery context below is the only place
        // this output travels.
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(ParallelEvent::Log(
                    crate::events::LogEntry::error(&error_msg)
                        .with_change_id(change_id)
                        .with_operation("acceptance"),
                ))
                .await;
            let _ = tx
                .send(ParallelEvent::AcceptanceCompleted {
                    change_id: change_id.to_string(),
                })
                .await;
        }

        return Ok((
            crate::orchestration::AcceptanceResult::CommandFailed {
                diagnostic: crate::orchestration::acceptance::AcceptanceCommandDiagnostic {
                    error: error_msg.clone(),
                    exit_code: status.code(),
                    stdout_tail: stdout_tail.clone(),
                    stderr_tail: stderr_tail.clone(),
                },
                error: error_msg,
                findings: tail_findings,
            },
            // No canonical attempt was recorded, so no attempt number is
            // reported.
            0,
        ));
    }

    // Process parsed result
    match parse_result {
        ParseResult::Pass => {
            info!("Acceptance passed for: {}", change_id);
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: true,
                duration: start_time.elapsed(),
                findings: None,
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            agent.record_acceptance_attempt(change_id, attempt.clone());
            agent.clear_acceptance_follow_up(change_id);
            let mut shared_history = acceptance_history.lock().await;
            shared_history.record(change_id, attempt);
            shared_history.clear_follow_up_findings(change_id);
            drop(shared_history);
            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::info("Acceptance test passed")
                            .with_change_id(change_id)
                            .with_operation("acceptance")
                            .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((crate::orchestration::AcceptanceResult::Pass, attempt_number))
        }
        ParseResult::Continue => {
            info!("Acceptance requires continuation for: {}", change_id);
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(crate::acceptance::legacy_findings([
                    "Investigation incomplete - continue later",
                ])),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            // Reset acceptance tail injection flag so next apply can receive new output

            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::info("Acceptance test requires continuation")
                            .with_change_id(change_id)
                            .with_operation("acceptance")
                            .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::Continue,
                attempt_number,
            ))
        }
        ParseResult::MissingVerdict => {
            // The acceptance command completed but never emitted a canonical
            // verdict (for example it exited after status-only "waiting for
            // verification" prose). This is a protocol failure and must stay
            // distinguishable from an explicit canonical CONTINUE: record a
            // dedicated diagnostic (never the CONTINUE history marker) so the
            // consecutive-CONTINUE retry counter is not consumed.
            warn!(
                "Acceptance completed without a canonical verdict for: {} (missing-verdict protocol failure)",
                change_id
            );
            let mut evidence =
                vec![crate::orchestration::acceptance::MISSING_VERDICT_DIAGNOSTIC.to_string()];
            evidence.extend(tail_findings.iter().cloned());
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(crate::acceptance::legacy_findings(evidence.clone())),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                // Non-terminal here: the caller owns the protocol-retry budget and
                // emits either retry progress or the terminal exhaustion error.
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(
                            "Acceptance completed without a canonical verdict (missing-verdict \
                             protocol failure); status-only or waiting output is not a verdict. \
                             The acceptance agent must wait for owned verification results and \
                             emit exactly one canonical verdict before exiting.",
                        )
                        .with_change_id(change_id)
                        .with_operation("acceptance")
                        .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::MissingVerdict {
                    findings: tail_findings,
                },
                attempt_number,
            ))
        }
        ParseResult::Stalled { blocker } => {
            info!(
                "Acceptance reported a validated external blocker for: {} (category {})",
                change_id, blocker.category
            );
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let mut findings = vec![format!(
                "Validated external blocker (category {}): {}",
                blocker.category, blocker.next_action
            )];
            findings.extend(blocker.evidence.iter().cloned());
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(crate::acceptance::legacy_findings(findings)),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            // Record to both agent history (local) and shared acceptance history
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            // Reset acceptance tail injection flag so next apply can receive new output

            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(format!(
                            "Acceptance stalled ({}) on a validated external blocker: {}",
                            blocker.category, blocker.next_action
                        ))
                        .with_change_id(change_id)
                        .with_operation("acceptance")
                        .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::Stalled { blocker },
                attempt_number,
            ))
        }
        ParseResult::BareBlocker { rejection } => {
            // A gated compatibility token with no validated blocker payload is a
            // protocol error. The caller owns the bounded retry budget, so this
            // arm emits no stalled lifecycle transition and no blocker category.
            warn!(
                "Acceptance emitted a bare blocker compatibility token for {}: {}",
                change_id,
                rejection.reason()
            );
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(crate::acceptance::legacy_findings([
                    crate::orchestration::acceptance::BARE_BLOCKER_DIAGNOSTIC.to_string(),
                    rejection.reason(),
                ])),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(format!(
                            "Acceptance emitted a gated compatibility token without a validated \
                             structured blocker ({}); a stalled hold requires an explicit supported \
                             category, concrete evidence, next action, and resumability.",
                            rejection.reason()
                        ))
                        .with_change_id(change_id)
                        .with_operation("acceptance")
                        .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::BareBlocker { rejection },
                attempt_number,
            ))
        }
        ParseResult::MalformedFinding { rejection } => {
            // A structured finding that does not validate is a protocol error.
            // The caller owns the bounded retry budget, so this arm dispatches no
            // repair work and records no follow-up: reducing the finding to a
            // path-only instruction is exactly what must not happen.
            warn!(
                "Acceptance emitted a malformed structured finding for {}: {}",
                change_id,
                rejection.reason()
            );
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(crate::acceptance::legacy_findings([
                    crate::orchestration::acceptance::MALFORMED_FINDING_DIAGNOSTIC.to_string(),
                    rejection.reason(),
                ])),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            agent.record_acceptance_attempt(change_id, attempt.clone());
            acceptance_history.lock().await.record(change_id, attempt);
            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(format!(
                            "Acceptance emitted a FAIL verdict whose structured finding did not \
                             validate ({}); runtime will not convert it into a path-only repair \
                             instruction.",
                            rejection.reason()
                        ))
                        .with_change_id(change_id)
                        .with_operation("acceptance")
                        .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::MalformedFinding { rejection },
                attempt_number,
            ))
        }
        ParseResult::Fail { findings } => {
            let findings_for_tasks = if findings.is_empty() {
                crate::acceptance::legacy_findings([
                    "Investigate acceptance failure and apply the required fix",
                ])
            } else {
                findings
            };
            let findings_text = crate::acceptance::finding_texts(&findings_for_tasks).join("\n");
            let current_denial = crate::permission::classify_permission_denial(&[
                stdout_tail.as_deref(),
                stderr_tail.as_deref(),
                Some(findings_text.as_str()),
            ]);
            if let Some(denial) = &current_denial {
                let repeated_unresolved = previous_acceptance_denial
                    .as_ref()
                    .is_some_and(|previous| previous.signature() == denial.signature())
                    && end_revision == start_revision;
                if repeated_unresolved {
                    let attempt_number = agent.next_acceptance_attempt_number(change_id);
                    let attempt = crate::history::AcceptanceAttempt {
                        attempt: attempt_number,
                        passed: false,
                        duration: start_time.elapsed(),
                        findings: Some(findings_for_tasks.clone()),
                        exit_code: status.code(),
                        stdout_tail: stdout_tail.clone(),
                        stderr_tail: stderr_tail.clone(),
                        commit_hash: revision_to_history_commit_hash(&end_revision),
                    };
                    agent.record_acceptance_attempt(change_id, attempt.clone());
                    acceptance_history.lock().await.record(change_id, attempt);
                    acceptance_tail_injected.lock().await.remove(change_id);

                    let blocker =
                        crate::events::StalledBlocker::permission_denial("acceptance", denial);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::ExecutionBlocked {
                                change_id: change_id.to_string(),
                                blocker: blocker.clone(),
                            })
                            .await;
                        let _ = tx
                            .send(ParallelEvent::AcceptanceCompleted {
                                change_id: change_id.to_string(),
                            })
                            .await;
                    }
                    return Ok((
                        crate::orchestration::AcceptanceResult::PermissionStalled { blocker },
                        attempt_number,
                    ));
                }
            }
            let blocking_gate_context = findings_for_tasks
                .first()
                .map(|finding| finding.text().to_string())
                .unwrap_or_else(|| "no acceptance findings captured".to_string());
            info!(
                "Acceptance failed for: {} ({} findings), blocking gate context: {}",
                change_id,
                findings_for_tasks.len(),
                blocking_gate_context
            );
            let attempt_number = agent.next_acceptance_attempt_number(change_id);
            let attempt = crate::history::AcceptanceAttempt {
                attempt: attempt_number,
                passed: false,
                duration: start_time.elapsed(),
                findings: Some(findings_for_tasks.clone()),
                exit_code: status.code(),
                stdout_tail: stdout_tail.clone(),
                stderr_tail: stderr_tail.clone(),
                commit_hash: revision_to_history_commit_hash(&end_revision),
            };
            agent.record_acceptance_attempt(change_id, attempt.clone());
            let repository_findings =
                crate::orchestration::acceptance::repository_findings(&findings_for_tasks);
            if !repository_findings.is_empty() {
                agent.record_acceptance_follow_up(
                    change_id,
                    attempt_number,
                    repository_findings.clone(),
                );
            }
            let mut shared_history = acceptance_history.lock().await;
            shared_history.record(change_id, attempt);
            if !repository_findings.is_empty() {
                shared_history.set_follow_up_findings(
                    change_id,
                    attempt_number,
                    repository_findings,
                );
            }
            drop(shared_history);
            acceptance_tail_injected.lock().await.remove(change_id);

            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::Log(
                        crate::events::LogEntry::warn(format_acceptance_failure_log_message(
                            &findings_for_tasks,
                        ))
                        .with_change_id(change_id)
                        .with_operation("acceptance")
                        .with_iteration(attempt_number),
                    ))
                    .await;
                let _ = tx
                    .send(ParallelEvent::AcceptanceCompleted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }

            Ok((
                crate::orchestration::AcceptanceResult::Fail {
                    findings: findings_for_tasks,
                },
                attempt_number,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_acceptance_failure_log_message, mark_acceptance_context_injected,
        prepare_acceptance_context_for_apply, resolve_acceptance_state_revision,
        run_post_apply_cleanup_review,
    };
    use crate::agent::AgentRunner;
    use crate::ai_command_runner::AiCommandRunner;
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::default_retry_patterns;
    use crate::config::OrchestratorConfig;
    use crate::task_parser::TaskProgress;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::process::Command;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn latest_shared_acceptance_findings_seed_parallel_apply_runner() {
        use crate::history::{AcceptanceAttempt, AcceptanceHistory};
        use std::time::Duration;

        let mut history = AcceptanceHistory::new();
        history.record(
            "change-a",
            AcceptanceAttempt {
                attempt: 3,
                passed: false,
                duration: Duration::from_secs(1),
                findings: Some(vec!["fix canonical finding".to_string().into()]),
                exit_code: Some(0),
                stdout_tail: Some("unstructured noise".to_string()),
                stderr_tail: None,
                commit_hash: None,
            },
        );
        history.set_follow_up_findings(
            "change-a",
            3,
            vec!["fix canonical finding".to_string().into()],
        );
        let history = Arc::new(Mutex::new(history));
        let injected = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let mut first_agent = AgentRunner::new(OrchestratorConfig::default());
        prepare_acceptance_context_for_apply(&mut first_agent, "change-a", &history, &injected)
            .await;

        let context = first_agent.get_acceptance_tail_context_for_apply("change-a");
        assert!(context.contains("fix canonical finding"));
        assert!(!context.contains("unstructured noise"));
        mark_acceptance_context_injected(&first_agent, "change-a", &injected).await;

        let mut resumed_agent = AgentRunner::new(OrchestratorConfig::default());
        prepare_acceptance_context_for_apply(&mut resumed_agent, "change-a", &history, &injected)
            .await;
        assert!(resumed_agent
            .get_acceptance_tail_context_for_apply("change-a")
            .is_empty());
    }

    #[test]
    fn test_progress_commit_message_format() {
        // Verify the commit message format matches the spec
        let change_id = "add-feature";
        let progress = TaskProgress {
            completed: 5,
            total: 10,
        };

        let expected = "WIP: add-feature (5/10 tasks)";
        let actual = format!(
            "WIP: {} ({}/{} tasks)",
            change_id, progress.completed, progress.total
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_all_complete() {
        let change_id = "fix-bug";
        let progress = TaskProgress {
            completed: 7,
            total: 7,
        };

        let expected = "WIP: fix-bug (7/7 tasks)";
        let actual = format!(
            "WIP: {} ({}/{} tasks)",
            change_id, progress.completed, progress.total
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_zero_progress() {
        let change_id = "new-change";
        let progress = TaskProgress {
            completed: 0,
            total: 5,
        };

        let expected = "WIP: new-change (0/5 tasks)";
        let actual = format!(
            "WIP: {} ({}/{} tasks)",
            change_id, progress.completed, progress.total
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_progress_commit_message_special_characters() {
        // Test with change IDs that contain hyphens (common case)
        let change_id = "add-web-monitoring-feature";
        let progress = TaskProgress {
            completed: 50,
            total: 70,
        };

        let expected = "WIP: add-web-monitoring-feature (50/70 tasks)";
        let actual = format!(
            "WIP: {} ({}/{} tasks)",
            change_id, progress.completed, progress.total
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_acceptance_failure_log_message_includes_blocking_gate_context() {
        let message = format_acceptance_failure_log_message(&[
            "archive-readiness gate failed: cargo clippy -- -D warnings (src/lib.rs:42)"
                .to_string()
                .into(),
            "second finding".to_string().into(),
        ]);

        assert!(
            message.contains("Acceptance failed (2 findings), blocking gate context:"),
            "message should include finding count and blocking gate context prefix"
        );
        assert!(
            message.contains("cargo clippy -- -D warnings"),
            "message should preserve gate-specific failure context"
        );
    }

    #[test]
    fn test_format_acceptance_failure_log_message_handles_empty_findings() {
        let message = format_acceptance_failure_log_message(&[]);

        assert_eq!(
            message,
            "Acceptance failed (0 findings), blocking gate context: no acceptance findings captured"
        );
    }

    #[test]
    fn test_resolve_acceptance_state_revision_prefers_end_revision() {
        let resolved = resolve_acceptance_state_revision("start-rev", Some("end-rev".to_string()));
        assert_eq!(resolved, "end-rev");
    }

    #[test]
    fn test_resolve_acceptance_state_revision_falls_back_to_start_revision() {
        let resolved = resolve_acceptance_state_revision("start-rev", None);
        assert_eq!(resolved, "start-rev");
    }

    #[test]
    fn test_progress_check_condition() {
        // Test the condition for creating progress commits:
        // new_progress.completed > progress.completed
        let old_progress = TaskProgress {
            completed: 3,
            total: 10,
        };
        let new_progress_same = TaskProgress {
            completed: 3,
            total: 10,
        };
        let new_progress_increased = TaskProgress {
            completed: 5,
            total: 10,
        };
        let new_progress_decreased = TaskProgress {
            completed: 2,
            total: 10,
        };

        // Should NOT create commit when no progress
        assert!(new_progress_same.completed <= old_progress.completed);

        // Should create commit when progress increased
        assert!(new_progress_increased.completed > old_progress.completed);

        // Should NOT create commit when progress decreased (edge case)
        assert!(new_progress_decreased.completed <= old_progress.completed);
    }

    async fn init_test_git_repo(repo_root: &std::path::Path) {
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();

        std::fs::write(repo_root.join("README.md"), "base\n").unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(repo_root)
            .output()
            .await
            .unwrap();
    }

    fn test_ai_runner() -> AiCommandRunner {
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: 0,
            max_retries: 0,
            retry_delay_ms: 0,
            retry_error_patterns: default_retry_patterns(),
            retry_if_duration_under_secs: 0,
            inactivity_timeout_secs: 0,
            inactivity_kill_grace_secs: 1,
            inactivity_timeout_max_retries: 0,
            strict_process_cleanup: true,
            max_runtime_secs: 0,
        };
        let shared_stagger_state = Arc::new(Mutex::new(None));
        AiCommandRunner::new(queue_config, shared_stagger_state)
    }

    #[tokio::test]
    async fn test_post_apply_cleanup_review_succeeds_with_single_clean_marker() {
        let temp_dir = TempDir::new().unwrap();
        init_test_git_repo(temp_dir.path()).await;
        std::fs::write(temp_dir.path().join("dirty.txt"), "dirty\n").unwrap();

        let config = OrchestratorConfig {
            acceptance_command: Some(
                "sh -c 'git add dirty.txt && git commit -m cleanup && echo CLEANUP_REVIEW: CLEAN'"
                    .to_string(),
            ),
            ..Default::default()
        };
        let ai_runner = test_ai_runner();

        run_post_apply_cleanup_review("change-a", temp_dir.path(), &config, &ai_runner, None, None)
            .await
            .expect("cleanup review should succeed");

        let (is_dirty, status) =
            crate::vcs::git::commands::has_uncommitted_changes(temp_dir.path())
                .await
                .unwrap();
        assert!(
            !is_dirty,
            "worktree must be clean after successful cleanup review: {status}"
        );
    }

    #[tokio::test]
    async fn test_post_apply_cleanup_review_fails_when_marker_missing() {
        let temp_dir = TempDir::new().unwrap();
        init_test_git_repo(temp_dir.path()).await;
        std::fs::write(temp_dir.path().join("dirty.txt"), "dirty\n").unwrap();

        let config = OrchestratorConfig {
            acceptance_command: Some(
                // Idempotent so every operation attempt exits successfully and
                // the only failure observed is the missing marker.
                "sh -c 'git add dirty.txt; git diff --cached --quiet || git commit -m cleanup; \
                 echo done'"
                    .to_string(),
            ),
            ..Default::default()
        };
        let ai_runner = test_ai_runner();

        let err = run_post_apply_cleanup_review(
            "change-a",
            temp_dir.path(),
            &config,
            &ai_runner,
            None,
            None,
        )
        .await
        .expect_err("cleanup review must fail without marker");
        let message = err.to_string();
        assert!(
            message.contains("marker_missing"),
            "error should name the missing-marker failure kind: {message}"
        );
        assert!(
            message.contains("3 operation attempts"),
            "error should report the exhausted attempt count: {message}"
        );
    }

    /// Post-Apply cleanup-review corrective loop: ordered validation, bounded
    /// correction, cancellation, and the non-terminal permission hold.
    ///
    /// Nested under `tests` so the change proposal's declared verification command
    /// `cargo test --lib parallel::executor::tests` covers this whole set.
    mod cleanup_review_recovery {
        use super::super::run_post_apply_cleanup_review;
        use crate::ai_command_runner::AiCommandRunner;
        use crate::command_queue::CommandQueueConfig;
        use crate::config::OrchestratorConfig;
        use crate::error::OrchestratorError;
        use std::path::Path;
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::Mutex;
        use tokio_util::sync::CancellationToken;

        fn ai_runner() -> AiCommandRunner {
            let queue_config = CommandQueueConfig {
                stagger_delay_ms: 0,
                max_retries: 0,
                retry_delay_ms: 0,
                retry_error_patterns: Vec::new(),
                retry_if_duration_under_secs: 0,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 1,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
                max_runtime_secs: 0,
            };
            AiCommandRunner::new(queue_config, Arc::new(Mutex::new(None)))
        }

        fn git(repo: &Path, args: &[&str]) {
            let output = std::process::Command::new("git")
                .args(args)
                .current_dir(repo)
                .output()
                .expect("git should run");
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        /// A task-complete managed worktree that is dirty after Apply.
        fn dirty_worktree() -> TempDir {
            let temp_dir = TempDir::new().unwrap();
            let repo = temp_dir.path();
            git(repo, &["init", "-b", "main"]);
            git(repo, &["config", "user.email", "test@example.com"]);
            git(repo, &["config", "user.name", "Test User"]);
            std::fs::write(repo.join("README.md"), "base\n").unwrap();
            git(repo, &["add", "README.md"]);
            git(repo, &["commit", "-m", "base"]);
            std::fs::write(repo.join("leftover.txt"), "apply artifact\n").unwrap();
            temp_dir
        }

        /// Install an executable cleanup-review fixture that receives the expanded
        /// prompt as `$1`, records every attempt, and runs `body` with `$ATTEMPT`
        /// set. Using a script keeps the prompt out of nested shell quoting so the
        /// corrective context can be asserted verbatim.
        fn fixture(state: &Path, body: &str) -> String {
            let script = state.join("cleanup-review.sh");
            std::fs::write(
                &script,
                format!(
                    "#!/bin/sh\n\
                 STATE={state}\n\
                 ATTEMPT=$(cat \"$STATE/attempts\" 2>/dev/null || echo 0)\n\
                 ATTEMPT=$((ATTEMPT+1))\n\
                 echo $ATTEMPT > \"$STATE/attempts\"\n\
                 printf '%s' \"$1\" > \"$STATE/prompt-$ATTEMPT.txt\"\n\
                 {body}\n",
                    state = state.display(),
                    body = body
                ),
            )
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            format!("{} {{prompt}}", script.display())
        }

        fn config(command: String) -> OrchestratorConfig {
            OrchestratorConfig {
                acceptance_command: Some(command),
                ..Default::default()
            }
        }

        fn attempts(state: &Path) -> u32 {
            std::fs::read_to_string(state.join("attempts"))
                .map(|text| text.trim().parse().unwrap_or(0))
                .unwrap_or(0)
        }

        fn prompt(state: &Path, attempt: u32) -> String {
            std::fs::read_to_string(state.join(format!("prompt-{attempt}.txt")))
                .unwrap_or_else(|e| panic!("attempt {attempt} prompt should exist: {e}"))
        }

        async fn is_dirty(repo: &Path) -> bool {
            crate::vcs::git::commands::has_uncommitted_changes(repo)
                .await
                .unwrap()
                .0
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_corrective_attempt_recovers_a_dirty_worktree() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            // Attempt 1 commits nothing and emits no marker; attempt 2 does the work.
            let command = fixture(
                state.path(),
                "if [ \"$ATTEMPT\" = \"1\" ]; then echo 'reviewed, nothing done'; exit 0; fi\n\
             git add leftover.txt && git commit -q -m cleanup && echo 'CLEANUP_REVIEW: CLEAN'",
            );

            run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect("the corrective attempt must recover the handoff");

            assert_eq!(
                attempts(state.path()),
                2,
                "exactly one correction was needed"
            );
            assert!(!is_dirty(repo_dir.path()).await);

            // The corrective prompt carries only the latest structured diagnosis.
            let first = prompt(state.path(), 1);
            assert!(
                !first.contains("<cleanup_review_correction>"),
                "the initial attempt has nothing to correct"
            );
            let second = prompt(state.path(), 2);
            assert!(second.contains("<cleanup_review_correction>"), "{second}");
            assert!(
                second.contains("\"failure_kind\":\"marker_missing\""),
                "{second}"
            );
            assert!(
                second.contains("\"standalone_clean_marker_count\":0"),
                "{second}"
            );
            assert!(
                second.contains("leftover.txt"),
                "the corrective prompt must carry fresh porcelain evidence: {second}"
            );
            assert!(second.contains("Success is decided by Conflux"), "{second}");
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_marker_without_a_clean_repository_is_not_success() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            // Emits the marker every time but never cleans the worktree.
            let command = fixture(state.path(), "echo 'CLEANUP_REVIEW: CLEAN'");

            let error = run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect_err("a marker can never override Git state");

            let message = error.to_string();
            assert!(message.contains("dirty_remains"), "{message}");
            assert!(message.contains("leftover.txt"), "{message}");
            assert_eq!(attempts(state.path()), 3);
            assert!(
                prompt(state.path(), 2).contains("\"failure_kind\":\"dirty_remains\""),
                "the correction must name the observed failure"
            );
            assert!(
                is_dirty(repo_dir.path()).await,
                "exhaustion preserves the managed workspace for explicit retry"
            );
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_clean_repository_without_exactly_one_marker_is_not_success() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            // Cleans the worktree but duplicates the marker, so the protocol fails
            // even though repository truth is satisfied.
            let command = fixture(
                state.path(),
                "git add leftover.txt >/dev/null 2>&1; \
             git diff --cached --quiet || git commit -q -m cleanup; \
             echo 'CLEANUP_REVIEW: CLEAN'; echo 'CLEANUP_REVIEW: CLEAN'",
            );

            let error = run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect_err("a clean worktree never invents the marker contract");

            let message = error.to_string();
            assert!(message.contains("marker_duplicate"), "{message}");
            assert!(
                message.contains("standalone_clean_marker_count: 2"),
                "{message}"
            );
            assert_eq!(
                attempts(state.path()),
                3,
                "exactly three operation attempts"
            );
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn exhaustion_reports_the_attempt_count_and_latest_diagnosis() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            let command = fixture(
                state.path(),
                "echo \"cleanup crashed on attempt $ATTEMPT\" >&2; exit 5",
            );

            let error = run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect_err("three failed operation attempts are terminal");

            let message = error.to_string();
            assert!(message.contains("3 operation attempts"), "{message}");
            assert!(
                message.contains("failure_kind: command_failed"),
                "{message}"
            );
            assert!(message.contains("exit_code: 5"), "{message}");
            assert!(
                message.contains("cleanup crashed on attempt 3"),
                "the terminal error reports the latest diagnosis: {message}"
            );
            assert_eq!(
                attempts(state.path()),
                super::super::MAX_CLEANUP_REVIEW_RETRIES + 1,
                "no fourth operation attempt may start"
            );
            assert!(is_dirty(repo_dir.path()).await);
        }

        /// Cancellation observed before an attempt starts is an intentional stop.
        ///
        /// Deterministic and process-free: no fixture runs, so this is the fast
        /// default-suite coverage of the cancellation contract that the
        /// process-boundary test below exercises against a live child.
        #[tokio::test]
        async fn an_already_cancelled_token_starts_no_attempt() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            let command = fixture(state.path(), "echo 'CLEANUP_REVIEW: CLEAN'");
            let cancel = CancellationToken::new();
            cancel.cancel();

            let error = run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                Some(&cancel),
                None,
            )
            .await
            .expect_err("cancellation is an intentional stop, not a cleanup success");

            assert!(
                error.is_cancellation(),
                "the run boundary must be able to classify the stop without parsing text: {error}"
            );
            assert!(
                matches!(
                    &error,
                    OrchestratorError::Cancelled { operation, change_id, .. }
                        if operation == "cleanup-review" && change_id == "change-a"
                ),
                "the typed stop names the operation and change: {error}"
            );
            assert!(
                error.to_string().contains("Cancelled cleanup-review"),
                "the existing rendering is unchanged: {error}"
            );
            assert_eq!(
                attempts(state.path()),
                0,
                "explicit cancellation must not start an attempt"
            );
        }

        /// Cancellation while a real child is streaming terminates that child and
        /// starts no corrective attempt.
        ///
        /// Gated: it owns a live process that must be observed running before it
        /// is cancelled, so its wall clock follows process spawn latency rather
        /// than this crate's logic. The fast test above keeps the cancellation
        /// contract itself in the default suite.
        #[cfg_attr(windows, ignore)]
        #[cfg_attr(not(feature = "heavy-tests"), ignore)]
        #[tokio::test]
        async fn cancellation_terminates_the_child_and_starts_no_further_attempt() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            // Signals that it started, then blocks until terminated.
            let command = fixture(
                state.path(),
                "touch \"$STATE/started\"; sleep 30; echo 'CLEANUP_REVIEW: CLEAN'",
            );
            let cancel = CancellationToken::new();

            // Cancel only once the child has actually started. Cancelling on a
            // fixed deadline would race process spawn and stop the loop before
            // any attempt ran, which is the other code path. Waiting for the
            // marker instead of a deadline is what makes the observation, rather
            // than the machine's load, decide when to cancel.
            let started_marker = state.path().join("started");
            let waiter = cancel.clone();
            let starter = tokio::spawn(async move {
                // Generous, not tight: the bound exists only so a fixture that
                // never starts fails the assertions instead of hanging.
                for _ in 0..12_000 {
                    if started_marker.exists() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                waiter.cancel();
            });

            let error = run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                Some(&cancel),
                None,
            )
            .await
            .expect_err("cancellation is an intentional stop, not a cleanup success");

            starter.abort();
            assert!(
                error.is_cancellation(),
                "cancellation keeps the typed intentional-stop routing: {error}"
            );
            assert_eq!(
                attempts(state.path()),
                1,
                "explicit cancellation must not start a corrective attempt"
            );
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_classified_permission_denial_holds_without_consuming_the_failure_budget() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            let command = fixture(
                state.path(),
                "echo 'Tool access denied: Bash(git commit)'; exit 0",
            );

            let error = run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect_err("a classified denial enters the non-terminal hold");

            assert!(
                matches!(error, OrchestratorError::PermissionStalled { .. }),
                "cleanup permission denial reuses the existing non-terminal hold, got {error:?}"
            );
            assert_eq!(
                attempts(state.path()),
                1,
                "permission denial starts no corrective attempt and consumes no generic budget"
            );
            assert!(
                is_dirty(repo_dir.path()).await,
                "the managed workspace is preserved for an explicit retry"
            );
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_restarted_run_derives_cleanup_from_workspace_evidence_with_a_fresh_budget() {
            let repo_dir = dirty_worktree();

            // First process run exhausts its active-run budget and writes nothing
            // durable: no report, marker file, or retry checkpoint.
            let first_state = TempDir::new().unwrap();
            let first = fixture(first_state.path(), "echo nope; exit 1");
            run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(first),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect_err("the first run exhausts its budget");

            let workspace_entries: Vec<String> = std::fs::read_dir(repo_dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
                .filter(|name| name != ".git")
                .collect();
            assert_eq!(
                workspace_entries
                    .iter()
                    .filter(|name| name.as_str() != "README.md" && name.as_str() != "leftover.txt")
                    .count(),
                0,
                "no durable retry artifact may be created: {workspace_entries:?}"
            );

            // A restart is just a fresh call against the same workspace evidence.
            let second_state = TempDir::new().unwrap();
            let second = fixture(
                second_state.path(),
                "git add leftover.txt && git commit -q -m cleanup && echo 'CLEANUP_REVIEW: CLEAN'",
            );
            run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(second),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect("a restarted run gets a fresh active-run cleanup budget");

            assert_eq!(
                attempts(second_state.path()),
                1,
                "the restarted run starts from a clean budget"
            );
            assert!(
                !prompt(second_state.path(), 1).contains("<cleanup_review_correction>"),
                "no prior-run diagnosis may be restored from outside the workspace"
            );
        }

        // === Simultaneous status-inspection failure ===

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_command_failure_that_also_breaks_status_reports_both() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            // The command fails *and* leaves the repository unreadable, so the
            // fresh status query cannot answer either. An absent status must not
            // read as "nothing reported".
            let command = fixture(
                state.path(),
                "rm -rf .git; echo 'cleanup crashed' >&2; exit 5",
            );

            let error = run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect_err("a broken repository is never a handoff-ready worktree");

            let message = error.to_string();
            assert!(
                message.contains("failure_kind: command_failed"),
                "the primary failure kind is preserved: {message}"
            );
            assert!(message.contains("exit_code: 5"), "{message}");
            assert!(
                message.contains("status_error: "),
                "the simultaneous status-inspection failure must be reported: {message}"
            );
            assert!(message.contains("status inspection failed"), "{message}");
            assert!(
                !message.contains("| status: "),
                "an unanswerable query must not present itself as observed status: {message}"
            );

            // The corrective prompt distinguishes unavailable evidence from an
            // empty status.
            let second = prompt(state.path(), 2);
            assert!(
                second.contains("\"failure_kind\":\"command_failed\""),
                "{second}"
            );
            assert!(
                second.contains("\"status_inspection_error\""),
                "the corrective prompt must say cleanliness is unproven: {second}"
            );
            assert!(
                !second.contains("\"current_porcelain_status\""),
                "no status may be claimed when the query failed: {second}"
            );
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_marker_failure_that_also_breaks_status_reports_both() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            // Exits successfully with no marker, and breaks the repository.
            let command = fixture(state.path(), "rm -rf .git; echo 'reviewed'; exit 0");

            let error = run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect_err("a missing marker with unprovable cleanliness is terminal");

            let message = error.to_string();
            assert!(
                message.contains("failure_kind: marker_missing"),
                "the marker contract still owns the primary kind: {message}"
            );
            assert!(
                message.contains("standalone_clean_marker_count: 0"),
                "{message}"
            );
            assert!(
                message.contains("status_error: "),
                "the simultaneous status-inspection failure must be reported: {message}"
            );

            let second = prompt(state.path(), 2);
            assert!(
                second.contains("\"failure_kind\":\"marker_missing\""),
                "{second}"
            );
            assert!(second.contains("\"status_inspection_error\""), "{second}");
        }

        // === Bounded marker accounting ===

        #[test]
        fn the_marker_scanner_state_is_fixed_size_regardless_of_stream_length() {
            use crate::agent::CleanupMarkerScanner;

            let mut scanner = CleanupMarkerScanner::new();
            // Far more output than any bounded tail would keep.
            for i in 0..200_000 {
                scanner.observe(&format!("noise line {i} with some padding text"));
            }
            scanner.observe("CLEANUP_REVIEW: CLEAN");
            for i in 0..200_000 {
                scanner.observe(&format!("trailing noise {i}"));
            }

            assert_eq!(scanner.count(), 1, "the marker contract still holds");
            // The scanner is a plain Copy value: it cannot retain the stream.
            assert!(
                std::mem::size_of::<CleanupMarkerScanner>() <= 2 * std::mem::size_of::<usize>(),
                "marker state must stay bounded, got {} bytes",
                std::mem::size_of::<CleanupMarkerScanner>()
            );
        }

        #[test]
        fn the_marker_scanner_ignores_fenced_markers_across_chunk_boundaries() {
            use crate::agent::CleanupMarkerScanner;

            let mut scanner = CleanupMarkerScanner::new();
            for chunk in [
                "```",
                "CLEANUP_REVIEW: CLEAN",
                "```",
                "  CLEANUP_REVIEW: CLEAN  ",
                "prefix CLEANUP_REVIEW: CLEAN",
            ] {
                scanner.observe(chunk);
            }

            assert_eq!(
                scanner.count(),
                1,
                "only the standalone unfenced marker counts, even when the fence spans chunks"
            );
        }

        #[cfg_attr(windows, ignore)]
        #[tokio::test]
        async fn a_large_stream_still_validates_the_marker_exactly_once() {
            let repo_dir = dirty_worktree();
            let state = TempDir::new().unwrap();
            // Emits a fenced decoy, a large body, then the one real marker.
            let command = fixture(
                state.path(),
                "echo '```'; echo 'CLEANUP_REVIEW: CLEAN'; echo '```'; \
             i=0; while [ $i -lt 4000 ]; do echo \"filler line $i\"; i=$((i+1)); done; \
             git add leftover.txt && git commit -q -m cleanup && echo 'CLEANUP_REVIEW: CLEAN'",
            );

            run_post_apply_cleanup_review(
                "change-a",
                repo_dir.path(),
                &config(command),
                &ai_runner(),
                None,
                None,
            )
            .await
            .expect("one standalone marker outside fences plus a clean worktree is success");

            assert_eq!(attempts(state.path()), 1, "no correction was needed");
            assert!(!is_dirty(repo_dir.path()).await);
        }
    }
}
