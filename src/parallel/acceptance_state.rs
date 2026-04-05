use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{OrchestratorError, Result};

const ACCEPTANCE_STATE_DIR_NAME: &str = "acceptance-state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceStateStatus {
    Pending,
    Running,
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceState {
    pub state: AcceptanceStateStatus,
    pub revision: String,
    pub updated_at: String,
    pub workspace_path: String,
    pub change_id: Option<String>,
}

impl AcceptanceState {
    fn new(
        state: AcceptanceStateStatus,
        revision: impl Into<String>,
        workspace_path: &Path,
        change_id: Option<&str>,
    ) -> Self {
        Self {
            state,
            revision: revision.into(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            workspace_path: workspace_path.to_string_lossy().into_owned(),
            change_id: change_id.map(ToOwned::to_owned),
        }
    }
}

fn acceptance_state_root_dir() -> PathBuf {
    if let Ok(xdg_state_home) = std::env::var("XDG_STATE_HOME") {
        if !xdg_state_home.is_empty() {
            return PathBuf::from(xdg_state_home)
                .join("cflx")
                .join(ACCEPTANCE_STATE_DIR_NAME);
        }
    }

    if let Some(home_dir) = dirs::home_dir() {
        return home_dir
            .join(".local")
            .join("state")
            .join("cflx")
            .join(ACCEPTANCE_STATE_DIR_NAME);
    }

    std::env::temp_dir().join("cflx-acceptance-state")
}

fn workspace_state_file_name(workspace_path: &Path) -> String {
    let canonical = workspace_path
        .canonicalize()
        .unwrap_or_else(|_| workspace_path.to_path_buf());
    let digest = md5::compute(canonical.to_string_lossy().as_bytes());
    format!("{:x}.json", digest)
}

pub fn acceptance_state_path(workspace_path: &Path) -> PathBuf {
    acceptance_state_root_dir().join(workspace_state_file_name(workspace_path))
}

pub fn load_acceptance_state(workspace_path: &Path) -> Result<Option<AcceptanceState>> {
    let state_path = acceptance_state_path(workspace_path);
    if !state_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&state_path).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed reading acceptance state from '{}': {}",
            state_path.display(),
            e
        ))
    })?;

    let state = serde_json::from_str::<AcceptanceState>(&content).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed parsing acceptance state from '{}': {}",
            state_path.display(),
            e
        ))
    })?;

    Ok(Some(state))
}

pub fn save_acceptance_state(
    workspace_path: &Path,
    state: AcceptanceStateStatus,
    revision: impl Into<String>,
    change_id: Option<&str>,
) -> Result<()> {
    let state_dir = acceptance_state_root_dir();
    std::fs::create_dir_all(&state_dir).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed creating acceptance state directory '{}': {}",
            state_dir.display(),
            e
        ))
    })?;

    let state = AcceptanceState::new(state, revision, workspace_path, change_id);
    let serialized = serde_json::to_string_pretty(&state).map_err(|e| {
        OrchestratorError::AgentCommand(format!("Failed serializing acceptance state: {}", e))
    })?;

    let state_path = acceptance_state_path(workspace_path);
    std::fs::write(&state_path, serialized).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed writing acceptance state to '{}': {}",
            state_path.display(),
            e
        ))
    })?;

    Ok(())
}

pub fn delete_acceptance_state(workspace_path: &Path) -> Result<()> {
    let state_path = acceptance_state_path(workspace_path);
    if !state_path.exists() {
        return Ok(());
    }

    std::fs::remove_file(&state_path).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed deleting acceptance state '{}': {}",
            state_path.display(),
            e
        ))
    })
}

pub fn mark_apply_completed(workspace_path: &Path, revision: &str, change_id: &str) -> Result<()> {
    save_acceptance_state(
        workspace_path,
        AcceptanceStateStatus::Pending,
        revision,
        Some(change_id),
    )
}

pub fn mark_acceptance_started(
    workspace_path: &Path,
    revision: &str,
    change_id: &str,
) -> Result<()> {
    save_acceptance_state(
        workspace_path,
        AcceptanceStateStatus::Running,
        revision,
        Some(change_id),
    )
}

pub fn mark_acceptance_passed(
    workspace_path: &Path,
    revision: &str,
    change_id: Option<&str>,
) -> Result<()> {
    save_acceptance_state(
        workspace_path,
        AcceptanceStateStatus::Passed,
        revision,
        change_id,
    )
}

pub fn mark_acceptance_failed(
    workspace_path: &Path,
    revision: &str,
    change_id: Option<&str>,
) -> Result<()> {
    save_acceptance_state(
        workspace_path,
        AcceptanceStateStatus::Failed,
        revision,
        change_id,
    )
}

pub fn has_durable_acceptance_pass(workspace_path: &Path, current_revision: &str) -> Result<bool> {
    let Some(state) = load_acceptance_state(workspace_path)? else {
        return Ok(false);
    };

    Ok(state.state == AcceptanceStateStatus::Passed && state.revision == current_revision)
}

pub fn acceptance_resume_ready_for_archive(
    workspace_path: &Path,
    current_revision: &str,
) -> Result<bool> {
    has_durable_acceptance_pass(workspace_path, current_revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn acceptance_state_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();

        save_acceptance_state(
            workspace,
            AcceptanceStateStatus::Running,
            "abc123",
            Some("change-a"),
        )
        .unwrap();
        let loaded = load_acceptance_state(workspace).unwrap().unwrap();

        assert_eq!(loaded.state, AcceptanceStateStatus::Running);
        assert_eq!(loaded.revision, "abc123");
        assert_eq!(loaded.workspace_path, workspace.to_string_lossy().as_ref());
        assert_eq!(loaded.change_id.as_deref(), Some("change-a"));
        assert!(!loaded.updated_at.is_empty());
    }

    #[test]
    fn durable_pass_requires_matching_revision() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();

        mark_acceptance_passed(workspace, "rev-a", Some("change-a")).unwrap();

        assert!(has_durable_acceptance_pass(workspace, "rev-a").unwrap());
        assert!(!has_durable_acceptance_pass(workspace, "rev-b").unwrap());
    }

    #[test]
    fn acceptance_state_is_not_created_under_worktree() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();

        mark_apply_completed(workspace, "rev-a", "change-a").unwrap();

        assert!(
            !workspace
                .join(".cflx")
                .join("acceptance-state.json")
                .exists(),
            "legacy acceptance state file must not be created in worktree"
        );
        assert!(
            acceptance_state_path(workspace).exists(),
            "acceptance state should be persisted outside worktree"
        );
    }

    #[test]
    fn acceptance_state_can_be_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();

        mark_acceptance_passed(workspace, "rev-a", Some("change-a")).unwrap();
        let path = acceptance_state_path(workspace);
        assert!(path.exists());

        delete_acceptance_state(workspace).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn workspace_path_key_survives_relative_and_absolute_paths() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&workspace)
            .output()
            .unwrap();

        let canonical = workspace.canonicalize().unwrap();
        let non_canonical = workspace.join(".");

        mark_acceptance_passed(&canonical, "rev-a", Some("change-a")).unwrap();

        let loaded = load_acceptance_state(&non_canonical).unwrap().unwrap();
        assert_eq!(loaded.state, AcceptanceStateStatus::Passed);
        assert_eq!(loaded.revision, "rev-a");
    }
}
