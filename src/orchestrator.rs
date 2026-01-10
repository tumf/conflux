use crate::agent::AgentRunner;
use crate::analyzer::{ParallelGroup, ParallelizationAnalyzer};
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::{self, Change};
use crate::parallel_executor::ParallelExecutor;
use crate::progress::ProgressDisplay;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

pub struct Orchestrator {
    agent: AgentRunner,
    config: OrchestratorConfig,
    progress: Option<ProgressDisplay>,
    /// Target changes specified by --change option (comma-separated)
    target_changes: Option<Vec<String>>,
    /// Snapshot of change IDs captured at run start.
    /// Only changes present in this snapshot will be processed during the run.
    /// This prevents mid-run proposals from being processed before they are ready.
    initial_change_ids: Option<HashSet<String>>,
    /// Hook runner for executing hooks at various stages
    hooks: HookRunner,
    /// Current change ID being processed (for on_change_start/on_change_end detection)
    current_change_id: Option<String>,
    /// Completed change IDs (after archive, on_change_end called)
    completed_change_ids: HashSet<String>,
    /// Apply counts per change (how many times each change has been applied)
    apply_counts: HashMap<String, u32>,
    /// Number of changes processed (archived)
    changes_processed: usize,
    /// Maximum iterations limit (0 = no limit)
    max_iterations: u32,
    /// Current iteration number (for max_iterations check)
    iteration: u32,
    /// Enable parallel execution mode
    parallel: bool,
    /// Maximum concurrent workspaces for parallel execution
    max_concurrent: Option<usize>,
    /// Dry run mode (preview without execution)
    dry_run: bool,
}

impl Orchestrator {
    /// Create a new orchestrator with optional custom config path and max iterations override
    pub fn new(
        target_changes: Option<Vec<String>>,
        config_path: Option<PathBuf>,
        max_iterations_override: Option<u32>,
        parallel: bool,
        max_concurrent: Option<usize>,
        dry_run: bool,
    ) -> Result<Self> {
        let config = OrchestratorConfig::load(config_path.as_deref())?;
        let hooks = HookRunner::new(config.get_hooks());
        // CLI override takes precedence over config file value
        let max_iterations = max_iterations_override.unwrap_or_else(|| config.get_max_iterations());
        let agent = AgentRunner::new(config.clone());

        Ok(Self {
            agent,
            config,
            progress: None,
            target_changes,
            initial_change_ids: None,
            hooks,
            current_change_id: None,
            completed_change_ids: HashSet::new(),
            apply_counts: HashMap::new(),
            changes_processed: 0,
            max_iterations,
            iteration: 0,
            parallel,
            max_concurrent,
            dry_run,
        })
    }

    /// Create a new orchestrator with explicit configuration (for testing)
    #[cfg(test)]
    pub fn with_config(
        target_changes: Option<Vec<String>>,
        config: OrchestratorConfig,
    ) -> Result<Self> {
        let hooks = HookRunner::new(config.get_hooks());
        let max_iterations = config.get_max_iterations();
        let agent = AgentRunner::new(config.clone());

        Ok(Self {
            agent,
            config,
            progress: None,
            target_changes,
            initial_change_ids: None,
            hooks,
            current_change_id: None,
            completed_change_ids: HashSet::new(),
            apply_counts: HashMap::new(),
            changes_processed: 0,
            max_iterations,
            iteration: 0,
            parallel: false,
            max_concurrent: None,
            dry_run: false,
        })
    }

    /// Run the orchestration loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting orchestration loop");

        // Capture initial snapshot of change IDs at run start.
        // Only changes present at this point will be processed during the run.
        // This prevents mid-run proposals from being processed before they are ready.
        let initial_changes = openspec::list_changes_native()?;

        // Handle parallel mode with dry_run
        if self.parallel && self.dry_run {
            return self.run_parallel_dry_run(&initial_changes).await;
        }

        // Handle parallel execution mode
        if self.parallel {
            return self.run_parallel(&initial_changes).await;
        }

        if initial_changes.is_empty() {
            info!("No changes found");
            return Ok(());
        }

