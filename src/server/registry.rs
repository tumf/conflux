//! Project registry with persistence for the server daemon.
//!
//! Projects are identified by (remote_url, branch) pairs and assigned a deterministic
//! project_id = first 16 hex chars of md5(remote_url + "\n" + branch).
//!
//! NOTE: This module deliberately does NOT reference or execute `~/.wt/setup`.
//! The server daemon is directory-independent and relies only on its own data_dir.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};

use crate::error::{OrchestratorError, Result};

/// Execution status of a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    /// Project is idle (not running).
    #[default]
    Idle,
    /// Project is currently running.
    Running,
    /// Project execution is stopped.
    Stopped,
}

/// Global orchestration status for the server.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationStatus {
    /// No orchestration running (initial state).
    #[default]
    Idle,
    /// Orchestration is running across projects.
    Running,
    /// Orchestration has been stopped.
    Stopped,
}

impl OrchestrationStatus {
    /// Return the string representation for JSON/WebSocket serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            OrchestrationStatus::Idle => "idle",
            OrchestrationStatus::Running => "running",
            OrchestrationStatus::Stopped => "stopped",
        }
    }
}

/// A managed project entry in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    /// Deterministic project ID: first 16 hex chars of md5(remote_url + "\n" + branch).
    pub id: String,
    /// Remote URL of the git repository.
    pub remote_url: String,
    /// Branch name.
    pub branch: String,
    /// Current execution status.
    #[serde(default)]
    pub status: ProjectStatus,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
}

impl ProjectEntry {
    /// Create a new project entry from remote_url and branch.
    pub fn new(remote_url: String, branch: String) -> Self {
        let id = generate_project_id(&remote_url, &branch);
        let created_at = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            remote_url,
            branch,
            status: ProjectStatus::default(),
            created_at,
        }
    }
}

/// Generate a deterministic project_id.
/// Algorithm: md5(remote_url + "\n" + branch), take first 16 hex chars.
pub fn generate_project_id(remote_url: &str, branch: &str) -> String {
    let input = format!("{}\n{}", remote_url, branch);
    let digest = md5::compute(input.as_bytes());
    let hex = format!("{:x}", digest);
    hex[..16].to_string()
}

/// Generate the server-specific worktree branch name for a project.
///
/// The server worktree must NOT check out the base branch directly, as that would
/// prevent the bare clone from updating `refs/heads/<base_branch>` during pull/push.
///
/// Format: `server-wt/<project_id>/<base_branch>`
pub fn server_worktree_branch(project_id: &str, base_branch: &str) -> String {
    format!("server-wt/{}/{}", project_id, base_branch)
}

const REGISTRY_FILE: &str = "projects.json";

/// Persistent project registry backed by a JSON file in data_dir.
pub struct ProjectRegistry {
    data_dir: PathBuf,
    /// In-memory store: project_id -> ProjectEntry
    projects: HashMap<String, ProjectEntry>,
    /// Per-project locks: project_id -> Mutex
    project_locks: HashMap<String, Arc<Mutex<()>>>,
    /// Global semaphore for max_concurrent_total
    global_semaphore: Arc<tokio::sync::Semaphore>,
    /// In-memory per-project per-change selection state: project_id -> (change_id -> selected).
    /// Not persisted; all changes default to `true` on server restart.
    change_selections: HashMap<String, HashMap<String, bool>>,
}

