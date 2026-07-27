//! Workspace state detection module for OpenSpec Orchestrator.
//!
//! This module provides state detection logic for workspace resume operations,
//! enabling idempotent parallel execution. It detects the current state of a
//! workspace and determines the appropriate action to take during resume.
//!
//! # Workspace States
//!
//! 1. **Created**: Workspace created, no apply commits → Start apply
//! 2. **Applying**: WIP commits exist, apply in progress → Resume apply (next iteration)
//! 3. **Applied**: Apply complete, archive not complete → Resume decision is delegated to worktree routing (non-terminal resumes go to apply or acceptance, never direct archive)
//! 4. **Archiving**: Archive files moved, commit not complete → Resume archive to finish in-progress archive step (this state occurs only after acceptance has already handed off to archive)
//! 5. **Archived**: Archive complete, not merged to main → Merge only
//! 6. **Merged**: Merged to main → Skip & Cleanup
//!
//! # Example
//!
//! ```ignore
//! use crate::execution::state::{detect_workspace_state, WorkspaceState};
//!
//! let state = detect_workspace_state("add-feature", &workspace_path).await?;
//! match state {
//!     WorkspaceState::Created => { /* start apply */ }
//!     WorkspaceState::Applying { iteration } => { /* resume from iteration */ }
//!     WorkspaceState::Blocked => { /* wait for unblock/resume */ }
//!     WorkspaceState::Rejecting => { /* run rejecting review */ }
//!     WorkspaceState::Applied => { /* defer to resume-action router (apply or acceptance) */ }
//!     WorkspaceState::Archiving => { /* resume archive loop after acceptance handoff */ }
//!     WorkspaceState::Archived => { /* merge only */ }
//!     WorkspaceState::Merged => { /* skip & cleanup */ }
//! }
//! ```

use std::path::Path;
use tokio::process::Command;
use tracing::debug;

use crate::error::{OrchestratorError, Result};
use crate::execution::archive::is_archive_commit_complete;
use crate::parallel::acceptance_state::{
    parse_blocked_marker, reconcile_acceptance_stall, repository_identity, worktree_identity,
    AcceptanceStallRecord, AcceptanceStallStore, StallReconciliation, WorkspaceFacts,
};

/// Workspace state for resume detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceState {
    /// Workspace created, no apply commits yet.
    Created,
    /// Apply in progress, WIP commits exist.
    /// The iteration number indicates the next iteration to resume from.
    Applying { iteration: u32 },
    /// Apply-generated blocker marker exists and change should remain blocked.
    Blocked,
    /// Rejection proposal exists and requires rejection review.
    Rejecting,
    /// Apply complete, archive not complete.
    Applied,
    /// Archive files moved but commit not complete.
    Archiving,
    /// Archive complete, not merged to main.
    Archived,
    /// Merged to main.
    Merged,
}

/// Check if a change has been merged to the base branch.
///
/// This function checks if the archive entry exists and the change directory
/// has been removed in the base branch's HEAD tree. It uses file state only
/// and does NOT check commit messages.
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path
/// * `base_branch` - The base branch to check against
///
/// # Returns
///
/// * `Ok(true)` - Archive entry exists in base branch HEAD tree and change directory is gone
/// * `Ok(false)` - Archive entry does not exist or change directory still exists in base branch
/// * `Err` - Failed to check merge status
pub async fn is_merged_to_base(
    change_id: &str,
    repo_root: &Path,
    base_branch: &str,
) -> Result<bool> {
    // Check if base branch exists
    let rev_parse_output = Command::new("git")
        .args(["rev-parse", "--verify", base_branch])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| {
            OrchestratorError::GitCommand(format!("Failed to verify base branch: {}", e))
        })?;

    if !rev_parse_output.status.success() {
        debug!(
            base_branch = %base_branch,
            "is_merged_to_base: base branch does not exist"
        );
        return Ok(false);
    }

    // Check if archive entry exists in base branch HEAD tree
    let archive_path = format!("{}:openspec/changes/archive/", base_branch);
    let ls_tree_output = Command::new("git")
        .args(["ls-tree", "-d", &archive_path])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| {
            OrchestratorError::GitCommand(format!("Failed to list archive tree: {}", e))
        })?;

    if !ls_tree_output.status.success() {
        // Archive directory doesn't exist in base branch
        debug!(
            base_branch = %base_branch,
            "is_merged_to_base: archive directory does not exist in base branch"
        );
        return Ok(false);
    }

    // Parse ls-tree output to find matching archive entries
    let output = String::from_utf8_lossy(&ls_tree_output.stdout);
    let archive_entry_exists = output.lines().any(|line| {
        // Parse line format: "040000 tree <hash>\t<name>"
        if let Some(name) = line.split('\t').nth(1) {
            name == change_id || name.ends_with(&format!("-{}", change_id))
        } else {
            false
        }
    });

    // Check if change directory exists in base branch HEAD tree
    let change_path = format!("{}:openspec/changes/{}", base_branch, change_id);
    let change_exists_output = Command::new("git")
        .args(["ls-tree", "-d", &change_path])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| {
            OrchestratorError::GitCommand(format!("Failed to check change tree: {}", e))
        })?;

    let change_dir_exists = change_exists_output.status.success()
        && !String::from_utf8_lossy(&change_exists_output.stdout)
            .trim()
            .is_empty();

    debug!(
        change_id = %change_id,
        base_branch = %base_branch,
        archive_entry_exists = archive_entry_exists,
        change_dir_exists = change_dir_exists,
        "is_merged_to_base: checking base branch HEAD tree file state"
    );

    // Only consider merged if archive entry exists in base AND change directory is gone from base
    if archive_entry_exists && change_dir_exists {
        tracing::warn!(
            change_id = %change_id,
            base_branch = %base_branch,
            "Archive entry found in base branch but change directory still exists in base branch tree"
        );
        return Ok(false);
    }

    Ok(archive_entry_exists && !change_dir_exists)
}

