use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{OrchestratorError, Result};

const ACCEPTANCE_STATE_DIR: &str = ".cflx";
const ACCEPTANCE_STATE_FILE: &str = "acceptance-state.json";

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
}

impl AcceptanceState {
    fn new(state: AcceptanceStateStatus, revision: impl Into<String>) -> Self {
        Self {
            state,
            revision: revision.into(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub fn acceptance_state_path(workspace_path: &Path) -> PathBuf {
    workspace_path
        .join(ACCEPTANCE_STATE_DIR)
        .join(ACCEPTANCE_STATE_FILE)
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
) -> Result<()> {
    let state_dir = workspace_path.join(ACCEPTANCE_STATE_DIR);
    std::fs::create_dir_all(&state_dir).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed creating acceptance state directory '{}': {}",
            state_dir.display(),
            e
        ))
    })?;

    let state = AcceptanceState::new(state, revision);
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

pub fn mark_apply_completed(workspace_path: &Path, revision: &str) -> Result<()> {
    save_acceptance_state(workspace_path, AcceptanceStateStatus::Pending, revision)
}

pub fn mark_acceptance_started(workspace_path: &Path, revision: &str) -> Result<()> {
    save_acceptance_state(workspace_path, AcceptanceStateStatus::Running, revision)
}

pub fn mark_acceptance_passed(workspace_path: &Path, revision: &str) -> Result<()> {
    save_acceptance_state(workspace_path, AcceptanceStateStatus::Passed, revision)
}

pub fn mark_acceptance_failed(workspace_path: &Path, revision: &str) -> Result<()> {
    save_acceptance_state(workspace_path, AcceptanceStateStatus::Failed, revision)
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

    #[test]
    fn acceptance_state_roundtrip() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();

        save_acceptance_state(workspace, AcceptanceStateStatus::Running, "abc123").unwrap();
        let loaded = load_acceptance_state(workspace).unwrap().unwrap();

        assert_eq!(loaded.state, AcceptanceStateStatus::Running);
        assert_eq!(loaded.revision, "abc123");
        assert!(!loaded.updated_at.is_empty());
    }

    #[test]
    fn durable_pass_requires_matching_revision() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();

        mark_acceptance_passed(workspace, "rev-a").unwrap();

        assert!(has_durable_acceptance_pass(workspace, "rev-a").unwrap());
        assert!(!has_durable_acceptance_pass(workspace, "rev-b").unwrap());
    }
}
