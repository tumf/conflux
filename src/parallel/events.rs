//! Event definitions and sending helpers for parallel execution.

use tokio::sync::mpsc;
use tracing::debug;

/// Events emitted during parallel execution
#[derive(Debug, Clone)]
pub enum ParallelEvent {
    /// An existing workspace was found and is being reused
    WorkspaceResumed {
        change_id: String,
        workspace: String,
    },
    /// A workspace was created
    WorkspaceCreated {
        change_id: String,
        workspace: String,
    },
    /// Apply started in a workspace
    ApplyStarted { change_id: String },
    /// Apply output (summary of command output)
    ApplyOutput { change_id: String, output: String },
    /// Progress updated for a change (task completion tracking)
    ProgressUpdated {
        change_id: String,
        completed: u32,
        total: u32,
    },
    /// Apply completed in a workspace
    ApplyCompleted {
        change_id: String,
        #[allow(dead_code)] // Available for event consumers to log/display
        revision: String,
    },
    /// Apply failed in a workspace
    ApplyFailed { change_id: String, error: String },
    /// Archive started for a change
    ArchiveStarted { change_id: String },
    /// Archive output (streaming)
    ArchiveOutput { change_id: String, output: String },
    /// Change archived successfully
    ChangeArchived { change_id: String },
    /// Change archive failed
    ArchiveFailed { change_id: String, error: String },
    /// Merge started
    MergeStarted { revisions: Vec<String> },
    /// Merge completed
    MergeCompleted {
        #[allow(dead_code)] // Available for event consumers to log/display
        revision: String,
    },
    /// Merge resulted in conflicts
    MergeConflict { files: Vec<String> },
    /// Conflict resolution started
    ConflictResolutionStarted,
    /// Conflict resolution completed
    ConflictResolutionCompleted,
    /// Conflict resolution failed
    ConflictResolutionFailed { error: String },
    /// Workspace cleanup started
    CleanupStarted { workspace: String },
    /// Workspace cleanup completed
    CleanupCompleted {
        #[allow(dead_code)] // Available for event consumers to log/display
        workspace: String,
    },
    /// Group execution started
    GroupStarted { group_id: u32, changes: Vec<String> },
    /// Group execution completed
    GroupCompleted { group_id: u32 },
    /// A change was skipped because a dependency failed
    ChangeSkipped { change_id: String, reason: String },
    /// Analysis started for remaining changes
    AnalysisStarted { remaining_changes: usize },
    /// Analysis output (streaming)
    AnalysisOutput { output: String },
    /// Analysis completed (reserved for future dynamic re-analysis UI)
    #[allow(dead_code)]
    AnalysisCompleted { groups_found: usize },
    /// Resolve output (streaming)
    ResolveOutput { output: String },
    /// All groups completed
    AllCompleted,
    /// A workspace was preserved due to an error (not cleaned up)
    WorkspacePreserved {
        change_id: String,
        workspace_name: String,
    },
    /// Error during parallel execution
    Error { message: String },
    /// Hook execution started
    HookStarted {
        change_id: String,
        hook_type: String,
    },
    /// Hook execution completed successfully
    HookCompleted {
        change_id: String,
        hook_type: String,
    },
    /// Hook execution failed
    HookFailed {
        change_id: String,
        hook_type: String,
        error: String,
    },
}

/// Helper to send events through the channel.
///
/// Logs debug message if sending fails (channel closed).
pub async fn send_event(tx: &Option<mpsc::Sender<ParallelEvent>>, event: ParallelEvent) {
    if let Some(ref tx) = tx {
        if let Err(e) = tx.send(event).await {
            debug!("Failed to send parallel event: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_event_debug() {
        let event = ParallelEvent::WorkspaceCreated {
            change_id: "test".to_string(),
            workspace: "ws-test".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("WorkspaceCreated"));
    }

    #[test]
    fn test_hook_started_event() {
        let event = ParallelEvent::HookStarted {
            change_id: "test-change".to_string(),
            hook_type: "pre_apply".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookStarted"));
        assert!(debug_str.contains("test-change"));
        assert!(debug_str.contains("pre_apply"));
    }

    #[test]
    fn test_hook_completed_event() {
        let event = ParallelEvent::HookCompleted {
            change_id: "test-change".to_string(),
            hook_type: "post_apply".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookCompleted"));
        assert!(debug_str.contains("post_apply"));
    }

    #[test]
    fn test_hook_failed_event() {
        let event = ParallelEvent::HookFailed {
            change_id: "test-change".to_string(),
            hook_type: "pre_archive".to_string(),
            error: "Hook timed out".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("HookFailed"));
        assert!(debug_str.contains("pre_archive"));
        assert!(debug_str.contains("Hook timed out"));
    }
}
