//! Shared change selection logic for CLI and TUI modes.
//!
//! Provides unified change selection that both modes can use,
//! with optional LLM-based dependency analysis.
//!
//! Note: This module's functions are currently unused as selection logic
//! has been migrated to SerialRunService. Keeping for reference and potential
//! future use in parallel mode.

#![allow(dead_code)]

use crate::agent::AgentRunner;
use crate::error::{OrchestratorError, Result};
use crate::openspec::Change;
use tracing::{info, warn};

/// Select the next change to process.
///
/// Selection priority:
/// 1. Complete changes (ready for archive)
/// 2. LLM-based selection (if agent provided)
/// 3. Fallback to highest progress percentage
///
/// # Arguments
/// * `changes` - Available changes to select from
/// * `agent` - Optional agent for LLM-based selection
/// * `ai_runner` - Optional AI command runner for shared stagger state
///
/// # Returns
/// The selected change, or an error if no changes available
pub async fn select_next_change(
    changes: &[Change],
    agent: Option<&AgentRunner>,
    ai_runner: Option<&crate::ai_command_runner::AiCommandRunner>,
) -> Result<Change> {
    if changes.is_empty() {
        return Err(OrchestratorError::NoChanges);
    }

    // Priority 1: Complete changes (ready for archive)
    if let Some(complete) = changes.iter().find(|c| c.is_complete()) {
        info!("Found complete change: {}", complete.id);
        return Ok(complete.clone());
    }

    // Priority 2: Use LLM for dependency analysis (if agent available)
    if let Some(agent) = agent {
        match analyze_with_llm(changes, agent, ai_runner).await {
            Ok(selected) => {
                info!("LLM selected: {}", selected.id);
                return Ok(selected);
            }
            Err(e) => {
                warn!("LLM analysis failed, using fallback: {}", e);
            }
        }
    }

    // Priority 3: Fallback - highest progress
    select_by_progress(changes)
}

/// Select change with highest progress percentage.
pub fn select_by_progress(changes: &[Change]) -> Result<Change> {
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
        "Selected by progress: {} ({:.1}%)",
        selected.id,
        selected.progress_percent()
    );
    Ok(selected)
}

/// Analyze dependencies using LLM.
async fn analyze_with_llm(
    changes: &[Change],
    agent: &AgentRunner,
    ai_runner: Option<&crate::ai_command_runner::AiCommandRunner>,
) -> Result<Change> {
    let prompt = crate::agent::append_optional_prompt(
        build_analysis_prompt_with_skill(agent.config().get_analyze_skill(), changes),
        agent.config().get_analyze_append_prompt(),
    );
    let response = if let Some(ai_runner) = ai_runner {
        agent
            .analyze_dependencies_with_runner(&prompt, ai_runner)
            .await?
    } else {
        agent.analyze_dependencies(&prompt).await?
    };

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

/// Build prompt for LLM dependency analysis.
fn build_analysis_prompt(changes: &[Change]) -> String {
    build_analysis_prompt_with_skill(crate::config::defaults::DEFAULT_ANALYZE_SKILL, changes)
}

fn build_analysis_prompt_with_skill(analyze_skill: &str, changes: &[Change]) -> String {
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
        "{}\n\n\
         Queued changes:\n\
         {}\n",
        crate::agent::prompt::skill_prelude(analyze_skill),
        change_list
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            dependencies: Vec::new(),
            metadata: crate::openspec::ProposalMetadata::default(),
        }
    }

    #[test]
    fn test_select_by_progress() {
        let changes = vec![
            test_change("low", 1, 10),    // 10%
            test_change("high", 8, 10),   // 80%
            test_change("medium", 5, 10), // 50%
        ];

        let selected = select_by_progress(&changes).unwrap();
        assert_eq!(selected.id, "high");
    }

    #[test]
    fn test_select_by_progress_empty() {
        let changes: Vec<Change> = vec![];
        let result = select_by_progress(&changes);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_select_next_change_complete_first() {
        let changes = vec![
            test_change("incomplete", 5, 10),
            test_change("complete", 10, 10), // 100% complete
        ];

        let selected = select_next_change(&changes, None, None).await.unwrap();
        assert_eq!(selected.id, "complete");
    }

    #[tokio::test]
    async fn test_select_next_change_fallback_to_progress() {
        let changes = vec![test_change("low", 1, 10), test_change("high", 8, 10)];

        let selected = select_next_change(&changes, None, None).await.unwrap();
        assert_eq!(selected.id, "high");
    }

    #[tokio::test]
    async fn test_select_next_change_empty() {
        let changes: Vec<Change> = vec![];
        let result = select_next_change(&changes, None, None).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_build_analysis_prompt() {
        let changes = vec![
            test_change("add-feature", 2, 5),
            test_change("fix-bug", 4, 4),
        ];

        let prompt = build_analysis_prompt(&changes);

        // Verify skill prelude
        assert!(prompt.contains("$cflx-analyze"));
        assert!(prompt.contains("load skills: cflx-analyze"));

        // Verify prompt contains variable context (change IDs and progress)
        assert!(prompt.contains("add-feature"));
        assert!(prompt.contains("fix-bug"));
        assert!(prompt.contains("2/5 tasks"));
        assert!(prompt.contains("40.0%"));
        assert!(prompt.contains("4/4 tasks"));
        assert!(prompt.contains("100.0%"));

        // Verify prompt contains queued changes header
        assert!(prompt.contains("Queued changes:"));
    }

    #[test]
    fn test_build_analysis_prompt_uses_custom_skill_prelude() {
        let changes = vec![test_change("add-feature", 2, 5)];
        let prompt = build_analysis_prompt_with_skill("team-analyze", &changes);

        assert!(prompt.contains("$team-analyze"));
        assert!(prompt.contains("load skills: team-analyze"));
        assert!(!prompt.contains("$cflx-analyze"));
        assert!(prompt.contains("add-feature"));
    }

    #[test]
    fn test_build_analysis_prompt_empty() {
        let changes: Vec<Change> = vec![];
        let prompt = build_analysis_prompt(&changes);

        // Prompt should still have skill prelude and structure
        assert!(prompt.contains("$cflx-analyze"));
        assert!(prompt.contains("load skills: cflx-analyze"));
        assert!(prompt.contains("Queued changes:"));
    }

    #[test]
    fn test_build_analysis_prompt_no_fixed_guidance() {
        let changes = vec![test_change("some-change", 3, 5)];
        let prompt = build_analysis_prompt(&changes);

        // Fixed guidance must NOT appear in Rust-side prompt (owned by cflx-analyze skill)
        assert!(
            !prompt.contains("選択基準"),
            "Selection criteria must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("回答は変更IDのみ"),
            "Output contract must not be in Rust prompt"
        );
        assert!(
            !prompt.contains("依存関係がない"),
            "Dependency rules must not be in Rust prompt"
        );
    }
}
