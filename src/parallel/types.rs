//! Common types for parallel execution.

/// Result of a workspace execution (VCS-agnostic)
#[derive(Debug, Clone)]
pub struct WorkspaceResult {
    /// OpenSpec change ID
    pub change_id: String,
    /// Workspace name
    pub workspace_name: String,
    /// Final revision if successful
    pub final_revision: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}
