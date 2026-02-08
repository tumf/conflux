//! Git commit operations.
//!
//! This module provides functions for creating, managing, and querying Git commits.

use super::basic::run_git;
use crate::vcs::{VcsError, VcsResult};
use std::path::Path;
use tracing::debug;

/// Create a WIP archive commit for a retry attempt.
pub async fn create_archive_wip_commit<P: AsRef<Path>>(
    cwd: P,
    change_id: &str,
    attempt: u32,
) -> VcsResult<()> {
    let message = format!("WIP(archive): {} (attempt#{})", change_id, attempt);
    run_git(&["add", "-A"], &cwd).await?;
    run_git(&["commit", "--allow-empty", "-m", &message], cwd).await?;
    Ok(())
}

/// Squash all archive WIP commits into a final Archive commit.
pub async fn squash_archive_wip_commits<P: AsRef<Path>>(cwd: P, change_id: &str) -> VcsResult<()> {
    let wip_pattern = format!("WIP(archive): {}", change_id);
    let wip_commits = run_git(
        &[
            "rev-list",
            "--reverse",
            "--grep",
            &wip_pattern,
            "--fixed-strings",
            "HEAD",
        ],
        &cwd,
    )
    .await?;
    let first_wip = wip_commits
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| {
            VcsError::git_command(format!("No archive WIP commits found for {}", change_id))
        })?;

    let parent_revision = run_git(&["rev-parse", &format!("{}^", first_wip)], &cwd).await?;
    let parent_revision = parent_revision.trim();

    run_git(&["reset", "--soft", parent_revision], &cwd).await?;
    let archive_message = format!("Archive: {}", change_id);
    run_git(&["commit", "--allow-empty", "-m", &archive_message], cwd).await?;
    Ok(())
}

/// List change IDs from the HEAD commit tree.
///
/// Reads directories under `openspec/changes` in the HEAD tree and filters out
/// archive, hidden entries, and changes without proposal.md.
pub async fn list_changes_in_head<P: AsRef<Path>>(cwd: P) -> VcsResult<Vec<String>> {
    let cwd_ref = cwd.as_ref();

    let output = run_git(
        &["ls-tree", "-d", "--name-only", "HEAD:openspec/changes"],
        cwd_ref,
    )
    .await?;

    let mut change_ids: Vec<String> = Vec::new();

    for name in output.lines().map(str::trim) {
        if name.is_empty() || name == "archive" || name.starts_with('.') {
            continue;
        }

        // Check if proposal.md exists in HEAD for this change
        let proposal_path = format!("HEAD:openspec/changes/{}/proposal.md", name);
        match run_git(&["cat-file", "-e", &proposal_path], cwd_ref).await {
            Ok(_) => {
                // proposal.md exists, include this change
                change_ids.push(name.to_string());
            }
            Err(_) => {
                // proposal.md doesn't exist, skip this change
                debug!("Skipping change '{}' in HEAD - no proposal.md found", name);
            }
        }
    }

    change_ids.sort();
    Ok(change_ids)
}

/// Stage all changes and commit with the given message.
#[allow(dead_code)]
pub async fn add_and_commit<P: AsRef<Path>>(cwd: P, message: &str) -> VcsResult<()> {
    run_git(&["add", "-A"], &cwd).await?;
    run_git(&["commit", "-m", message], cwd).await?;
    Ok(())
}

/// Check if there are staged or unstaged changes to commit.
#[allow(dead_code)]
pub async fn has_changes_to_commit<P: AsRef<Path>>(cwd: P) -> VcsResult<bool> {
    let output = run_git(&["status", "--porcelain"], cwd).await?;
    Ok(!output.is_empty())
}

