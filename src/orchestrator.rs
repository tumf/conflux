use crate::agent::AgentRunner;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::{self, Change};
use crate::progress::ProgressDisplay;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

pub struct Orchestrator {
    agent: AgentRunner,
    openspec_cmd: String,
    progress: Option<ProgressDisplay>,
    target_change: Option<String>,
    /// Snapshot of change IDs captured at run start.
    /// Only changes present in this snapshot will be processed during the run.
    /// This prevents mid-run proposals from being processed before they are ready.
    initial_change_ids: Option<HashSet<String>>,
    /// Hook runner for executing hooks at various stages
    hooks: HookRunner,
    /// Whether the first apply has been executed (for on_first_apply hook)
    first_apply_executed: bool,
    /// Current iteration number
    iteration: u32,
    /// Previous queue size (for on_queue_change detection)
    prev_queue_size: Option<usize>,
}

impl Orchestrator {
    /// Create a new orchestrator with optional custom config path
    pub fn new(
        openspec_cmd: &str,
        target_change: Option<String>,
        config_path: Option<PathBuf>,
    ) -> Result<Self> {
        let config = OrchestratorConfig::load(config_path.as_deref())?;
        let hooks = HookRunner::new(config.get_hooks());
        let agent = AgentRunner::new(config);

        Ok(Self {
            agent,
            openspec_cmd: openspec_cmd.to_string(),
            progress: None,
            target_change,
            initial_change_ids: None,
            hooks,
            first_apply_executed: false,
            iteration: 0,
            prev_queue_size: None,
        })
    }

    /// Create a new orchestrator with explicit configuration (for testing)
    #[cfg(test)]
    pub fn with_config(
        openspec_cmd: &str,
        target_change: Option<String>,
        config: OrchestratorConfig,
    ) -> Result<Self> {
        let hooks = HookRunner::new(config.get_hooks());
        let agent = AgentRunner::new(config);

        Ok(Self {
            agent,
            openspec_cmd: openspec_cmd.to_string(),
            progress: None,
            target_change,
            initial_change_ids: None,
            hooks,
            first_apply_executed: false,
            iteration: 0,
            prev_queue_size: None,
        })
    }

    /// Run the orchestration loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting orchestration loop");

        // Capture initial snapshot of change IDs at run start.
        // Only changes present at this point will be processed during the run.
        // This prevents mid-run proposals from being processed before they are ready.
        let initial_changes = openspec::list_changes(&self.openspec_cmd).await?;

        if initial_changes.is_empty() {
            info!("No changes found");
            return Ok(());
        }

        // Store snapshot of change IDs
        let snapshot_ids: HashSet<String> = initial_changes.iter().map(|c| c.id.clone()).collect();
        info!(
            "Captured snapshot of {} changes: {:?}",
            snapshot_ids.len(),
            snapshot_ids
        );
        self.initial_change_ids = Some(snapshot_ids.clone());

        // Initialize progress display
        self.progress = Some(ProgressDisplay::new(initial_changes.len()));

        let total_changes = initial_changes.len();

        // Run on_start hook
        let start_context = HookContext::new(0, total_changes, total_changes, false);
        self.hooks
            .run_hook(HookType::OnStart, &start_context)
            .await?;

        let finish_status;

