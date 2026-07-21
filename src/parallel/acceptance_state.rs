use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static WRITE_NONCE: AtomicU64 = AtomicU64::new(0);

use crate::error::{OrchestratorError, Result};

const CHECKPOINT_FILE: &str = ".cflx/acceptance-state.json";
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockedMarkerOrigin {
    Apply,
    Acceptance,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedMarker {
    pub origin: BlockedMarkerOrigin,
    pub reason: String,
    pub phase: String,
    pub evidence: Vec<String>,
    pub finding_identities: Vec<String>,
    pub retry_count: u32,
    pub semantic_fingerprint: Option<String>,
    pub semantic_progress: String,
    pub external_blockers: Vec<String>,
    pub resumable: bool,
    pub next_action: String,
    pub worktree_preserved: bool,
}

#[derive(Serialize, Deserialize)]
struct AcceptanceMarkerDocument {
    schema: String,
    #[serde(flatten)]
    marker: BlockedMarker,
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
        ".{}.tmp-{}-{}",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id(),
        WRITE_NONCE.fetch_add(1, Ordering::Relaxed),
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
) -> Result<AcceptanceState> {
    let previous = load_acceptance_state(workspace_path)?;
    Ok(AcceptanceState {
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
    })
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
        )?,
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
        )?,
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
        )?,
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
        )?,
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
    )?;
    state.previous_finding_identities = identities;
    state.semantic_fingerprint = semantic_fingerprint;
    state.cycle_count = cycle_count;
    save_acceptance_state(workspace_path, state)
}

#[allow(dead_code)]
pub fn write_acceptance_blocked_marker(
    workspace_path: &Path,
    change_id: &str,
    reason: &str,
    evidence: &[String],
    resumable: bool,
    next_action: &str,
) -> Result<()> {
    write_acceptance_blocked_marker_with_context(
        workspace_path,
        change_id,
        reason,
        evidence,
        "unknown",
        &[],
        resumable,
        next_action,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn write_acceptance_blocked_marker_with_context(
    workspace_path: &Path,
    change_id: &str,
    reason: &str,
    evidence: &[String],
    semantic_progress: &str,
    external_blockers: &[String],
    resumable: bool,
    next_action: &str,
) -> Result<()> {
    let state = match load_acceptance_state(workspace_path)? {
        Some(state) => state,
        None => state_for(
            workspace_path,
            AcceptanceStateStatus::Failed,
            "unknown",
            Some(change_id),
        )?,
    };
    let document = AcceptanceMarkerDocument {
        schema: MARKER_VERSION.to_string(),
        marker: BlockedMarker {
            origin: BlockedMarkerOrigin::Acceptance,
            reason: reason.to_string(),
            phase: "acceptance".to_string(),
            evidence: evidence.to_vec(),
            finding_identities: state.previous_finding_identities,
            retry_count: state.cycle_count,
            semantic_fingerprint: state.semantic_fingerprint,
            semantic_progress: semantic_progress.to_string(),
            external_blockers: external_blockers.to_vec(),
            resumable,
            next_action: next_action.to_string(),
            worktree_preserved: true,
        },
    };
    atomic_write(
        &marker_path(workspace_path, change_id),
        &serde_json::to_vec_pretty(&document)?,
    )
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
    if content.trim_start().starts_with('{') {
        let document: AcceptanceMarkerDocument = serde_json::from_str(&content)?;
        if document.schema != MARKER_VERSION {
            return Err(OrchestratorError::AgentCommand(
                "acceptance marker has unsupported schema".to_string(),
            ));
        }
        return Ok(Some(document.marker));
    }

    let origin = if content.lines().any(|line| line.trim() == "origin: apply") {
        BlockedMarkerOrigin::Apply
    } else {
        BlockedMarkerOrigin::Unknown
    };
    let reason = content
        .lines()
        .find_map(|line| line.trim().strip_prefix("reason: ").map(str::to_string))
        .unwrap_or_else(|| "legacy blocked marker".to_string());
    Ok(Some(BlockedMarker {
        origin,
        reason,
        phase: "unknown".to_string(),
        evidence: content
            .lines()
            .filter_map(|line| line.strip_prefix("- ").map(str::to_string))
            .collect(),
        finding_identities: Vec::new(),
        retry_count: 0,
        semantic_fingerprint: None,
        semantic_progress: "unknown".to_string(),
        external_blockers: Vec::new(),
        resumable: false,
        next_action: "preserve marker and inspect evidence".to_string(),
        worktree_preserved: true,
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
    fn checkpoint_missing_malformed_and_apply_restart_are_safe() {
        let temp = TempDir::new().unwrap();
        assert!(load_acceptance_state(temp.path()).unwrap().is_none());
        record_acceptance_retry_context(
            temp.path(),
            "abc",
            "change",
            &["Finding A".to_string()],
            2,
        )
        .unwrap();
        mark_apply_completed(temp.path(), "def", "change").unwrap();
        let state = load_acceptance_state(temp.path()).unwrap().unwrap();
        assert_eq!(state.previous_finding_identities, ["finding a"]);
        assert_eq!(state.cycle_count, 2);
        fs::write(checkpoint_path(temp.path()), "not json").unwrap();
        assert!(mark_apply_completed(temp.path(), "ghi", "change").is_err());
    }

    #[test]
    fn checkpoint_write_failure_leaves_no_partial_state() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join(".cflx"), "not a directory").unwrap();

        assert!(record_acceptance_retry_context(
            temp.path(),
            "abc",
            "change",
            &["Finding A".to_string()],
            1,
        )
        .is_err());
        assert!(!checkpoint_path(temp.path()).exists());
    }

    #[test]
    fn marker_round_trip_preserves_structured_evidence_and_foreign_markers() {
        let temp = TempDir::new().unwrap();
        record_acceptance_retry_context(temp.path(), "abc", "change", &["evidence".to_string()], 2)
            .unwrap();
        write_acceptance_blocked_marker(
            temp.path(),
            "change",
            "blocked: details\nnext line",
            &["external: detail\n- nested".to_string()],
            true,
            "explicit retry",
        )
        .unwrap();
        let marker = parse_blocked_marker(temp.path(), "change")
            .unwrap()
            .unwrap();
        assert_eq!(marker.origin, BlockedMarkerOrigin::Acceptance);
        assert_eq!(marker.finding_identities, ["evidence"]);
        assert_eq!(marker.evidence, ["external: detail\n- nested"]);
        assert_eq!(marker.semantic_progress, "unknown");
        assert!(marker.external_blockers.is_empty());
        assert!(marker.worktree_preserved);
        assert!(consume_resumable_acceptance_marker(temp.path(), "change").unwrap());

        let path = marker_path(temp.path(), "change");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "origin: apply\nreason: blocked\n").unwrap();
        assert_eq!(
            parse_blocked_marker(temp.path(), "change")
                .unwrap()
                .unwrap()
                .origin,
            BlockedMarkerOrigin::Apply
        );
        assert!(!consume_resumable_acceptance_marker(temp.path(), "change").unwrap());
        assert!(path.exists());

        fs::write(&path, "{not json").unwrap();
        assert!(parse_blocked_marker(temp.path(), "change").is_err());
        assert!(path.exists());
    }
}