/// List change IDs that have uncommitted or untracked files under `openspec/changes/<change_id>/`.
///
/// This function detects changes with:
/// - Staged changes (added, modified, deleted)
/// - Unstaged changes (modified, deleted)
/// - Untracked files
///
/// Returns a sorted vector of change IDs that have uncommitted modifications.
pub async fn list_changes_with_uncommitted_files<P: AsRef<Path>>(cwd: P) -> VcsResult<Vec<String>> {
    let cwd_ref = cwd.as_ref();

    // Get all files with uncommitted changes or untracked status
    // Use -u to show individual untracked files instead of just directories
    let output = run_git(&["status", "--porcelain", "-u"], cwd_ref).await?;

    let mut change_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        // Parse porcelain format path field.
        // Normal: "XY <path>" (2 status columns + separator)
        // Fallback: "Y <path>" for first line when run_git() trimmed a leading space.
        let bytes = line.as_bytes();
        let path_field = if bytes.len() >= 3 && bytes[2] == b' ' {
            &line[3..]
        } else if bytes.len() >= 2 && bytes[1] == b' ' {
            &line[2..]
        } else {
            continue;
        };

        // Handle rename format: "old/path -> new/path". Use destination path.
        let path = path_field
            .split_once(" -> ")
            .map(|(_, new_path)| new_path)
            .unwrap_or(path_field)
            .trim();

        // Check if path is under openspec/changes/<change_id>/
        if let Some(change_id) = extract_change_id_from_path(path) {
            change_ids.insert(change_id);
        }
    }

    let mut result: Vec<String> = change_ids.into_iter().collect();
    result.sort();
    Ok(result)
}