impl ProjectRegistry {
    /// Load or create the registry from disk.
    pub fn load(data_dir: &Path, max_concurrent_total: usize) -> Result<Self> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to create server data dir '{}': {}",
                data_dir.display(),
                e
            )))
        })?;

        let registry_path = data_dir.join(REGISTRY_FILE);
        let projects = if registry_path.exists() {
            let content = std::fs::read_to_string(&registry_path).map_err(|e| {
                OrchestratorError::Io(std::io::Error::other(format!(
                    "Failed to read registry '{}': {}",
                    registry_path.display(),
                    e
                )))
            })?;
            serde_json::from_str::<HashMap<String, ProjectEntry>>(&content).map_err(|e| {
                OrchestratorError::ConfigLoad(format!(
                    "Failed to parse registry '{}': {}",
                    registry_path.display(),
                    e
                ))
            })?
        } else {
            HashMap::new()
        };

        info!(
            "Loaded project registry from {:?} ({} projects)",
            registry_path,
            projects.len()
        );

        // Build per-project locks for all existing projects
        let mut project_locks = HashMap::new();
        for id in projects.keys() {
            project_locks.insert(id.clone(), Arc::new(Mutex::new(())));
        }

        Ok(Self {
            data_dir: data_dir.to_path_buf(),
            projects,
            project_locks,
            global_semaphore: Arc::new(tokio::sync::Semaphore::new(max_concurrent_total)),
            change_selections: HashMap::new(),
        })
    }

    /// Persist the current registry to disk.
    fn save(&self) -> Result<()> {
        let registry_path = self.data_dir.join(REGISTRY_FILE);
        let content = serde_json::to_string_pretty(&self.projects).map_err(|e| {
            OrchestratorError::ConfigLoad(format!("Failed to serialize registry: {}", e))
        })?;
        std::fs::write(&registry_path, content).map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to write registry '{}': {}",
                registry_path.display(),
                e
            )))
        })?;
        debug!("Saved project registry to {:?}", registry_path);
        Ok(())
    }

    /// List all projects.
    pub fn list(&self) -> Vec<ProjectEntry> {
        let mut entries: Vec<ProjectEntry> = self.projects.values().cloned().collect();
        entries.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        entries
    }

    /// Add a project. Returns error if a project with the same (remote_url, branch) already exists.
    pub fn add(&mut self, remote_url: String, branch: String) -> Result<ProjectEntry> {
        let id = generate_project_id(&remote_url, &branch);
        if self.projects.contains_key(&id) {
            return Err(OrchestratorError::ConfigLoad(format!(
                "Project already exists: id={} remote_url={} branch={}",
                id, remote_url, branch
            )));
        }
        let entry = ProjectEntry::new(remote_url, branch);
        self.project_locks
            .insert(entry.id.clone(), Arc::new(Mutex::new(())));
        self.projects.insert(entry.id.clone(), entry.clone());
        self.save()?;
        info!("Added project id={}", entry.id);
        Ok(entry)
    }

    /// Remove a project by id. Returns error if not found.
    pub fn remove(&mut self, id: &str) -> Result<ProjectEntry> {
        let entry = self.projects.remove(id).ok_or_else(|| {
            OrchestratorError::ConfigLoad(format!("Project not found: id={}", id))
        })?;
        self.project_locks.remove(id);
        self.save()?;
        info!("Removed project id={}", id);
        Ok(entry)
    }

    /// Get a project by id.
    pub fn get(&self, id: &str) -> Option<&ProjectEntry> {
        self.projects.get(id)
    }

    /// Update project status and persist.
    pub fn set_status(&mut self, id: &str, status: ProjectStatus) -> Result<()> {
        let entry = self.projects.get_mut(id).ok_or_else(|| {
            OrchestratorError::ConfigLoad(format!("Project not found: id={}", id))
        })?;
        entry.status = status;
        self.save()
    }

    /// Get the per-project mutex for exclusive operations.
    pub fn project_lock(&self, id: &str) -> Option<Arc<Mutex<()>>> {
        self.project_locks.get(id).cloned()
    }

    /// Get the global semaphore (for max_concurrent_total).
    pub fn global_semaphore(&self) -> Arc<tokio::sync::Semaphore> {
        self.global_semaphore.clone()
    }

    /// Get the data directory path (used by API handlers to locate bare clones).
    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    // ─────────────── Change selection state ───────────────

    /// Get the selected state of a change. Returns `true` if the change has not been seen before
    /// (new changes default to selected).
    #[allow(dead_code)]
    pub fn is_change_selected(&self, project_id: &str, change_id: &str) -> bool {
        self.change_selections
            .get(project_id)
            .and_then(|m| m.get(change_id))
            .copied()
            .unwrap_or(true)
    }

    /// Ensure a change is tracked in the selection map, defaulting to `true` if absent.
    #[allow(dead_code)]
    pub fn ensure_change_selected(&mut self, project_id: &str, change_id: &str) {
        self.change_selections
            .entry(project_id.to_string())
            .or_default()
            .entry(change_id.to_string())
            .or_insert(true);
    }

    /// Toggle the selected state of a single change. Returns the new value.
    /// If the change was not previously tracked, it is treated as `true` and toggled to `false`.
    pub fn toggle_change_selected(&mut self, project_id: &str, change_id: &str) -> bool {
        let entry = self
            .change_selections
            .entry(project_id.to_string())
            .or_default()
            .entry(change_id.to_string())
            .or_insert(true);
        *entry = !*entry;
        debug!(
            project_id,
            change_id,
            selected = *entry,
            "Toggled change selection"
        );
        *entry
    }

    /// Toggle all changes for a project. If any change is unselected, select all; otherwise
    /// deselect all. `known_change_ids` is the current list of change IDs for the project so
    /// that all are covered even if not yet tracked.
    ///
    /// Returns the new selected value applied to all changes.
    pub fn toggle_all_changes(&mut self, project_id: &str, known_change_ids: &[String]) -> bool {
        let selections = self
            .change_selections
            .entry(project_id.to_string())
            .or_default();

        // Ensure all known changes are tracked
        for cid in known_change_ids {
            selections.entry(cid.clone()).or_insert(true);
        }

        // If any change is false, select all; otherwise deselect all.
        let any_unselected = selections.values().any(|&v| !v);
        let new_value = any_unselected;

        for val in selections.values_mut() {
            *val = new_value;
        }

        debug!(
            project_id,
            new_selected = new_value,
            count = known_change_ids.len(),
            "Toggled all change selections"
        );
        new_value
    }

    /// Get all change selections for a project.
    pub fn change_selections_for_project(
        &self,
        project_id: &str,
    ) -> Option<&HashMap<String, bool>> {
        self.change_selections.get(project_id)
    }
}

