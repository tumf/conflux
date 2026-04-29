//! Workspace creation and management for parallel execution.
//!
//! This module handles:
//! - Workspace creation and reuse (resumption logic)
//! - Workspace status tracking
//! - Workspace lifecycle management (create, resume, cleanup)

use crate::error::Result;
use crate::parallel::events::send_event;
use crate::parallel::ParallelEvent;
use crate::vcs::{Workspace, WorkspaceManager};
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Get or create a workspace for a change.
///
/// This function handles workspace creation/resumption logic:
/// - Checks for existing workspaces if no_resume is false
/// - Creates new workspaces when needed
/// - Sends appropriate events for workspace creation/resumption
///
/// Returns `(workspace, was_resumed)` where `was_resumed` is `true` when an
/// existing workspace was reused and `false` when a new workspace was created.
/// Callers that need to resume from a known state should use the `was_resumed`
/// flag to call [`crate::execution::state::detect_workspace_state`] and route
/// accordingly.
pub async fn get_or_create_workspace(
    workspace_manager: &mut dyn WorkspaceManager,
    change_id: &str,
    base_revision: &str,
    no_resume: bool,
    force_recreate_worktree: &HashSet<String>,
    event_tx: &Option<mpsc::Sender<ParallelEvent>>,
) -> Result<(Workspace, bool)> {
    let force_recreate = force_recreate_worktree.contains(change_id);

    // Check for existing workspace (resume scenario)
    if !no_resume && !force_recreate {
        if let Ok(Some(workspace_info)) = workspace_manager.find_existing_workspace(change_id).await
        {
            info!(
                "Resuming existing workspace for '{}' (last modified: {:?})",
                change_id, workspace_info.last_modified
            );
            if let Ok(ws) = workspace_manager.reuse_workspace(&workspace_info).await {
                send_event(
                    event_tx,
                    ParallelEvent::WorkspaceResumed {
                        change_id: change_id.to_string(),
                        workspace: ws.name.clone(),
                    },
                )
                .await;
                return Ok((ws, true));
            }
        }
    }

    if force_recreate {
        if let Ok(Some(workspace_info)) = workspace_manager.find_existing_workspace(change_id).await
        {
            info!(
                "Dependency resolved for '{}': cleaning up stale workspace '{}' before fresh recreation",
                change_id, workspace_info.workspace_name
            );
            if let Err(err) = workspace_manager
                .cleanup_workspace(&workspace_info.workspace_name)
                .await
            {
                warn!(
                    "Failed to cleanup stale workspace '{}' for '{}': {}",
                    workspace_info.workspace_name, change_id, err
                );
            }
        }
    }

    // Create new workspace
    let ws = workspace_manager
        .create_workspace(change_id, Some(base_revision))
        .await?;

    send_event(
        event_tx,
        ParallelEvent::WorkspaceCreated {
            change_id: change_id.to_string(),
            workspace: ws.name.clone(),
        },
    )
    .await;

    Ok((ws, false))
}