/// Extract change_id from a file path if it matches `openspec/changes/<change_id>/...`
///
/// Returns None if the path doesn't match the expected pattern or refers to the archive directory.
fn extract_change_id_from_path(path: &str) -> Option<String> {
    // Normalize path separators for cross-platform compatibility
    let normalized_path = path.replace('\\', "/");

    // Expected pattern: openspec/changes/<change_id>/...
    let prefix = "openspec/changes/";
    if !normalized_path.starts_with(prefix) {
        return None;
    }

    // Extract the part after "openspec/changes/"
    let remainder = &normalized_path[prefix.len()..];

    // Find the first path separator to get the change_id
    let change_id = if let Some(pos) = remainder.find('/') {
        &remainder[..pos]
    } else {
        // Path is exactly "openspec/changes/<change_id>" (no trailing slash)
        remainder
    };

    // Filter out special directories
    if change_id.is_empty() || change_id == "archive" || change_id.starts_with('.') {
        return None;
    }

    Some(change_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::process::Command;

    #[tokio::test]
    async fn test_list_changes_in_head_filters_special_dirs() {
        let temp_dir = TempDir::new().unwrap();

        debug!(
            module = module_path!(),
            "Executing git command: git init (cwd: {:?})",
            temp_dir.path()
        );
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        if init_result.is_err() {
            return;
        }

        debug!(
            module = module_path!(),
            "Executing git command: git config user.email test@example.com (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        debug!(
            module = module_path!(),
            "Executing git command: git config user.name Test User (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let base_dir = temp_dir.path().join("openspec/changes");
        std::fs::create_dir_all(base_dir.join("change-b")).unwrap();
        std::fs::create_dir_all(base_dir.join("change-a")).unwrap();
        std::fs::create_dir_all(base_dir.join("archive")).unwrap();
        std::fs::create_dir_all(base_dir.join(".hidden")).expect("create hidden dir");
        std::fs::write(base_dir.join("change-a").join("proposal.md"), "test").unwrap();
        std::fs::write(base_dir.join("change-b").join("proposal.md"), "test").unwrap();
        std::fs::write(base_dir.join("archive").join("proposal.md"), "test").unwrap();

        debug!(
            module = module_path!(),
            "Executing git command: git add . (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        debug!(
            module = module_path!(),
            "Executing git command: git commit -m add changes (cwd: {:?})",
            temp_dir.path()
        );
        let _ = Command::new("git")
            .args(["commit", "-m", "add changes"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let changes = list_changes_in_head(temp_dir.path()).await.unwrap();
        assert_eq!(
            changes,
            vec!["change-a".to_string(), "change-b".to_string()]
        );
    }

    #[tokio::test]
    async fn test_list_changes_in_head_excludes_without_proposal() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        if init_result.is_err() {
            return; // Skip if git not available
        }

        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let base_dir = temp_dir.path().join("openspec/changes");

        // Create change-a WITH proposal.md
        std::fs::create_dir_all(base_dir.join("change-a")).unwrap();
        std::fs::write(base_dir.join("change-a").join("proposal.md"), "test").unwrap();
        std::fs::write(base_dir.join("change-a").join("tasks.md"), "- [ ] Task 1").unwrap();

        // Create change-b WITHOUT proposal.md (only tasks.md)
        std::fs::create_dir_all(base_dir.join("change-b")).unwrap();
        std::fs::write(base_dir.join("change-b").join("tasks.md"), "- [ ] Task 1").unwrap();

        // Create change-c WITH proposal.md
        std::fs::create_dir_all(base_dir.join("change-c")).unwrap();
        std::fs::write(base_dir.join("change-c").join("proposal.md"), "test").unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "add changes"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let changes = list_changes_in_head(temp_dir.path()).await.unwrap();

        // Should only include change-a and change-c (have proposal.md)
        // change-b should be excluded (no proposal.md)
        assert_eq!(
            changes,
            vec!["change-a".to_string(), "change-c".to_string()]
        );
    }

    #[tokio::test]
    async fn test_list_changes_with_uncommitted_files() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        if init_result.is_err() {
            return; // Skip if git not available
        }

        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let base_dir = temp_dir.path().join("openspec/changes");

        // Create change-a and commit it
        std::fs::create_dir_all(base_dir.join("change-a")).unwrap();
        std::fs::write(base_dir.join("change-a").join("proposal.md"), "test").unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "add change-a"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Create change-b with uncommitted files
        std::fs::create_dir_all(base_dir.join("change-b")).unwrap();
        std::fs::write(base_dir.join("change-b").join("proposal.md"), "uncommitted").unwrap();

        // Modify change-a after commit (staged change)
        std::fs::write(base_dir.join("change-a").join("tasks.md"), "new task").unwrap();
        let _ = Command::new("git")
            .args(["add", "openspec/changes/change-a/tasks.md"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let uncommitted_changes = list_changes_with_uncommitted_files(temp_dir.path())
            .await
            .unwrap();

        // Should include both change-a (staged) and change-b (untracked)
        assert_eq!(
            uncommitted_changes,
            vec!["change-a".to_string(), "change-b".to_string()]
        );
    }

    #[tokio::test]
    async fn test_list_changes_with_uncommitted_files_filters_archive() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        if init_result.is_err() {
            return; // Skip if git not available
        }

        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let base_dir = temp_dir.path().join("openspec/changes");

        // Create initial commit to establish git repository
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::write(temp_dir.path().join("README.md"), "test").unwrap();
        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "initial commit"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Create archive directory with uncommitted files (should be ignored)
        std::fs::create_dir_all(base_dir.join("archive")).unwrap();
        std::fs::write(base_dir.join("archive").join("old.md"), "archived").unwrap();

        // Create regular change with uncommitted files
        std::fs::create_dir_all(base_dir.join("change-c")).unwrap();
        std::fs::write(base_dir.join("change-c").join("proposal.md"), "test").unwrap();

        let uncommitted_changes = list_changes_with_uncommitted_files(temp_dir.path())
            .await
            .unwrap();

        // Should only include change-c, not archive
        assert_eq!(uncommitted_changes, vec!["change-c".to_string()]);
    }

    #[tokio::test]
    async fn test_list_changes_with_uncommitted_files_detects_unstaged_modified_first_line() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        if init_result.is_err() {
            return; // Skip if git not available
        }

        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let change_dir = temp_dir.path().join("openspec/changes/change-a");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "test").unwrap();
        std::fs::write(change_dir.join("tasks.md"), "- [ ] task").unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "add change-a"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        // Create unstaged modification. Status line is typically " M ...",
        // and the leading space can be trimmed by run_git() for the first line.
        std::fs::write(change_dir.join("tasks.md"), "- [x] task").unwrap();

        let uncommitted_changes = list_changes_with_uncommitted_files(temp_dir.path())
            .await
            .unwrap();

        assert_eq!(uncommitted_changes, vec!["change-a".to_string()]);
    }

    #[test]
    fn test_extract_change_id_from_path() {
        assert_eq!(
            extract_change_id_from_path("openspec/changes/my-change/proposal.md"),
            Some("my-change".to_string())
        );
        assert_eq!(
            extract_change_id_from_path("openspec/changes/my-change/specs/auth/spec.md"),
            Some("my-change".to_string())
        );
        assert_eq!(
            extract_change_id_from_path("openspec/changes/archive/old.md"),
            None
        );
        assert_eq!(
            extract_change_id_from_path("openspec/changes/.hidden/file.md"),
            None
        );
        assert_eq!(extract_change_id_from_path("src/main.rs"), None);
        assert_eq!(
            extract_change_id_from_path("openspec/specs/auth/spec.md"),
            None
        );
    }
}