/// Get the latest WIP snapshot iteration number.
///
/// This function searches for WIP commit messages in the format
/// `WIP(apply): <change_id> (iteration N/M)` and returns the highest
/// iteration number found.
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path
///
/// # Returns
///
/// * `Ok(Some(n))` - WIP commits exist, highest iteration is n
/// * `Ok(None)` - No WIP commits found
/// * `Err` - Failed to check WIP commits
pub async fn get_latest_wip_snapshot(change_id: &str, repo_root: &Path) -> Result<Option<u32>> {
    let wip_prefix = format!("WIP(apply): {}", change_id);

    let log_output = Command::new("git")
        .args(["log", "--format=%s", "--grep", &wip_prefix, "--all-match"])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| OrchestratorError::GitCommand(format!("Failed to read git log: {}", e)))?;

    if !log_output.status.success() {
        let stderr = String::from_utf8_lossy(&log_output.stderr);
        return Err(OrchestratorError::GitCommand(format!(
            "Failed to read git log: {}",
            stderr
        )));
    }

    let commits = String::from_utf8_lossy(&log_output.stdout);
    let mut max_iteration = None;

    // Parse WIP commit messages: "WIP(apply): <change_id> (iteration N/M)"
    for line in commits.lines() {
        if let Some(iteration_part) = line.strip_prefix(&wip_prefix) {
            // Extract iteration number from "(iteration N/M)"
            if let Some(captures) = iteration_part
                .trim()
                .strip_prefix("(iteration ")
                .and_then(|s| s.split_once('/'))
            {
                if let Ok(iteration) = captures.0.trim().parse::<u32>() {
                    max_iteration =
                        Some(max_iteration.map_or(iteration, |m: u32| m.max(iteration)));
                }
            }
        }
    }

    debug!(
        change_id = %change_id,
        max_iteration = ?max_iteration,
        "get_latest_wip_snapshot: found WIP commits"
    );

    Ok(max_iteration)
}

/// Check if the workspace is in the "archiving" state.
///
/// The archiving state occurs when files have been moved to the archive directory
/// but the commit is not yet complete (working tree is dirty).
///
/// This function checks if:
/// 1. The worktree is dirty (has uncommitted changes)
/// 2. The change directory does NOT exist in `openspec/changes/<change_id>`
/// 3. An archive entry exists in `openspec/changes/archive/`
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path (workspace path)
///
/// # Returns
///
/// * `Ok(true)` - In archiving state (dirty, change gone, archive exists)
/// * `Ok(false)` - Not in archiving state
/// * `Err` - Failed to check archiving state
pub async fn has_archive_files(change_id: &str, repo_root: &Path) -> Result<bool> {
    // Check if working tree is dirty
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| OrchestratorError::GitCommand(format!("Failed to check git status: {}", e)))?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(OrchestratorError::GitCommand(format!(
            "Failed to check git status: {}",
            stderr
        )));
    }

    let is_dirty = !String::from_utf8_lossy(&status_output.stdout)
        .trim()
        .is_empty();

    // Check if change directory exists (should NOT exist for archiving state)
    let change_path = repo_root.join("openspec/changes").join(change_id);
    let change_exists = change_path.exists();

    // Check for archive directory (supports both formats)
    // 1. openspec/changes/archive/{change_id}
    // 2. openspec/changes/archive/{date}-{change_id}
    let archive_base = repo_root.join("openspec/changes/archive");
    let mut archive_entry_exists = false;

    if archive_base.exists() {
        // Check for exact match first
        let exact_match = archive_base.join(change_id);
        if exact_match.exists() && exact_match.is_dir() {
            archive_entry_exists = true;
        } else {
            // Check for date-prefixed match
            if let Ok(entries) = std::fs::read_dir(&archive_base) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();

                    // Check if it ends with "-{change_id}" and is a directory
                    if name_str.ends_with(&format!("-{}", change_id)) && entry.path().is_dir() {
                        archive_entry_exists = true;
                        break;
                    }
                }
            }
        }
    }

    debug!(
        change_id = %change_id,
        is_dirty = is_dirty,
        change_exists = change_exists,
        archive_entry_exists = archive_entry_exists,
        "has_archive_files: checking archiving state (dirty={}, change_gone={}, archive_exists={})",
        is_dirty,
        !change_exists,
        archive_entry_exists
    );

    // Archiving state requires:
    // 1. Worktree is dirty
    // 2. Change directory is gone
    // 3. Archive entry exists
    Ok(is_dirty && !change_exists && archive_entry_exists)
}

/// Check if an apply commit exists for a change.
///
/// An apply commit is a non-WIP commit that indicates apply completion.
/// This function checks for commits with the message `Apply: <change_id>`.
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path
///
/// # Returns
///
/// * `Ok(true)` - Apply commit exists
/// * `Ok(false)` - Apply commit does not exist
/// * `Err` - Failed to check apply commit
pub async fn has_apply_commit(change_id: &str, repo_root: &Path) -> Result<bool> {
    let expected_subject = format!("Apply: {}", change_id);

    let log_output = Command::new("git")
        .args([
            "log",
            "--format=%s",
            "--grep",
            &expected_subject,
            "--all-match",
        ])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| OrchestratorError::GitCommand(format!("Failed to read git log: {}", e)))?;

    if !log_output.status.success() {
        let stderr = String::from_utf8_lossy(&log_output.stderr);
        return Err(OrchestratorError::GitCommand(format!(
            "Failed to read git log: {}",
            stderr
        )));
    }

    let commits = String::from_utf8_lossy(&log_output.stdout);
    let found = commits.lines().any(|line| line.trim() == expected_subject);

    debug!(
        change_id = %change_id,
        expected_subject = %expected_subject,
        found = found,
        "has_apply_commit: checking apply commit"
    );

    Ok(found)
}

