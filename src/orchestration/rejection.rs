use std::path::{Path, PathBuf};

use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::error::{OrchestratorError, Result};
use crate::vcs::git::commands as git_commands;

fn rejected_file_path(workspace_path: &Path, change_id: &str) -> PathBuf {
    workspace_path
        .join("openspec")
        .join("changes")
        .join(change_id)
        .join("REJECTED.md")
}

pub fn has_rejection_proposal(workspace_path: &Path, change_id: &str) -> bool {
    rejected_file_path(workspace_path, change_id).is_file()
}

fn rejected_markdown(change_id: &str, reason: &str) -> String {
    format!(
        "# REJECTED\n\n- change_id: {}\n- reason: {}\n",
        change_id, reason
    )
}

fn extract_rejected_reason(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("- reason:").map(str::trim))
        .filter(|reason| !reason.is_empty())
        .map(ToString::to_string)
}

async fn resolve_rejection_reason(
    workspace_path: &Path,
    change_id: &str,
    fallback_reason: &str,
) -> String {
    let rejected_path = rejected_file_path(workspace_path, change_id);

    match tokio::fs::read_to_string(&rejected_path).await {
        Ok(content) => {
            if let Some(reason) = extract_rejected_reason(&content) {
                info!(
                    change_id = %change_id,
                    rejected_path = %rejected_path.display(),
                    "Using reason extracted from existing apply-generated REJECTED.md proposal"
                );
                reason
            } else {
                fallback_reason.to_string()
            }
        }
        Err(_) => fallback_reason.to_string(),
    }
}

async fn cleanup_worktree(repo_root: &Path, worktree_path: &Path) {
    let worktree_path_str = worktree_path.to_string_lossy();
    match git_commands::worktree_remove(repo_root, &worktree_path_str).await {
        Ok(()) => {
            info!(
                worktree = %worktree_path.display(),
                repo_root = %repo_root.display(),
                "Removed rejected worktree"
            );
        }
        Err(e) => {
            warn!(
                error = %e,
                worktree = %worktree_path.display(),
                repo_root = %repo_root.display(),
                "Failed to remove rejected worktree (may already be removed)"
            );
        }
    }
}

