//! Workspace execution logic for apply and archive operations.

use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::task_parser::TaskProgress;
use crate::vcs::VcsBackend;
use std::path::Path;
use std::process::Stdio as StdStdio;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::events::ParallelEvent;

/// Create a progress commit to save current work state.
///
/// This function creates a WIP (work-in-progress) commit after each apply iteration
/// where progress was made. This ensures that work is not lost if the process is
/// interrupted or reaches the maximum iteration limit.
///
/// # Arguments
///
/// * `workspace_path` - Path to the workspace directory
/// * `change_id` - The change identifier
/// * `progress` - Current task progress (completed/total)
/// * `vcs_backend` - The VCS backend to use (jj or Git)
///
/// # Commit Message Format
///
/// The commit message follows the format: `WIP: {change_id} ({completed}/{total} tasks)`
/// For example: `WIP: add-feature (5/10 tasks)`
pub async fn create_progress_commit(
    workspace_path: &Path,
    change_id: &str,
    progress: &TaskProgress,
    vcs_backend: VcsBackend,
) -> Result<()> {
    let commit_message = format!(
        "WIP: {} ({}/{} tasks)",
        change_id, progress.completed, progress.total
    );

    debug!(
        "Creating progress commit for {}: {}",
        change_id, commit_message
    );

    match vcs_backend {
        VcsBackend::Jj => {
            // jj automatically snapshots working copy changes
            // Just update the commit message with --ignore-working-copy
            // to avoid stale working copy errors in workspaces
            let output = Command::new("jj")
                .args(["describe", "--ignore-working-copy", "-m", &commit_message])
                .current_dir(workspace_path)
                .stdin(StdStdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::JjCommand(format!("Failed to create progress commit: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!(
                    "Failed to set progress commit message for {}: {}",
                    change_id, stderr
                );
            } else {
                debug!("Progress commit created for {} (jj)", change_id);
            }
        }
        VcsBackend::Git | VcsBackend::Auto => {
            // For Git: stage all changes and amend the commit
            let add_output = Command::new("git")
                .args(["add", "-A"])
                .current_dir(workspace_path)
                .stdin(StdStdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to stage changes: {}", e))
                })?;

            if !add_output.status.success() {
                let stderr = String::from_utf8_lossy(&add_output.stderr);
                warn!("Failed to stage changes for {}: {}", change_id, stderr);
                return Ok(());
            }

            // Amend the existing commit with the new changes and message
            let commit_output = Command::new("git")
                .args(["commit", "--amend", "-m", &commit_message])
                .current_dir(workspace_path)
                .stdin(StdStdio::null())
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to amend commit: {}", e))
                })?;

            if !commit_output.status.success() {
                let stderr = String::from_utf8_lossy(&commit_output.stderr);
                // If amend fails (e.g., no prior commit), try a regular commit
                if stderr.contains("No HEAD") || stderr.contains("does not have any commits") {
                    let initial_output = Command::new("git")
                        .args(["commit", "-m", &commit_message])
                        .current_dir(workspace_path)
                        .stdin(StdStdio::null())
                        .output()
                        .await;

                    if let Ok(output) = initial_output {
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            warn!(
                                "Failed to create initial commit for {}: {}",
                                change_id, stderr
                            );
                        } else {
                            debug!("Initial progress commit created for {} (git)", change_id);
                        }
                    }
                } else {
                    warn!("Failed to amend commit for {}: {}", change_id, stderr);
                }
            } else {
                debug!("Progress commit amended for {} (git)", change_id);
            }
        }
    }

    Ok(())
}

/// Check task progress for a change in the given workspace.
///
/// Reads and parses the tasks.md file to determine completion status.
/// Returns None if the file doesn't exist (e.g., after archiving).
pub fn check_task_progress(
    workspace_path: &Path,
    change_id: &str,
) -> Option<crate::task_parser::TaskProgress> {
    let tasks_path = workspace_path
        .join("openspec/changes")
        .join(change_id)
        .join("tasks.md");

    debug!("Checking tasks at: {:?}", tasks_path);

    if tasks_path.exists() {
        let progress = crate::task_parser::parse_file(&tasks_path).unwrap_or_default();
        debug!(
            "Tasks file found for {}: {}/{} complete",
            change_id, progress.completed, progress.total
        );
        Some(progress)
    } else {
        debug!("Tasks file not found at {:?}", tasks_path);
        None
    }
}

