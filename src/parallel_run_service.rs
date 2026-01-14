//! Shared parallel execution service for CLI and TUI modes.
//!
//! This module provides a unified service for running parallel execution
//! that can be used by both CLI and TUI orchestrators, eliminating
//! code duplication between the two modes.

use crate::agent::AgentRunner;
use crate::analyzer::{ParallelGroup, ParallelizationAnalyzer};
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::openspec::Change;
use crate::parallel::{ParallelEvent, ParallelExecutor};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Service for parallel execution of changes.
///
/// This service encapsulates the shared logic between CLI and TUI
/// parallel execution modes, including:
/// - Git availability checking
/// - Dependency-based grouping
/// - ParallelExecutor coordination
pub struct ParallelRunService {
    /// Configuration for the orchestrator
    config: OrchestratorConfig,
    /// Repository root directory
    repo_root: PathBuf,
    /// Disable automatic workspace resume (always create new workspaces)
    no_resume: bool,
}

impl ParallelRunService {
    /// Create a new parallel run service
    pub fn new(repo_root: PathBuf, config: OrchestratorConfig) -> Self {
        Self {
            config,
            repo_root,
            no_resume: false,
        }
    }

    /// Set whether to disable automatic workspace resume.
    ///
    /// When `no_resume` is true, existing workspaces are always deleted
    /// and new ones are created. When false (default), existing workspaces
    /// are reused to resume interrupted work.
    pub fn set_no_resume(&mut self, no_resume: bool) {
        self.no_resume = no_resume;
    }

    /// Check if git is available for parallel execution
    pub async fn check_vcs_available(&self) -> Result<bool> {
        Ok(crate::cli::check_parallel_available())
    }

    async fn filter_committed_changes(
        &self,
        changes: Vec<Change>,
    ) -> Result<(Vec<Change>, Vec<String>)> {
        let committed_change_ids: HashSet<String> =
            match crate::vcs::git::commands::list_changes_in_head(&self.repo_root).await {
                Ok(ids) => ids.into_iter().collect(),
                Err(err) => {
                    warn!(
                        "Failed to load committed change snapshot; assuming all changes are committed: {}",
                        err
                    );
                    return Ok((changes, Vec::new()));
                }
            };

        let mut committed = Vec::new();
        let mut skipped = Vec::new();

        for change in changes {
            if committed_change_ids.contains(&change.id) {
                committed.push(change);
            } else {
                skipped.push(change.id);
            }
        }

        skipped.sort();
        Ok((committed, skipped))
    }

