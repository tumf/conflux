//! Acceptance retry state.
//!
//! Ordinary acceptance retry bookkeeping (cycle count, previous finding
//! identities, semantic baseline) lives in memory for the duration of one
//! orchestration run only. Conflux deliberately persists no acceptance
//! checkpoint: after a restart, complete but unarchived work is accepted again
//! rather than trusting an inferred prior PASS.
//!
//! The only durable acceptance artifact is the tracked
//! `APPLY_BLOCKED/marker.md` stalled hold, which has an independent
//! repository-visible purpose and is written only after retry safeguards decide
//! that further automatic work must stop.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static WRITE_NONCE: AtomicU64 = AtomicU64::new(0);

use crate::error::{OrchestratorError, Result};

const BLOCKED_MARKER_FILE: &str = "APPLY_BLOCKED/marker.md";
const MARKER_VERSION: &str = "acceptance-stalled-v1";

/// In-memory acceptance retry context for one active orchestration run.
///
/// This is never serialized to a generated checkpoint file. It is discarded on
/// process restart, which forces a fresh acceptance sequence instead of an
/// inferred prior verdict.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcceptanceRetryContext {
    pub finding_identities: Vec<String>,
    pub semantic_fingerprint: Option<String>,
    pub cycle_count: u32,
}

impl AcceptanceRetryContext {
    pub fn previous_identities(&self) -> &[String] {
        &self.finding_identities
    }

    pub fn previous_fingerprint(&self) -> Option<&str> {
        self.semantic_fingerprint.as_deref()
    }
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
        &AcceptanceRetryContext::default(),
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
    retry: &AcceptanceRetryContext,
    semantic_progress: &str,
    external_blockers: &[String],
    resumable: bool,
    next_action: &str,
) -> Result<()> {
    let document = AcceptanceMarkerDocument {
        schema: MARKER_VERSION.to_string(),
        marker: BlockedMarker {
            origin: BlockedMarkerOrigin::Acceptance,
            reason: reason.to_string(),
            phase: "acceptance".to_string(),
            evidence: evidence.to_vec(),
            finding_identities: retry.finding_identities.clone(),
            retry_count: retry.cycle_count,
            semantic_fingerprint: retry.semantic_fingerprint.clone(),
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

    fn retry_context(
        identities: &[&str],
        fingerprint: Option<&str>,
        cycle_count: u32,
    ) -> AcceptanceRetryContext {
        AcceptanceRetryContext {
            finding_identities: identities.iter().map(|value| value.to_string()).collect(),
            semantic_fingerprint: fingerprint.map(str::to_string),
            cycle_count,
        }
    }

    #[test]
    fn module_persists_no_acceptance_checkpoint_file() {
        let temp = TempDir::new().unwrap();
        write_acceptance_blocked_marker_with_context(
            temp.path(),
            "change",
            "acceptance_gated",
            &[],
            &retry_context(&["repository|evidence|verification"], Some("baseline"), 2),
            "no_semantic_progress",
            &[],
            true,
            "explicit retry",
        )
        .unwrap();

        assert!(!temp.path().join(".cflx/acceptance-state.json").exists());
        assert!(!temp.path().join(".cflx").exists());
    }

    #[test]
    fn marker_write_failure_leaves_no_partial_marker() {
        let temp = TempDir::new().unwrap();
        let change_dir = temp.path().join("openspec/changes/change");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("APPLY_BLOCKED"), "not a directory").unwrap();

        assert!(write_acceptance_blocked_marker_with_context(
            temp.path(),
            "change",
            "acceptance_gated",
            &[],
            &AcceptanceRetryContext::default(),
            "no_semantic_progress",
            &[],
            true,
            "explicit retry",
        )
        .is_err());
        assert!(!marker_path(temp.path(), "change").exists());
    }

    #[test]
    fn marker_round_trip_preserves_in_memory_retry_context_and_foreign_markers() {
        let temp = TempDir::new().unwrap();
        write_acceptance_blocked_marker_with_context(
            temp.path(),
            "change",
            "blocked: details\nnext line",
            &["external: detail\n- nested".to_string()],
            &retry_context(
                &["repository|evidence|verification"],
                Some("semantic baseline"),
                2,
            ),
            "no_semantic_progress",
            &["recoverable verification blocker".to_string()],
            true,
            "explicit retry",
        )
        .unwrap();
        let marker = parse_blocked_marker(temp.path(), "change")
            .unwrap()
            .unwrap();
        assert_eq!(marker.origin, BlockedMarkerOrigin::Acceptance);
        assert_eq!(
            marker.finding_identities,
            ["repository|evidence|verification"]
        );
        assert_eq!(marker.retry_count, 2);
        assert_eq!(
            marker.semantic_fingerprint.as_deref(),
            Some("semantic baseline")
        );
        assert_eq!(marker.evidence, ["external: detail\n- nested"]);
        assert_eq!(marker.semantic_progress, "no_semantic_progress");
        assert_eq!(
            marker.external_blockers,
            ["recoverable verification blocker"]
        );
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

    #[test]
    fn default_marker_write_has_no_retry_context() {
        let temp = TempDir::new().unwrap();
        write_acceptance_blocked_marker(
            temp.path(),
            "change",
            "permission_stalled",
            &["external blocker".to_string()],
            true,
            "explicit retry",
        )
        .unwrap();

        let marker = parse_blocked_marker(temp.path(), "change")
            .unwrap()
            .unwrap();
        assert!(marker.finding_identities.is_empty());
        assert_eq!(marker.retry_count, 0);
        assert_eq!(marker.semantic_fingerprint, None);
        assert_eq!(marker.evidence, ["external blocker"]);
    }
}