/// Summarize command output for logging and event reporting.
///
/// If output exceeds max_lines, returns the last few lines with a count prefix.
#[allow(dead_code)] // Utility function for future use
pub fn summarize_output(output: &str, max_lines: usize) -> String {
    if output.is_empty() {
        return String::new();
    }

    let lines: Vec<&str> = output.lines().collect();
    if lines.len() > max_lines {
        // Show last 5 lines with total count
        let tail_lines = 5.min(lines.len());
        format!(
            "... ({} lines) ...\n{}",
            lines.len(),
            lines[lines.len() - tail_lines..].join("\n")
        )
    } else {
        output.to_string()
    }
}

/// Execute apply command in a single workspace, repeating until tasks are 100% complete
pub async fn execute_apply_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    apply_cmd_template: &str,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
) -> Result<String> {
    const MAX_ITERATIONS: u32 = 50;
    let mut iteration = 0;
    let mut first_apply = true;

    loop {
        iteration += 1;
        if iteration > MAX_ITERATIONS {
            return Err(OrchestratorError::AgentCommand(format!(
                "Max iterations ({}) reached for change {}",
                MAX_ITERATIONS, change_id
            )));
        }

        // Check current task progress using helper
        let progress = check_task_progress(workspace_path, change_id).unwrap_or_default();

        // Send progress event only if we have valid progress data
        if progress.total > 0 {
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ProgressUpdated {
                        change_id: change_id.to_string(),
                        completed: progress.completed,
                        total: progress.total,
                    })
                    .await;
            }
        }

        // Check if already complete
        if progress.total > 0 && progress.completed == progress.total {
            info!(
                "Change {} is already complete ({}/{})",
                change_id, progress.completed, progress.total
            );
            break;
        }

        info!(
            "Executing apply #{} for {} in workspace ({}/{} tasks)",
            iteration, change_id, progress.completed, progress.total
        );

        // Send ApplyStarted event on first apply
        if first_apply {
            first_apply = false;
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ApplyStarted {
                        change_id: change_id.to_string(),
                    })
                    .await;
            }
        }

        // Expand change_id and prompt in command
        let command = OrchestratorConfig::expand_change_id(apply_cmd_template, change_id);
        let command = OrchestratorConfig::expand_prompt(&command, "");
        debug!("Workspace path: {:?}", workspace_path);
        debug!("Apply command: {}", command);

        // Execute command in workspace directory with streaming output
        // Use null stdin to prevent any interactive behavior
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(workspace_path)
            .stdin(StdStdio::null())
            .stdout(StdStdio::piped())
            .stderr(StdStdio::piped())
            .spawn()
            .map_err(|e| OrchestratorError::AgentCommand(format!("Failed to spawn: {}", e)))?;

        // Stream stdout and stderr in real-time
        let stdout = child.stdout.take().ok_or_else(|| {
            OrchestratorError::AgentCommand("Failed to capture stdout".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            OrchestratorError::AgentCommand("Failed to capture stderr".to_string())
        })?;

        let change_id_for_stdout = change_id.to_string();
        let event_tx_for_stdout = event_tx.clone();
        let stdout_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(ref tx) = event_tx_for_stdout {
                    let _ = tx
                        .send(ParallelEvent::ApplyOutput {
                            change_id: change_id_for_stdout.clone(),
                            output: line,
                        })
                        .await;
                }
            }
        });

        let change_id_for_stderr = change_id.to_string();
        let event_tx_for_stderr = event_tx.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(ref tx) = event_tx_for_stderr {
                    let _ = tx
                        .send(ParallelEvent::ApplyOutput {
                            change_id: change_id_for_stderr.clone(),
                            output: line,
                        })
                        .await;
                }
            }
        });

        // Wait for streams to complete
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        // Wait for process to finish
        let status = child
            .wait()
            .await
            .map_err(|e| OrchestratorError::AgentCommand(format!("Failed to wait: {}", e)))?;

        if !status.success() {
            return Err(OrchestratorError::AgentCommand(format!(
                "Apply command failed with exit code: {:?}",
                status.code()
            )));
        }

        // Snapshot working copy changes (for jj; no-op for git)
        // This ensures file modifications are visible for task progress check
        if vcs_backend == VcsBackend::Jj {
            let _ = Command::new("jj")
                .arg("status")
                .current_dir(workspace_path)
                .output()
                .await;
        }

        // Check task progress after apply using helper
        let new_progress = check_task_progress(workspace_path, change_id).unwrap_or_default();

        // Send progress event after apply only if we have valid progress data
        if new_progress.total > 0 {
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::ProgressUpdated {
                        change_id: change_id.to_string(),
                        completed: new_progress.completed,
                        total: new_progress.total,
                    })
                    .await;
            }
        }

        info!(
            "After apply #{}: {}/{} tasks complete",
            iteration, new_progress.completed, new_progress.total
        );

        // Create progress commit if progress was made
        // This ensures work is not lost if the process is interrupted
        if new_progress.completed > progress.completed {
            if let Err(e) =
                create_progress_commit(workspace_path, change_id, &new_progress, vcs_backend).await
            {
                warn!("Failed to create progress commit for {}: {}", change_id, e);
            }
        }

        // Check if complete
        if new_progress.total > 0 && new_progress.completed == new_progress.total {
            info!(
                "Change {} completed after {} iteration(s)",
                change_id, iteration
            );
            break;
        }

        // Check for progress (avoid infinite loops)
        if new_progress.completed <= progress.completed && iteration > 1 {
            warn!(
                "No progress made for {} (still {}/{}), continuing...",
                change_id, new_progress.completed, new_progress.total
            );
        }
    }

    // Set a meaningful commit message for the completed change
    let commit_message = format!("Apply: {}", change_id);

    match vcs_backend {
        VcsBackend::Jj => {
            // Use --ignore-working-copy to avoid stale working copy errors in workspaces
            let describe_output = Command::new("jj")
                .args(["describe", "--ignore-working-copy", "-m", &commit_message])
                .current_dir(workspace_path)
                .stdin(StdStdio::null())
                .output()
                .await;

            if let Err(e) = describe_output {
                warn!("Failed to set commit message for {}: {}", change_id, e);
            } else if let Ok(output) = describe_output {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("Failed to set commit message for {}: {}", change_id, stderr);
                }
            }
        }
        VcsBackend::Git | VcsBackend::Auto => {
            // For Git: stage all changes and commit (or amend if changes exist)
            let add_output = Command::new("git")
                .args(["add", "-A"])
                .current_dir(workspace_path)
                .stdin(StdStdio::null())
                .output()
                .await;

            if add_output.is_ok() {
                // Try to commit; if nothing to commit, that's fine
                let commit_output = Command::new("git")
                    .args(["commit", "-m", &commit_message, "--allow-empty"])
                    .current_dir(workspace_path)
                    .stdin(StdStdio::null())
                    .output()
                    .await;

                if let Err(e) = commit_output {
                    warn!("Failed to commit for {}: {}", change_id, e);
                }
            }
        }
    }

    // Get the resulting revision
    let revision = match vcs_backend {
        VcsBackend::Jj => {
            // Use --ignore-working-copy to avoid triggering automatic snapshot
            let revision_output = Command::new("jj")
                .args([
                    "log",
                    "-r",
                    "@",
                    "--no-graph",
                    "--ignore-working-copy",
                    "-T",
                    "change_id",
                ])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::JjCommand(format!("Failed to get revision: {}", e))
                })?;

            if !revision_output.status.success() {
                let stderr = String::from_utf8_lossy(&revision_output.stderr);
                return Err(OrchestratorError::JjCommand(format!(
                    "Failed to get workspace revision: {}",
                    stderr
                )));
            }

            String::from_utf8_lossy(&revision_output.stdout)
                .trim()
                .to_string()
        }
        VcsBackend::Git | VcsBackend::Auto => {
            let revision_output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to get revision: {}", e))
                })?;

            if !revision_output.status.success() {
                let stderr = String::from_utf8_lossy(&revision_output.stderr);
                return Err(OrchestratorError::GitCommand(format!(
                    "Failed to get workspace revision: {}",
                    stderr
                )));
            }

            String::from_utf8_lossy(&revision_output.stdout)
                .trim()
                .to_string()
        }
    };

    Ok(revision)
}

