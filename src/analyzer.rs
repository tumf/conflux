//! Parallelization analyzer for identifying independent changes.
//!
//! Uses LLM-based analysis to determine which changes can be executed
//! in parallel and what dependencies exist between them.

use crate::ai_command_runner::OutputLine as AiOutputLine;
use crate::dependency_targets::{
    classify_dependency_target, collect_active_change_ids, collect_archived_change_ids,
    collect_rejected_change_ids, union_metadata_dependencies, DependencyTargetClass,
};
use crate::error::{OrchestratorError, Result};
use crate::openspec::{Change, ProposalFrontmatterMetadata};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;
use tracing::{debug, info, warn};

/// A group of changes that can be executed in parallel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelGroup {
    /// Group identifier
    pub id: u32,
    /// Change IDs in this group
    pub changes: Vec<String>,
    /// Group IDs this group depends on (must complete before this group starts)
    #[serde(default)]
    pub depends_on: Vec<u32>,
}

/// Result of parallelization analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Execution order (recommended execution sequence considering dependencies)
    pub order: Vec<String>,
    /// Dependencies between changes (change_id -> list of dependencies)
    #[serde(default)]
    pub dependencies: HashMap<String, Vec<String>>,
    /// Legacy groups field (deprecated, for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<ParallelGroup>>,
}

/// Analyzer for determining parallel execution groups
pub struct ParallelizationAnalyzer {
    ai_runner: crate::ai_command_runner::AiCommandRunner,
    config: crate::config::OrchestratorConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnalyzePromptMetadata {
    priority: Option<String>,
    references: Vec<String>,
    warnings: Vec<String>,
}

impl AnalyzePromptMetadata {
    fn from_frontmatter(metadata: &ProposalFrontmatterMetadata) -> Self {
        Self {
            priority: metadata.priority.clone(),
            references: metadata.references.clone(),
            warnings: metadata
                .warnings
                .iter()
                .map(|warning| warning.message.clone())
                .collect(),
        }
    }
}

impl ParallelizationAnalyzer {
    /// Create a new analyzer with the given AI command runner and configuration
    pub fn new(
        ai_runner: crate::ai_command_runner::AiCommandRunner,
        config: crate::config::OrchestratorConfig,
    ) -> Self {
        Self { ai_runner, config }
    }

    /// Analyze changes and return the raw analysis result with order and dependencies.
    ///
    /// This is the preferred method for order-based execution.
    /// Returns the analysis result containing:
    /// - `order`: Recommended execution sequence considering dependencies
    /// - `dependencies`: Change-level dependency constraints
    #[allow(dead_code)]
    pub async fn analyze(&self, changes: &[Change]) -> Result<AnalysisResult> {
        self.analyze_with_callback(changes, &[], |_| {}).await
    }

