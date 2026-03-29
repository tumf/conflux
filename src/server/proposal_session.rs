//! Proposal session manager for the dashboard.
//!
//! Manages interactive proposal creation sessions backed by ACP (Agent Client
//! Protocol) subprocesses. Each session creates an independent worktree and
//! ACP subprocess for conversational proposal generation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::ProposalSessionConfig;
use crate::openspec::ProposalMetadata;
use crate::server::acp_client::{AcpClient, AcpError};
use crate::vcs::git::commands as git;

// ── Types ─────────────────────────────────────────────────────────────────

/// Status of a proposal session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalSessionStatus {
    /// Session is active with a running ACP process.
    Active,
    /// Session is in the process of merging.
    Merging,
    /// ACP process has been stopped (e.g., by inactivity timeout).
    TimedOut,
    /// Session has been closed.
    Closed,
}

/// Information about a single proposal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSessionInfo {
    pub id: String,
    pub project_id: String,
    pub worktree_path: String,
    pub worktree_branch: String,
    pub status: ProposalSessionStatus,
    pub is_dirty: bool,
    pub uncommitted_files: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_activity: String,
}

/// A detected OpenSpec change in a proposal worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedChange {
    pub id: String,
    pub title: Option<String>,
    pub metadata: ProposalMetadata,
}

/// Internal state of a proposal session.
pub struct ProposalSession {
    pub id: String,
    pub project_id: String,
    pub worktree_path: PathBuf,
    pub worktree_branch: String,
    pub acp_client: Arc<AcpClient>,
    pub acp_session_id: String,
    pub status: ProposalSessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

impl ProposalSession {
    /// Convert to API-facing info struct.
    pub fn to_info(&self) -> ProposalSessionInfo {
        ProposalSessionInfo {
            id: self.id.clone(),
            project_id: self.project_id.clone(),
            worktree_path: self.worktree_path.display().to_string(),
            worktree_branch: self.worktree_branch.clone(),
            status: self.status.clone(),
            is_dirty: false,
            uncommitted_files: Vec::new(),
            created_at: self.created_at.to_rfc3339(),
            updated_at: self.last_activity.to_rfc3339(),
            last_activity: self.last_activity.to_rfc3339(),
        }
    }

    /// Update the last_activity timestamp to now.
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }
}

// ── ProposalSessionManager ────────────────────────────────────────────────

/// Shared proposal session manager handle.
pub type SharedProposalSessionManager = Arc<RwLock<ProposalSessionManager>>;

/// Create a new shared proposal session manager.
pub fn create_proposal_session_manager(
    config: ProposalSessionConfig,
) -> SharedProposalSessionManager {
    Arc::new(RwLock::new(ProposalSessionManager::new(config)))
}

/// Manages proposal sessions across projects.
pub struct ProposalSessionManager {
    config: ProposalSessionConfig,
    /// Active sessions keyed by session ID.
    sessions: HashMap<String, ProposalSession>,
}