/// Execute archive command in a workspace with streaming output
pub async fn execute_archive_in_workspace(
    change_id: &str,
    workspace_path: &Path,
    archive_cmd_template: &str,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    vcs_backend: VcsBackend,
) -> Result<String> {
    // Verify task completion before archiving
    let tasks_path = workspace_path
        .join("openspec/changes")
        .join(change_id)
        .join("tasks.md");

    if tasks_path.exists() {
        let progress = crate::task_parser::parse_file(&tasks_path).unwrap_or_default();
        if progress.total > 0 && progress.completed < progress.total {
            return Err(OrchestratorError::AgentCommand(format!(
                "Cannot archive {}: tasks not complete ({}/{})",
                change_id, progress.completed, progress.total
            )));
        }
        info!(
            "Task verification passed for {}: {}/{}",
            change_id, progress.completed, progress.total
        );
    } else {
        warn!(
            "Tasks file not found for {} in workspace, proceeding with archive",
            change_id
        );
    }

    // Expand change_id and prompt in archive command
    let command = OrchestratorConfig::expand_change_id(archive_cmd_template, change_id);
    let command = OrchestratorConfig::expand_prompt(&command, config.get_archive_prompt());

    debug!("Archive command in workspace: {}", command);

    // Execute command with streaming output
    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(workspace_path)
        .stdin(StdStdio::null())
        .stdout(StdStdio::piped())
        .stderr(StdStdio::piped())
        .spawn()
        .map_err(|e| {
            OrchestratorError::AgentCommand(format!("Failed to spawn archive command: {}", e))
        })?;

    // Stream stdout
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let change_id_clone = change_id.to_string();
    let event_tx_clone = event_tx.clone();

    let stdout_handle = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(ref tx) = event_tx_clone {
                    let _ = tx
                        .send(ParallelEvent::ArchiveOutput {
                            change_id: change_id_clone.clone(),
                            output: line,
                        })
                        .await;
                }
            }
        }
    });

    let change_id_clone2 = change_id.to_string();
    let event_tx_clone2 = event_tx.clone();
    let stderr_handle = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(ref tx) = event_tx_clone2 {
                    let _ = tx
                        .send(ParallelEvent::ArchiveOutput {
                            change_id: change_id_clone2.clone(),
                            output: line,
                        })
                        .await;
                }
            }
        }
    });

    // Wait for streams to complete
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    // Wait for process to complete
    let status = child
        .wait()
        .await
        .map_err(|e| OrchestratorError::AgentCommand(format!("Archive command failed: {}", e)))?;

    if !status.success() {
        return Err(OrchestratorError::AgentCommand(format!(
            "Archive command failed with exit code: {:?}",
            status.code()
        )));
    }

    // Verify that the change was actually archived
    // The change directory should no longer exist in openspec/changes/ (except in archive/)
    let change_path = workspace_path.join("openspec/changes").join(change_id);
    let archive_dir = workspace_path.join("openspec/changes/archive");

    // Check if change was moved: original path should not exist,
    // and there should be an archive entry (format: {date}-{change_id} or just {change_id})
    let archived = if change_path.exists() {
        false
    } else if archive_dir.exists() {
        // Look for any directory in archive that ends with the change_id
        std::fs::read_dir(&archive_dir)
            .map(|entries| {
                entries.filter_map(|e| e.ok()).any(|entry| {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    name_str == change_id || name_str.ends_with(&format!("-{}", change_id))
                })
            })
            .unwrap_or(false)
    } else {
        false
    };

    if !archived {
        return Err(OrchestratorError::AgentCommand(format!(
            "Archive command succeeded but change '{}' was not actually archived. \
             The change directory still exists in openspec/changes/. \
             The archive command may not have executed 'openspec archive' correctly.",
            change_id
        )));
    }

    info!(
        "Archive verification passed for {}: change moved to archive",
        change_id
    );

    // Commit the archive changes if there are uncommitted changes
    // openspec archive moves files but doesn't commit them
    match vcs_backend {
        VcsBackend::Git | VcsBackend::Auto => {
            // Check for uncommitted changes
            let status_output = Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to check git status: {}", e))
                })?;

            let status = String::from_utf8_lossy(&status_output.stdout);
            if !status.trim().is_empty() {
                debug!("Committing archive changes for {}", change_id);

                // Stage all changes
                let add_output = Command::new("git")
                    .args(["add", "-A"])
                    .current_dir(workspace_path)
                    .output()
                    .await
                    .map_err(|e| {
                        OrchestratorError::GitCommand(format!("Failed to stage changes: {}", e))
                    })?;

                if !add_output.status.success() {
                    let stderr = String::from_utf8_lossy(&add_output.stderr);
                    return Err(OrchestratorError::GitCommand(format!(
                        "Failed to stage archive changes: {}",
                        stderr
                    )));
                }

                // Commit
                let commit_output = Command::new("git")
                    .args(["commit", "-m", &format!("Archive: {}", change_id)])
                    .current_dir(workspace_path)
                    .output()
                    .await
                    .map_err(|e| {
                        OrchestratorError::GitCommand(format!("Failed to commit: {}", e))
                    })?;

                if !commit_output.status.success() {
                    let stderr = String::from_utf8_lossy(&commit_output.stderr);
                    return Err(OrchestratorError::GitCommand(format!(
                        "Failed to commit archive changes: {}",
                        stderr
                    )));
                }

                info!("Committed archive changes for {}", change_id);
            }
        }
        VcsBackend::Jj => {
            // jj automatically tracks changes, just need to snapshot
            // Use --ignore-working-copy to avoid stale working copy errors in workspaces
            let snapshot_output = Command::new("jj")
                .args([
                    "describe",
                    "--ignore-working-copy",
                    "-m",
                    &format!("Archive: {}", change_id),
                ])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| OrchestratorError::JjCommand(format!("Failed to describe: {}", e)))?;

            if !snapshot_output.status.success() {
                let stderr = String::from_utf8_lossy(&snapshot_output.stderr);
                warn!("Failed to describe jj revision: {}", stderr);
            }
        }
    }

    // Get the current revision after archive
    // Use --ignore-working-copy to avoid stale working copy errors in workspaces
    let revision = match vcs_backend {
        VcsBackend::Jj => {
            let revision_output = Command::new("jj")
                .args([
                    "log",
                    "-r",
                    "@",
                    "--no-graph",
                    "--ignore-working-copy",
                    "-T",
                    "change_id",
                ])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::JjCommand(format!("Failed to get revision: {}", e))
                })?;

            if !revision_output.status.success() {
                let stderr = String::from_utf8_lossy(&revision_output.stderr);
                return Err(OrchestratorError::JjCommand(format!(
                    "Failed to get revision: {}",
                    stderr
                )));
            }

            String::from_utf8_lossy(&revision_output.stdout)
                .trim()
                .to_string()
        }
        VcsBackend::Git | VcsBackend::Auto => {
            let revision_output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(workspace_path)
                .output()
                .await
                .map_err(|e| {
                    OrchestratorError::GitCommand(format!("Failed to get revision: {}", e))
                })?;

            if !revision_output.status.success() {
                let stderr = String::from_utf8_lossy(&revision_output.stderr);
                return Err(OrchestratorError::GitCommand(format!(
                    "Failed to get revision: {}",
                    stderr
                )));
            }

            String::from_utf8_lossy(&revision_output.stdout)
                .trim()
                .to_string()
        }
    };

    Ok(revision)
}

#[cfg(test)]
mod tests {
    use crate::task_parser::TaskProgress;

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
        assert!(!(new_progress_same.completed > old_progress.completed));

        // Should create commit when progress increased
        assert!(new_progress_increased.completed > old_progress.completed);

        // Should NOT create commit when progress decreased (edge case)
        assert!(!(new_progress_decreased.completed > old_progress.completed));
    }
}