    /// Run parallel execution with an event callback
    ///
    /// The event_handler receives ParallelEvents as they occur during execution.
    /// Returns the execution result.
    pub async fn run_parallel<F>(&self, changes: Vec<Change>, event_handler: F) -> Result<()>
    where
        F: Fn(ParallelEvent) + Send + Sync + 'static,
    {
        let (changes, skipped) = self.filter_committed_changes(changes).await?;

        if !skipped.is_empty() {
            let message = format!(
                "Skipping uncommitted changes in parallel mode: {}",
                skipped.join(", ")
            );
            warn!("{}", message);
            event_handler(ParallelEvent::Warning {
                title: "Uncommitted changes skipped".to_string(),
                message,
            });
        }

        if changes.is_empty() {
            info!("No committed changes available for parallel execution");
            return Ok(());
        }

        info!("Starting parallel execution for {} changes", changes.len());

        // Group changes - try LLM analysis first, fall back to declarative dependencies
        let groups = self.analyze_and_group(&changes).await;
        info!("Created {} groups for parallel execution", groups.len());

        // Create event channel
        let (event_tx, mut event_rx) = mpsc::channel::<ParallelEvent>(100);

        // Spawn event forwarding task
        let forward_handle = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let is_completed =
                    matches!(event, ParallelEvent::AllCompleted | ParallelEvent::Stopped);
                event_handler(event);
                if is_completed {
                    break;
                }
            }
        });

        // Create and run executor
        let mut executor =
            ParallelExecutor::new(self.repo_root.clone(), self.config.clone(), Some(event_tx));
        executor.set_no_resume(self.no_resume);

        let result = executor.execute_groups(groups).await;

        // Wait for event forwarding to complete
        let _ = forward_handle.await;

        result
    }

    /// Run parallel execution with an mpsc sender for events
    ///
    /// This variant is useful when integrating with existing channel-based
    /// event systems (e.g., TUI).
    ///
    /// Uses dynamic re-analysis: after each group completes, the remaining changes
    /// are re-analyzed to determine the next group.
    pub async fn run_parallel_with_channel(
        &self,
        changes: Vec<Change>,
        event_tx: mpsc::Sender<ParallelEvent>,
        cancel_token: Option<CancellationToken>,
    ) -> Result<()> {
        let (changes, skipped) = self.filter_committed_changes(changes).await?;

        if !skipped.is_empty() {
            let message = format!(
                "Skipping uncommitted changes in parallel mode: {}",
                skipped.join(", ")
            );
            warn!("{}", message);
            let _ = event_tx
                .send(ParallelEvent::Warning {
                    title: "Uncommitted changes skipped".to_string(),
                    message,
                })
                .await;
        }

        if changes.is_empty() {
            info!("No committed changes available for parallel execution");
            return Ok(());
        }

        info!(
            "Starting parallel execution with re-analysis for {} changes",
            changes.len()
        );

        // Create executor with the provided channel
        let mut executor = ParallelExecutor::new(
            self.repo_root.clone(),
            self.config.clone(),
            Some(event_tx.clone()),
        );
        executor.set_no_resume(self.no_resume);
        if let Some(token) = cancel_token {
            executor.set_cancel_token(token);
        }

        // Clone config for the analyzer closure
        let config = self.config.clone();
        let repo_root = self.repo_root.clone();

        // Use execute_with_reanalysis to re-analyze after each group
        executor
            .execute_with_reanalysis(changes, move |remaining| {
                let config = config.clone();
                let repo_root = repo_root.clone();
                let event_tx = event_tx.clone();
                Box::pin(async move {
                    let service = ParallelRunService::new(repo_root, config);
                    service
                        .analyze_and_group_with_sender(remaining, Some(&event_tx))
                        .await
                })
            })
            .await
    }

    /// Analyze changes and group them for parallel execution (public API).
    ///
    /// If `use_llm_analysis` is enabled (default), uses LLM to analyze dependencies.
    /// Otherwise, runs all changes in parallel (no dependency inference).
    pub async fn analyze_and_group_public(&self, changes: &[Change]) -> Vec<ParallelGroup> {
        self.analyze_and_group(changes).await
    }

    /// Analyze changes and group them for parallel execution.
    ///
    /// If `use_llm_analysis` is enabled (default), uses LLM to analyze dependencies.
    /// Otherwise, runs all changes in parallel (no dependency inference).
    async fn analyze_and_group(&self, changes: &[Change]) -> Vec<ParallelGroup> {
        self.analyze_and_group_with_sender(changes, None).await
    }

    /// Analyze changes and group them for parallel execution with optional event sender.
    ///
    /// If `use_llm_analysis` is enabled (default), uses LLM to analyze dependencies.
    /// Otherwise, runs all changes in parallel (no dependency inference).
    /// When a sender is provided, AnalysisOutput events are sent for streaming output.
    async fn analyze_and_group_with_sender(
        &self,
        changes: &[Change],
        event_tx: Option<&mpsc::Sender<ParallelEvent>>,
    ) -> Vec<ParallelGroup> {
        // Check if LLM analysis is enabled (default: true)
        if self.config.use_llm_analysis() {
            info!("Using LLM analysis for parallelization (analyze_command)");
            match self.analyze_with_llm_streaming(changes, event_tx).await {
                Ok(groups) => {
                    info!("LLM analysis successful: {} groups", groups.len());
                    return groups;
                }
                Err(e) => {
                    error!("LLM analysis failed: {}", e);
                    warn!(
                        "Falling back to running all changes in parallel (no dependency analysis)"
                    );
                }
            }
        } else {
            info!("LLM analysis disabled, running all changes in parallel");
        }

        // Fall back: run all changes in a single parallel group
        Self::all_parallel(changes)
    }

    /// Create a single group with all changes (no dependencies, full parallelism)
    fn all_parallel(changes: &[Change]) -> Vec<ParallelGroup> {
        if changes.is_empty() {
            return Vec::new();
        }

        vec![ParallelGroup {
            id: 1,
            changes: changes.iter().map(|c| c.id.clone()).collect(),
            depends_on: Vec::new(),
        }]
    }

    /// Analyze changes using LLM (analyze_command) with streaming output
    async fn analyze_with_llm_streaming(
        &self,
        changes: &[Change],
        event_tx: Option<&mpsc::Sender<ParallelEvent>>,
    ) -> Result<Vec<ParallelGroup>> {
        let agent = AgentRunner::new(self.config.clone());
        let analyzer = ParallelizationAnalyzer::new(agent);

        if let Some(tx) = event_tx {
            let tx = tx.clone();
            analyzer
                .analyze_groups_with_callback(changes, move |output| {
                    let _ = tx.try_send(ParallelEvent::AnalysisOutput {
                        output: output.clone(),
                    });
                })
                .await
        } else {
            analyzer.analyze_groups(changes).await
        }
    }

    /// Group changes by their declared dependencies (deterministic, no LLM)
    ///
    /// Returns groups in topological order where:
    /// - Group 1: Changes with no dependencies
    /// - Group 2: Changes that depend only on Group 1 changes
    /// - And so on...
    #[allow(dead_code)]
    pub fn group_by_dependencies(changes: &[Change]) -> Vec<ParallelGroup> {
        if changes.is_empty() {
            return Vec::new();
        }

        // Build lookup maps with owned strings to avoid lifetime issues
        let change_ids: HashSet<String> = changes.iter().map(|c| c.id.clone()).collect();
        let mut remaining: HashSet<String> = change_ids.clone();
        let mut completed_changes: HashSet<String> = HashSet::new();

        // Map from change_id to its dependencies (filtered to only include changes in our set)
        let deps_map: HashMap<String, Vec<String>> = changes
            .iter()
            .map(|c| {
                let deps: Vec<String> = c
                    .dependencies
                    .iter()
                    .filter(|d| change_ids.contains(*d))
                    .cloned()
                    .collect();
                (c.id.clone(), deps)
            })
            .collect();

        let mut groups: Vec<ParallelGroup> = Vec::new();
        let mut group_id = 1u32;

        // Iteratively find changes whose dependencies are all complete
        while !remaining.is_empty() {
            let mut current_group: Vec<String> = Vec::new();

            for change_id in &remaining {
                let deps = deps_map.get(change_id).map(|d| d.as_slice()).unwrap_or(&[]);
                // A change can be in this group if all its dependencies are completed
                if deps.iter().all(|d| completed_changes.contains(d)) {
                    current_group.push(change_id.clone());
                }
            }

            if current_group.is_empty() {
                // Circular dependency or missing dependency - add remaining changes to last group
                warn!(
                    "Unable to resolve dependencies for: {:?}",
                    remaining.iter().collect::<Vec<_>>()
                );
                current_group = remaining.iter().cloned().collect();
            }

            // Calculate depends_on (previous group if any)
            let depends_on = if group_id > 1 {
                vec![group_id - 1]
            } else {
                Vec::new()
            };

            // Remove completed changes from remaining
            for change_id in &current_group {
                remaining.remove(change_id);
                completed_changes.insert(change_id.clone());
            }

            // Sort for deterministic output
            current_group.sort();

            groups.push(ParallelGroup {
                id: group_id,
                changes: current_group,
                depends_on,
            });

            group_id += 1;
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::process::Command;

    fn create_test_change(id: &str, dependencies: Vec<&str>) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "1m ago".to_string(),
            is_approved: true,
            dependencies: dependencies.into_iter().map(String::from).collect(),
        }
    }

    async fn init_git_repo(temp_dir: &TempDir) -> bool {
        let init_result = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        let init_ok = init_result
            .as_ref()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !init_ok {
            return false;
        }

        let _ = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        true
    }

    #[test]
    fn test_group_by_dependencies_empty() {
        let changes: Vec<Change> = vec![];
        let groups = ParallelRunService::group_by_dependencies(&changes);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_group_by_dependencies_no_deps() {
        let changes = vec![
            create_test_change("a", vec![]),
            create_test_change("b", vec![]),
            create_test_change("c", vec![]),
        ];

        let groups = ParallelRunService::group_by_dependencies(&changes);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].changes.len(), 3);
        assert!(groups[0].depends_on.is_empty());
    }

    #[test]
    fn test_group_by_dependencies_linear() {
        let changes = vec![
            create_test_change("a", vec![]),
            create_test_change("b", vec!["a"]),
            create_test_change("c", vec!["b"]),
        ];

        let groups = ParallelRunService::group_by_dependencies(&changes);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].changes, vec!["a"]);
        assert_eq!(groups[1].changes, vec!["b"]);
        assert_eq!(groups[2].changes, vec!["c"]);
    }

    #[test]
    fn test_group_by_dependencies_diamond() {
        // a -> b, c -> d (diamond pattern)
        let changes = vec![
            create_test_change("a", vec![]),
            create_test_change("b", vec!["a"]),
            create_test_change("c", vec!["a"]),
            create_test_change("d", vec!["b", "c"]),
        ];

        let groups = ParallelRunService::group_by_dependencies(&changes);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].changes, vec!["a"]);
        // b and c should be in the same group
        assert!(groups[1].changes.contains(&"b".to_string()));
        assert!(groups[1].changes.contains(&"c".to_string()));
        assert_eq!(groups[2].changes, vec!["d"]);
    }

    #[test]
    fn test_group_by_dependencies_filters_external() {
        // b depends on "external" which is not in our change set
        let changes = vec![
            create_test_change("a", vec![]),
            create_test_change("b", vec!["external"]),
        ];

        let groups = ParallelRunService::group_by_dependencies(&changes);

        // Both should be in group 1 since "external" is filtered out
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].changes.len(), 2);
    }

    #[tokio::test]
    async fn test_filter_committed_changes_skips_uncommitted() {
        let temp_dir = TempDir::new().expect("tempdir");
        if !init_git_repo(&temp_dir).await {
            return;
        }

        let base_dir = temp_dir.path().join("openspec/changes");
        std::fs::create_dir_all(base_dir.join("change-a")).unwrap();
        std::fs::write(base_dir.join("change-a/proposal.md"), "test").unwrap();

        let _ = Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .await;
        let _ = Command::new("git")
            .args(["commit", "-m", "add change-a"])
            .current_dir(temp_dir.path())
            .output()
            .await;

        std::fs::create_dir_all(base_dir.join("change-b")).unwrap();
        std::fs::write(base_dir.join("change-b/proposal.md"), "test").unwrap();

        let service =
            ParallelRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let changes = vec![
            create_test_change("change-a", vec![]),
            create_test_change("change-b", vec![]),
        ];

        let (committed, skipped) = service
            .filter_committed_changes(changes)
            .await
            .expect("filter changes");

        let committed_ids: Vec<String> = committed.into_iter().map(|change| change.id).collect();
        assert_eq!(committed_ids, vec!["change-a".to_string()]);
        assert_eq!(skipped, vec!["change-b".to_string()]);
    }
}
