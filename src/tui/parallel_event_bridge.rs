//! Bridge for converting ParallelExecutor events to TUI OrchestratorEvents
//!
//! This module provides a clean abstraction for translating parallel execution
//! events into TUI-compatible events, separating event transformation logic
//! from the orchestrator control flow.

use crate::parallel_executor::ParallelEvent;
use crate::tui::events::{LogEntry, OrchestratorEvent};

/// Converts a ParallelEvent into a list of OrchestratorEvents
///
/// Some ParallelEvents map to multiple OrchestratorEvents (e.g., ApplyStarted
/// generates both a log entry and a ProcessingStarted event).
pub fn convert(event: ParallelEvent) -> Vec<OrchestratorEvent> {
    match event {
        ParallelEvent::WorkspaceCreated {
            change_id,
            workspace,
        } => vec![OrchestratorEvent::Log(
            LogEntry::info(format!("Created workspace: {}", workspace)).with_change_id(&change_id),
        )],

        ParallelEvent::ApplyStarted { change_id } => vec![
            OrchestratorEvent::Log(
                LogEntry::info("Apply started".to_string()).with_change_id(&change_id),
            ),
            OrchestratorEvent::ProcessingStarted(change_id),
        ],

        ParallelEvent::ApplyOutput { change_id, output } => {
            // Split output into multiple log entries for each non-empty line
            output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| {
                    OrchestratorEvent::Log(
                        LogEntry::info(line.to_string()).with_change_id(&change_id),
                    )
                })
                .collect()
        }

        ParallelEvent::ProgressUpdated {
            change_id,
            completed,
            total,
        } => vec![OrchestratorEvent::ProgressUpdated {
            id: change_id,
            completed,
            total,
        }],

        ParallelEvent::ApplyCompleted { change_id, .. } => vec![
            OrchestratorEvent::Log(
                LogEntry::success("Apply completed".to_string()).with_change_id(&change_id),
            ),
            OrchestratorEvent::ProcessingCompleted(change_id),
        ],

        ParallelEvent::ApplyFailed { change_id, error } => vec![
            OrchestratorEvent::Log(
                LogEntry::error(format!("Apply failed: {}", error)).with_change_id(&change_id),
            ),
            OrchestratorEvent::ProcessingError {
                id: change_id,
                error,
            },
        ],

        ParallelEvent::MergeStarted { revisions } => vec![OrchestratorEvent::Log(LogEntry::info(
            format!("Merging {} revisions", revisions.len()),
        ))],

        ParallelEvent::MergeCompleted { .. } => {
            vec![OrchestratorEvent::Log(LogEntry::success(
                "Merge completed".to_string(),
            ))]
        }

        ParallelEvent::MergeConflict { files } => vec![OrchestratorEvent::Log(LogEntry::warn(
            format!("Merge conflicts in {} files", files.len()),
        ))],

        ParallelEvent::ConflictResolutionStarted => vec![OrchestratorEvent::Log(LogEntry::info(
            "Starting conflict resolution...".to_string(),
        ))],

        ParallelEvent::ConflictResolutionCompleted => vec![OrchestratorEvent::Log(
            LogEntry::success("Conflict resolution completed".to_string()),
        )],

        ParallelEvent::ConflictResolutionFailed { error } => vec![OrchestratorEvent::Log(
            LogEntry::error(format!("Conflict resolution failed: {}", error)),
        )],

        ParallelEvent::CleanupStarted { workspace } => vec![OrchestratorEvent::Log(
            LogEntry::info(format!("Cleaning up workspace: {}", workspace)),
        )],

        ParallelEvent::CleanupCompleted { .. } => {
            // Silent cleanup completion - no event needed
            vec![]
        }

        ParallelEvent::GroupStarted { group_id, changes } => {
            vec![OrchestratorEvent::Log(LogEntry::info(format!(
                "Starting group {} with {} change(s): {}",
                group_id,
                changes.len(),
                changes.join(", ")
            )))]
        }

        ParallelEvent::GroupCompleted { group_id } => vec![OrchestratorEvent::Log(
            LogEntry::success(format!("Group {} completed", group_id)),
        )],

        ParallelEvent::AnalysisStarted { remaining_changes } => {
            vec![OrchestratorEvent::Log(LogEntry::info(format!(
                "Analyzing {} remaining change(s)...",
                remaining_changes
            )))]
        }

        ParallelEvent::AnalysisOutput { output } => {
            vec![OrchestratorEvent::Log(LogEntry::info(format!(
                "[analyze] {}",
                output
            )))]
        }

        ParallelEvent::AnalysisCompleted { groups_found } => {
            vec![OrchestratorEvent::Log(LogEntry::info(format!(
                "Analysis complete: {} group(s) identified",
                groups_found
            )))]
        }

        ParallelEvent::ResolveOutput { output } => {
            vec![OrchestratorEvent::Log(LogEntry::info(format!(
                "[resolve] {}",
                output
            )))]
        }

        ParallelEvent::ArchiveStarted { change_id } => vec![
            OrchestratorEvent::Log(
                LogEntry::info("Archiving...".to_string()).with_change_id(&change_id),
            ),
            OrchestratorEvent::ArchiveStarted(change_id),
        ],

        ParallelEvent::ArchiveOutput { change_id, output } => {
            vec![OrchestratorEvent::Log(
                LogEntry::info(format!("[archive] {}", output)).with_change_id(&change_id),
            )]
        }

        ParallelEvent::ChangeArchived { change_id } => vec![
            OrchestratorEvent::Log(
                LogEntry::success("Archived".to_string()).with_change_id(&change_id),
            ),
            OrchestratorEvent::ChangeArchived(change_id),
        ],

        ParallelEvent::ArchiveFailed { change_id, error } => vec![
            OrchestratorEvent::Log(
                LogEntry::error(format!("Archive failed: {}", error)).with_change_id(&change_id),
            ),
            OrchestratorEvent::ProcessingError {
                id: change_id,
                error,
            },
        ],

        ParallelEvent::AllCompleted => {
            // This is handled specially in the orchestrator
            // Return empty to signal completion externally
            vec![]
        }

        ParallelEvent::Error { message } => {
            vec![OrchestratorEvent::Log(LogEntry::error(message))]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_workspace_created() {
        let event = ParallelEvent::WorkspaceCreated {
            change_id: "test-change".to_string(),
            workspace: "ws-test".to_string(),
        };

        let events = convert(event);
        assert_eq!(events.len(), 1);
        match &events[0] {
            OrchestratorEvent::Log(entry) => {
                assert!(entry.message.contains("Created workspace"));
                assert_eq!(entry.change_id, Some("test-change".to_string()));
            }
            _ => panic!("Expected Log event"),
        }
    }

    #[test]
    fn test_convert_apply_started_generates_two_events() {
        let event = ParallelEvent::ApplyStarted {
            change_id: "test-change".to_string(),
        };

        let events = convert(event);
        assert_eq!(events.len(), 2);

        // First event should be a log
        matches!(&events[0], OrchestratorEvent::Log(_));

        // Second event should be ProcessingStarted
        match &events[1] {
            OrchestratorEvent::ProcessingStarted(id) => {
                assert_eq!(id, "test-change");
            }
            _ => panic!("Expected ProcessingStarted event"),
        }
    }

    #[test]
    fn test_convert_apply_output_splits_lines() {
        let event = ParallelEvent::ApplyOutput {
            change_id: "test-change".to_string(),
            output: "line1\nline2\n\nline3".to_string(),
        };

        let events = convert(event);
        // Should have 3 events (empty line filtered out)
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_convert_cleanup_completed_silent() {
        let event = ParallelEvent::CleanupCompleted {
            workspace: "ws-test".to_string(),
        };

        let events = convert(event);
        assert!(events.is_empty());
    }

    #[test]
    fn test_convert_all_completed_empty() {
        let event = ParallelEvent::AllCompleted;
        let events = convert(event);
        assert!(events.is_empty());
    }

    #[test]
    fn test_convert_archive_failed_generates_log_and_processing_error() {
        let event = ParallelEvent::ArchiveFailed {
            change_id: "test-change".to_string(),
            error: "disk full".to_string(),
        };

        let events = convert(event);
        assert_eq!(events.len(), 2);

        // First event should be a log with error message
        match &events[0] {
            OrchestratorEvent::Log(entry) => {
                assert!(entry.message.contains("Archive failed"));
                assert!(entry.message.contains("disk full"));
                assert_eq!(entry.change_id, Some("test-change".to_string()));
            }
            _ => panic!("Expected Log event"),
        }

        // Second event should be ProcessingError
        match &events[1] {
            OrchestratorEvent::ProcessingError { id, error } => {
                assert_eq!(id, "test-change");
                assert_eq!(error, "disk full");
            }
            _ => panic!("Expected ProcessingError event"),
        }
    }
}
