use crate::error::{OrchestratorError, Result};
use crate::opencode::OpenCodeRunner;
use crate::openspec::{self, Change};
use crate::progress::ProgressDisplay;
use crate::state::OrchestratorState;
use tracing::{error, info, warn};

pub struct Orchestrator {
    opencode: OpenCodeRunner,
    openspec_path: String,
    state: OrchestratorState,
    progress: Option<ProgressDisplay>,
    dry_run: bool,
    target_change: Option<String>,
}

impl Orchestrator {
    /// Create a new orchestrator
    pub fn new(
        opencode_path: &str,
        openspec_path: &str,
        dry_run: bool,
        target_change: Option<String>,
    ) -> Result<Self> {
        let state = OrchestratorState::load()?.unwrap_or_else(OrchestratorState::new);
        let opencode = OpenCodeRunner::new(opencode_path);

        Ok(Self {
            opencode,
            openspec_path: openspec_path.to_string(),
            state,
            progress: None,
            dry_run,
            target_change,
        })
    }

    /// Run the orchestration loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting orchestration loop (dry_run: {})", self.dry_run);

        loop {
            // 1. List all changes
            let changes = openspec::list_changes(&self.openspec_path).await?;

            if changes.is_empty() {
                info!("No changes found");
                if let Some(progress) = &mut self.progress {
                    progress.complete_all();
                }
                break;
            }

            // Initialize progress display on first iteration
            if self.progress.is_none() {
                self.progress = Some(ProgressDisplay::new(changes.len()));
            }

            // Filter by target change if specified
            let filtered_changes: Vec<Change> = if let Some(target) = &self.target_change {
                changes
                    .into_iter()
                    .filter(|c| &c.id == target)
                    .collect()
            } else {
                changes
            };

            if filtered_changes.is_empty() {
                if self.target_change.is_some() {
                    return Err(OrchestratorError::NoChanges);
                }
                info!("All changes processed");
                if let Some(progress) = &mut self.progress {
                    progress.complete_all();
                }
                break;
            }

            // 2. Select next change to process
            let next = self.select_next_change(&filtered_changes).await?;
            info!("Selected change: {}", next.id);

            self.state.current_change = Some(next.id.clone());
            self.state.touch();
            self.state.save()?;

            if let Some(progress) = &mut self.progress {
                progress.update_change(&next);
            }

            // 3. Process the change
            if next.is_complete() {
                // Archive completed change
                info!("Change {} is complete, archiving...", next.id);
                if !self.dry_run {
                    match self.archive_change(&next).await {
                        Ok(_) => {
                            self.state.archived_changes.push(next.id.clone());
                            if let Some(progress) = &mut self.progress {
                                progress.archive_change(&next.id);
                            }
                        }
                        Err(e) => {
                            error!("Archive failed for {}: {}", next.id, e);
                            self.state.failed_changes.push(next.id.clone());
                            if let Some(progress) = &mut self.progress {
                                progress.error(&format!("Archive failed: {}", next.id));
                            }
                        }
                    }
                } else {
                    info!("[DRY RUN] Would archive: {}", next.id);
                }
            } else {
                // Apply change
                info!("Applying change: {}", next.id);
                if !self.dry_run {
                    match self.apply_change(&next).await {
                        Ok(_) => {
                            self.state.processed_changes.push(next.id.clone());
                            if let Some(progress) = &mut self.progress {
                                progress.complete_change(&next.id);
                            }
                        }
                        Err(e) => {
                            error!("Apply failed for {}: {}", next.id, e);
                            self.state.failed_changes.push(next.id.clone());
                            if let Some(progress) = &mut self.progress {
                                progress.error(&format!("Apply failed: {}", next.id));
                            }
                            // Continue with other changes
                            continue;
                        }
                    }
                } else {
                    info!("[DRY RUN] Would apply: {}", next.id);
                }
            }

            // 4. Update state
            self.state.total_iterations += 1;
            self.state.touch();
            self.state.save()?;

            // Check if we should continue
            if self.should_stop() {
                break;
            }
        }

        info!("Orchestration completed");
        self.print_summary();
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

        info!("Fallback selected: {} ({:.1}%)", selected.id, selected.progress_percent());
        Ok(selected)
    }

    /// Analyze dependencies using LLM
    async fn analyze_with_llm(&self, changes: &[Change]) -> Result<Change> {
        let prompt = self.build_analysis_prompt(changes);
        let response = self.opencode.analyze_dependencies(&prompt).await?;

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
            .map(|c| format!("- {} ({}/{} tasks, {:.1}%)", c.id, c.completed_tasks, c.total_tasks, c.progress_percent()))
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

    /// Apply a change using OpenCode
    async fn apply_change(&self, change: &Change) -> Result<()> {
        info!("Executing /openspec-apply for {}", change.id);

        let status = self
            .opencode
            .run_command("/openspec-apply", &change.id)
            .await?;

        if !status.success() {
            return Err(OrchestratorError::OpenCodeCommand(format!(
                "Apply command failed with exit code: {:?}",
                status.code()
            )));
        }

        info!("Successfully applied: {}", change.id);
        Ok(())
    }

    /// Archive a change using openspec
    async fn archive_change(&self, change: &Change) -> Result<()> {
        openspec::archive_change(&self.openspec_path, &change.id).await
    }

    /// Check if orchestrator should stop
    fn should_stop(&self) -> bool {
        // Stop if target change was processed
        if let Some(target) = &self.target_change {
            if self.state.archived_changes.contains(target)
                || self.state.failed_changes.contains(target)
            {
                return true;
            }
        }

        false
    }

    /// Print execution summary
    fn print_summary(&self) {
        println!("\n=== Orchestration Summary ===");
        println!("Total iterations: {}", self.state.total_iterations);
        println!("Processed changes: {}", self.state.processed_changes.len());
        println!("Archived changes: {}", self.state.archived_changes.len());
        println!("Failed changes: {}", self.state.failed_changes.len());

        if !self.state.archived_changes.is_empty() {
            println!("\nArchived:");
            for change in &self.state.archived_changes {
                println!("  ✓ {}", change);
            }
        }

        if !self.state.failed_changes.is_empty() {
            println!("\nFailed:");
            for change in &self.state.failed_changes {
                println!("  ✗ {}", change);
            }
        }
    }
}