/// Execute rejection flow for acceptance-blocked changes.
///
/// Flow:
/// 1. checkout base branch
/// 2. write openspec/changes/<id>/REJECTED.md
/// 3. stage only openspec/changes/<id>/REJECTED.md
/// 4. commit on base branch
/// 5. cleanup rejected worktree
pub async fn execute_rejection_flow(
    change_id: &str,
    reason: &str,
    workspace_path: &Path,
    base_branch: &str,
    repo_root: &Path,
) -> Result<()> {
    info!(
        change_id = %change_id,
        workspace = %workspace_path.display(),
        repo_root = %repo_root.display(),
        base_branch = %base_branch,
        "Starting rejection flow"
    );

    let effective_reason = resolve_rejection_reason(workspace_path, change_id, reason).await;

    git_commands::checkout(repo_root, base_branch)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;

    let rejected_path = rejected_file_path(repo_root, change_id);
    let rejected_parent = rejected_path.parent().ok_or_else(|| {
        OrchestratorError::AgentCommand(format!(
            "Invalid REJECTED.md path for change '{}'",
            change_id
        ))
    })?;

    tokio::fs::create_dir_all(rejected_parent).await?;
    tokio::fs::write(
        &rejected_path,
        rejected_markdown(change_id, &effective_reason),
    )
    .await?;

    let relative_rejected_path = format!("openspec/changes/{}/REJECTED.md", change_id);
    let add_output = Command::new("git")
        .args(["add", &relative_rejected_path])
        .current_dir(repo_root)
        .output()
        .await?;
    if !add_output.status.success() {
        return Err(OrchestratorError::AgentCommand(format!(
            "git add failed for '{}': {}",
            relative_rejected_path,
            String::from_utf8_lossy(&add_output.stderr).trim()
        )));
    }

    let staged_paths_output = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(repo_root)
        .output()
        .await?;
    if !staged_paths_output.status.success() {
        return Err(OrchestratorError::AgentCommand(format!(
            "git diff --cached --name-only failed for rejection '{}': {}",
            change_id,
            String::from_utf8_lossy(&staged_paths_output.stderr).trim()
        )));
    }

    let staged_paths_stdout = String::from_utf8_lossy(&staged_paths_output.stdout).into_owned();
    let staged_paths = staged_paths_stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if staged_paths != vec![relative_rejected_path.clone()] {
        return Err(OrchestratorError::AgentCommand(format!(
            "rejection flow staged unexpected files for '{}': {:?}",
            change_id, staged_paths
        )));
    }

    let commit_message = format!("reject(openspec): {}", change_id);
    let commit_output = Command::new("git")
        .args(["commit", "-m", &commit_message, "--", &relative_rejected_path])
        .current_dir(repo_root)
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

    cleanup_worktree(repo_root, workspace_path).await;

    info!(change_id = %change_id, "Rejection flow completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    async fn init_git_repo(path: &Path) {
        let status = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .status()
            .await
            .expect("git init failed");
        assert!(status.success());

        let status = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(path)
            .status()
            .await
            .expect("git config email failed");
        assert!(status.success());

        let status = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .status()
            .await
            .expect("git config name failed");
        assert!(status.success());

        fs::write(path.join("README.md"), "# test\n").expect("write readme");
        let status = Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .status()
            .await
            .expect("git add failed");
        assert!(status.success());

        let status = Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .status()
            .await
            .expect("git commit failed");
        assert!(status.success());
    }

    #[test]
    fn test_rejected_markdown_contains_reason() {
        let content = rejected_markdown("change-a", "spec mismatch");
        assert!(content.contains("change_id: change-a"));
        assert!(content.contains("reason: spec mismatch"));
    }

    #[test]
    fn test_extract_rejected_reason_parses_reason_line() {
        let content = "# REJECTED\n\n- change_id: change-a\n- reason: apply blocked handoff\n";
        let reason = extract_rejected_reason(content);
        assert_eq!(reason.as_deref(), Some("apply blocked handoff"));
    }

    #[test]
    fn test_extract_rejected_reason_returns_none_without_reason_line() {
        let content = "# REJECTED\n\n- change_id: change-a\n";
        let reason = extract_rejected_reason(content);
        assert!(reason.is_none());
    }

    #[test]
    fn test_rejected_file_path_layout() {
        let path = rejected_file_path(Path::new("/tmp/ws"), "change-a");
        assert!(path.ends_with("openspec/changes/change-a/REJECTED.md"));
    }

    #[test]
    fn test_has_rejection_proposal_detects_marker_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let change_dir = temp_dir.path().join("openspec/changes/change-a");
        std::fs::create_dir_all(&change_dir).unwrap();

        assert!(
            !has_rejection_proposal(temp_dir.path(), "change-a"),
            "proposal should be absent before REJECTED.md exists"
        );

        std::fs::write(change_dir.join("REJECTED.md"), "# REJECTED").unwrap();
        assert!(
            has_rejection_proposal(temp_dir.path(), "change-a"),
            "proposal should be detected after REJECTED.md is created"
        );
    }

    #[tokio::test]
    async fn test_execute_rejection_flow_creates_marker_commits_and_cleans_worktree() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let repo_root = temp_dir.path();
        init_git_repo(repo_root).await;

        let change_id = "blocked-change";
        let change_dir = repo_root.join("openspec").join("changes").join(change_id);
        fs::create_dir_all(&change_dir).expect("create change dir");
        fs::write(change_dir.join("proposal.md"), "# proposal\n").expect("write proposal");
        fs::write(change_dir.join("tasks.md"), "- [ ] task\n").expect("write tasks");

        let current_branch = git_commands::get_current_branch(repo_root)
            .await
            .expect("current branch")
            .expect("branch name");

        let worktree_parent = repo_root.join(".worktrees");
        fs::create_dir_all(&worktree_parent).expect("create worktree parent");
        let worktree_path = worktree_parent.join(change_id);
        git_commands::worktree_add(
            repo_root,
            worktree_path.to_str().expect("worktree path"),
            &format!("wt/{}", change_id),
            &current_branch,
        )
        .await
        .expect("create worktree");

        let result = execute_rejection_flow(
            change_id,
            "Implementation blocker detected",
            &worktree_path,
            &current_branch,
            repo_root,
        )
        .await;

        assert!(result.is_ok(), "rejection flow should succeed: {result:?}");

        let marker_path = change_dir.join("REJECTED.md");
        assert!(marker_path.exists(), "REJECTED.md must be created");
        let marker = fs::read_to_string(&marker_path).expect("read marker");
        assert!(marker.contains("change_id: blocked-change"));
        assert!(marker.contains("reason: Implementation blocker detected"));

        let head_message = Command::new("git")
            .args(["log", "-1", "--pretty=%s"])
            .current_dir(repo_root)
            .output()
            .await
            .expect("read commit message");
        assert!(head_message.status.success());
        let message = String::from_utf8_lossy(&head_message.stdout);
        assert!(message
            .trim()
            .starts_with("reject(openspec): blocked-change"));

        let committed_paths = Command::new("git")
            .args(["show", "--name-only", "--pretty=format:", "HEAD"])
            .current_dir(repo_root)
            .output()
            .await
            .expect("read committed paths");
        assert!(committed_paths.status.success());
        let committed_paths_stdout =
            String::from_utf8_lossy(&committed_paths.stdout).into_owned();
        let committed_paths = committed_paths_stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            committed_paths,
            vec!["openspec/changes/blocked-change/REJECTED.md".to_string()],
            "rejection commit must contain only REJECTED.md"
        );

        let list = git_commands::list_worktrees(repo_root)
            .await
            .expect("list worktrees after cleanup");
        assert!(
            !list
                .iter()
                .any(|(path, _, _, _, _)| path == &worktree_path.to_string_lossy()),
            "rejected worktree must be removed"
        );
    }
}
