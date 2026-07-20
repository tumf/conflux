use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{OrchestratorError, Result};

const CHECKPOINT_FILE: &str = "ACCEPTANCE_STATE.json";
const BLOCKED_MARKER_FILE: &str = "APPLY_BLOCKED/marker.md";
const MARKER_VERSION: &str = "acceptance-stalled-v1";

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
    pub phase: String,
    pub evidence: Vec<String>,
    pub finding_identities: Vec<String>,
    pub retry_count: u32,
    pub semantic_fingerprint: Option<String>,
    pub external_blockers: Vec<String>,
    pub resumable: bool,
    pub next_action: String,
}

fn change_dir(workspace_path: &Path, change_id: &str) -> PathBuf {
    workspace_path.join("openspec/changes").join(change_id)
}

fn checkpoint_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(CHECKPOINT_FILE)
}

fn marker_path(workspace_path: &Path, change_id: &str) -> PathBuf {
    change_dir(workspace_path, change_id).join(BLOCKED_MARKER_FILE)
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
    let previous = load_acceptance_state(workspace_path).ok().flatten();
    AcceptanceState {
        state,
        revision: revision.to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        workspace_path: workspace_path.display().to_string(),
        change_id: change_id.map(str::to_string),
        previous_finding_identities: previous
            .as_ref()
            .map_or_else(Vec::new, |value| value.previous_finding_identities.clone()),
        semantic_fingerprint: previous
            .as_ref()
            .and_then(|value| value.semantic_fingerprint.clone()),
        cycle_count: previous.map_or(0, |value| value.cycle_count),
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

pub fn record_acceptance_retry_context(
    workspace_path: &Path,
    revision: &str,
    change_id: &str,
    findings: &[String],
    cycle_count: u32,
) -> Result<()> {
    let identities = findings
        .iter()
        .map(|finding| finding.trim().to_ascii_lowercase())
        .filter(|finding| !finding.is_empty())
        .collect::<Vec<_>>();
    let semantic_fingerprint = (!identities.is_empty()).then(|| identities.join("\n"));
    let mut state = state_for(
        workspace_path,
        AcceptanceStateStatus::Failed,
        revision,
        Some(change_id),
    );
    state.previous_finding_identities = identities;
    state.semantic_fingerprint = semantic_fingerprint;
    state.cycle_count = cycle_count;
    save_acceptance_state(workspace_path, state)
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
    let state = load_acceptance_state(workspace_path)?.unwrap_or_else(|| {
        state_for(
            workspace_path,
            AcceptanceStateStatus::Failed,
            "unknown",
            Some(change_id),
        )
    });
    let marker = format!(
        "schema: {MARKER_VERSION}\norigin: acceptance\nreason: {reason}\nphase: acceptance\nretry_count: {}\nsemantic_fingerprint: {}\nresumable: {resumable}\nnext_action: {next_action}\nfinding_identities:\n{}external_blockers:\nevidence:\n{}",
        state.cycle_count,
        state.semantic_fingerprint.as_deref().unwrap_or("none"),
        state.previous_finding_identities.iter().map(|value| format!("- {value}\n")).collect::<String>(),
        evidence.iter().map(|value| format!("- {value}\n")).collect::<String>(),
    );
    atomic_write(&marker_path(workspace_path, change_id), marker.as_bytes())
}

fn field(content: &str, name: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}: ")).map(str::to_string))
}

fn list_after(content: &str, heading: &str) -> Vec<String> {
    content
        .lines()
        .skip_while(|line| *line != heading)
        .skip(1)
        .take_while(|line| !line.ends_with(':'))
        .filter_map(|line| line.strip_prefix("- ").map(str::to_string))
        .collect()
}

pub fn parse_blocked_marker(
    workspace_path: &Path,
    change_id: &str,
) -> Result<Option<BlockedMarker>> {
    let path = marker_path(workspace_path, change_id);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let schema = field(&content, "schema");
    if schema.as_deref() != Some(MARKER_VERSION) {
        return Ok(Some(BlockedMarker {
            origin: match field(&content, "origin").as_deref() {
                Some("apply") => BlockedMarkerOrigin::Apply,
                _ => BlockedMarkerOrigin::Unknown,
            },
            reason: field(&content, "reason")
                .unwrap_or_else(|| "legacy blocked marker".to_string()),
            phase: "unknown".to_string(),
            evidence: content
                .lines()
                .filter_map(|line| line.strip_prefix("- ").map(str::to_string))
                .collect(),
            finding_identities: Vec::new(),
            retry_count: 0,
            semantic_fingerprint: None,
            external_blockers: Vec::new(),
            resumable: false,
            next_action: "preserve marker and inspect evidence".to_string(),
        }));
    }
    let resumable = field(&content, "resumable")
        .ok_or_else(|| {
            OrchestratorError::AgentCommand("acceptance marker missing resumable".to_string())
        })?
        .parse::<bool>()
        .map_err(|_| {
            OrchestratorError::AgentCommand("acceptance marker has invalid resumable".to_string())
        })?;
    let retry_count = field(&content, "retry_count")
        .ok_or_else(|| {
            OrchestratorError::AgentCommand("acceptance marker missing retry_count".to_string())
        })?
        .parse::<u32>()
        .map_err(|_| {
            OrchestratorError::AgentCommand("acceptance marker has invalid retry_count".to_string())
        })?;
    let origin = match field(&content, "origin").as_deref() {
        Some("acceptance") => BlockedMarkerOrigin::Acceptance,
        _ => {
            return Err(OrchestratorError::AgentCommand(
                "acceptance marker has invalid origin".to_string(),
            ))
        }
    };
    Ok(Some(BlockedMarker {
        origin,
        reason: field(&content, "reason").ok_or_else(|| {
            OrchestratorError::AgentCommand("acceptance marker missing reason".to_string())
        })?,
        phase: field(&content, "phase").ok_or_else(|| {
            OrchestratorError::AgentCommand("acceptance marker missing phase".to_string())
        })?,
        evidence: list_after(&content, "evidence:"),
        finding_identities: list_after(&content, "finding_identities:"),
        retry_count,
        semantic_fingerprint: field(&content, "semantic_fingerprint")
            .filter(|value| value != "none"),
        external_blockers: list_after(&content, "external_blockers:"),
        resumable,
        next_action: field(&content, "next_action").ok_or_else(|| {
            OrchestratorError::AgentCommand("acceptance marker missing next_action".to_string())
        })?,
    }))
}

pub fn consume_resumable_acceptance_marker(workspace_path: &Path, change_id: &str) -> Result<bool> {
    let path = marker_path(workspace_path, change_id);
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
    fn checkpoint_round_trip_preserves_retry_context() {
        let temp = TempDir::new().unwrap();
        record_acceptance_retry_context(
            temp.path(),
            "abc",
            "change",
            &["Finding A".to_string()],
            2,
        )
        .unwrap();
        let state = load_acceptance_state(temp.path()).unwrap().unwrap();
        assert_eq!(state.previous_finding_identities, ["finding a"]);
        assert_eq!(state.cycle_count, 2);
        assert_eq!(state.semantic_fingerprint.as_deref(), Some("finding a"));
    }

    #[test]
    fn marker_round_trip_legacy_and_malformed_input() {
        let temp = TempDir::new().unwrap();
        record_acceptance_retry_context(temp.path(), "abc", "change", &["evidence".to_string()], 2)
            .unwrap();
        write_acceptance_blocked_marker(
            temp.path(),
            "change",
            "blocked",
            &["external".to_string()],
            true,
            "explicit retry",
        )
        .unwrap();
        assert!(consume_resumable_acceptance_marker(temp.path(), "change").unwrap());
        let path = marker_path(temp.path(), "change");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "legacy marker").unwrap();
        assert!(!consume_resumable_acceptance_marker(temp.path(), "change").unwrap());
        fs::write(
            &path,
            "schema: acceptance-stalled-v1\norigin: acceptance\nresumable: nope",
        )
        .unwrap();
        assert!(parse_blocked_marker(temp.path(), "change").is_err());
        assert!(path.exists());
    }
}
