use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;
use crate::history::ArchivePrimaryReason;

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

pub fn load_archive_state(_workspace_path: &Path) -> Result<Option<ArchiveResumeState>> {
    Ok(None)
}

pub fn load_archive_state_matching(
    _workspace_path: &Path,
    _change_id: &str,
    _expected_revision: &str,
) -> Result<Option<ArchiveResumeState>> {
    Ok(None)
}

#[allow(dead_code)]
pub fn delete_archive_state(_workspace_path: &Path) -> Result<()> {
    Ok(())
}

pub fn save_archive_state_entry(
    _workspace_path: &Path,
    _change_id: &str,
    _revision: &str,
    _attempt: u32,
    _status: ArchiveResumeStatus,
    _primary_reason: Option<ArchivePrimaryReason>,
    _summary: impl Into<String>,
) -> Result<()> {
    Ok(())
}