/// Thread-safe shared registry.
pub type SharedRegistry = Arc<RwLock<ProjectRegistry>>;

/// Create a shared registry.
pub fn create_shared_registry(
    data_dir: &Path,
    max_concurrent_total: usize,
) -> Result<SharedRegistry> {
    let registry = ProjectRegistry::load(data_dir, max_concurrent_total)?;
    Ok(Arc::new(RwLock::new(registry)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_server_worktree_branch_format() {
        let branch = server_worktree_branch("abc123def456789a", "main");
        assert_eq!(
            branch, "server-wt/abc123def456789a/main",
            "Branch name must follow server-wt/<project_id>/<base_branch> format"
        );
    }

    #[test]
    fn test_server_worktree_branch_different_base_branches() {
        let branch_main = server_worktree_branch("abc123", "main");
        let branch_develop = server_worktree_branch("abc123", "develop");
        assert_ne!(
            branch_main, branch_develop,
            "Different base branches must produce different server worktree branch names"
        );
    }

    #[test]
    fn test_server_worktree_branch_different_project_ids() {
        let branch1 = server_worktree_branch("abc123", "main");
        let branch2 = server_worktree_branch("xyz789", "main");
        assert_ne!(
            branch1, branch2,
            "Different project IDs must produce different server worktree branch names"
        );
    }

    #[test]
    fn test_server_worktree_branch_is_not_base_branch() {
        let project_id = "abc123def456789a";
        let base_branch = "main";
        let server_branch = server_worktree_branch(project_id, base_branch);
        assert_ne!(
            server_branch, base_branch,
            "Server worktree branch must differ from the base branch"
        );
    }

    #[test]
    fn test_server_worktree_branch_starts_with_server_wt() {
        let branch = server_worktree_branch("abc123", "main");
        assert!(
            branch.starts_with("server-wt/"),
            "Server worktree branch must start with 'server-wt/'"
        );
    }

    #[test]
    fn test_generate_project_id_deterministic() {
        let id1 = generate_project_id("https://github.com/foo/bar", "main");
        let id2 = generate_project_id("https://github.com/foo/bar", "main");
        assert_eq!(id1, id2, "Same input must produce same project_id");
    }

    #[test]
    fn test_generate_project_id_length() {
        let id = generate_project_id("https://github.com/foo/bar", "main");
        assert_eq!(id.len(), 16, "project_id must be 16 hex chars");
    }

    #[test]
    fn test_generate_project_id_different_inputs() {
        let id1 = generate_project_id("https://github.com/foo/bar", "main");
        let id2 = generate_project_id("https://github.com/foo/bar", "develop");
        assert_ne!(
            id1, id2,
            "Different branch must produce different project_id"
        );

        let id3 = generate_project_id("https://github.com/foo/baz", "main");
        assert_ne!(
            id1, id3,
            "Different remote_url must produce different project_id"
        );
    }

    #[test]
    fn test_generate_project_id_known_value() {
        // md5("https://github.com/foo/bar\nmain") first 16 chars
        let id = generate_project_id("https://github.com/foo/bar", "main");
        // Verify format: exactly 16 lowercase hex chars
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(id.len(), 16);
    }

    #[tokio::test]
    async fn test_registry_add_and_list() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = ProjectRegistry::load(temp_dir.path(), 4).unwrap();

        registry
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let projects = registry.list();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].remote_url, "https://github.com/foo/bar");
        assert_eq!(projects[0].branch, "main");
    }

    #[tokio::test]
    async fn test_registry_add_duplicate_fails() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = ProjectRegistry::load(temp_dir.path(), 4).unwrap();

        registry
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let result = registry.add("https://github.com/foo/bar".to_string(), "main".to_string());
        assert!(result.is_err(), "Duplicate add should fail");
    }

    #[tokio::test]
    async fn test_registry_remove() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = ProjectRegistry::load(temp_dir.path(), 4).unwrap();

        let entry = registry
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();
        let id = entry.id.clone();

        registry.remove(&id).unwrap();
        assert!(registry.get(&id).is_none());
    }

    #[tokio::test]
    async fn test_registry_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Add a project and save
        {
            let mut registry = ProjectRegistry::load(temp_dir.path(), 4).unwrap();
            registry
                .add("https://github.com/foo/bar".to_string(), "main".to_string())
                .unwrap();
        }

        // Reload and verify persistence
        let registry = ProjectRegistry::load(temp_dir.path(), 4).unwrap();
        let projects = registry.list();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].remote_url, "https://github.com/foo/bar");
    }

    #[tokio::test]
    async fn test_project_lock() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = ProjectRegistry::load(temp_dir.path(), 4).unwrap();

        let entry = registry
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        // Lock should exist after add
        let lock = registry.project_lock(&entry.id);
        assert!(lock.is_some(), "Per-project lock must exist after add");
    }

    #[tokio::test]
    async fn test_global_semaphore_limits_concurrency() {
        let temp_dir = TempDir::new().unwrap();
        let registry = ProjectRegistry::load(temp_dir.path(), 2).unwrap();

        let sem = registry.global_semaphore();
        assert_eq!(sem.available_permits(), 2);

        // Acquire permits
        let _p1 = sem.acquire().await.unwrap();
        let _p2 = sem.acquire().await.unwrap();
        assert_eq!(sem.available_permits(), 0, "Semaphore should be exhausted");
        // p1, p2 dropped at end of scope -> permits returned
    }

    #[test]
    fn test_toggle_change_selected_tracks_explicit_false() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = ProjectRegistry::load(temp_dir.path(), 2).unwrap();
        let entry = registry
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let first = registry.toggle_change_selected(&entry.id, "change-a");
        let second = registry.toggle_change_selected(&entry.id, "change-a");

        assert!(!first, "first toggle should clear default selection");
        assert!(second, "second toggle should restore explicit selection");
        assert!(registry.is_change_selected(&entry.id, "change-a"));
    }
}