    /// Analyze changes with in-flight changes context.
    ///
    /// The `in_flight_ids` parameter specifies changes that are currently executing.
    /// These changes are not selectable for the execution order but can be referenced
    /// as dependencies by queued changes.
    pub async fn analyze_with_inflight(
        &self,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Result<AnalysisResult> {
        self.analyze_with_callback(changes, in_flight_ids, |_| {})
            .await
    }

    /// Analyze changes with output callback and return raw analysis result.
    ///
    /// The `in_flight_ids` parameter specifies changes that are currently executing.
    /// These changes are not selectable for the execution order but can be referenced
    /// as dependencies by queued changes.
    pub async fn analyze_with_callback<F>(
        &self,
        changes: &[Change],
        in_flight_ids: &[String],
        mut on_output: F,
    ) -> Result<AnalysisResult>
    where
        F: FnMut(String),
    {
        if changes.is_empty() {
            return Ok(AnalysisResult {
                order: Vec::new(),
                dependencies: HashMap::new(),
                groups: None,
            });
        }

        // For a single change, LLM analysis is unnecessary, but proposal
        // metadata dependencies are still authoritative hard dependencies.
        if changes.len() == 1 {
            let result = AnalysisResult {
                order: vec![changes[0].id.clone()],
                dependencies: HashMap::new(),
                groups: None,
            };
            return self.normalize_and_validate_result(result, changes, in_flight_ids);
        }

        // Build prompt for LLM analysis
        let prompt = self.build_parallelization_prompt(changes, in_flight_ids);
        info!(
            "Analyzing {} changes for parallelization (with {} in-flight)",
            changes.len(),
            in_flight_ids.len()
        );
        for c in changes {
            info!("  - {}", c.id);
        }
        if !in_flight_ids.is_empty() {
            info!("In-flight changes (executing):");
            for id in in_flight_ids {
                info!("  - {}", id);
            }
        }
        debug!("Analysis prompt: {}", prompt);

        // Execute analysis command and collect output
        let (full_output, status) = self
            .execute_analysis_command(&prompt, changes, &mut on_output)
            .await?;

        // Parse and validate the output
        let result =
            self.parse_and_validate_output(&full_output, &status, changes, in_flight_ids)?;

        info!("Analysis complete: {} changes in order", result.order.len());
        Ok(result)
    }

    /// Execute the analysis command with streaming output.
    ///
    /// Runs the AI command via AiCommandRunner, streams output to the callback,
    /// and returns the full output string and exit status.
    async fn execute_analysis_command<F>(
        &self,
        prompt: &str,
        changes: &[Change],
        on_output: &mut F,
    ) -> Result<(String, std::process::ExitStatus)>
    where
        F: FnMut(String),
    {
        // Call LLM for analysis with streaming output via AiCommandRunner
        let template = self.config.get_analyze_command()?;
        let prompt = crate::agent::append_optional_prompt(
            prompt.to_string(),
            self.config.get_analyze_append_prompt(),
        );
        let command = crate::config::OrchestratorConfig::expand_prompt(template, &prompt);
        let (mut child, mut rx) = self
            .ai_runner
            .execute_streaming_with_retry(&command, None, Some("analyze"), None)
            .await?;

        // Collect output while streaming to callback
        let mut full_output = String::new();
        while let Some(line) = rx.recv().await {
            let text = match &line {
                AiOutputLine::Stdout(s) | AiOutputLine::Stderr(s) => s.clone(),
            };
            full_output.push_str(&text);
            full_output.push('\n');
            on_output(text);
        }

        // Wait for process to complete
        let status = child.wait().await.map_err(|e| {
            let change_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
            OrchestratorError::AgentCommand(format!(
                "Analysis process failed for changes [{}]: {}",
                change_ids.join(", "),
                e
            ))
        })?;

        Ok((full_output, status))
    }

    /// Parse and validate the analysis output.
    ///
    /// Extracts JSON from stream-json format if applicable, validates the schema,
    /// and checks the exit status. Returns the parsed AnalysisResult.
    ///
    /// The `in_flight_ids` parameter specifies changes that are currently executing.
    /// These IDs are accepted as valid dependency targets during validation even
    /// though they are not present in the `order` array.
    fn parse_and_validate_output(
        &self,
        full_output: &str,
        status: &std::process::ExitStatus,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Result<AnalysisResult> {
        let archived_ids = self.collect_archived_change_ids();
        // Extract result from stream-json format if applicable
        let response = self.extract_stream_json_result(full_output);
        debug!("LLM response: {}", response);

        // Parse JSON response with strict validation
        let result = self
            .parse_response(&response, changes, in_flight_ids)
            .map_err(|e| {
            let preview = response.chars().take(200).collect::<String>();
            let change_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
            let err_text = e.to_string();
            if err_text.contains("Invalid dependency reference") || err_text.contains("Missing dependency reference") || err_text.contains("Rejected dependency reference") {
                let decorated = self.decorate_dependency_error_with_archive_context(
                    &err_text,
                    changes,
                    in_flight_ids,
                    &archived_ids,
                );
                OrchestratorError::Parse(format!(
                    "Analysis dependency contract failure for changes [{}] (exit code: {:?}): {}. Response preview: {}",
                    change_ids.join(", "),
                    status.code(),
                    decorated,
                    preview
                ))
            } else {
                OrchestratorError::Parse(format!(
                    "Analysis returned invalid JSON for changes [{}] (exit code: {:?}): {}. Response preview: {}",
                    change_ids.join(", "),
                    status.code(),
                    e,
                    preview
                ))
            }
        })?;

        // Check exit code after successful JSON parsing
        if !status.success() {
            let change_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
            return Err(OrchestratorError::AgentCommand(format!(
                "Analysis failed for changes [{}] with exit code: {:?}",
                change_ids.join(", "),
                status.code()
            )));
        }

        Ok(result)
    }

    /// Analyze changes and return parallel execution groups
    ///
    /// Returns groups in topological order (dependencies first).
    ///
    /// # Deprecated
    ///
    /// This method converts order-based results to group-based format.
    /// Prefer using `analyze()` for order-based execution.
    pub async fn analyze_groups(&self, changes: &[Change]) -> Result<Vec<ParallelGroup>> {
        self.analyze_groups_with_callback(changes, |_| {}).await
    }

    /// Analyze changes and return parallel execution groups with output callback
    ///
    /// The callback is called for each line of output from the analysis command.
    /// Returns groups in topological order (dependencies first).
    ///
    /// # Deprecated
    ///
    /// This method converts order-based results to group-based format.
    /// Prefer using `analyze_with_callback()` for order-based execution.
    pub async fn analyze_groups_with_callback<F>(
        &self,
        changes: &[Change],
        on_output: F,
    ) -> Result<Vec<ParallelGroup>>
    where
        F: FnMut(String),
    {
        // Use the new analyze_with_callback and convert to groups (no in-flight for deprecated method)
        let result = self.analyze_with_callback(changes, &[], on_output).await?;

        if result.order.is_empty() {
            return Ok(Vec::new());
        }

        // Convert order-based result to group-based format for backward compatibility
        let groups = self.order_to_groups(&result);
        info!("Analysis complete: {} groups identified", groups.len());
        Ok(groups)
    }

    /// Extract final assistant text from supported agent JSONL output formats.
    fn extract_stream_json_result(&self, output: &str) -> String {
        crate::stream_json_textifier::extract_final_text_from_stream_json_output(output)
    }

    /// Build the prompt for parallelization analysis
    ///
    /// Formats all changes with:
    /// - Full proposal file path for each change (e.g., `openspec/changes/{id}/proposal.md`)
    ///
    /// Also includes in-flight changes (currently executing) for dependency analysis,
    /// marked as not selectable but available as dependency targets.
    ///
    /// This makes it clear to the LLM which changes need analysis and where
    /// to find their proposal files.
    fn build_parallelization_prompt(&self, changes: &[Change], in_flight_ids: &[String]) -> String {
        let change_list: String = changes
            .iter()
            .map(|change| self.format_change_prompt_entry(change))
            .collect::<Vec<_>>()
            .join("\n\n");

        let executing_section = if in_flight_ids.is_empty() {
            String::new()
        } else {
            let executing_list: String = in_flight_ids
                .iter()
                .map(|id| format!("- {} (openspec/changes/{}/proposal.md)", id, id))
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                r#"

Currently executing changes (NOT selectable, but available as dependencies):
{executing_list}
"#
            )
        };

        format!(
            r#"You are planning the execution order for OpenSpec changes.

Analyze ONLY the changes marked with [x] below.
Read the proposal files at the specified paths to understand their dependencies:

{change_list}{executing_section}

Your task:
1. Read each change's proposal.md at the given path to understand what it does
2. Use proposal frontmatter `dependencies` as the dependency source when present; only fall back to the body `## Dependencies` section when frontmatter dependencies are absent
3. Treat proposal frontmatter `priority` as a soft ordering hint for `order` only
4. Treat proposal frontmatter `references` as supplemental analysis context only
5. Consider currently executing changes as potential dependencies (but DO NOT include them in the order)
6. Return execution order and dependencies
7. `dependencies` may reference ONLY queued change IDs and explicitly listed in-flight IDs
8. NEVER reference unrelated active changes, archived changes, or any ID outside that allowed set

Return ONLY valid JSON in this exact format:
{{
  "order": ["change-a", "change-b", "change-c"],
  "dependencies": {{
    "change-c": ["change-a"]
  }}
}}

Rules:
- `order`: Array of change IDs in recommended execution sequence
  - This represents the RECOMMENDED execution order considering dependencies, priorities, and efficiency
  - Independent changes can be ordered by priority or logical flow
  - DO NOT include currently executing changes in the order (they are already running)
- `dependencies`: Object mapping change IDs to arrays of their REQUIRED dependency IDs
  - STRICT CRITERIA: Only include a dependency if one change REQUIRES the artifacts, specs, or APIs from another change to function
  - DO NOT include dependencies based on priority, references, preferred order, or efficiency alone
  - Example of REQUIRED dependency: "change-b implements a feature using the API defined in change-a"
  - Example of NOT a dependency: "change-a should ideally be done before change-b for efficiency"
  - Dependencies CAN reference currently executing changes if a queued change requires their output
- Dependencies MUST reference only IDs from the queued `order` set and explicitly provided in-flight IDs
- Do NOT reference active-but-not-queued IDs, archived IDs, or unrelated change IDs
- Proposal metadata warnings are informational only; continue analysis using known metadata
- Every change ID in the input list must appear exactly once in `order`
- Dependencies are hard constraints: a change CANNOT start until all its dependencies are merged to base
- Order preferences without required dependencies should be reflected in `order` only, not in `dependencies`
- Return valid JSON only, no markdown, no explanation"#
        )
    }

    fn format_change_prompt_entry(&self, change: &Change) -> String {
        let proposal_path = format!("openspec/changes/{}/proposal.md", change.id);
        let metadata = self.read_prompt_metadata(change);

        let mut lines = vec![format!("- {} ({})", change.id, proposal_path)];
        lines.push(format!(
            "  dependency_source: {}",
            if change.dependencies.is_empty() {
                "none"
            } else {
                "proposal metadata or body fallback"
            }
        ));
        lines.push(format!(
            "  dependencies_for_analysis: {}",
            if change.dependencies.is_empty() {
                "[]".to_string()
            } else {
                format!("[{}]", change.dependencies.join(", "))
            }
        ));

        if let Some(metadata) = metadata {
            if let Some(priority) = metadata.priority {
                lines.push(format!("  priority_hint: {}", priority));
            }
            if !metadata.references.is_empty() {
                lines.push(format!(
                    "  references: [{}]",
                    metadata.references.join(", ")
                ));
            }
            if !metadata.warnings.is_empty() {
                lines.push(format!(
                    "  metadata_warnings: [{}]",
                    metadata.warnings.join(" | ")
                ));
            }
        }

        lines.join("\n")
    }

    fn read_prompt_metadata(&self, change: &Change) -> Option<AnalyzePromptMetadata> {
        let proposal = crate::openspec::read_proposal(&change.id);
        let metadata = proposal.metadata?;

        for warning_message in metadata
            .warnings
            .iter()
            .map(|warning| warning.message.as_str())
        {
            warn!(
                change_id = %change.id,
                warning = warning_message,
                "Continuing analyze with proposal frontmatter warning"
            );
        }

        Some(AnalyzePromptMetadata::from_frontmatter(&metadata))
    }

    /// Parse LLM response into AnalysisResult
    ///
    /// The `in_flight_ids` parameter specifies currently executing change IDs that
    /// are valid dependency targets but not expected in `order`.
    fn parse_response(
        &self,
        response: &str,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Result<AnalysisResult> {
        // Try to extract JSON from response (may have surrounding text)
        let json_str = self.extract_json(response)?;

        // Strict JSON schema validation
        self.validate_json_schema(&json_str)?;

        // Parse JSON into structured type
        let result: AnalysisResult = serde_json::from_str(&json_str).map_err(|e| {
            OrchestratorError::Parse(format!("Failed to parse parallelization response: {}", e))
        })?;

        // Validate all change IDs exist
        self.validate_change_ids(&result, changes)?;

        self.normalize_and_validate_result(result, changes, in_flight_ids)
    }

    fn normalize_and_validate_result(
        &self,
        mut result: AnalysisResult,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Result<AnalysisResult> {
        for change in changes {
            union_metadata_dependencies(&mut result.dependencies, &change.id, &change.dependencies);
        }

        let archived_ids = self.collect_archived_change_ids();
        let rejected_ids = self.collect_rejected_change_ids();
        self.validate_dependency_graph_with_changes(
            &result,
            changes,
            in_flight_ids,
            &archived_ids,
            &rejected_ids,
        )?;

        Ok(result)
    }

    /// Validate JSON schema for analysis result.
    ///
    /// Checks that:
    /// - JSON is parseable
    /// - `order` key exists
    /// - `order` is an array
    /// - `dependencies` key exists (optional but should be present)
    fn validate_json_schema(&self, json_str: &str) -> Result<()> {
        // Parse as generic JSON value first
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| OrchestratorError::Parse(format!("Invalid JSON syntax: {}", e)))?;

        // Check for required root object
        if !value.is_object() {
            return Err(OrchestratorError::Parse(
                "JSON root must be an object".to_string(),
            ));
        }

        // Check for required `order` key
        let order = value.get("order").ok_or_else(|| {
            OrchestratorError::Parse("Missing required key 'order' in JSON".to_string())
        })?;

        // Check that order is an array
        if !order.is_array() {
            return Err(OrchestratorError::Parse(
                "Key 'order' must be an array".to_string(),
            ));
        }

        // Check for dependencies key (should be present)
        if let Some(dependencies) = value.get("dependencies") {
            if !dependencies.is_object() {
                return Err(OrchestratorError::Parse(
                    "Key 'dependencies' must be an object".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Extract JSON from response text (handles markdown code blocks)
    fn extract_json(&self, response: &str) -> Result<String> {
        let trimmed = response.trim();
        let candidate = if let Some(start) = trimmed.find("```json") {
            let after_marker = &trimmed[start + 7..];
            after_marker
                .find("```")
                .map(|end| after_marker[..end].trim())
        } else if let Some(start) = trimmed.find("```") {
            let after_marker = &trimmed[start + 3..];
            let content_start = after_marker.find('\n').unwrap_or(0);
            let content = &after_marker[content_start..];
            content.find("```").map(|end| content[..end].trim())
        } else {
            trimmed.find('{').map(|start| &trimmed[start..])
        }
        .ok_or_else(|| {
            OrchestratorError::Parse("Could not extract JSON from response".to_string())
        })?;

        let value = serde_json::Deserializer::from_str(candidate)
            .into_iter::<serde_json::Value>()
            .next()
            .ok_or_else(|| {
                OrchestratorError::Parse("Could not extract JSON from response".to_string())
            })?
            .map_err(|e| OrchestratorError::Parse(format!("Invalid JSON syntax: {}", e)))?;

        serde_json::to_string(&value).map_err(|e| {
            OrchestratorError::Parse(format!("Failed to normalize JSON response: {}", e))
        })
    }

    /// Validate that all change IDs in the result exist in the input
    fn validate_change_ids(&self, result: &AnalysisResult, changes: &[Change]) -> Result<()> {
        let valid_ids: HashSet<&str> = changes.iter().map(|c| c.id.as_str()).collect();
        let mut seen_ids: HashSet<&str> = HashSet::new();

        // Check all IDs in order
        for change_id in &result.order {
            // Check ID exists
            if !valid_ids.contains(change_id.as_str()) {
                return Err(OrchestratorError::Parse(format!(
                    "Unknown change ID in order: {}",
                    change_id
                )));
            }

            // Check for duplicates
            if seen_ids.contains(change_id.as_str()) {
                return Err(OrchestratorError::Parse(format!(
                    "Duplicate change ID in order: {}",
                    change_id
                )));
            }
            seen_ids.insert(change_id.as_str());
        }

        // Check all changes are accounted for
        if seen_ids.len() != valid_ids.len() {
            let missing: Vec<_> = valid_ids.difference(&seen_ids).collect();
            return Err(OrchestratorError::Parse(format!(
                "Missing change IDs in response: {:?}",
                missing
            )));
        }

        Ok(())
    }

    /// Validate dependency graph for circular dependencies
    ///
    /// The `in_flight_ids` parameter specifies currently executing change IDs.
    /// Dependencies referencing these IDs are considered valid even though
    /// they are not present in the `order` array.
    #[cfg_attr(not(test), allow(dead_code))]
    fn validate_dependency_graph(
        &self,
        result: &AnalysisResult,
        in_flight_ids: &[String],
    ) -> Result<()> {
        let in_flight_set: HashSet<&str> = in_flight_ids.iter().map(|s| s.as_str()).collect();

        // Check for self-dependencies
        for (change_id, deps) in &result.dependencies {
            if deps.contains(change_id) {
                return Err(OrchestratorError::Parse(format!(
                    "Self-dependency detected: change '{}' depends on itself",
                    change_id
                )));
            }

            // Check all dependencies exist in order or in-flight
            for dep_id in deps {
                if !result.order.contains(dep_id) && !in_flight_set.contains(dep_id.as_str()) {
                    let mut allowed_ids: Vec<String> = result.order.clone();
                    allowed_ids.extend(in_flight_ids.iter().cloned());
                    allowed_ids.sort();
                    allowed_ids.dedup();

                    return Err(OrchestratorError::Parse(format!(
                        "Invalid dependency reference: change '{}' depends on '{}' outside allowed dependency targets. allowed_queued_ids={:?}, allowed_in_flight_ids={:?}, allowed_ids={:?}",
                        change_id,
                        dep_id,
                        result.order,
                        in_flight_ids,
                        allowed_ids
                    )));
                }
            }
        }

        // Check for circular dependencies using DFS
        self.detect_cycles_from_dependencies(&result.dependencies)?;

        Ok(())
    }

    fn validate_dependency_graph_with_changes(
        &self,
        result: &AnalysisResult,
        changes: &[Change],
        in_flight_ids: &[String],
        archived_ids: &HashSet<String>,
        rejected_ids: &HashSet<String>,
    ) -> Result<()> {
        let queued_ids: Vec<&str> = changes.iter().map(|change| change.id.as_str()).collect();
        let in_flight_refs: Vec<&str> = in_flight_ids.iter().map(String::as_str).collect();
        let active_ids = self.collect_active_change_ids();
        let active_refs: Vec<&str> = active_ids.iter().map(String::as_str).collect();

        for (change_id, deps) in &result.dependencies {
            if deps.contains(change_id) {
                return Err(OrchestratorError::Parse(format!(
                    "Self-dependency detected: change '{}' depends on itself",
                    change_id
                )));
            }

            for dep_id in deps {
                match classify_dependency_target(
                    dep_id,
                    queued_ids.iter().copied(),
                    in_flight_refs.iter().copied(),
                    active_refs.iter().copied(),
                    archived_ids,
                    rejected_ids,
                ) {
                    DependencyTargetClass::Queued
                    | DependencyTargetClass::InFlight
                    | DependencyTargetClass::Resolving
                    | DependencyTargetClass::ActiveButNotQueued => {}
                    DependencyTargetClass::Error => unreachable!(
                        "repository-visible dependency classification cannot produce terminal-error state"
                    ),
                    DependencyTargetClass::Archived => {
                        debug!(
                            change_id,
                            dependency = dep_id,
                            "Accepted archived dependency target as already satisfied"
                        );
                    }
                    DependencyTargetClass::Rejected => {
                        let mut allowed_ids: Vec<String> = result.order.clone();
                        allowed_ids.extend(in_flight_ids.iter().cloned());
                        allowed_ids.extend(active_ids.iter().cloned());
                        allowed_ids.extend(archived_ids.iter().cloned());
                        allowed_ids.extend(rejected_ids.iter().cloned());
                        allowed_ids.sort();
                        allowed_ids.dedup();

                        return Err(OrchestratorError::Parse(format!(
                            "Rejected dependency reference: change '{}' depends on '{}' classified as rejected dependency target. allowed_queued_ids={:?}, allowed_in_flight_ids={:?}, allowed_active_ids={:?}, allowed_archived_ids={:?}, allowed_rejected_ids={:?}, allowed_ids={:?}",
                            change_id,
                            dep_id,
                            result.order,
                            in_flight_ids,
                            active_ids,
                            archived_ids,
                            rejected_ids,
                            allowed_ids
                        )));
                    }
                    DependencyTargetClass::Missing => {
                        let mut allowed_ids: Vec<String> = result.order.clone();
                        allowed_ids.extend(in_flight_ids.iter().cloned());
                        allowed_ids.extend(active_ids.iter().cloned());
                        allowed_ids.extend(archived_ids.iter().cloned());
                        allowed_ids.extend(rejected_ids.iter().cloned());
                        allowed_ids.sort();
                        allowed_ids.dedup();

                        return Err(OrchestratorError::Parse(format!(
                            "Missing dependency reference: change '{}' depends on '{}' classified as missing dependency target. allowed_queued_ids={:?}, allowed_in_flight_ids={:?}, allowed_active_ids={:?}, allowed_archived_ids={:?}, allowed_rejected_ids={:?}, allowed_ids={:?}",
                            change_id,
                            dep_id,
                            result.order,
                            in_flight_ids,
                            active_ids,
                            archived_ids,
                            rejected_ids,
                            allowed_ids
                        )));
                    }
                }
            }
        }

        self.detect_cycles_from_dependencies(&result.dependencies)?;

        Ok(())
    }

    fn collect_active_change_ids(&self) -> HashSet<String> {
        collect_active_change_ids(Path::new("."))
    }

    fn collect_archived_change_ids(&self) -> HashSet<String> {
        collect_archived_change_ids(Path::new("."))
    }

    fn collect_rejected_change_ids(&self) -> HashSet<String> {
        collect_rejected_change_ids(Path::new("."))
    }

    fn decorate_dependency_error_with_archive_context(
        &self,
        err_text: &str,
        changes: &[Change],
        in_flight_ids: &[String],
        archived_ids: &HashSet<String>,
    ) -> String {
        static DEP_RE: OnceLock<Regex> = OnceLock::new();
        let dep_re = DEP_RE.get_or_init(|| {
            Regex::new(r"change '([^']+)' depends on '([^']+)'(?: outside allowed dependency targets| classified as (?:missing|rejected) dependency target)")
                .unwrap()
        });

        let Some(caps) = dep_re.captures(err_text) else {
            return err_text.to_string();
        };

        let change_id = caps.get(1).map_or("", |m| m.as_str());
        let dep_id = caps.get(2).map_or("", |m| m.as_str());
        if change_id.is_empty() || dep_id.is_empty() {
            return err_text.to_string();
        }

        let queued_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
        let in_flight_set: HashSet<&str> = in_flight_ids.iter().map(|id| id.as_str()).collect();
        let active_ids = self.collect_active_change_ids();
        let active_refs: Vec<&str> = active_ids.iter().map(String::as_str).collect();

        let rejected_ids = self.collect_rejected_change_ids();
        let classification = classify_dependency_target(
            dep_id,
            queued_ids.iter().copied(),
            in_flight_set.iter().copied(),
            active_refs.iter().copied(),
            archived_ids,
            &rejected_ids,
        );

        format!(
            "{} dependency_target_classification={{change:'{}', dependency:'{}', class:'{}'}}",
            err_text,
            change_id,
            dep_id,
            classification.as_str()
        )
    }

    /// Detect cycles in dependency graph (change-level dependencies)
    fn detect_cycles_from_dependencies(
        &self,
        dependencies: &HashMap<String, Vec<String>>,
    ) -> Result<()> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut rec_stack: HashSet<String> = HashSet::new();

        for change_id in dependencies.keys() {
            if !visited.contains(change_id)
                && self.has_cycle_in_dependencies(
                    change_id,
                    dependencies,
                    &mut visited,
                    &mut rec_stack,
                )
            {
                return Err(OrchestratorError::Parse(
                    "Circular dependency detected in change dependencies".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// DFS helper for cycle detection in change-level dependencies
    fn has_cycle_in_dependencies(
        &self,
        node: &str,
        dependencies: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());

        if let Some(deps) = dependencies.get(node) {
            for dep in deps {
                if !visited.contains(dep) {
                    if self.has_cycle_in_dependencies(dep, dependencies, visited, rec_stack) {
                        return true;
                    }
                } else if rec_stack.contains(dep) {
                    return true;
                }
            }
        }

        rec_stack.remove(node);
        false
    }

    /// Convert order-based analysis result to group-based format.
    ///
    /// Groups changes that have no dependencies on each other into parallel groups.
    /// Changes are processed in the order specified by the `order` field, respecting
    /// the constraints defined in the `dependencies` field.
    fn order_to_groups(&self, result: &AnalysisResult) -> Vec<ParallelGroup> {
        let mut groups: Vec<ParallelGroup> = Vec::new();
        let mut processed: HashSet<String> = HashSet::new();
        let mut group_id = 1u32;

        // Process changes in order
        for change_id in &result.order {
            if processed.contains(change_id) {
                continue;
            }

            // Find all changes that can be executed in parallel with this one
            let mut group_changes = vec![change_id.clone()];
            processed.insert(change_id.clone());

            // Check remaining unprocessed changes
            for other_id in &result.order {
                if processed.contains(other_id) {
                    continue;
                }

                // Can run in parallel if:
                // 1. No dependency between them
                // 2. All dependencies are already processed
                let can_parallel =
                    !self.has_dependency_between(change_id, other_id, &result.dependencies)
                        && self.dependencies_satisfied(other_id, &result.dependencies, &processed);

                if can_parallel {
                    group_changes.push(other_id.clone());
                    processed.insert(other_id.clone());
                }
            }

            // Create a group for these parallel changes
            groups.push(ParallelGroup {
                id: group_id,
                changes: group_changes,
                depends_on: Vec::new(), // Dependencies are tracked at change level
            });
            group_id += 1;
        }

        groups
    }

    /// Check if there's a dependency relationship between two changes (in either direction)
    fn has_dependency_between(
        &self,
        a: &str,
        b: &str,
        dependencies: &HashMap<String, Vec<String>>,
    ) -> bool {
        // Check if a depends on b
        if let Some(a_deps) = dependencies.get(a) {
            if a_deps.contains(&b.to_string()) {
                return true;
            }
        }
        // Check if b depends on a
        if let Some(b_deps) = dependencies.get(b) {
            if b_deps.contains(&a.to_string()) {
                return true;
            }
        }
        false
    }

    /// Check if all dependencies for a change are satisfied (already processed)
    fn dependencies_satisfied(
        &self,
        change_id: &str,
        dependencies: &HashMap<String, Vec<String>>,
        processed: &HashSet<String>,
    ) -> bool {
        if let Some(deps) = dependencies.get(change_id) {
            deps.iter().all(|dep| processed.contains(dep))
        } else {
            true // No dependencies
        }
    }
}

/// Extract change-level dependencies from parallel groups.
///
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::*;
    use crate::openspec::ProposalMetadata;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: crate::openspec::ProposalMetadata::default(),
        }
    }

    fn create_test_analyzer() -> ParallelizationAnalyzer {
        let config = crate::config::OrchestratorConfig::default();
        let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
            inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
            inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
            inactivity_timeout_max_retries: config.get_command_inactivity_timeout_max_retries(),
            strict_process_cleanup: config.get_command_strict_process_cleanup(),
        };
        let stream_json_textify = config.get_stream_json_textify();
        let mut ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
        ai_runner.set_stream_json_textify(stream_json_textify);
        ai_runner.set_strict_process_cleanup(config.get_command_strict_process_cleanup());
        ParallelizationAnalyzer::new(ai_runner, config)
    }

    #[test]
    fn test_extract_json_pure() {
        let analyzer = create_test_analyzer();

        let json = r#"{"order": ["a"], "dependencies": {}}"#;
        let result = analyzer.extract_json(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_json_markdown() {
        let analyzer = create_test_analyzer();

        let response = r#"Here's the analysis:

```json
{"order": ["a"], "dependencies": {}}
```

That's all."#;
        let result = analyzer.extract_json(response);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_json_ignores_trailing_agent_output() {
        let analyzer = create_test_analyzer();
        let json = r#"{"order":["a"],"dependencies":{}}"#;
        let response = format!("{json}{json}[tool_use:Read] filePath=/tmp/proposal.md");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&analyzer.extract_json(&response).unwrap())
                .unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn test_extract_json_rejects_malformed_first_object() {
        let analyzer = create_test_analyzer();
        let response = r#"{"order":["a"],"dependencies":{} {"valid":true}"#;

        assert!(analyzer.extract_json(response).is_err());
    }

    #[test]
    fn test_validate_change_ids_missing() {
        let analyzer = create_test_analyzer();

        let changes = vec![create_test_change("a"), create_test_change("b")];
        let result = AnalysisResult {
            order: vec!["a".to_string()], // Missing "b"
            dependencies: HashMap::new(),
            groups: None,
        };

        let validation = analyzer.validate_change_ids(&result, &changes);
        assert!(validation.is_err());
    }

    #[test]
    fn test_validate_change_ids_duplicate() {
        let analyzer = create_test_analyzer();

        let changes = vec![create_test_change("a"), create_test_change("b")];
        let result = AnalysisResult {
            order: vec!["a".to_string(), "a".to_string(), "b".to_string()], // Duplicate "a"
            dependencies: HashMap::new(),
            groups: None,
        };

        let validation = analyzer.validate_change_ids(&result, &changes);
        assert!(validation.is_err());
    }

    #[test]
    fn test_validate_dependency_graph_valid() {
        let analyzer = create_test_analyzer();

        let mut deps = HashMap::new();
        deps.insert("b".to_string(), vec!["a".to_string()]);
        let result = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps,
            groups: None,
        };

        let validation = analyzer.validate_dependency_graph(&result, &[]);
        assert!(validation.is_ok());
    }

    #[test]
    fn test_validate_dependency_graph_self_reference() {
        let analyzer = create_test_analyzer();

        let mut deps = HashMap::new();
        deps.insert("a".to_string(), vec!["a".to_string()]); // Self-reference
        let result = AnalysisResult {
            order: vec!["a".to_string()],
            dependencies: deps,
            groups: None,
        };

        let validation = analyzer.validate_dependency_graph(&result, &[]);
        assert!(validation.is_err());
    }

    #[test]
    fn test_validate_dependency_graph_cycle() {
        let analyzer = create_test_analyzer();

        let mut deps = HashMap::new();
        deps.insert("a".to_string(), vec!["b".to_string()]); // Cycle: a -> b -> a
        deps.insert("b".to_string(), vec!["a".to_string()]);
        let result = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps,
            groups: None,
        };

        let validation = analyzer.validate_dependency_graph(&result, &[]);
        assert!(validation.is_err());
    }

    #[test]
    fn test_build_prompt_includes_frontmatter_metadata_context() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("change-a");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(
            change_dir.join("proposal.md"),
            "---\npriority: high\ndependencies:\n  - base-change\nreferences:\n  - src/analyzer.rs\nowner: tumf\n---\n# Change: Sample\n\n## Dependencies\n\n- legacy-dep\n",
        )
        .unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let metadata =
            crate::openspec::parse_proposal_metadata_from_file(&change_dir.join("proposal.md"));
        let changes = vec![Change {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: metadata.dependencies.clone(),
            metadata,
        }];

        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(prompt.contains("priority_hint: high"));
        assert!(prompt.contains("dependencies_for_analysis: [base-change]"));
        assert!(prompt.contains("references: [src/analyzer.rs]"));
        assert!(prompt.contains("metadata_warnings: [Unknown proposal frontmatter key: owner]"));
        assert!(prompt.contains(
            "Use proposal frontmatter `dependencies` as the dependency source when present"
        ));
        assert!(prompt.contains("Treat proposal frontmatter `priority` as a soft ordering hint"));
        assert!(prompt.contains(
            "Treat proposal frontmatter `references` as supplemental analysis context only"
        ));
    }

    #[test]
    fn test_build_prompt_with_selected_markers() {
        let analyzer = create_test_analyzer();

        let changes = vec![
            Change {
                id: "selected-a".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "unselected-b".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "selected-c".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
        ];

        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        // All changes should be included with proposal.md path (no approval markers)
        assert!(prompt.contains("- selected-a (openspec/changes/selected-a/proposal.md)"));
        assert!(prompt.contains("- selected-c (openspec/changes/selected-c/proposal.md)"));
        assert!(prompt.contains("- unselected-b (openspec/changes/unselected-b/proposal.md)"));

        // Check that instruction mentions reading proposal files
        assert!(prompt.contains("Read the proposal files at the specified paths"));
    }

    #[test]
    fn test_build_prompt_all_selected() {
        let analyzer = create_test_analyzer();

        let changes = vec![
            Change {
                id: "change-1".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "change-2".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
        ];

        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        // All should be included with proposal.md path (no approval markers)
        assert!(prompt.contains("- change-1 (openspec/changes/change-1/proposal.md)"));
        assert!(prompt.contains("- change-2 (openspec/changes/change-2/proposal.md)"));
    }

    #[test]
    fn test_build_prompt_none_selected() {
        let analyzer = create_test_analyzer();

        let changes = vec![Change {
            id: "change-1".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }];

        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        // All changes should be included with proposal.md path (no approval markers)
        assert!(prompt.contains("- change-1 (openspec/changes/change-1/proposal.md)"));

        // Check that prompt structure is still there
        assert!(prompt.contains("Analyze ONLY the changes marked with [x]"));
    }

    #[test]
    fn test_prompt_clarifies_dependency_vs_order() {
        let analyzer = create_test_analyzer();

        let changes = vec![create_test_change("a")];
        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        // Verify prompt clarifies that dependencies are REQUIRED relationships
        assert!(prompt.contains("REQUIRED"));
        assert!(prompt.contains("artifacts, specs, or APIs"));

        // Verify prompt explains order is for recommended sequence
        assert!(prompt.contains("recommended execution"));

        // Verify prompt warns against confusing priority with dependency
        assert!(prompt.contains("DO NOT include dependencies based on priority"));
    }

    #[test]
    fn test_validate_dependency_strict_criteria() {
        let analyzer = create_test_analyzer();

        // Valid case: b requires a's API
        let mut deps_valid = HashMap::new();
        deps_valid.insert("b".to_string(), vec!["a".to_string()]);

        let result_valid = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps_valid,
            groups: None,
        };

        // Should pass validation (dependency graph is valid)
        assert!(analyzer
            .validate_dependency_graph(&result_valid, &[])
            .is_ok());

        // Invalid case: self-dependency (still caught)
        let mut deps_invalid = HashMap::new();
        deps_invalid.insert("a".to_string(), vec!["a".to_string()]);

        let result_invalid = AnalysisResult {
            order: vec!["a".to_string()],
            dependencies: deps_invalid,
            groups: None,
        };

        // Should fail validation (self-dependency)
        assert!(analyzer
            .validate_dependency_graph(&result_invalid, &[])
            .is_err());
    }

    #[test]
    fn test_order_can_differ_from_dependency_graph() {
        let analyzer = create_test_analyzer();

        // Case: a and b are independent, but order suggests b before a (by priority)
        let result = AnalysisResult {
            order: vec!["b".to_string(), "a".to_string(), "c".to_string()],
            dependencies: HashMap::new(), // No dependencies!
            groups: None,
        };

        let changes = vec![
            create_test_change("a"),
            create_test_change("b"),
            create_test_change("c"),
        ];

        // Should validate successfully - order is just a recommendation
        assert!(analyzer.validate_change_ids(&result, &changes).is_ok());
        assert!(analyzer.validate_dependency_graph(&result, &[]).is_ok());
    }

    #[test]
    fn test_build_prompt_with_inflight_changes() {
        let analyzer = create_test_analyzer();

        let queued_changes = vec![
            Change {
                id: "queued-a".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "queued-b".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
        ];

        let in_flight_ids = vec!["inflight-1".to_string(), "inflight-2".to_string()];

        let prompt = analyzer.build_parallelization_prompt(&queued_changes, &in_flight_ids);

        // Verify queued changes are in the main list (no approval markers)
        assert!(prompt.contains("- queued-a (openspec/changes/queued-a/proposal.md)"));
        assert!(prompt.contains("- queued-b (openspec/changes/queued-b/proposal.md)"));

        // Verify in-flight changes are in the executing section
        assert!(prompt.contains("Currently executing changes"));
        assert!(prompt.contains("- inflight-1 (openspec/changes/inflight-1/proposal.md)"));
        assert!(prompt.contains("- inflight-2 (openspec/changes/inflight-2/proposal.md)"));

        // Verify instructions about in-flight changes
        assert!(prompt.contains("NOT selectable"));
        assert!(prompt.contains("DO NOT include currently executing changes in the order"));
        assert!(prompt.contains("Dependencies CAN reference currently executing changes"));
        assert!(prompt.contains("`dependencies` may reference ONLY queued change IDs and explicitly listed in-flight IDs"));
        assert!(prompt.contains("NEVER reference unrelated active changes, archived changes"));
    }

    #[test]
    fn test_validate_dependency_graph_with_inflight() {
        let analyzer = create_test_analyzer();

        // Dependency on an in-flight change (not in order) should be valid
        let mut deps = HashMap::new();
        deps.insert("b".to_string(), vec!["inflight-x".to_string()]);
        let result = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps,
            groups: None,
        };

        let in_flight_ids = vec!["inflight-x".to_string()];
        let validation = analyzer.validate_dependency_graph(&result, &in_flight_ids);
        assert!(
            validation.is_ok(),
            "Dependency on in-flight change should be valid"
        );
    }

    #[test]
    fn test_validate_dependency_graph_invalid_inflight_ref() {
        let analyzer = create_test_analyzer();

        // Dependency on an ID that is neither in order nor in-flight should fail
        let mut deps = HashMap::new();
        deps.insert("b".to_string(), vec!["nonexistent".to_string()]);
        let result = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps,
            groups: None,
        };

        let in_flight_ids = vec!["inflight-x".to_string()];
        let validation = analyzer.validate_dependency_graph(&result, &in_flight_ids);
        assert!(
            validation.is_err(),
            "Dependency on unknown ID should be rejected"
        );

        let error_text = validation.err().unwrap().to_string();
        assert!(error_text.contains("Invalid dependency reference"));
        assert!(error_text.contains("nonexistent"));
        assert!(error_text.contains("allowed_queued_ids"));
        assert!(error_text.contains("allowed_in_flight_ids"));
    }

    #[test]
    fn test_decorate_dependency_error_classifies_archived_dependency() {
        let analyzer = create_test_analyzer();
        let changes = vec![create_test_change("change-a")];
        let in_flight_ids = vec!["inflight-x".to_string()];
        let archived_ids = HashSet::from(["archived-x".to_string()]);
        let err = "Invalid dependency reference: change 'change-a' depends on 'archived-x' outside allowed dependency targets";

        let decorated = analyzer.decorate_dependency_error_with_archive_context(
            err,
            &changes,
            &in_flight_ids,
            &archived_ids,
        );

        assert!(decorated.contains("dependency_target_classification"));
        assert!(decorated.contains("class:'archived'"));
    }

    #[test]
    fn test_collect_archived_change_ids_strips_date_prefix() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let archived_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("archive")
            .join("2026-04-29-sample-change");
        std::fs::create_dir_all(&archived_dir).unwrap();
        std::fs::write(archived_dir.join("proposal.md"), "# Archived").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        let archived_ids = analyzer.collect_archived_change_ids();
        std::env::set_current_dir(original_dir).unwrap();

        assert!(archived_ids.contains("sample-change"));
    }

    #[tokio::test]
    async fn test_single_change_fast_path_preserves_metadata_dependency() {
        let analyzer = create_test_analyzer();
        let mut change = create_test_change("route");
        change.dependencies = vec!["policy".to_string()];

        let result = analyzer
            .analyze_with_callback(&[change], &["policy".to_string()], |_| {})
            .await
            .expect("single change analysis should succeed");

        assert_eq!(result.order, vec!["route".to_string()]);
        assert_eq!(
            result.dependencies.get("route"),
            Some(&vec!["policy".to_string()])
        );
    }

    #[test]
    fn test_parse_response_unions_metadata_dependency_omitted_by_llm() {
        let analyzer = create_test_analyzer();
        let mut route = create_test_change("route");
        route.dependencies = vec!["policy".to_string()];
        let policy = create_test_change("policy");
        let changes = vec![route, policy];
        let response = r#"{"order":["policy","route"],"dependencies":{}}"#;

        let result = analyzer
            .parse_response(response, &changes, &[])
            .expect("metadata dependency should be unioned into parsed result");

        assert_eq!(
            result.dependencies.get("route"),
            Some(&vec!["policy".to_string()])
        );
    }

    #[test]
    fn test_validate_dependency_graph_accepts_active_but_not_queued_dependency() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let policy_dir = temp_dir.path().join("openspec/changes/policy");
        std::fs::create_dir_all(&policy_dir).unwrap();
        std::fs::write(policy_dir.join("proposal.md"), "# Policy").unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut route = create_test_change("route");
        route.dependencies = vec!["policy".to_string()];
        let result =
            analyzer.parse_response(r#"{"order":["route"],"dependencies":{}}"#, &[route], &[]);

        std::env::set_current_dir(original_dir).unwrap();
        assert!(
            result.is_ok(),
            "active-but-not-queued metadata dependency should be retained for scheduler gating"
        );
        assert_eq!(
            result.unwrap().dependencies.get("route"),
            Some(&vec!["policy".to_string()])
        );
    }

    #[test]
    fn test_validate_dependency_graph_accepts_archived_dependency() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let archived_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("archive")
            .join("2026-04-29-contracts");
        std::fs::create_dir_all(&archived_dir).unwrap();
        std::fs::write(archived_dir.join("proposal.md"), "# Archived").unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut route = create_test_change("route");
        route.dependencies = vec!["contracts".to_string()];
        let result =
            analyzer.parse_response(r#"{"order":["route"],"dependencies":{}}"#, &[route], &[]);

        std::env::set_current_dir(original_dir).unwrap();
        assert!(result.is_ok(), "archived dependency should be accepted");
    }

    #[test]
    fn test_validate_dependency_graph_reports_rejected_dependency() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let rejected_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("contracts");
        std::fs::create_dir_all(&rejected_dir).unwrap();
        std::fs::write(rejected_dir.join("proposal.md"), "# Rejected").unwrap();
        std::fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED").unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut route = create_test_change("route");
        route.dependencies = vec!["contracts".to_string()];
        let error = analyzer
            .parse_response(r#"{"order":["route"],"dependencies":{}}"#, &[route], &[])
            .expect_err("rejected dependency should fail closed")
            .to_string();

        std::env::set_current_dir(original_dir).unwrap();
        assert!(error.contains("Rejected dependency reference"));
        assert!(error.contains("classified as rejected dependency target"));
    }

    #[test]
    fn test_validate_dependency_graph_reports_missing_dependency() {
        let analyzer = create_test_analyzer();
        let mut route = create_test_change("route");
        route.dependencies = vec!["ghost".to_string()];

        let error = analyzer
            .parse_response(r#"{"order":["route"],"dependencies":{}}"#, &[route], &[])
            .expect_err("missing dependency should fail closed")
            .to_string();

        assert!(error.contains("Missing dependency reference"));
        assert!(error.contains("classified as missing dependency target"));
        assert!(!error.contains("Analysis returned invalid JSON"));
    }

    #[test]
    fn test_build_prompt_without_inflight_changes() {
        let analyzer = create_test_analyzer();

        let queued_changes = vec![Change {
            id: "queued-a".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }];

        let in_flight_ids: Vec<String> = vec![];

        let prompt = analyzer.build_parallelization_prompt(&queued_changes, &in_flight_ids);

        // Verify queued changes are in the main list (no approval markers)
        assert!(prompt.contains("- queued-a (openspec/changes/queued-a/proposal.md)"));

        // Verify no executing section when in_flight_ids is empty
        assert!(!prompt.contains("Currently executing changes"));
    }
}
