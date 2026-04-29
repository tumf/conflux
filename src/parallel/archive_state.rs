use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{OrchestratorError, Result};
use crate::history::ArchivePrimaryReason;

const ARCHIVE_STATE_DIR_NAME: &str = "archive-resume-state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveResumeStatus {
    Running,
    Failed,
    Stalled,
    Passed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveResumeState {
    pub change_id: String,
    pub revision: String,
    pub attempt: u32,
    pub status: ArchiveResumeStatus,
    pub primary_reason: Option<ArchivePrimaryReason>,
    pub summary: String,
    pub updated_at: String,
}

fn archive_state_root_dir() -> PathBuf {
    if let Ok(xdg_state_home) = std::env::var("XDG_STATE_HOME") {
        if !xdg_state_home.is_empty() {
            return PathBuf::from(xdg_state_home)
                .join("cflx")
                .join(ARCHIVE_STATE_DIR_NAME);
        }
    }

    if let Some(home_dir) = dirs::home_dir() {
        return home_dir
            .join(".local")
            .join("state")
            .join("cflx")
            .join(ARCHIVE_STATE_DIR_NAME);
    }

    std::env::temp_dir().join("cflx-archive-resume-state")
}

fn workspace_state_file_name(workspace_path: &Path) -> String {
    let canonical = workspace_path
        .canonicalize()
        .unwrap_or_else(|_| workspace_path.to_path_buf());
    let digest = md5::compute(canonical.to_string_lossy().as_bytes());
    format!("{:x}.json", digest)
}

pub fn archive_state_path(workspace_path: &Path) -> PathBuf {
    archive_state_root_dir().join(workspace_state_file_name(workspace_path))
}

pub fn load_archive_state(workspace_path: &Path) -> Result<Option<ArchiveResumeState>> {
    let state_path = archive_state_path(workspace_path);
    if !state_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&state_path).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed reading archive state from '{}': {}",
            state_path.display(),
            e
        ))
    })?;

    let state = serde_json::from_str::<ArchiveResumeState>(&content).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed parsing archive state from '{}': {}",
            state_path.display(),
            e
        ))
    })?;

    Ok(Some(state))
}

pub fn load_archive_state_matching(
    workspace_path: &Path,
    change_id: &str,
    expected_revision: &str,
) -> Result<Option<ArchiveResumeState>> {
    let Some(state) = load_archive_state(workspace_path)? else {
        return Ok(None);
    };

    if state.change_id != change_id || state.revision != expected_revision {
        delete_archive_state(workspace_path)?;
        return Ok(None);
    }

    Ok(Some(state))
}

pub fn save_archive_state(workspace_path: &Path, state: ArchiveResumeState) -> Result<()> {
    let state_dir = archive_state_root_dir();
    std::fs::create_dir_all(&state_dir).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed creating archive state directory '{}': {}",
            state_dir.display(),
            e
        ))
    })?;

    let serialized = serde_json::to_string_pretty(&state).map_err(|e| {
        OrchestratorError::AgentCommand(format!("Failed serializing archive state: {}", e))
    })?;

    let state_path = archive_state_path(workspace_path);
    std::fs::write(&state_path, serialized).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed writing archive state to '{}': {}",
            state_path.display(),
            e
        ))
    })?;

    Ok(())
}

#[allow(dead_code)]
pub fn delete_archive_state(workspace_path: &Path) -> Result<()> {
    let state_path = archive_state_path(workspace_path);
    if !state_path.exists() {
        return Ok(());
    }

    std::fs::remove_file(&state_path).map_err(|e| {
        OrchestratorError::AgentCommand(format!(
            "Failed deleting archive state '{}': {}",
            state_path.display(),
            e
        ))
    })
}

pub fn save_archive_state_entry(
    workspace_path: &Path,
    change_id: &str,
    revision: &str,
    attempt: u32,
    status: ArchiveResumeStatus,
    primary_reason: Option<ArchivePrimaryReason>,
    summary: impl Into<String>,
) -> Result<()> {
    save_archive_state(
        workspace_path,
        ArchiveResumeState {
            change_id: change_id.to_string(),
            revision: revision.to_string(),
            attempt,
            status,
            primary_reason,
            summary: summary.into(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_state_roundtrip_and_delete() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();

        save_archive_state_entry(
            workspace,
            "change-a",
            "rev-a",
            3,
            ArchiveResumeStatus::Failed,
            Some(ArchivePrimaryReason::VerificationFailed),
            "verification failed",
        )
        .unwrap();

        let loaded = load_archive_state(workspace).unwrap().unwrap();
        assert_eq!(loaded.change_id, "change-a");
        assert_eq!(loaded.revision, "rev-a");
        assert_eq!(loaded.attempt, 3);
        assert_eq!(loaded.status, ArchiveResumeStatus::Failed);
        assert_eq!(
            loaded.primary_reason,
            Some(ArchivePrimaryReason::VerificationFailed)
        );

        delete_archive_state(workspace).unwrap();
        assert!(load_archive_state(workspace).unwrap().is_none());
    }

    #[test]
    fn load_archive_state_matching_deletes_mismatched_revision() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path();

        save_archive_state_entry(
            workspace,
            "change-a",
            "rev-a",
            1,
            ArchiveResumeStatus::Running,
            None,
            "running",
        )
        .unwrap();

        let loaded = load_archive_state_matching(workspace, "change-a", "rev-b").unwrap();
        assert!(loaded.is_none());
        assert!(load_archive_state(workspace).unwrap().is_none());
    }
}
