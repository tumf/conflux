use crate::error::{OrchestratorError, Result};
use crate::opencode::OpenCodeRunner;
use crate::openspec::{self, Change};
use crate::progress::ProgressDisplay;
use tracing::{error, info, warn};

pub struct Orchestrator {
    opencode: OpenCodeRunner,
    openspec_cmd: String,
    progress: Option<ProgressDisplay>,
    target_change: Option<String>,
}

impl Orchestrator {
    /// Create a new orchestrator
    pub fn new(
        opencode_path: &str,
        openspec_cmd: &str,
        target_change: Option<String>,
    ) -> Result<Self> {
        let opencode = OpenCodeRunner::new(opencode_path);

        Ok(Self {
            opencode,
            openspec_cmd: openspec_cmd.to_string(),
            progress: None,
            target_change,
        })
    }

    /// Run the orchestration loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting orchestration loop");

        loop {
            // 1. List all changes from openspec
            let changes = openspec::list_changes(&self.openspec_cmd).await?;

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
                    info!("Target change not found or already processed");
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

            if let Some(progress) = &mut self.progress {
                progress.update_change(&next);
            }

            // 3. Process the change
            if next.is_complete() {
                // Archive completed change
                info!("Change {} is complete, archiving...", next.id);
                match self.archive_change(&next).await {
                    Ok(_) => {
                        if let Some(progress) = &mut self.progress {
                            progress.archive_change(&next.id);
                        }
                        // If targeting specific change and it's done, stop
                        if self.target_change.as_ref() == Some(&next.id) {
                            break;
                        }
                    }
                    Err(e) => {
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
                match self.apply_change(&next).await {
                    Ok(_) => {
                        if let Some(progress) = &mut self.progress {
                            progress.complete_change(&next.id);
                        }
                    }
                    Err(e) => {
                        error!("Apply failed for {}: {}", next.id, e);
                        if let Some(progress) = &mut self.progress {
                            progress.error(&format!("Apply failed: {}", next.id));
                        }
                        return Err(e);
                    }
                }
            }
        }

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

    /// Archive a change using OpenCode
    async fn archive_change(&self, change: &Change) -> Result<()> {
        info!("Executing /openspec-archive for {}", change.id);

        let status = self
            .opencode
            .run_command("/openspec-archive", &change.id)
            .await?;

        if !status.success() {
            return Err(OrchestratorError::OpenCodeCommand(format!(
                "Archive command failed with exit code: {:?}",
                status.code()
            )));
        }

        info!("Successfully archived: {}", change.id);
        Ok(())
    }
}