/// Detect the current state of a workspace.
///
/// This function analyzes the workspace's git history and filesystem to determine
/// the current state for resume operations.
///
/// # State Detection Logic
///
/// 1. Check if merged to base branch → `Merged`
/// 2. Check if archive commit complete → `Archived`
/// 3. Check if archive files exist (but commit incomplete) → `Archiving`
///    - This only resumes an archive step that was already started after acceptance handoff.
/// 4. Check if worktree-local apply blocker marker exists → `Blocked`
///    - Apply-generated blockers resume as blocked, not rejecting.
/// 5. Check if worktree-local `REJECTED.md` exists → `Rejecting`
///    - Terminal rejection proposals resume into dedicated rejecting review.
/// 6. Check if apply commit exists → `Applied`
///    - Resume router decides apply vs acceptance from worktree task progress; no direct archive jump for non-terminal resumes.
/// 6. Check for WIP commits → `Applying { iteration }`
/// 7. Otherwise → `Created`
///
/// # Arguments
///
/// * `change_id` - The change ID to check
/// * `repo_root` - The repository root path (workspace path)
/// * `base_branch` - The base branch to check against
///
/// # Returns
///
/// * `Ok(WorkspaceState)` - Detected workspace state
/// * `Err` - Failed to detect state
pub async fn detect_workspace_state(
    change_id: &str,
    repo_root: &Path,
    base_branch: &str,
) -> Result<WorkspaceState> {
    // 1. Check if merged to base branch
    if is_merged_to_base(change_id, repo_root, base_branch).await? {
        debug!(change_id = %change_id, "State: Merged");
        return Ok(WorkspaceState::Merged);
    }

    // 2. Check if archive commit is complete
    if is_archive_commit_complete(change_id, Some(repo_root)).await? {
        debug!(change_id = %change_id, "State: Archived");
        return Ok(WorkspaceState::Archived);
    }

    // 3. Check if archive files exist (but commit incomplete) → Archiving
    if has_archive_files(change_id, repo_root).await? {
        debug!(change_id = %change_id, "State: Archiving (files moved, commit incomplete)");
        return Ok(WorkspaceState::Archiving);
    }

    // 4. Check if apply-blocked marker exists in workspace
    let blocked_marker_path = repo_root
        .join("openspec")
        .join("changes")
        .join(change_id)
        .join("APPLY_BLOCKED")
        .join("marker.md");
    if blocked_marker_path.exists() {
        parse_blocked_marker(repo_root, change_id)?;
        debug!(
            change_id = %change_id,
            blocked_marker_path = %blocked_marker_path.display(),
            "State: Blocked"
        );
        return Ok(WorkspaceState::Blocked);
    }

    // 5. Check if rejection proposal exists in workspace
    let rejected_path = repo_root
        .join("openspec")
        .join("changes")
        .join(change_id)
        .join("REJECTED.md");
    if rejected_path.exists() {
        debug!(
            change_id = %change_id,
            rejected_path = %rejected_path.display(),
            "State: Rejecting"
        );
        return Ok(WorkspaceState::Rejecting);
    }

    // 6. Check if apply commit exists
    if has_apply_commit(change_id, repo_root).await? {
        debug!(change_id = %change_id, "State: Applied");
        return Ok(WorkspaceState::Applied);
    }

    // 6. Check for WIP commits
    if let Some(iteration) = get_latest_wip_snapshot(change_id, repo_root).await? {
        debug!(change_id = %change_id, iteration = iteration, "State: Applying");
        return Ok(WorkspaceState::Applying {
            iteration: iteration + 1,
        });
    }

    // 6. No commits found - workspace just created
    debug!(change_id = %change_id, "State: Created");
    Ok(WorkspaceState::Created)
}

/// Gather the current repository/worktree facts that an acceptance stall record
/// must reconcile against before it may control any routing.
///
/// Every value here is read from git and the filesystem right now. The stall
/// record contributes only the `apply_revision` being checked, so a stale or
/// forged record cannot make its own binding look valid.
pub async fn gather_workspace_facts(
    repo_root: &Path,
    workspace_path: &Path,
    change_id: &str,
    apply_revision: &str,
    base_branch: &str,
) -> Result<WorkspaceFacts> {
    let merged = is_merged_to_base(change_id, workspace_path, base_branch)
        .await
        .unwrap_or(false);
    let archived = is_archive_commit_complete(change_id, Some(workspace_path))
        .await
        .unwrap_or(false);

    let apply_revision_exists = revision_exists(workspace_path, apply_revision).await;
    let head_descends_from_apply_revision = apply_revision_exists
        && crate::vcs::git::commands::is_ancestor(workspace_path, apply_revision, "HEAD")
            .await
            .unwrap_or(false);

    Ok(WorkspaceFacts {
        repository_id: repository_identity(repo_root),
        change_id: change_id.to_string(),
        worktree_id: worktree_identity(workspace_path),
        worktree_path: canonical_path_string(workspace_path),
        branch: crate::vcs::git::commands::get_current_branch(workspace_path)
            .await
            .ok()
            .flatten(),
        apply_revision_exists,
        head_descends_from_apply_revision,
        change_active: !merged && !archived,
    })
}