        loop {
            // Increment iteration counter
            self.iteration += 1;

            // List all changes from openspec (to get updated progress)
            let changes = openspec::list_changes(&self.openspec_cmd).await?;

            // Filter to only include changes from initial snapshot
            let snapshot_changes = self.filter_to_snapshot(&changes);

            // Log any new changes that appeared after run started
            self.log_new_changes(&changes);

            let queue_size = snapshot_changes.len();

            // Check for queue change and run hook if needed
            if let Some(prev_size) = self.prev_queue_size {
                if prev_size != queue_size {
                    let queue_context =
                        HookContext::new(self.iteration, total_changes, queue_size, false);
                    self.hooks
                        .run_hook(HookType::OnQueueChange, &queue_context)
                        .await?;
                }
            }
            self.prev_queue_size = Some(queue_size);

            if snapshot_changes.is_empty() {
                info!("All changes from initial snapshot processed");
                if let Some(progress) = &mut self.progress {
                    progress.complete_all();
                }
                finish_status = "completed";
                break;
            }

            // Filter by target change if specified
            let filtered_changes: Vec<Change> = if let Some(target) = &self.target_change {
                snapshot_changes
                    .into_iter()
                    .filter(|c| &c.id == target)
                    .collect()
            } else {
                snapshot_changes
            };

            if filtered_changes.is_empty() {
                if self.target_change.is_some() {
                    info!("Target change not found or already processed");
                }
                info!("All changes processed");
                if let Some(progress) = &mut self.progress {
                    progress.complete_all();
                }
                finish_status = "completed";
                break;
            }

            // Run on_iteration_start hook
            let iter_start_context =
                HookContext::new(self.iteration, total_changes, queue_size, false);
            self.hooks
                .run_hook(HookType::OnIterationStart, &iter_start_context)
                .await?;

            // 2. Select next change to process
            let next = self.select_next_change(&filtered_changes).await?;
            info!("Selected change: {}", next.id);

            if let Some(progress) = &mut self.progress {
                progress.update_change(&next);
            }

            // 3. Process the change
            if next.is_complete() {
                // Archive completed change
                info!("Change {} is complete, archiving...", next.id);

                // Run on_change_complete hook (task 100%)
                let complete_context =
                    HookContext::new(self.iteration, total_changes, queue_size, false).with_change(
                        &next.id,
                        next.completed_tasks,
                        next.total_tasks,
                    );
                self.hooks
                    .run_hook(HookType::OnChangeComplete, &complete_context)
                    .await?;

                // Run pre_archive hook
                let pre_archive_context =
                    HookContext::new(self.iteration, total_changes, queue_size, false).with_change(
                        &next.id,
                        next.completed_tasks,
                        next.total_tasks,
                    );
                self.hooks
                    .run_hook(HookType::PreArchive, &pre_archive_context)
                    .await?;

                match self.archive_change(&next).await {
                    Ok(_) => {
                        // Run post_archive hook
                        let post_archive_context =
                            HookContext::new(self.iteration, total_changes, queue_size - 1, false)
                                .with_change(&next.id, next.completed_tasks, next.total_tasks);
                        self.hooks
                            .run_hook(HookType::PostArchive, &post_archive_context)
                            .await?;

                        if let Some(progress) = &mut self.progress {
                            progress.archive_change(&next.id);
                        }
                        // If targeting specific change and it's done, stop
                        if self.target_change.as_ref() == Some(&next.id) {
                            finish_status = "completed";
                            break;
                        }
                    }
                    Err(e) => {
                        // Run on_error hook
                        let error_context =
                            HookContext::new(self.iteration, total_changes, queue_size, false)
                                .with_change(&next.id, next.completed_tasks, next.total_tasks)
                                .with_error(&e.to_string());
                        let _ = self.hooks.run_hook(HookType::OnError, &error_context).await;

                        error!("Archive failed for {}: {}", next.id, e);
                        if let Some(progress) = &mut self.progress {
                            progress.error(&format!("Archive failed: {}", next.id));
                        }
                        return Err(e);
                    }
                }
            } else {
                // Apply change
                info!("Applying change: {}", next.id);

                // Run on_first_apply hook if this is the first apply
                if !self.first_apply_executed {
                    let first_apply_context =
                        HookContext::new(self.iteration, total_changes, queue_size, false)
                            .with_change(&next.id, next.completed_tasks, next.total_tasks);
                    self.hooks
                        .run_hook(HookType::OnFirstApply, &first_apply_context)
                        .await?;
                    self.first_apply_executed = true;
                }

                // Run pre_apply hook
                let pre_apply_context =
                    HookContext::new(self.iteration, total_changes, queue_size, false).with_change(
                        &next.id,
                        next.completed_tasks,
                        next.total_tasks,
                    );
                self.hooks
                    .run_hook(HookType::PreApply, &pre_apply_context)
                    .await?;

                match self.apply_change(&next).await {
                    Ok(_) => {
                        // Run post_apply hook
                        let post_apply_context =
                            HookContext::new(self.iteration, total_changes, queue_size, false)
                                .with_change(&next.id, next.completed_tasks, next.total_tasks);
                        self.hooks
                            .run_hook(HookType::PostApply, &post_apply_context)
                            .await?;

                        if let Some(progress) = &mut self.progress {
                            progress.complete_change(&next.id);
                        }
                    }
                    Err(e) => {
                        // Run on_error hook
                        let error_context =
                            HookContext::new(self.iteration, total_changes, queue_size, false)
                                .with_change(&next.id, next.completed_tasks, next.total_tasks)
                                .with_error(&e.to_string());
                        let _ = self.hooks.run_hook(HookType::OnError, &error_context).await;

                        error!("Apply failed for {}: {}", next.id, e);
                        if let Some(progress) = &mut self.progress {
                            progress.error(&format!("Apply failed: {}", next.id));
                        }
                        return Err(e);
                    }
                }
            }

            // Run on_iteration_end hook
            let iter_end_context =
                HookContext::new(self.iteration, total_changes, queue_size, false);
            self.hooks
                .run_hook(HookType::OnIterationEnd, &iter_end_context)
                .await?;
        }

