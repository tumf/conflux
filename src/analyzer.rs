//! Parallelization analyzer for identifying independent changes.
//!
//! Uses LLM-based analysis to determine which changes can be executed
//! in parallel and what dependencies exist between them.

use crate::ai_command_runner::OutputLine as AiOutputLine;
use crate::error::{OrchestratorError, Result};
use crate::openspec::Change;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

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
    pub async fn analyze(&self, changes: &[Change]) -> Result<AnalysisResult> {
        self.analyze_with_callback(changes, |_| {}).await
    }

    /// Analyze changes with output callback and return raw analysis result.
    pub async fn analyze_with_callback<F>(
        &self,
        changes: &[Change],
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

        // For single change, no parallelization needed
        if changes.len() == 1 {
            return Ok(AnalysisResult {
                order: vec![changes[0].id.clone()],
                dependencies: HashMap::new(),
                groups: None,
            });
        }

        // Build prompt for LLM analysis
        let prompt = self.build_parallelization_prompt(changes);
        info!("Analyzing {} changes for parallelization", changes.len());
        for c in changes {
            info!("  - {}", c.id);
        }
        debug!("Analysis prompt: {}", prompt);

        // Execute analysis command and collect output
        let (full_output, status) = self
            .execute_analysis_command(&prompt, changes, &mut on_output)
            .await?;

        // Parse and validate the output
        let result = self.parse_and_validate_output(&full_output, &status, changes)?;

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
        let command = crate::config::OrchestratorConfig::expand_prompt(template, prompt);
        let (mut child, mut rx) = self
            .ai_runner
            .execute_streaming_with_retry(&command, None)
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
    fn parse_and_validate_output(
        &self,
        full_output: &str,
        status: &std::process::ExitStatus,
        changes: &[Change],
    ) -> Result<AnalysisResult> {
        // Extract result from stream-json format if applicable
        let response = self.extract_stream_json_result(full_output);
        debug!("LLM response: {}", response);

        // Parse JSON response with strict validation
        let result = self.parse_response(&response, changes).map_err(|e| {
            let preview = response.chars().take(200).collect::<String>();
            let change_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
            OrchestratorError::Parse(format!(
                "Analysis returned invalid JSON for changes [{}] (exit code: {:?}): {}. Response preview: {}",
                change_ids.join(", "),
                status.code(),
                e,
                preview
            ))
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
        // Use the new analyze_with_callback and convert to groups
        let result = self.analyze_with_callback(changes, on_output).await?;

        if result.order.is_empty() {
            return Ok(Vec::new());
        }

        // Convert order-based result to group-based format for backward compatibility
        let groups = self.order_to_groups(&result);
        info!("Analysis complete: {} groups identified", groups.len());
        Ok(groups)
    }

    /// Extract the result from stream-json output format
    fn extract_stream_json_result(&self, output: &str) -> String {
        // Try to find and parse the result line from stream-json output
        for line in output.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Try to parse as JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                // Check if this is a result message
                if json.get("type").and_then(|t| t.as_str()) == Some("result") {
                    if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                        return result.to_string();
                    }
                }
                // Also check for assistant message content
                if json.get("type").and_then(|t| t.as_str()) == Some("assistant") {
                    if let Some(message) = json.get("message") {
                        if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                            for item in content {
                                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                    return text.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
        // Fallback: return entire output if not stream-json format
        output.to_string()
    }

    /// Build the prompt for parallelization analysis
    ///
    /// Formats selected changes (with `is_approved = true`) as a list with:
    /// - `[x]` marker to indicate selection status
    /// - Full proposal file path for each change (e.g., `openspec/changes/{id}/proposal.md`)
    ///
    /// This makes it clear to the LLM which changes need analysis and where
    /// to find their proposal files.
    fn build_parallelization_prompt(&self, changes: &[Change]) -> String {
        let change_list: String = changes
            .iter()
            .filter(|c| c.is_approved) // Only selected/approved changes
            .map(|c| format!("[x] {} (openspec/changes/{}/proposal.md)", c.id, c.id))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"You are planning the execution order for OpenSpec changes.

Analyze ONLY the changes marked with [x] below.
Read the proposal files at the specified paths to understand their dependencies:

{change_list}

Your task:
1. Read each change's proposal.md at the given path to understand what it does
2. Identify dependencies between these changes
3. Determine the recommended execution order (considering dependencies and priorities)
4. Return execution order and dependencies

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
- `dependencies`: Object mapping change IDs to arrays of their REQUIRED dependency IDs
  - STRICT CRITERIA: Only include a dependency if one change REQUIRES the artifacts, specs, or APIs from another change to function
  - DO NOT include dependencies based on priority, preferred order, or efficiency alone
  - Example of REQUIRED dependency: "change-b implements a feature using the API defined in change-a"
  - Example of NOT a dependency: "change-a should ideally be done before change-b for efficiency"
- Every change ID must appear exactly once in `order`
- Dependencies are hard constraints: a change CANNOT start until all its dependencies are merged to base
- Order preferences without required dependencies should be reflected in `order` only, not in `dependencies`
- Return valid JSON only, no markdown, no explanation"#
        )
    }

    /// Parse LLM response into AnalysisResult
    fn parse_response(&self, response: &str, changes: &[Change]) -> Result<AnalysisResult> {
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

        // Validate dependency graph (no circular dependencies)
        self.validate_dependency_graph(&result)?;

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

        // If it starts with {, assume it's pure JSON
        if trimmed.starts_with('{') {
            return Ok(trimmed.to_string());
        }

        // Try to extract from markdown code block
        if let Some(start) = trimmed.find("```json") {
            let after_marker = &trimmed[start + 7..];
            if let Some(end) = after_marker.find("```") {
                return Ok(after_marker[..end].trim().to_string());
            }
        }

        // Try to extract from generic code block
        if let Some(start) = trimmed.find("```") {
            let after_marker = &trimmed[start + 3..];
            // Skip language identifier if present
            let content_start = after_marker.find('\n').unwrap_or(0);
            let content = &after_marker[content_start..];
            if let Some(end) = content.find("```") {
                return Ok(content[..end].trim().to_string());
            }
        }

        // Try to find JSON object anywhere in response
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                if end > start {
                    return Ok(trimmed[start..=end].to_string());
                }
            }
        }

        Err(OrchestratorError::Parse(
            "Could not extract JSON from response".to_string(),
        ))
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
    fn validate_dependency_graph(&self, result: &AnalysisResult) -> Result<()> {
        // Check for self-dependencies
        for (change_id, deps) in &result.dependencies {
            if deps.contains(change_id) {
                return Err(OrchestratorError::Parse(format!(
                    "Self-dependency detected: change '{}' depends on itself",
                    change_id
                )));
            }

            // Check all dependencies exist in order
            for dep_id in deps {
                if !result.order.contains(dep_id) {
                    return Err(OrchestratorError::Parse(format!(
                        "Invalid dependency reference: change '{}' depends on non-existent change '{}'",
                        change_id, dep_id
                    )));
                }
            }
        }

        // Check for circular dependencies using DFS
        self.detect_cycles_from_dependencies(&result.dependencies)?;

        Ok(())
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
/// Converts group-level dependencies into change-level dependencies.
/// When a group depends on another group, all changes in the dependent group
/// are considered to depend on all changes in the prerequisite group.
///
/// # Arguments
///
/// * `groups` - The parallel execution groups from LLM analysis
///
/// # Returns
///
/// A HashMap where keys are change IDs and values are lists of change IDs
/// that the key change depends on.
#[allow(dead_code)] // Used by deprecated group-based analysis
pub fn extract_change_dependencies(groups: &[ParallelGroup]) -> HashMap<String, Vec<String>> {
    let mut deps: HashMap<String, Vec<String>> = HashMap::new();
    let mut group_changes: HashMap<u32, Vec<String>> = HashMap::new();

    // Collect changes by group ID
    for group in groups {
        group_changes.insert(group.id, group.changes.clone());
    }

    // For each group, map its dependencies to change-level dependencies
    for group in groups {
        for dep_group_id in &group.depends_on {
            if let Some(dep_changes) = group_changes.get(dep_group_id) {
                for change_id in &group.changes {
                    deps.entry(change_id.clone())
                        .or_default()
                        .extend(dep_changes.iter().cloned());
                }
            }
        }
    }

    deps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            is_approved: true,
            dependencies: Vec::new(),
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
        };
        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
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

        let validation = analyzer.validate_dependency_graph(&result);
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

        let validation = analyzer.validate_dependency_graph(&result);
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

        let validation = analyzer.validate_dependency_graph(&result);
        assert!(validation.is_err());
    }

    #[test]
    fn test_extract_change_dependencies_empty() {
        let groups: Vec<ParallelGroup> = vec![];
        let deps = extract_change_dependencies(&groups);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_extract_change_dependencies_no_dependencies() {
        let groups = vec![
            ParallelGroup {
                id: 1,
                changes: vec!["a".to_string(), "b".to_string()],
                depends_on: Vec::new(),
            },
            ParallelGroup {
                id: 2,
                changes: vec!["c".to_string()],
                depends_on: Vec::new(),
            },
        ];
        let deps = extract_change_dependencies(&groups);
        // No group depends on another, so no change-level dependencies
        assert!(deps.is_empty());
    }

    #[test]
    fn test_extract_change_dependencies_simple() {
        let groups = vec![
            ParallelGroup {
                id: 1,
                changes: vec!["a".to_string(), "b".to_string()],
                depends_on: Vec::new(),
            },
            ParallelGroup {
                id: 2,
                changes: vec!["c".to_string()],
                depends_on: vec![1],
            },
        ];
        let deps = extract_change_dependencies(&groups);

        // "c" depends on "a" and "b" (all changes in group 1)
        assert!(deps.contains_key("c"));
        let c_deps = deps.get("c").unwrap();
        assert!(c_deps.contains(&"a".to_string()));
        assert!(c_deps.contains(&"b".to_string()));

        // "a" and "b" have no dependencies
        assert!(!deps.contains_key("a"));
        assert!(!deps.contains_key("b"));
    }

    #[test]
    fn test_extract_change_dependencies_chain() {
        // Group 1 -> Group 2 -> Group 3
        let groups = vec![
            ParallelGroup {
                id: 1,
                changes: vec!["a".to_string()],
                depends_on: Vec::new(),
            },
            ParallelGroup {
                id: 2,
                changes: vec!["b".to_string()],
                depends_on: vec![1],
            },
            ParallelGroup {
                id: 3,
                changes: vec!["c".to_string()],
                depends_on: vec![2],
            },
        ];
        let deps = extract_change_dependencies(&groups);

        // "b" depends on "a"
        assert!(deps.contains_key("b"));
        assert!(deps.get("b").unwrap().contains(&"a".to_string()));

        // "c" depends on "b"
        assert!(deps.contains_key("c"));
        assert!(deps.get("c").unwrap().contains(&"b".to_string()));

        // "a" has no dependencies
        assert!(!deps.contains_key("a"));
    }

    #[test]
    fn test_build_prompt_with_selected_markers() {
        let analyzer = create_test_analyzer();

        let changes = vec![
            Change {
                id: "selected-a".to_string(),
                is_approved: true,
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
            },
            Change {
                id: "unselected-b".to_string(),
                is_approved: false,
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
            },
            Change {
                id: "selected-c".to_string(),
                is_approved: true,
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
            },
        ];

        let prompt = analyzer.build_parallelization_prompt(&changes);

        // Check that selected changes are marked with [x] and include proposal.md path
        assert!(prompt.contains("[x] selected-a (openspec/changes/selected-a/proposal.md)"));
        assert!(prompt.contains("[x] selected-c (openspec/changes/selected-c/proposal.md)"));

        // Check that unselected change is NOT included
        assert!(!prompt.contains("unselected-b"));

        // Check that instruction mentions "marked with [x]"
        assert!(prompt.contains("marked with [x]"));

        // Check that instruction mentions reading proposal files
        assert!(prompt.contains("Read the proposal files at the specified paths"));
    }

    #[test]
    fn test_build_prompt_all_selected() {
        let analyzer = create_test_analyzer();

        let changes = vec![
            Change {
                id: "change-1".to_string(),
                is_approved: true,
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
            },
            Change {
                id: "change-2".to_string(),
                is_approved: true,
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
            },
        ];

        let prompt = analyzer.build_parallelization_prompt(&changes);

        // All should be included with [x] marker and proposal.md path
        assert!(prompt.contains("[x] change-1 (openspec/changes/change-1/proposal.md)"));
        assert!(prompt.contains("[x] change-2 (openspec/changes/change-2/proposal.md)"));
    }

    #[test]
    fn test_build_prompt_none_selected() {
        let analyzer = create_test_analyzer();

        let changes = vec![Change {
            id: "change-1".to_string(),
            is_approved: false,
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
        }];

        let prompt = analyzer.build_parallelization_prompt(&changes);

        // No changes should be included
        assert!(!prompt.contains("change-1"));

        // But structure should still be there
        assert!(prompt.contains("Analyze ONLY the changes marked with [x]"));
    }

    #[test]
    fn test_prompt_clarifies_dependency_vs_order() {
        let analyzer = create_test_analyzer();

        let changes = vec![create_test_change("a")];
        let prompt = analyzer.build_parallelization_prompt(&changes);

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
        assert!(analyzer.validate_dependency_graph(&result_valid).is_ok());

        // Invalid case: self-dependency (still caught)
        let mut deps_invalid = HashMap::new();
        deps_invalid.insert("a".to_string(), vec!["a".to_string()]);

        let result_invalid = AnalysisResult {
            order: vec!["a".to_string()],
            dependencies: deps_invalid,
            groups: None,
        };

        // Should fail validation (self-dependency)
        assert!(analyzer.validate_dependency_graph(&result_invalid).is_err());
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
        assert!(analyzer.validate_dependency_graph(&result).is_ok());
    }
}