        // Filter by target_changes if specified (early filtering)
        // Both explicit targets and default mode require approval check
        let filtered_initial = if let Some(targets) = &self.target_changes {
            // Explicit targets specified via --change option
            let mut found = Vec::new();
            for target in targets {
                let trimmed = target.trim();
                if let Some(change) = initial_changes.iter().find(|c| c.id == trimmed) {
                    // Check approval status even for explicitly specified changes
                    if change.is_approved {
                        found.push(change.clone());
                    } else {
                        warn!(
                            "Skipping unapproved change '{}'. Approve it first with: openspec-orchestrator approve set {}",
                            trimmed, trimmed
                        );
                    }
                } else {
                    warn!("Specified change '{}' not found, skipping", trimmed);
                }
            }
            found
        } else {
            // No explicit target: filter to only approved changes
            let (approved, unapproved): (Vec<_>, Vec<_>) =
                initial_changes.into_iter().partition(|c| c.is_approved);

            // Warn about unapproved changes
            for change in &unapproved {
                warn!(
                    "Skipping unapproved change '{}'. Approve it first with: openspec-orchestrator approve set {}",
                    change.id, change.id
                );
            }

            if approved.is_empty() && !unapproved.is_empty() {
                info!(
                    "No approved changes found. {} change(s) are pending approval.",
                    unapproved.len()
                );
                return Ok(());
            }

            approved
        };

        if filtered_initial.is_empty() {
            info!("No changes found matching specified targets");
            return Ok(());
        }

        // Store snapshot of change IDs (only the filtered ones)
        let snapshot_ids: HashSet<String> = filtered_initial.iter().map(|c| c.id.clone()).collect();
        info!(
            "Captured snapshot of {} changes: {:?}",
            snapshot_ids.len(),
            snapshot_ids
        );
        self.initial_change_ids = Some(snapshot_ids.clone());

        // Initialize progress display
        self.progress = Some(ProgressDisplay::new(filtered_initial.len()));

        let total_changes = filtered_initial.len();

        // Run on_start hook
        let start_context = HookContext::new(0, total_changes, total_changes, false);
        self.hooks
            .run_hook(HookType::OnStart, &start_context)
            .await?;

        let finish_status;