impl ProposalSessionManager {
    pub fn new(config: ProposalSessionConfig) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
        }
    }

    /// Create a new proposal session for a project.
    ///
    /// This will:
    /// 1. Create a new worktree on branch `proposal/<session_id>`
    /// 2. Spawn an ACP subprocess in the worktree directory
    /// 3. Perform the ACP initialize + session/create handshake
    pub async fn create_session(
        &mut self,
        project_id: &str,
        repo_root: &Path,
    ) -> Result<ProposalSessionInfo, ProposalSessionError> {
        let session_id = generate_session_id();
        let branch_name = format!("proposal/{}", session_id);

        info!(
            session_id = %session_id,
            project_id = %project_id,
            branch = %branch_name,
            "Creating proposal session"
        );

        // Get HEAD commit for worktree creation
        let head_commit = git::get_current_commit(repo_root)
            .await
            .map_err(|e| ProposalSessionError::Git(format!("Failed to get HEAD: {}", e)))?;

        // Determine worktree path
        let worktree_path = repo_root
            .parent()
            .unwrap_or(repo_root)
            .join(format!("proposal-{}", &session_id));

        // Create worktree
        let worktree_path_str = worktree_path
            .to_str()
            .ok_or_else(|| ProposalSessionError::Git("Invalid worktree path".into()))?
            .to_string();
        git::worktree_add(repo_root, &worktree_path_str, &branch_name, &head_commit)
            .await
            .map_err(|e| ProposalSessionError::Git(format!("Failed to create worktree: {}", e)))?;

        info!(
            worktree = %worktree_path.display(),
            branch = %branch_name,
            "Worktree created for proposal session"
        );

        // Spawn ACP subprocess
        let acp_client = AcpClient::spawn(&self.config, &worktree_path)
            .await
            .map_err(ProposalSessionError::Acp)?;

        // Initialize ACP
        acp_client
            .initialize()
            .await
            .map_err(ProposalSessionError::Acp)?;

        // Create ACP session
        let acp_session_id = acp_client
            .create_session()
            .await
            .map_err(ProposalSessionError::Acp)?;

        let now = Utc::now();
        let session = ProposalSession {
            id: session_id.clone(),
            project_id: project_id.to_string(),
            worktree_path: worktree_path.clone(),
            worktree_branch: branch_name.clone(),
            acp_client,
            acp_session_id,
            status: ProposalSessionStatus::Active,
            created_at: now,
            last_activity: now,
        };

        let info = session.to_info();
        self.sessions.insert(session_id, session);

        Ok(info)
    }

    /// List all active sessions for a project.
    pub fn list_sessions(&self, project_id: &str) -> Vec<ProposalSessionInfo> {
        self.sessions
            .values()
            .filter(|s| s.project_id == project_id)
            .map(|s| s.to_info())
            .collect()
    }

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Option<&ProposalSession> {
        self.sessions.get(session_id)
    }

    /// Get a mutable session by ID.
    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut ProposalSession> {
        self.sessions.get_mut(session_id)
    }

    /// Check if a session's worktree has uncommitted changes.
    pub async fn check_dirty(
        &self,
        session_id: &str,
    ) -> Result<(bool, Vec<String>), ProposalSessionError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        let (has_changes, status_output) = git::has_uncommitted_changes(&session.worktree_path)
            .await
            .map_err(|e| {
                ProposalSessionError::Git(format!("Failed to check dirty state: {}", e))
            })?;

        let files: Vec<String> = if has_changes {
            status_output
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        Ok((has_changes, files))
    }

    /// Close (delete) a proposal session.
    ///
    /// If `force` is false and the worktree is dirty, returns an error with the
    /// list of uncommitted files.
    pub async fn close_session(
        &mut self,
        session_id: &str,
        force: bool,
        repo_root: &Path,
    ) -> Result<(), ProposalSessionError> {
        // Check dirty state if not forcing
        if !force {
            let (is_dirty, files) = self.check_dirty(session_id).await?;
            if is_dirty {
                return Err(ProposalSessionError::DirtyWorktree { files });
            }
        }

        let session = self
            .sessions
            .remove(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        info!(
            session_id = %session_id,
            force = %force,
            "Closing proposal session"
        );

        // Kill ACP process
        session.acp_client.kill().await;

        // Remove worktree
        let wt_path_str = session.worktree_path.to_string_lossy().to_string();
        if let Err(e) = git::worktree_remove(repo_root, &wt_path_str).await {
            warn!(
                error = %e,
                worktree = %session.worktree_path.display(),
                "Failed to remove worktree (may already be removed)"
            );
        }

        // Delete the branch
        if let Err(e) = git::branch_delete(repo_root, &session.worktree_branch).await {
            debug!(
                error = %e,
                branch = %session.worktree_branch,
                "Failed to delete proposal branch"
            );
        }

        Ok(())
    }

    /// Merge a proposal session's worktree into the project base branch.
    pub async fn merge_session(
        &mut self,
        session_id: &str,
        repo_root: &Path,
        base_branch: &str,
    ) -> Result<(), ProposalSessionError> {
        // Check dirty state first
        let (is_dirty, files) = self.check_dirty(session_id).await?;
        if is_dirty {
            return Err(ProposalSessionError::DirtyWorktree { files });
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        session.status = ProposalSessionStatus::Merging;
        let worktree_branch = session.worktree_branch.clone();

        info!(
            session_id = %session_id,
            branch = %worktree_branch,
            base = %base_branch,
            "Merging proposal session"
        );

        // Merge the proposal branch into the base branch
        git::merge_branch(repo_root, &worktree_branch)
            .await
            .map_err(|e| ProposalSessionError::MergeConflict(format!("{}", e)))?;

        // Now close the session (force=true since we just merged)
        // Remove from sessions map
        let session = self
            .sessions
            .remove(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        // Kill ACP process
        session.acp_client.kill().await;

        // Remove worktree
        let wt_path_str = session.worktree_path.to_string_lossy().to_string();
        if let Err(e) = git::worktree_remove(repo_root, &wt_path_str).await {
            warn!(
                error = %e,
                worktree = %session.worktree_path.display(),
                "Failed to remove worktree after merge"
            );
        }

        // Delete the branch
        if let Err(e) = git::branch_delete(repo_root, &worktree_branch).await {
            debug!(
                error = %e,
                branch = %worktree_branch,
                "Failed to delete proposal branch after merge"
            );
        }

        Ok(())
    }

    /// Detect OpenSpec changes in a session's worktree.
    pub async fn detect_changes(
        &self,
        session_id: &str,
    ) -> Result<Vec<DetectedChange>, ProposalSessionError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        let changes_dir = session.worktree_path.join("openspec").join("changes");
        let mut detected = Vec::new();

        if !changes_dir.exists() {
            return Ok(detected);
        }

        let entries = std::fs::read_dir(&changes_dir).map_err(|e| {
            ProposalSessionError::Git(format!("Failed to read changes directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ProposalSessionError::Git(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Skip archive directory
            if path.file_name().and_then(|n| n.to_str()) == Some("archive") {
                continue;
            }

            let proposal_path = path.join("proposal.md");
            if !proposal_path.exists() {
                continue;
            }

            let change_id = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Try to extract title and metadata from proposal.md
            let title = extract_proposal_title(&proposal_path);
            let metadata = crate::openspec::parse_proposal_metadata_from_file(&proposal_path);

            detected.push(DetectedChange {
                id: change_id,
                title,
                metadata,
            });
        }

        Ok(detected)
    }

    /// Scan for sessions that have exceeded the inactivity timeout and stop their ACP processes.
    pub async fn scan_timeouts(&mut self) {
        let timeout_secs = self.config.session_inactivity_timeout_secs;
        if timeout_secs == 0 {
            return;
        }

        let now = Utc::now();
        let mut timed_out = Vec::new();

        for (id, session) in &self.sessions {
            if session.status != ProposalSessionStatus::Active {
                continue;
            }
            let elapsed = (now - session.last_activity).num_seconds();
            if elapsed > timeout_secs as i64 {
                timed_out.push(id.clone());
            }
        }

        for id in timed_out {
            if let Some(session) = self.sessions.get_mut(&id) {
                info!(
                    session_id = %id,
                    "Proposal session timed out, stopping ACP process"
                );
                session.acp_client.kill().await;
                session.status = ProposalSessionStatus::TimedOut;
            }
        }
    }

    /// Kill all ACP processes and remove clean worktrees (shutdown cleanup).
    pub async fn cleanup_all(&mut self, repo_root: Option<&Path>) {
        let session_ids: Vec<String> = self.sessions.keys().cloned().collect();

        for id in session_ids {
            if let Some(session) = self.sessions.remove(&id) {
                info!(session_id = %id, "Cleaning up proposal session");
                session.acp_client.kill().await;

                if let Some(root) = repo_root {
                    // Only remove clean worktrees
                    let is_dirty = git::has_uncommitted_changes(&session.worktree_path)
                        .await
                        .map(|(has_changes, _)| has_changes)
                        .unwrap_or(true);

                    if !is_dirty {
                        let wt_path_str = session.worktree_path.to_string_lossy().to_string();
                        if let Err(e) = git::worktree_remove(root, &wt_path_str).await {
                            warn!(
                                error = %e,
                                worktree = %session.worktree_path.display(),
                                "Failed to remove worktree during cleanup"
                            );
                        }
                    } else {
                        info!(
                            worktree = %session.worktree_path.display(),
                            "Preserving dirty worktree during cleanup"
                        );
                    }
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Generate a unique session ID.
fn generate_session_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let id: u64 = rng.gen();
    format!("ps-{:016x}", id)
}

/// Extract the title from a proposal.md file (first `# ` heading).
fn extract_proposal_title(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            // Strip common prefixes like "Change: "
            let title = title.strip_prefix("Change: ").unwrap_or(title);
            return Some(title.trim().to_string());
        }
    }
    None
}

// ── Error types ───────────────────────────────────────────────────────────

/// Errors from proposal session operations.
#[derive(Debug, thiserror::Error)]
pub enum ProposalSessionError {
    #[error("Proposal session not found: {0}")]
    NotFound(String),

    #[error("Git operation failed: {0}")]
    Git(String),

    #[error("ACP error: {0}")]
    Acp(#[from] AcpError),

    #[error("Worktree has uncommitted changes")]
    DirtyWorktree { files: Vec<String> },

    #[error("Merge conflict: {0}")]
    MergeConflict(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_id() {
        let id1 = generate_session_id();
        let id2 = generate_session_id();
        assert!(id1.starts_with("ps-"));
        assert_ne!(id1, id2);
        // ps- prefix + 16 hex chars
        assert_eq!(id1.len(), 3 + 16);
    }

    #[test]
    fn test_extract_proposal_title() {
        let dir = tempfile::TempDir::new().unwrap();
        let proposal = dir.path().join("proposal.md");

        std::fs::write(&proposal, "# Change: My Feature\n\n## Problem\nSomething\n").unwrap();

        let title = extract_proposal_title(&proposal);
        assert_eq!(title, Some("My Feature".to_string()));
    }

    #[test]
    fn test_extract_proposal_title_no_change_prefix() {
        let dir = tempfile::TempDir::new().unwrap();
        let proposal = dir.path().join("proposal.md");

        std::fs::write(&proposal, "# Add authentication\n\n## Why\nBecause\n").unwrap();

        let title = extract_proposal_title(&proposal);
        assert_eq!(title, Some("Add authentication".to_string()));
    }

    #[test]
    fn test_detected_change_metadata_serializes() {
        let change = DetectedChange {
            id: "add-auth".to_string(),
            title: Some("Add authentication".to_string()),
            metadata: ProposalMetadata {
                change_type: Some("implementation".to_string()),
                priority: Some(crate::openspec::ProposalPriority::High),
                dependencies: vec!["base-change".to_string()],
                references: vec!["src/demo.py".to_string()],
                warnings: vec![],
            },
        };

        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["metadata"]["priority"], "high");
        assert_eq!(json["metadata"]["dependencies"][0], "base-change");
        assert_eq!(json["metadata"]["references"][0], "src/demo.py");
    }

    #[test]
    fn test_extract_proposal_title_missing_file() {
        let title = extract_proposal_title(Path::new("/nonexistent/proposal.md"));
        assert!(title.is_none());
    }

    #[test]
    fn test_proposal_session_info_serialization() {
        let info = ProposalSessionInfo {
            id: "ps-abc123".to_string(),
            project_id: "proj1".to_string(),
            worktree_path: "/tmp/proposal-abc123".to_string(),
            worktree_branch: "proposal/ps-abc123".to_string(),
            status: ProposalSessionStatus::Active,
            is_dirty: false,
            uncommitted_files: Vec::new(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            last_activity: "2025-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["status"], "active");
        assert_eq!(json["id"], "ps-abc123");
    }

    #[test]
    fn test_detected_change_serialization() {
        let change = DetectedChange {
            id: "add-auth".to_string(),
            title: Some("Add authentication".to_string()),
            metadata: ProposalMetadata::default(),
        };
        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["id"], "add-auth");
        assert_eq!(json["title"], "Add authentication");
    }

    #[test]
    fn test_proposal_session_manager_new() {
        let config = ProposalSessionConfig::default();
        let manager = ProposalSessionManager::new(config);
        assert!(manager.sessions.is_empty());
    }

    #[test]
    fn test_proposal_session_manager_list_empty() {
        let config = ProposalSessionConfig::default();
        let manager = ProposalSessionManager::new(config);
        let sessions = manager.list_sessions("proj1");
        assert!(sessions.is_empty());
    }
}
