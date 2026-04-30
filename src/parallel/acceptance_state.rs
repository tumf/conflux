use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceStateStatus {
    Pending,
    Running,
    Passed,
    Failed,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceState {
    pub state: AcceptanceStateStatus,
    pub revision: String,
    pub updated_at: String,
    pub workspace_path: String,
    pub change_id: Option<String>,
}

#[allow(dead_code)]
pub fn load_acceptance_state(_workspace_path: &Path) -> Result<Option<AcceptanceState>> {
    Ok(None)
}

#[allow(dead_code)]
pub fn delete_acceptance_state(_workspace_path: &Path) -> Result<()> {
    Ok(())
}

#[allow(dead_code)]
pub fn mark_apply_completed(
    _workspace_path: &Path,
    _revision: &str,
    _change_id: &str,
) -> Result<()> {
    Ok(())
}

#[allow(dead_code)]
pub fn mark_acceptance_started(
    _workspace_path: &Path,
    _revision: &str,
    _change_id: &str,
) -> Result<()> {
    Ok(())
}

#[allow(dead_code)]
pub fn mark_acceptance_passed(
    _workspace_path: &Path,
    _revision: &str,
    _change_id: Option<&str>,
) -> Result<()> {
    Ok(())
}

#[allow(dead_code)]
pub fn mark_acceptance_failed(
    _workspace_path: &Path,
    _revision: &str,
    _change_id: Option<&str>,
) -> Result<()> {
    Ok(())
}

#[allow(dead_code)]
pub fn has_durable_acceptance_pass(
    _workspace_path: &Path,
    _current_revision: &str,
) -> Result<bool> {
    Ok(false)
}

#[allow(dead_code)]
pub fn acceptance_resume_ready_for_archive(
    _workspace_path: &Path,
    _current_revision: &str,
) -> Result<bool> {
    Ok(false)
}
