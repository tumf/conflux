use std::path::{Path, PathBuf};

use tokio::process::Command;
use tracing::{debug, info};

use crate::error::{OrchestratorError, Result};
use crate::vcs::git::commands as git_commands;

fn rejected_file_path(workspace_path: &Path, change_id: &str) -> PathBuf {
    workspace_path
        .join("openspec")
        .join("changes")
        .join(change_id)
        .join("REJECTED.md")
}

fn rejected_markdown(change_id: &str, reason: &str) -> String {
    format!(
        "# REJECTED\n\n- change_id: {}\n- reason: {}\n",
        change_id, reason
    )
}

async fn run_openspec_resolve(change_id: &str, workspace_path: &Path) -> Result<()> {
    let output = Command::new("openspec")
        .arg("resolve")
        .arg(change_id)
        .current_dir(workspace_path)
        .output()
        .await
        .map_err(|e| {
            OrchestratorError::AgentCommand(format!(
                "Failed to execute openspec resolve for '{}': {}",
                change_id, e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OrchestratorError::AgentCommand(format!(
            "openspec resolve failed for '{}': {}",
            change_id,
            stderr.trim()
        )));
    }

    Ok(())
}

/// Execute rejection flow for acceptance-blocked changes.
///
/// Flow:
/// 1. checkout base branch
/// 2. write openspec/changes/<id>/REJECTED.md
/// 3. commit on base branch
/// 4. run openspec resolve <id>
pub async fn execute_rejection_flow(
    change_id: &str,
    reason: &str,
    workspace_path: &Path,
    base_branch: &str,
) -> Result<()> {
    info!(
        change_id = %change_id,
        workspace = %workspace_path.display(),
        base_branch = %base_branch,
        "Starting rejection flow"
    );

    git_commands::checkout(workspace_path, base_branch)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;

    let rejected_path = rejected_file_path(workspace_path, change_id);
    let rejected_parent = rejected_path.parent().ok_or_else(|| {
        OrchestratorError::AgentCommand(format!(
            "Invalid REJECTED.md path for change '{}'",
            change_id
        ))
    })?;

    tokio::fs::create_dir_all(rejected_parent).await?;
    tokio::fs::write(&rejected_path, rejected_markdown(change_id, reason)).await?;

    let relative_rejected_path = format!("openspec/changes/{}/REJECTED.md", change_id);
    let add_output = Command::new("git")
        .args(["add", &relative_rejected_path])
        .current_dir(workspace_path)
        .output()
        .await?;
    if !add_output.status.success() {
        return Err(OrchestratorError::AgentCommand(format!(
            "git add failed for '{}': {}",
            relative_rejected_path,
            String::from_utf8_lossy(&add_output.stderr).trim()
        )));
    }

    let commit_message = format!("reject(openspec): {}", change_id);
    let commit_output = Command::new("git")
        .args(["commit", "-m", &commit_message])
        .current_dir(workspace_path)
        .output()
        .await?;
    if !commit_output.status.success() {
        return Err(OrchestratorError::AgentCommand(format!(
            "git commit failed for rejection '{}': {}",
            change_id,
            String::from_utf8_lossy(&commit_output.stderr).trim()
        )));
    }

    debug!(change_id = %change_id, "Committed REJECTED.md on base branch");

    run_openspec_resolve(change_id, workspace_path).await?;

    info!(change_id = %change_id, "Rejection flow completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejected_markdown_contains_reason() {
        let content = rejected_markdown("change-a", "spec mismatch");
        assert!(content.contains("change_id: change-a"));
        assert!(content.contains("reason: spec mismatch"));
    }

    #[test]
    fn test_rejected_file_path_layout() {
        let path = rejected_file_path(Path::new("/tmp/ws"), "change-a");
        assert!(path.ends_with("openspec/changes/change-a/REJECTED.md"));
    }
}