/// Canonical string form of a worktree path, so equivalent paths compare equal
/// across symlinked and relative spellings.
pub fn canonical_path_string(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

/// Whether `revision` resolves to an existing commit object in `workspace_path`.
async fn revision_exists(workspace_path: &Path, revision: &str) -> bool {
    if revision.trim().is_empty() {
        return false;
    }
    Command::new("git")
        .args(["cat-file", "-e", &format!("{revision}^{{commit}}")])
        .current_dir(workspace_path)
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Persist a validated external acceptance blocker as a revision-bound runtime
/// hold.
///
/// Shared by serial and parallel orchestration so equivalent blockers produce
/// equivalent records. The write lands outside the worktree, so the managed
/// worktree stays exactly as acceptance left it — clean, with the apply commit
/// intact.
#[allow(clippy::too_many_arguments)]
pub async fn persist_acceptance_stall(
    store: &AcceptanceStallStore,
    repo_root: &Path,
    workspace_path: &Path,
    change_id: &str,
    base_branch: &str,
    apply_revision: &str,
    blocker: &crate::acceptance::AcceptanceBlocker,
    retry_count: u32,
) -> Result<AcceptanceStallRecord> {
    let facts = gather_workspace_facts(
        repo_root,
        workspace_path,
        change_id,
        apply_revision,
        base_branch,
    )
    .await?;
    store.save(&AcceptanceStallRecord::new(
        &facts,
        blocker,
        apply_revision,
        retry_count,
    ))
}

/// Load and reconcile the acceptance stall hold for a change.
///
/// Returns the record only when it still binds to the current repository,
/// worktree, active change, and apply revision. A record that fails
/// reconciliation is quarantined with its diagnostic and reported as absent, so
/// routing falls back to repository evidence.
///
/// This never inspects or mutates the worktree contents, so a stalled change's
/// worktree stays exactly as acceptance left it.
pub async fn load_valid_acceptance_stall(
    store: &AcceptanceStallStore,
    repo_root: &Path,
    workspace_path: &Path,
    change_id: &str,
    base_branch: &str,
) -> Result<Option<AcceptanceStallRecord>> {
    let repository_id = repository_identity(repo_root);
    let Some(record) = store.load(&repository_id, change_id)? else {
        return Ok(None);
    };

    let facts = gather_workspace_facts(
        repo_root,
        workspace_path,
        change_id,
        &record.apply_revision,
        base_branch,
    )
    .await?;

    match reconcile_acceptance_stall(&record, &facts) {
        StallReconciliation::Valid => Ok(Some(record)),
        StallReconciliation::Invalid { reason } => {
            debug!(
                change_id = %change_id,
                reason = %reason,
                "Quarantining acceptance stall record that lost its binding"
            );
            store.quarantine(&repository_id, change_id, &reason)?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    fn init_git_repo(repo_root: &Path) {
        StdCommand::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_root)
            .output()
            .unwrap();
    }

    fn commit(repo_root: &Path, message: &str) {
        fs::write(repo_root.join("test.txt"), message).unwrap();
        StdCommand::new("git")
            .args(["add", "-A"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", message])
            .current_dir(repo_root)
            .output()
            .unwrap();
    }

    use crate::acceptance::AcceptanceBlocker;
    use crate::parallel::acceptance_state::{
        migrate_legacy_acceptance_marker, write_legacy_acceptance_marker, AcceptanceRetryContext,
        BlockedMarkerOrigin, MarkerMigration,
    };

    fn head_revision(repo_root: &Path) -> String {
        let output = StdCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn porcelain_status(repo_root: &Path) -> String {
        let output = StdCommand::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    }

    fn external_blocker() -> AcceptanceBlocker {
        AcceptanceBlocker {
            category: "credential".to_string(),
            evidence: vec!["STAGING_API_KEY is unset in the verification environment".to_string()],
            next_action: "provision STAGING_API_KEY then retry acceptance".to_string(),
            resumable: true,
            prerequisite_owner: Some("platform".to_string()),
            evidence_ids: Vec::new(),
        }
    }

    /// Entering a validated stall must leave the managed worktree exactly as
    /// acceptance found it: clean, with the apply commit intact.
    #[tokio::test]
    async fn acceptance_stall_persists_outside_a_clean_worktree() {
        let repo = TempDir::new().unwrap();
        let state_root = TempDir::new().unwrap();
        init_git_repo(repo.path());
        commit(repo.path(), "Apply: test-change");
        let apply_revision = head_revision(repo.path());

        let store = AcceptanceStallStore::new(state_root.path());
        let record = persist_acceptance_stall(
            &store,
            repo.path(),
            repo.path(),
            "test-change",
            "main",
            &apply_revision,
            &external_blocker(),
            2,
        )
        .await
        .unwrap();

        assert_eq!(record.category, "credential");
        assert_eq!(record.apply_revision, apply_revision);
        assert_eq!(record.phase, "acceptance");
        assert_eq!(record.retry_count, 2);
        assert!(record.resumable);
        assert_eq!(record.prerequisite_owner.as_deref(), Some("platform"));
        assert!(!record.created_at.is_empty() && !record.updated_at.is_empty());

        // Nothing entered the repository at all.
        assert_eq!(porcelain_status(repo.path()), "");
        assert!(!repo
            .path()
            .join("openspec/changes/test-change/APPLY_BLOCKED")
            .exists());
        assert!(!repo.path().join(".cflx/acceptance-state.json").exists());
        assert_eq!(head_revision(repo.path()), apply_revision);

        // The hold is reconstructed verbatim after a "restart".
        let restored =
            load_valid_acceptance_stall(&store, repo.path(), repo.path(), "test-change", "main")
                .await
                .unwrap()
                .expect("matching record must survive restart");
        assert_eq!(restored, record);

        let blocker = restored.to_stalled_blocker();
        assert_eq!(blocker.category, "credential");
        assert_eq!(blocker.phase, "acceptance");
        assert_eq!(
            blocker.next_action,
            "provision STAGING_API_KEY then retry acceptance"
        );
        assert!(blocker.resumable);
        assert!(blocker.worktree_preserved);
    }

    /// The workspace state machine must stay entirely repository-derived: a
    /// runtime stall never shadows the apply commit or invents archive
    /// readiness.
    #[tokio::test]
    async fn runtime_stall_does_not_shadow_repository_derived_workspace_state() {
        let repo = TempDir::new().unwrap();
        let state_root = TempDir::new().unwrap();
        init_git_repo(repo.path());
        commit(repo.path(), "Initial commit");
        commit(repo.path(), "Apply: test-change");
        let apply_revision = head_revision(repo.path());

        let before = detect_workspace_state("test-change", repo.path(), "main")
            .await
            .unwrap();

        let store = AcceptanceStallStore::new(state_root.path());
        persist_acceptance_stall(
            &store,
            repo.path(),
            repo.path(),
            "test-change",
            "main",
            &apply_revision,
            &external_blocker(),
            1,
        )
        .await
        .unwrap();

        assert_eq!(
            detect_workspace_state("test-change", repo.path(), "main")
                .await
                .unwrap(),
            before,
            "runtime stall state must not change repository-derived workspace state"
        );
        assert_ne!(
            detect_workspace_state("test-change", repo.path(), "main")
                .await
                .unwrap(),
            WorkspaceState::Archived
        );
        assert_eq!(porcelain_status(repo.path()), "");
    }

    /// Losing the runtime record must fail safe to acceptance, never to an
    /// inferred PASS or an archive jump.
    #[tokio::test]
    async fn missing_runtime_state_leaves_a_complete_apply_routed_to_acceptance() {
        let repo = TempDir::new().unwrap();
        let state_root = TempDir::new().unwrap();
        init_git_repo(repo.path());
        commit(repo.path(), "Apply: test-change");

        let store = AcceptanceStallStore::new(state_root.path());
        assert!(load_valid_acceptance_stall(
            &store,
            repo.path(),
            repo.path(),
            "test-change",
            "main"
        )
        .await
        .unwrap()
        .is_none());

        let state = detect_workspace_state("test-change", repo.path(), "main")
            .await
            .unwrap();
        assert_eq!(
            state,
            WorkspaceState::Applied,
            "a complete unarchived apply must route to acceptance, not archive"
        );
    }

    /// Reconciliation matrix over real git fixtures: every broken binding must
    /// quarantine the record and hand routing back to repository evidence.
    #[tokio::test]
    async fn stale_records_are_quarantined_and_cannot_control_routing() {
        let repo = TempDir::new().unwrap();
        let other_repo = TempDir::new().unwrap();
        init_git_repo(repo.path());
        init_git_repo(other_repo.path());
        commit(repo.path(), "Apply: test-change");
        commit(other_repo.path(), "unrelated");
        let apply_revision = head_revision(repo.path());
        let other_repo_identity = repository_identity(other_repo.path());

        // Each case breaks exactly one binding of an otherwise-valid record. The
        // mutated record is planted at the *correct* storage key so the loader
        // finds it and reconciliation — not key lookup — is what rejects it.
        type BreakBinding = Box<dyn Fn(&mut AcceptanceStallRecord)>;
        let cases: Vec<(&str, BreakBinding)> = vec![
            (
                "schema",
                Box::new(|record: &mut AcceptanceStallRecord| {
                    record.schema = "acceptance-stall-v99".to_string()
                }),
            ),
            (
                "repository",
                Box::new({
                    let other = other_repo_identity.clone();
                    move |record: &mut AcceptanceStallRecord| record.repository_id = other.clone()
                }),
            ),
            (
                "worktree identity (deleted, recreated, or path reused)",
                Box::new(|record: &mut AcceptanceStallRecord| {
                    record.worktree_id = "gitdir: /other/.git/worktrees/ws-other".to_string()
                }),
            ),
            (
                "worktree path",
                Box::new(|record: &mut AcceptanceStallRecord| {
                    record.worktree_path = "/moved/elsewhere".to_string()
                }),
            ),
            (
                "apply revision",
                Box::new(|record: &mut AcceptanceStallRecord| {
                    record.apply_revision = "0".repeat(40)
                }),
            ),
        ];

        let repository_id = repository_identity(repo.path());
        for (label, mutate) in cases {
            let state_root = TempDir::new().unwrap();
            let store = AcceptanceStallStore::new(state_root.path());
            let mut record = persist_acceptance_stall(
                &store,
                repo.path(),
                repo.path(),
                "test-change",
                "main",
                &apply_revision,
                &external_blocker(),
                0,
            )
            .await
            .unwrap();
            mutate(&mut record);

            let path = store.record_path(&repository_id, "test-change");
            std::fs::write(&path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();

            let loaded = load_valid_acceptance_stall(
                &store,
                repo.path(),
                repo.path(),
                "test-change",
                "main",
            )
            .await
            .unwrap();
            assert!(
                loaded.is_none(),
                "a record with a broken {label} binding must not control routing"
            );
            assert!(
                !path.exists(),
                "the invalid {label} record must be quarantined, not left in place"
            );
            assert_eq!(
                detect_workspace_state("test-change", repo.path(), "main")
                    .await
                    .unwrap(),
                WorkspaceState::Applied,
                "routing must fall back to repository evidence for broken {label}"
            );
            assert_eq!(porcelain_status(repo.path()), "");
        }
    }

    /// A record whose change is archived or merged loses authority even though
    /// every path and revision still matches.
    #[tokio::test]
    async fn archived_change_invalidates_its_stall_record() {
        let repo = TempDir::new().unwrap();
        let state_root = TempDir::new().unwrap();
        init_git_repo(repo.path());
        commit(repo.path(), "Apply: test-change");
        let apply_revision = head_revision(repo.path());

        let store = AcceptanceStallStore::new(state_root.path());
        let record = persist_acceptance_stall(
            &store,
            repo.path(),
            repo.path(),
            "test-change",
            "main",
            &apply_revision,
            &external_blocker(),
            0,
        )
        .await
        .unwrap();

        let facts = WorkspaceFacts {
            change_active: false,
            ..gather_workspace_facts(
                repo.path(),
                repo.path(),
                "test-change",
                &apply_revision,
                "main",
            )
            .await
            .unwrap()
        };
        match reconcile_acceptance_stall(&record, &facts) {
            StallReconciliation::Invalid { reason } => {
                assert!(reason.contains("archived or merged"), "got {reason}")
            }
            StallReconciliation::Valid => panic!("an archived change must invalidate its hold"),
        }
    }

    /// Explicit retry consumes the hold transactionally and is idempotent, so a
    /// retry racing a restart cannot double-consume or fail.
    #[tokio::test]
    async fn explicit_retry_consumption_is_idempotent_and_worktree_neutral() {
        let repo = TempDir::new().unwrap();
        let state_root = TempDir::new().unwrap();
        init_git_repo(repo.path());
        commit(repo.path(), "Apply: test-change");
        let apply_revision = head_revision(repo.path());

        let store = AcceptanceStallStore::new(state_root.path());
        let record = persist_acceptance_stall(
            &store,
            repo.path(),
            repo.path(),
            "test-change",
            "main",
            &apply_revision,
            &external_blocker(),
            0,
        )
        .await
        .unwrap();

        assert!(store.consume(&record.repository_id, "test-change").unwrap());
        assert!(
            !store.consume(&record.repository_id, "test-change").unwrap(),
            "consuming an absent hold must be a safe no-op"
        );
        assert!(load_valid_acceptance_stall(
            &store,
            repo.path(),
            repo.path(),
            "test-change",
            "main"
        )
        .await
        .unwrap()
        .is_none());
        assert_eq!(porcelain_status(repo.path()), "");
        assert_eq!(head_revision(repo.path()), apply_revision);
    }

    /// Legacy acceptance-origin markers migrate once, leave no residue, and are
    /// idempotent across restarts.
    #[tokio::test]
    async fn acceptance_marker_migration_converts_and_cleans_up() {
        let repo = TempDir::new().unwrap();
        let state_root = TempDir::new().unwrap();
        init_git_repo(repo.path());
        commit(repo.path(), "Apply: test-change");
        let apply_revision = head_revision(repo.path());

        write_legacy_acceptance_marker(
            repo.path(),
            "test-change",
            "acceptance_gated",
            &["managed verification job 42 is still running".to_string()],
            &AcceptanceRetryContext {
                finding_identities: vec!["external|job 42|verification".to_string()],
                semantic_fingerprint: Some("baseline".to_string()),
                cycle_count: 3,
            },
            "no_semantic_progress",
            &["verification job 42".to_string()],
            true,
            "wait for job 42 then retry acceptance",
        )
        .unwrap();
        assert_ne!(porcelain_status(repo.path()), "", "fixture must be dirty");

        let store = AcceptanceStallStore::new(state_root.path());
        let facts = gather_workspace_facts(
            repo.path(),
            repo.path(),
            "test-change",
            &apply_revision,
            "main",
        )
        .await
        .unwrap();

        assert_eq!(
            migrate_legacy_acceptance_marker(&store, repo.path(), &facts, &apply_revision).unwrap(),
            MarkerMigration::Migrated {
                category: "pending_verification".to_string()
            }
        );

        // The generated marker and its directory are gone, and the worktree is
        // clean again.
        assert!(!repo
            .path()
            .join("openspec/changes/test-change/APPLY_BLOCKED")
            .exists());
        assert_eq!(porcelain_status(repo.path()), "");
        assert_eq!(head_revision(repo.path()), apply_revision);

        let record =
            load_valid_acceptance_stall(&store, repo.path(), repo.path(), "test-change", "main")
                .await
                .unwrap()
                .expect("migration must produce a usable hold");
        assert_eq!(record.retry_count, 3);
        assert_eq!(record.apply_revision, apply_revision);
        assert!(record.resumable);
        assert_eq!(record.next_action, "wait for job 42 then retry acceptance");
        assert!(record
            .evidence
            .iter()
            .any(|entry| entry.contains("job 42 is still running")));

        // Idempotent: a second pass finds nothing left to migrate and leaves the
        // existing hold alone.
        assert_eq!(
            migrate_legacy_acceptance_marker(&store, repo.path(), &facts, &apply_revision).unwrap(),
            MarkerMigration::NotApplicable
        );
        assert_eq!(
            load_valid_acceptance_stall(&store, repo.path(), repo.path(), "test-change", "main")
                .await
                .unwrap(),
            Some(record)
        );
    }

    /// Migration must never delete evidence it cannot prove is acceptance-owned.
    #[tokio::test]
    async fn acceptance_marker_migration_preserves_foreign_and_malformed_markers() {
        let repo = TempDir::new().unwrap();
        init_git_repo(repo.path());
        commit(repo.path(), "Apply: test-change");
        let apply_revision = head_revision(repo.path());
        let marker_path = repo
            .path()
            .join("openspec/changes/test-change/APPLY_BLOCKED/marker.md");

        let facts = gather_workspace_facts(
            repo.path(),
            repo.path(),
            "test-change",
            &apply_revision,
            "main",
        )
        .await
        .unwrap();

        // Each fixture must survive migration untouched.
        type WriteFixture = Box<dyn Fn()>;
        let fixtures: [(&str, WriteFixture); 5] = [
            (
                "apply-origin",
                Box::new({
                    let marker_path = marker_path.clone();
                    move || {
                        std::fs::create_dir_all(marker_path.parent().unwrap()).unwrap();
                        std::fs::write(&marker_path, "origin: apply\nreason: blocked\n").unwrap();
                    }
                }),
            ),
            (
                "unknown-origin",
                Box::new({
                    let marker_path = marker_path.clone();
                    move || {
                        std::fs::create_dir_all(marker_path.parent().unwrap()).unwrap();
                        std::fs::write(&marker_path, "reason: something happened\n").unwrap();
                    }
                }),
            ),
            (
                "malformed",
                Box::new({
                    let marker_path = marker_path.clone();
                    move || {
                        std::fs::create_dir_all(marker_path.parent().unwrap()).unwrap();
                        std::fs::write(&marker_path, "{ malformed").unwrap();
                    }
                }),
            ),
            (
                "non-resumable acceptance",
                Box::new({
                    let repo_path = repo.path().to_path_buf();
                    move || {
                        write_legacy_acceptance_marker(
                            &repo_path,
                            "test-change",
                            "acceptance_gated",
                            &["evidence".to_string()],
                            &AcceptanceRetryContext::default(),
                            "no_semantic_progress",
                            &[],
                            false,
                            "inspect evidence",
                        )
                        .unwrap();
                    }
                }),
            ),
            (
                "acceptance without evidence",
                Box::new({
                    let repo_path = repo.path().to_path_buf();
                    move || {
                        write_legacy_acceptance_marker(
                            &repo_path,
                            "test-change",
                            "acceptance_gated",
                            &[],
                            &AcceptanceRetryContext::default(),
                            "no_semantic_progress",
                            &[],
                            true,
                            "explicit retry",
                        )
                        .unwrap();
                    }
                }),
            ),
        ];

        for (label, write_fixture) in fixtures {
            let state_root = TempDir::new().unwrap();
            let store = AcceptanceStallStore::new(state_root.path());
            let _ = std::fs::remove_dir_all(marker_path.parent().unwrap());
            write_fixture();
            let before = std::fs::read(&marker_path).unwrap();

            let outcome =
                migrate_legacy_acceptance_marker(&store, repo.path(), &facts, &apply_revision)
                    .unwrap();
            assert!(
                matches!(outcome, MarkerMigration::Preserved { .. }),
                "{label} marker must be preserved, got {outcome:?}"
            );
            assert_eq!(
                std::fs::read(&marker_path).unwrap(),
                before,
                "{label} marker must be left byte-identical"
            );
            assert!(
                load_valid_acceptance_stall(
                    &store,
                    repo.path(),
                    repo.path(),
                    "test-change",
                    "main"
                )
                .await
                .unwrap()
                .is_none(),
                "{label} marker must not create runtime stall state"
            );
        }

        // Apply-origin markers keep their existing conservative blocked routing.
        std::fs::write(&marker_path, "origin: apply\nreason: blocked\n").unwrap();
        assert_eq!(
            detect_workspace_state("test-change", repo.path(), "main")
                .await
                .unwrap(),
            WorkspaceState::Blocked
        );
        assert_eq!(
            parse_blocked_marker(repo.path(), "test-change")
                .unwrap()
                .unwrap()
                .origin,
            BlockedMarkerOrigin::Apply
        );
    }

    #[tokio::test]
    async fn test_detect_workspace_state_rejects_malformed_marker_without_consuming_it() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        let path = repo_root.join("openspec/changes/test-change/APPLY_BLOCKED/marker.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ malformed").unwrap();

        assert!(detect_workspace_state("test-change", repo_root, "main")
            .await
            .is_err());
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_detect_workspace_state_created() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Created);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_applying() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "WIP(apply): test-change (iteration 1/5)");

        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Applying { iteration: 2 });
    }

    #[tokio::test]
    async fn test_detect_workspace_state_applied() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "Apply: test-change");

        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Applied);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_archived() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create a branch to simulate workspace
        StdCommand::new("git")
            .args(["checkout", "-b", "workspace-test-change"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Create archive directory (change moved to archive)
        let archive_dir = repo_root.join("openspec/changes/archive/test-change");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Test").unwrap();

        commit(repo_root, "Archive: test-change");

        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Archived);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_merged() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create archive directory (change moved to archive)
        let archive_dir = repo_root.join("openspec/changes/archive/test-change");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Test").unwrap();

        commit(repo_root, "Archive: test-change");

        // We're on main, so the archive commit is in main
        // State should be Merged when change directory is gone
        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Merged);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_merged_with_remaining_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create changes directory before archiving
        let changes_dir = repo_root.join("openspec/changes/test-change");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "# Test Change").unwrap();

        commit(repo_root, "Archive: test-change");

        // Archive commit exists in main, but change directory still exists
        // With the new guardrails, this should NOT be considered Archived
        // because the archive is incomplete (change directory still exists)
        let state = detect_workspace_state("test-change", repo_root, "main")
            .await
            .unwrap();

        // Should be Created state (archive incomplete, no apply commit, no WIP)
        assert_eq!(state, WorkspaceState::Created);
    }

    #[tokio::test]
    async fn test_get_latest_wip_snapshot_multiple() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "WIP(apply): test-change (iteration 1/5)");
        commit(repo_root, "WIP(apply): test-change (iteration 2/5)");
        commit(repo_root, "WIP(apply): test-change (iteration 3/5)");

        let iteration = get_latest_wip_snapshot("test-change", repo_root)
            .await
            .unwrap();
        assert_eq!(iteration, Some(3));
    }

    #[tokio::test]
    async fn test_get_latest_wip_snapshot_none() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        let iteration = get_latest_wip_snapshot("test-change", repo_root)
            .await
            .unwrap();
        assert_eq!(iteration, None);
    }

    #[tokio::test]
    async fn test_has_apply_commit_true() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");
        commit(repo_root, "Apply: test-change");

        let result = has_apply_commit("test-change", repo_root).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_has_apply_commit_false() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        let result = has_apply_commit("test-change", repo_root).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_is_merged_to_base_true() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create archive directory (change moved to archive)
        let archive_dir = repo_root.join("openspec/changes/archive/test-change");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Test").unwrap();

        commit(repo_root, "Archive: test-change");

        let result = is_merged_to_base("test-change", repo_root, "main")
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_is_merged_to_base_false_on_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create a branch
        StdCommand::new("git")
            .args(["checkout", "-b", "feature-branch"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Create archive directory in feature branch
        let archive_dir = repo_root.join("openspec/changes/archive/test-change");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Test").unwrap();

        commit(repo_root, "Archive: test-change");

        // Archive is in feature branch, not in main
        let result = is_merged_to_base("test-change", repo_root, "main")
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_has_archive_files_exact_match() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create archive directory (exact match) with uncommitted changes (dirty)
        let archive_dir = repo_root.join("openspec/changes/archive/test-archiving");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Test").unwrap();

        // Archiving state requires dirty worktree (uncommitted archive files)
        let result = has_archive_files("test-archiving", repo_root)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_has_archive_files_date_prefixed() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create archive directory (date-prefixed) with uncommitted changes (dirty)
        let archive_dir = repo_root.join("openspec/changes/archive/2024-01-15-test-archiving");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Test").unwrap();

        // Archiving state requires dirty worktree (uncommitted archive files)
        let result = has_archive_files("test-archiving", repo_root)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_has_archive_files_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        let result = has_archive_files("nonexistent", repo_root).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_archiving() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create a branch to simulate workspace
        StdCommand::new("git")
            .args(["checkout", "-b", "workspace-test-archiving"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Add apply commit
        commit(repo_root, "Apply: test-archiving");

        // Create archive directory (files moved but no Archive commit yet)
        let archive_dir = repo_root.join("openspec/changes/archive/test-archiving");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Test").unwrap();

        let state = detect_workspace_state("test-archiving", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Archiving);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_archiving_date_prefixed() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create a branch to simulate workspace
        StdCommand::new("git")
            .args(["checkout", "-b", "workspace-test-date-arch"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Add apply commit
        commit(repo_root, "Apply: test-date-arch");

        // Create date-prefixed archive directory (files moved but no Archive commit yet)
        let archive_dir = repo_root.join("openspec/changes/archive/2024-01-15-test-date-arch");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# Test").unwrap();

        let state = detect_workspace_state("test-date-arch", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Archiving);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_archived_file_state_only() {
        // Test that archive detection uses file state only, not commit messages
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create a branch to simulate workspace
        StdCommand::new("git")
            .args(["checkout", "-b", "workspace-file-state-test"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Create archive directory (change moved to archive)
        let archive_dir = repo_root.join("openspec/changes/archive/file-state-test");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(archive_dir.join("proposal.md"), "# File State Test").unwrap();

        // Commit with ANY message (not necessarily "Archive: ...")
        // This tests that we don't rely on commit message matching
        commit(repo_root, "Some other commit message");

        // State should be Archived because:
        // 1. Working tree is clean (committed)
        // 2. Change directory does not exist in openspec/changes/
        // 3. Archive entry exists in openspec/changes/archive/
        let state = detect_workspace_state("file-state-test", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Archived);
    }

    #[tokio::test]
    async fn test_detect_workspace_state_not_archived_without_archive_entry() {
        // Test that archived state requires archive entry existence
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        init_git_repo(repo_root);
        commit(repo_root, "Initial commit");

        // Create a branch
        StdCommand::new("git")
            .args(["checkout", "-b", "workspace-no-archive-entry"])
            .current_dir(repo_root)
            .output()
            .unwrap();

        // Commit with "Archive: ..." message but NO archive directory
        // This tests that commit message alone is not sufficient
        commit(repo_root, "Archive: no-archive-entry");

        // State should NOT be Archived because archive entry does not exist
        // Should fall back to Created (no apply commit, no WIP)
        let state = detect_workspace_state("no-archive-entry", repo_root, "main")
            .await
            .unwrap();
        assert_eq!(state, WorkspaceState::Created);
    }
}
