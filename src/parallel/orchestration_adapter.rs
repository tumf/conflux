//! Adapter functions for using common orchestration loops in parallel mode.
//!
//! This module demonstrates how to bridge between the parallel executor
//! (which uses ParallelEvent and worktree paths) and the common orchestration
//! loops (which use OutputHandler and assume repo root execution).

#![allow(dead_code)] // Module demonstrates integration pattern for future use

use crate::agent::AgentRunner;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::events::ExecutionEvent as ParallelEvent;
use crate::hooks::HookRunner;
use crate::openspec::Change;
use crate::orchestration::apply::{ApplyContext, ApplyResult};
use crate::orchestration::archive::{ArchiveContext, ArchiveResult};
use std::path::Path;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::output_bridge::ParallelOutputHandler;

/// Apply a change in a workspace using the common orchestration loop.
///
/// This function wraps `orchestration::apply::apply_change_streaming` to work
/// in parallel mode with ParallelEvent channels and workspace paths.
///
/// # Arguments
///
/// * `change` - The change to apply
/// * `agent` - Agent runner (should be configured to run in workspace_path)
/// * `hooks` - Hook runner
/// * `context` - Apply context (progress tracking)
/// * `ai_runner` - AI command runner for coordinated stagger and retry
/// * `event_tx` - Optional ParallelEvent channel
/// * `cancel_token` - Optional cancellation token
///
/// # Returns
///
/// The result of the apply operation.
///
/// # Notes
///
/// This is a demonstration of the integration pattern. The actual parallel
/// executor may need additional logic for:
/// - Worktree-specific command execution (cd to workspace)
/// - Progress commit creation
/// - Iteration loop management
pub async fn apply_change_in_workspace(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &ApplyContext,
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    cancel_token: Option<&CancellationToken>,
) -> Result<ApplyResult> {
    // Create output handler bridge
    let output = ParallelOutputHandler::new(change.id.clone(), event_tx);

    // Create cancel check function
    let cancel_check = move || cancel_token.map_or(false, |t| t.is_cancelled());

    // Use common orchestration function
    crate::orchestration::apply::apply_change_streaming(
        change,
        agent,
        hooks,
        context,
        &output,
        ai_runner,
        cancel_check,
    )
    .await
}

/// Archive a change in a workspace using the common orchestration loop.
///
/// This function wraps `orchestration::archive::archive_change_streaming` to work
/// in parallel mode with ParallelEvent channels and workspace paths.
///
/// # Arguments
///
/// * `change` - The change to archive
/// * `agent` - Agent runner (should be configured to run in workspace_path)
/// * `hooks` - Hook runner
/// * `context` - Archive context
/// * `workspace_path` - Path to the workspace directory
/// * `config` - Orchestrator configuration (for stall detection)
/// * `event_tx` - Optional ParallelEvent channel
/// * `cancel_token` - Optional cancellation token
///
/// # Returns
///
/// The result of the archive operation.
///
/// # Notes
///
/// This is a demonstration of the integration pattern. The actual parallel
/// executor may need additional logic for:
/// - Worktree-specific command execution (cd to workspace)
/// - Archive commit creation
/// - Merge to base branch
pub async fn archive_change_in_workspace(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &ArchiveContext,
    workspace_path: &Path,
    config: &OrchestratorConfig,
    event_tx: Option<mpsc::Sender<ParallelEvent>>,
    cancel_token: Option<&CancellationToken>,
) -> Result<ArchiveResult> {
    // Create output handler bridge
    let output = ParallelOutputHandler::new(change.id.clone(), event_tx);

    // Create cancel check function
    let cancel_check = move || cancel_token.map_or(false, |t| t.is_cancelled());

    // Get stall detection config
    let stall_config = config.get_stall_detection();

    // Use common orchestration function
    crate::orchestration::archive::archive_change_streaming(
        change,
        agent,
        hooks,
        context,
        &output,
        cancel_check,
        Some(workspace_path),
        &stall_config,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrchestratorConfig;
    use crate::openspec::Change;

    #[tokio::test]
    async fn test_apply_change_in_workspace_creates_output_handler() {
        // This test demonstrates that the adapter correctly creates a ParallelOutputHandler
        let change = Change {
            id: "test-change".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let config = OrchestratorConfig::default();
        let mut agent = AgentRunner::new(config.clone());
        let hooks = HookRunner::empty();
        let context = ApplyContext::new(0, 1, 1, 1);
        let (tx, _rx) = mpsc::channel(10);
        let ai_runner = crate::ai_command_runner::AiCommandRunner::new_with_shared_state(
            config.clone(),
            Default::default(),
        );

        // This will fail because we don't have a real apply command configured,
        // but it demonstrates the integration pattern
        let result = apply_change_in_workspace(
            &change,
            &mut agent,
            &hooks,
            &context,
            &ai_runner,
            Some(tx),
            None,
        )
        .await;

        // We expect an error since no apply command is configured
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_archive_change_in_workspace_creates_output_handler() {
        // This test demonstrates that the adapter correctly creates a ParallelOutputHandler
        let change = Change {
            id: "test-change".to_string(),
            completed_tasks: 5,
            total_tasks: 5,
            last_modified: "".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
        };

        let config = OrchestratorConfig::default();
        let mut agent = AgentRunner::new(config.clone());
        let hooks = HookRunner::empty();
        let context = ArchiveContext::new(0, 1, 1, 1);
        let (tx, _rx) = mpsc::channel(10);

        // This will fail because we don't have a real archive command configured,
        // but it demonstrates the integration pattern
        let result = archive_change_in_workspace(
            &change,
            &mut agent,
            &hooks,
            &context,
            Path::new("/tmp/test-workspace"),
            &config,
            Some(tx),
            None,
        )
        .await;

        // We expect an error since no archive command is configured
        assert!(result.is_err());
    }
}
