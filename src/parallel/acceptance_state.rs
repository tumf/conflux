use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{OrchestratorError, Result};

const CHECKPOINT_FILE: &str = "ACCEPTANCE_STATE.json";
const BLOCKED_MARKER_FILE: &str = "APPLY_BLOCKED/marker.md";

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
    #[serde(default)]
    pub previous_finding_identities: Vec<String>,
    #[serde(default)]
    pub semantic_fingerprint: Option<String>,
    #[serde(default)]
    pub cycle_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedMarkerOrigin {
    Apply,
    Acceptance,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedMarker {
    pub origin: BlockedMarkerOrigin,
    pub reason: String,
    pub evidence: Vec<String>,
    pub resumable: bool,
    pub next_action: String,
}

fn change_dir(workspace_path: &Path, change_id: &str) -> PathBuf {
    workspace_path.join("openspec/changes").join(change_id)
}

fn checkpoint_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(CHECKPOINT_FILE)
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        OrchestratorError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "state path has no parent",
        ))
    })?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        OrchestratorError::Io(error)
    })
}

fn save_acceptance_state(workspace_path: &Path, state: AcceptanceState) -> Result<()> {
    atomic_write(
        &checkpoint_path(workspace_path),
        &serde_json::to_vec_pretty(&state)?,
    )
}

fn state_for(
    workspace_path: &Path,
    state: AcceptanceStateStatus,
    revision: &str,
    change_id: Option<&str>,
) -> AcceptanceState {
    AcceptanceState {
        state,
        revision: revision.to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        workspace_path: workspace_path.display().to_string(),
        change_id: change_id.map(str::to_string),
        previous_finding_identities: Vec::new(),
        semantic_fingerprint: None,
        cycle_count: 0,
    }
}

pub fn load_acceptance_state(workspace_path: &Path) -> Result<Option<AcceptanceState>> {
    let path = checkpoint_path(workspace_path);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

pub fn delete_acceptance_state(workspace_path: &Path) -> Result<()> {
    let path = checkpoint_path(workspace_path);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn mark_apply_completed(workspace_path: &Path, revision: &str, change_id: &str) -> Result<()> {
    save_acceptance_state(
        workspace_path,
        state_for(
            workspace_path,
            AcceptanceStateStatus::Pending,
            revision,
            Some(change_id),
        ),
    )
}

pub fn mark_acceptance_started(
    workspace_path: &Path,
    revision: &str,
    change_id: &str,
) -> Result<()> {
    save_acceptance_state(
        workspace_path,
        state_for(
            workspace_path,
            AcceptanceStateStatus::Running,
            revision,
            Some(change_id),
        ),
    )
}

pub fn mark_acceptance_passed(
    workspace_path: &Path,
    revision: &str,
    change_id: Option<&str>,
) -> Result<()> {
    save_acceptance_state(
        workspace_path,
        state_for(
            workspace_path,
            AcceptanceStateStatus::Passed,
            revision,
            change_id,
        ),
    )
}

pub fn mark_acceptance_failed(
    workspace_path: &Path,
    revision: &str,
    change_id: Option<&str>,
) -> Result<()> {
    save_acceptance_state(
        workspace_path,
        state_for(
            workspace_path,
            AcceptanceStateStatus::Failed,
            revision,
            change_id,
        ),
    )
}

pub fn has_durable_acceptance_pass(workspace_path: &Path, current_revision: &str) -> Result<bool> {
    Ok(load_acceptance_state(workspace_path)?.is_some_and(|state| {
        state.state == AcceptanceStateStatus::Passed && state.revision == current_revision
    }))
}

pub fn acceptance_resume_ready_for_archive(
    workspace_path: &Path,
    current_revision: &str,
) -> Result<bool> {
    has_durable_acceptance_pass(workspace_path, current_revision)
}

pub fn write_acceptance_blocked_marker(
    workspace_path: &Path,
    change_id: &str,
    reason: &str,
    evidence: &[String],
    resumable: bool,
    next_action: &str,
) -> Result<()> {
    let marker = format!(
        "origin: acceptance\nreason: {reason}\nresumable: {resumable}\nnext_action: {next_action}\nevidence:\n{}",
        evidence.iter().map(|value| format!("- {value}\n")).collect::<String>(),
    );
    atomic_write(
        &change_dir(workspace_path, change_id).join(BLOCKED_MARKER_FILE),
        marker.as_bytes(),
    )
}

pub fn parse_blocked_marker(
    workspace_path: &Path,
    change_id: &str,
) -> Result<Option<BlockedMarker>> {
    let path = change_dir(workspace_path, change_id).join(BLOCKED_MARKER_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let field = |name: &str| {
        content
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}: ")).map(str::to_string))
    };
    let origin = match field("origin").as_deref() {
        Some("acceptance") => BlockedMarkerOrigin::Acceptance,
        Some("apply") => BlockedMarkerOrigin::Apply,
        _ => BlockedMarkerOrigin::Unknown,
    };
    let evidence = content
        .lines()
        .filter_map(|line| line.strip_prefix("- ").map(str::to_string))
        .collect();
    Ok(Some(BlockedMarker {
        origin,
        reason: field("reason").unwrap_or_else(|| "legacy blocked marker".to_string()),
        evidence,
        resumable: field("resumable").is_some_and(|value| value == "true"),
        next_action: field("next_action")
            .unwrap_or_else(|| "preserve marker and inspect evidence".to_string()),
    }))
}

pub fn consume_resumable_acceptance_marker(workspace_path: &Path, change_id: &str) -> Result<bool> {
    let path = change_dir(workspace_path, change_id).join(BLOCKED_MARKER_FILE);
    if !matches!(
        parse_blocked_marker(workspace_path, change_id)?,
        Some(BlockedMarker {
            origin: BlockedMarkerOrigin::Acceptance,
            resumable: true,
            ..
        })
    ) {
        return Ok(false);
    }
    fs::remove_file(path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn checkpoint_and_acceptance_marker_round_trip() {
        let temp = TempDir::new().unwrap();
        mark_apply_completed(temp.path(), "abc", "change").unwrap();
        assert_eq!(
            load_acceptance_state(temp.path()).unwrap().unwrap().state,
            AcceptanceStateStatus::Pending
        );
        write_acceptance_blocked_marker(
            temp.path(),
            "change",
            "blocked",
            &["evidence".to_string()],
            true,
            "retry",
        )
        .unwrap();
        assert_eq!(
            parse_blocked_marker(temp.path(), "change")
                .unwrap()
                .unwrap()
                .origin,
            BlockedMarkerOrigin::Acceptance
        );
        assert!(consume_resumable_acceptance_marker(temp.path(), "change").unwrap());
    }

    #[test]
    fn malformed_checkpoint_is_an_error_and_foreign_marker_is_preserved() {
        let temp = TempDir::new().unwrap();
        fs::write(checkpoint_path(temp.path()), "{").unwrap();
        assert!(load_acceptance_state(temp.path()).is_err());
        let marker = change_dir(temp.path(), "change").join(BLOCKED_MARKER_FILE);
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, "legacy marker").unwrap();
        assert!(!consume_resumable_acceptance_marker(temp.path(), "change").unwrap());
        assert!(marker.exists());
    }
}