        // Run on_finish hook
        let finish_context =
            HookContext::new(self.iteration, total_changes, 0, false).with_status(finish_status);
        self.hooks
            .run_hook(HookType::OnFinish, &finish_context)
            .await?;

        info!("Orchestration completed");
        Ok(())
    }

    /// Select the next change to process
    async fn select_next_change(&self, changes: &[Change]) -> Result<Change> {
        // Priority 1: Complete changes (ready for archive)
        if let Some(complete) = changes.iter().find(|c| c.is_complete()) {
            info!("Found complete change: {}", complete.id);
            return Ok(complete.clone());
        }

        // Priority 2: Use LLM for dependency analysis
        match self.analyze_with_llm(changes).await {
            Ok(selected) => {
                info!("LLM selected: {}", selected.id);
                return Ok(selected);
            }
            Err(e) => {
                warn!("LLM analysis failed, using fallback: {}", e);
            }
        }

        // Priority 3: Fallback - highest progress
        let selected = changes
            .iter()
            .max_by(|a, b| {
                a.progress_percent()
                    .partial_cmp(&b.progress_percent())
                    .unwrap()
            })
            .cloned()
            .ok_or(OrchestratorError::NoChanges)?;

        info!(
            "Fallback selected: {} ({:.1}%)",
            selected.id,
            selected.progress_percent()
        );
        Ok(selected)
    }

    /// Analyze dependencies using LLM
    async fn analyze_with_llm(&self, changes: &[Change]) -> Result<Change> {
        let prompt = self.build_analysis_prompt(changes);
        let response = self.agent.analyze_dependencies(&prompt).await?;

        // Parse the response to extract change ID
        for change in changes {
            if response.contains(&change.id) {
                return Ok(change.clone());
            }
        }

        Err(OrchestratorError::Parse(
            "Could not parse LLM response".to_string(),
        ))
    }

    /// Build prompt for LLM dependency analysis
    fn build_analysis_prompt(&self, changes: &[Change]) -> String {
        let change_list = changes
            .iter()
            .map(|c| {
                format!(
                    "- {} ({}/{} tasks, {:.1}%)",
                    c.id,
                    c.completed_tasks,
                    c.total_tasks,
                    c.progress_percent()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"以下のOpenSpec変更から、次に実行すべきものを1つ選んでください。

変更一覧:
{}

選択基準:
1. 依存関係がない、または依存先が完了しているもの
2. 進捗が進んでいるもの（継続性）
3. 名前から推測される依存関係を考慮

回答は変更IDのみを1行で出力してください。
"#,
            change_list
        )
    }

    /// Apply a change using the configured agent
    async fn apply_change(&self, change: &Change) -> Result<()> {
        info!("Applying change: {}", change.id);

        let status = self.agent.run_apply(&change.id).await?;

        if !status.success() {
            return Err(OrchestratorError::AgentCommand(format!(
                "Apply command failed with exit code: {:?}",
                status.code()
            )));
        }

        info!("Successfully applied: {}", change.id);
        Ok(())
    }

    /// Archive a change using the configured agent
    async fn archive_change(&self, change: &Change) -> Result<()> {
        info!("Archiving change: {}", change.id);

        let status = self.agent.run_archive(&change.id).await?;

        if !status.success() {
            return Err(OrchestratorError::AgentCommand(format!(
                "Archive command failed with exit code: {:?}",
                status.code()
            )));
        }

        info!("Successfully archived: {}", change.id);
        Ok(())
    }

    /// Filter changes to only include those present in the initial snapshot.
    /// Returns an empty vector if no snapshot was captured.
    fn filter_to_snapshot(&self, changes: &[Change]) -> Vec<Change> {
        match &self.initial_change_ids {
            Some(snapshot) => changes
                .iter()
                .filter(|c| snapshot.contains(&c.id))
                .cloned()
                .collect(),
            None => changes.to_vec(),
        }
    }

    /// Log any changes that were not present in the initial snapshot.
    /// These are new changes added after the run started and will be ignored.
    fn log_new_changes(&self, changes: &[Change]) {
        if let Some(snapshot) = &self.initial_change_ids {
            for change in changes {
                if !snapshot.contains(&change.id) {
                    debug!(
                        "Ignoring new change '{}' added after run started (will be processed on next run)",
                        change.id
                    );
                }
            }
        }
    }

    /// Set initial change IDs snapshot directly (for testing purposes)
    #[cfg(test)]
    pub fn set_initial_change_ids(&mut self, ids: HashSet<String>) {
        self.initial_change_ids = Some(ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
        }
    }

    #[test]
    fn test_filter_to_snapshot_filters_new_changes() {
        // Create orchestrator with mock config (won't be used in this test)
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config("mock_openspec", None, config).unwrap();

        // Set up snapshot with only change-a and change-b
        let snapshot: HashSet<String> = ["change-a", "change-b"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        orchestrator.set_initial_change_ids(snapshot);

        // Create changes list including new change-c
        let all_changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 3, 5),
            create_test_change("change-c", 0, 3), // New change, not in snapshot
        ];

        // Filter to snapshot
        let filtered = orchestrator.filter_to_snapshot(&all_changes);

        // Should only include change-a and change-b
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|c| c.id == "change-a"));
        assert!(filtered.iter().any(|c| c.id == "change-b"));
        assert!(!filtered.iter().any(|c| c.id == "change-c"));
    }

    #[test]
    fn test_filter_to_snapshot_returns_all_when_no_snapshot() {
        // Create orchestrator without setting snapshot
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config("mock_openspec", None, config).unwrap();

        let all_changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-b", 3, 5),
        ];

        // Should return all changes when no snapshot is set
        let filtered = orchestrator.filter_to_snapshot(&all_changes);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_to_snapshot_removes_archived_changes() {
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config("mock_openspec", None, config).unwrap();

        // Set up snapshot with change-a, change-b, change-c
        let snapshot: HashSet<String> = ["change-a", "change-b", "change-c"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        orchestrator.set_initial_change_ids(snapshot);

        // Simulate change-b being archived (no longer in list)
        let current_changes = vec![
            create_test_change("change-a", 2, 5),
            create_test_change("change-c", 1, 5),
        ];

        // Filter should only return change-a and change-c (both in snapshot and in current list)
        let filtered = orchestrator.filter_to_snapshot(&current_changes);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|c| c.id == "change-a"));
        assert!(filtered.iter().any(|c| c.id == "change-c"));
    }

    #[test]
    fn test_filter_to_snapshot_handles_empty_changes() {
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config("mock_openspec", None, config).unwrap();

        let snapshot: HashSet<String> = ["change-a"].iter().map(|s| s.to_string()).collect();
        orchestrator.set_initial_change_ids(snapshot);

        // Empty changes list
        let current_changes: Vec<Change> = vec![];

        let filtered = orchestrator.filter_to_snapshot(&current_changes);
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_snapshot_preserves_updated_progress() {
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config("mock_openspec", None, config).unwrap();

        // Set up snapshot with change-a
        let snapshot: HashSet<String> = ["change-a"].iter().map(|s| s.to_string()).collect();
        orchestrator.set_initial_change_ids(snapshot);

        // Create changes with updated progress for change-a
        let current_changes = vec![
            create_test_change("change-a", 4, 5), // Progress updated from 2/5 to 4/5
        ];

        let filtered = orchestrator.filter_to_snapshot(&current_changes);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].completed_tasks, 4); // Progress should be updated
    }
}