        loop {
            // Increment iteration counter
            self.iteration += 1;

            // Check max iterations limit (0 = no limit)
            if self.max_iterations > 0 {
                // Log warning when approaching limit (80%)
                let warning_threshold = (self.max_iterations as f32 * 0.8) as u32;
                if self.iteration == warning_threshold {
                    warn!(
                        "Approaching max iterations: {}/{}",
                        self.iteration, self.max_iterations
                    );
                }

                // Stop if max iterations reached
                if self.iteration > self.max_iterations {
                    info!(
                        "Max iterations ({}) reached, stopping orchestration",
                        self.max_iterations
                    );
                    if let Some(progress) = &mut self.progress {
                        progress.complete_all();
                    }
                    finish_status = "iteration_limit";
                    break;
                }
            }

            // List all changes from openspec (to get updated progress)
            let changes = openspec::list_changes_native()?;

            // Filter to only include changes from initial snapshot
            let snapshot_changes = self.filter_to_snapshot(&changes);

            // Log any new changes that appeared after run started
            self.log_new_changes(&changes);

            let remaining_changes = snapshot_changes.len();

            if snapshot_changes.is_empty() {
                info!("All changes from initial snapshot processed");
                if let Some(progress) = &mut self.progress {
                    progress.complete_all();
                }
                finish_status = "completed";
                break;
            }

            // Select next change to process
            let next = self.select_next_change(&snapshot_changes).await?;
            info!("Selected change: {}", next.id);

            if let Some(progress) = &mut self.progress {
                progress.update_change(&next);
            }

            // Check if this is a new change (for on_change_start hook)
            let is_new_change = self.current_change_id.as_ref() != Some(&next.id);
            if is_new_change {
                // Run on_change_start hook
                let change_start_context = HookContext::new(
                    self.changes_processed,
                    total_changes,
                    remaining_changes,
                    false,
                )
                .with_change(&next.id, next.completed_tasks, next.total_tasks)
                .with_apply_count(0);
                self.hooks
                    .run_hook(HookType::OnChangeStart, &change_start_context)
                    .await?;
                self.current_change_id = Some(next.id.clone());
            }

            // Get current apply count for this change
            let apply_count = *self.apply_counts.get(&next.id).unwrap_or(&0);

            // Process the change
            if next.is_complete() {
                // Archive completed change
                info!("Change {} is complete, archiving...", next.id);

                // Run on_change_complete hook (task 100%)
                let complete_context = HookContext::new(
                    self.changes_processed,
                    total_changes,
                    remaining_changes,
                    false,
                )
                .with_change(&next.id, next.completed_tasks, next.total_tasks)
                .with_apply_count(apply_count);
                self.hooks
                    .run_hook(HookType::OnChangeComplete, &complete_context)
                    .await?;

                // Run pre_archive hook
                let pre_archive_context = HookContext::new(
                    self.changes_processed,
                    total_changes,
                    remaining_changes,
                    false,
                )
                .with_change(&next.id, next.completed_tasks, next.total_tasks)
                .with_apply_count(apply_count);
                self.hooks
                    .run_hook(HookType::PreArchive, &pre_archive_context)
                    .await?;

                match self.archive_change(&next).await {
                    Ok(_) => {
                        // Update changes_processed count
                        self.changes_processed += 1;
                        let new_remaining = remaining_changes - 1;

                        // Run post_archive hook
                        let post_archive_context = HookContext::new(
                            self.changes_processed,
                            total_changes,
                            new_remaining,
                            false,
                        )
                        .with_change(&next.id, next.completed_tasks, next.total_tasks)
                        .with_apply_count(apply_count);
                        self.hooks
                            .run_hook(HookType::PostArchive, &post_archive_context)
                            .await?;

                        // Run on_change_end hook
                        let change_end_context = HookContext::new(
                            self.changes_processed,
                            total_changes,
                            new_remaining,
                            false,
                        )
                        .with_change(&next.id, next.completed_tasks, next.total_tasks)
                        .with_apply_count(apply_count);
                        self.hooks
                            .run_hook(HookType::OnChangeEnd, &change_end_context)
                            .await?;

                        // Mark change as completed and clear current
                        self.completed_change_ids.insert(next.id.clone());
                        self.current_change_id = None;
                        self.apply_counts.remove(&next.id);

                        if let Some(progress) = &mut self.progress {
                            progress.archive_change(&next.id);
                        }
                    }
                    Err(e) => {
                        // Run on_error hook
                        let error_context = HookContext::new(
                            self.changes_processed,
                            total_changes,
                            remaining_changes,
                            false,
                        )
                        .with_change(&next.id, next.completed_tasks, next.total_tasks)
                        .with_apply_count(apply_count)
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

                // Increment apply count
                let new_apply_count = apply_count + 1;
                self.apply_counts.insert(next.id.clone(), new_apply_count);

                // Run pre_apply hook
                let pre_apply_context = HookContext::new(
                    self.changes_processed,
                    total_changes,
                    remaining_changes,
                    false,
                )
                .with_change(&next.id, next.completed_tasks, next.total_tasks)
                .with_apply_count(new_apply_count);
                self.hooks
                    .run_hook(HookType::PreApply, &pre_apply_context)
                    .await?;

                match self.apply_change(&next).await {
                    Ok(_) => {
                        // Run post_apply hook
                        let post_apply_context = HookContext::new(
                            self.changes_processed,
                            total_changes,
                            remaining_changes,
                            false,
                        )
                        .with_change(&next.id, next.completed_tasks, next.total_tasks)
                        .with_apply_count(new_apply_count);
                        self.hooks
                            .run_hook(HookType::PostApply, &post_apply_context)
                            .await?;

                        if let Some(progress) = &mut self.progress {
                            progress.complete_change(&next.id);
                        }
                    }
                    Err(e) => {
                        // Run on_error hook
                        let error_context = HookContext::new(
                            self.changes_processed,
                            total_changes,
                            remaining_changes,
                            false,
                        )
                        .with_change(&next.id, next.completed_tasks, next.total_tasks)
                        .with_apply_count(new_apply_count)
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
        }

        // Run on_finish hook
        let finish_context = HookContext::new(self.changes_processed, total_changes, 0, false)
            .with_status(finish_status);
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
    async fn apply_change(&mut self, change: &Change) -> Result<()> {
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
    async fn archive_change(&mut self, change: &Change) -> Result<()> {
        info!("Archiving change: {}", change.id);

        let status = self.agent.run_archive(&change.id).await?;

        if !status.success() {
            return Err(OrchestratorError::AgentCommand(format!(
                "Archive command failed with exit code: {:?}",
                status.code()
            )));
        }

        // Clear apply history for the archived change
        self.agent.clear_apply_history(&change.id);

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

    /// Run parallel mode with dry run (preview parallelization groups)
    async fn run_parallel_dry_run(&self, changes: &[Change]) -> Result<()> {
        info!("Running parallel mode dry run (preview only)");

        // Filter to approved changes only
        let approved: Vec<_> = changes.iter().filter(|c| c.is_approved).cloned().collect();

        if approved.is_empty() {
            println!("No approved changes found for parallel execution.");
            for change in changes {
                println!(
                    "  - {} (unapproved) - use: openspec-orchestrator approve set {}",
                    change.id, change.id
                );
            }
            return Ok(());
        }

        // For dry-run, use simple grouping (all changes in one group for preview)
        // This avoids LLM call overhead for preview mode
        let groups = vec![ParallelGroup {
            id: 1,
            changes: approved.iter().map(|c| c.id.clone()).collect(),
            depends_on: Vec::new(),
        }];

        // Display parallelization groups
        println!("\n=== Parallel Execution Plan (Dry Run) ===\n");
        println!("Total changes: {}", approved.len());
        println!("Parallelization groups: {}\n", groups.len());

        for group in &groups {
            println!("Group {} (can run in parallel):", group.id);
            for change_id in &group.changes {
                let change = approved.iter().find(|c| c.id == *change_id);
                if let Some(c) = change {
                    println!(
                        "  - {} ({}/{} tasks, {:.1}%)",
                        c.id,
                        c.completed_tasks,
                        c.total_tasks,
                        c.progress_percent()
                    );
                } else {
                    println!("  - {}", change_id);
                }
            }
            if !group.depends_on.is_empty() {
                println!("  (depends on group(s): {:?})", group.depends_on);
            }
            println!();
        }

        println!("Max concurrent workspaces: {}", self.max_concurrent.unwrap_or(4));
        println!("\nTo execute, run without --dry-run flag.");

        Ok(())
    }

    /// Run parallel execution mode
    async fn run_parallel(&mut self, changes: &[Change]) -> Result<()> {
        info!("Running parallel execution mode");

        // Filter to approved changes only
        let approved: Vec<_> = changes.iter().filter(|c| c.is_approved).cloned().collect();

        if approved.is_empty() {
            info!("No approved changes found for parallel execution");
            return Ok(());
        }

        // Create analyzer and analyze dependencies
        let agent = AgentRunner::new(self.config.clone());
        let analyzer = ParallelizationAnalyzer::new(agent);

        // Try LLM analysis, fall back to simple grouping on failure
        let groups = match analyzer.analyze_groups(&approved).await {
            Ok(g) => g,
            Err(e) => {
                warn!("LLM analysis failed ({}), using simple grouping (all changes in one group)", e);
                vec![ParallelGroup {
                    id: 1,
                    changes: approved.iter().map(|c| c.id.clone()).collect(),
                    depends_on: Vec::new(),
                }]
            }
        };

        info!(
            "Analyzed {} changes into {} groups",
            approved.len(),
            groups.len()
        );

        // Store snapshot of change IDs
        let snapshot_ids: HashSet<String> = approved.iter().map(|c| c.id.clone()).collect();
        self.initial_change_ids = Some(snapshot_ids);

        // Check if jj is available for true parallel execution
        let repo_root = std::env::current_dir()?;

        let mut executor = ParallelExecutor::new(repo_root, self.config.clone(), None);

        match executor.check_jj_available().await {
            Ok(true) => {
                info!("jj available, executing groups in parallel using workspaces");
                executor.execute_groups(groups).await?;
            }
            Ok(false) | Err(_) => {
                warn!("jj not available, falling back to sequential execution");
                self.run_sequential(&approved, groups).await?;
            }
        }

        Ok(())
    }

    /// Run groups sequentially (fallback when jj is not available)
    async fn run_sequential(&mut self, approved: &[Change], groups: Vec<ParallelGroup>) -> Result<()> {
        for group in groups {
            info!("Processing group {} sequentially", group.id);
            for change_id in group.changes {
                if let Some(change) = approved.iter().find(|c| c.id == change_id) {
                    info!("Processing change: {}", change.id);
                    if change.is_complete() {
                        self.archive_change(change).await?;
                    } else {
                        self.apply_change(change).await?;
                    }
                }
            }
        }
        Ok(())
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
            is_approved: false,
        }
    }

    #[test]
    fn test_filter_to_snapshot_filters_new_changes() {
        // Create orchestrator with mock config (won't be used in this test)
        let config = OrchestratorConfig::default();
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();

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
        let orchestrator = Orchestrator::with_config(None, config).unwrap();

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
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();

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
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();

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
        let mut orchestrator = Orchestrator::with_config(None, config).unwrap();

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

    #[test]
    fn test_build_analysis_prompt_format() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(None, config).unwrap();

        let changes = vec![
            create_test_change("add-feature", 2, 5),
            create_test_change("fix-bug", 4, 4),
        ];

        let prompt = orchestrator.build_analysis_prompt(&changes);

        // Verify prompt contains change IDs
        assert!(prompt.contains("add-feature"));
        assert!(prompt.contains("fix-bug"));

        // Verify prompt contains progress info
        assert!(prompt.contains("2/5 tasks"));
        assert!(prompt.contains("40.0%"));
        assert!(prompt.contains("4/4 tasks"));
        assert!(prompt.contains("100.0%"));

        // Verify prompt contains instruction header
        assert!(prompt.contains("変更一覧"));
        assert!(prompt.contains("選択基準"));
    }

    #[test]
    fn test_build_analysis_prompt_with_empty_changes() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(None, config).unwrap();

        let changes: Vec<Change> = vec![];
        let prompt = orchestrator.build_analysis_prompt(&changes);

        // Prompt should still have structure even with no changes
        assert!(prompt.contains("変更一覧"));
        assert!(prompt.contains("選択基準"));
    }

    #[test]
    fn test_build_analysis_prompt_with_single_change() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(None, config).unwrap();

        let changes = vec![create_test_change("only-change", 1, 3)];
        let prompt = orchestrator.build_analysis_prompt(&changes);

        assert!(prompt.contains("only-change"));
        assert!(prompt.contains("1/3 tasks"));
        assert!(prompt.contains("33.3%"));
    }

    #[test]
    fn test_orchestrator_creation() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(None, config).unwrap();

        assert!(orchestrator.target_changes.is_none());
        assert!(orchestrator.initial_change_ids.is_none());
        assert!(orchestrator.current_change_id.is_none());
        assert!(orchestrator.completed_change_ids.is_empty());
        assert!(orchestrator.apply_counts.is_empty());
        assert_eq!(orchestrator.changes_processed, 0);
        assert_eq!(orchestrator.iteration, 0);
    }

    #[test]
    fn test_orchestrator_with_single_target_change() {
        let config = OrchestratorConfig::default();
        let orchestrator =
            Orchestrator::with_config(Some(vec!["my-change".to_string()]), config).unwrap();

        assert_eq!(
            orchestrator.target_changes,
            Some(vec!["my-change".to_string()])
        );
    }

    #[test]
    fn test_orchestrator_with_multiple_target_changes() {
        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::with_config(
            Some(vec![
                "change-a".to_string(),
                "change-b".to_string(),
                "change-c".to_string(),
            ]),
            config,
        )
        .unwrap();

        assert_eq!(
            orchestrator.target_changes,
            Some(vec![
                "change-a".to_string(),
                "change-b".to_string(),
                "change-c".to_string()
            ])
        );
    }
}
