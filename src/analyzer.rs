//! Parallelization analyzer for identifying independent changes.
//!
//! Uses LLM-based analysis to determine which changes can be executed
//! in parallel and what dependencies exist between them.

use crate::agent::{AgentRunner, OutputLine};
use crate::error::{OrchestratorError, Result};
use crate::openspec::Change;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
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
    /// Parallel execution groups
    pub groups: Vec<ParallelGroup>,
    /// Dependencies between changes (change_id -> list of dependencies)
    #[serde(default)]
    pub dependencies: HashMap<String, Vec<String>>,
}

/// Analyzer for determining parallel execution groups
pub struct ParallelizationAnalyzer {
    agent: AgentRunner,
}

impl ParallelizationAnalyzer {
    /// Create a new analyzer with the given agent runner
    pub fn new(agent: AgentRunner) -> Self {
        Self { agent }
    }

    /// Analyze changes and return parallel execution groups
    ///
    /// Returns groups in topological order (dependencies first).
    pub async fn analyze_groups(&self, changes: &[Change]) -> Result<Vec<ParallelGroup>> {
        self.analyze_groups_with_callback(changes, |_| {}).await
    }

    /// Analyze changes and return parallel execution groups with output callback
    ///
    /// The callback is called for each line of output from the analysis command.
    /// Returns groups in topological order (dependencies first).
    pub async fn analyze_groups_with_callback<F>(
        &self,
        changes: &[Change],
        mut on_output: F,
    ) -> Result<Vec<ParallelGroup>>
    where
        F: FnMut(String),
    {
        if changes.is_empty() {
            return Ok(Vec::new());
        }

        // For single change, no parallelization needed
        if changes.len() == 1 {
            return Ok(vec![ParallelGroup {
                id: 1,
                changes: vec![changes[0].id.clone()],
                depends_on: Vec::new(),
            }]);
        }

        // Build prompt for LLM analysis
        let prompt = self.build_parallelization_prompt(changes);
        info!("Analyzing {} changes for parallelization", changes.len());
        for c in changes {
            info!("  - {}", c.id);
        }
        debug!("Analysis prompt: {}", prompt);

        // Call LLM for analysis with streaming output
        let (mut child, mut rx) = self.agent.analyze_dependencies_streaming(&prompt).await?;

        // Collect output while streaming to callback
        let mut full_output = String::new();
        while let Some(line) = rx.recv().await {
            let text = match &line {
                OutputLine::Stdout(s) | OutputLine::Stderr(s) => s.clone(),
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

        // Extract result from stream-json format if applicable
        let response = self.extract_stream_json_result(&full_output);
        debug!("LLM response: {}", response);

        // Parse JSON response with strict validation
        // Even if exit code is 0, fail if JSON is invalid
        let result = self.parse_response(&response, changes).map_err(|e| {
            // Provide context from output for debugging
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

        // Now check exit code after successful JSON parsing
        if !status.success() {
            let change_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
            return Err(OrchestratorError::AgentCommand(format!(
                "Analysis failed for changes [{}] with exit code: {:?}",
                change_ids.join(", "),
                status.code()
            )));
        }

        // Validate dependency graph (no circular dependencies)
        self.validate_dependency_graph(&result.groups)?;

        // Return groups in topological order
        let sorted = self.topological_sort(result.groups)?;

        info!("Analysis complete: {} groups identified", sorted.len());

        Ok(sorted)
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
    /// - Directory path for each change (e.g., `openspec/changes/{id}/`)
    ///
    /// This makes it clear to the LLM which changes need analysis and where
    /// to find their proposal files.
    fn build_parallelization_prompt(&self, changes: &[Change]) -> String {
        let change_list: String = changes
            .iter()
            .filter(|c| c.is_approved) // Only selected/approved changes
            .map(|c| format!("[x] {} (openspec/changes/{}/)", c.id, c.id))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"You are planning the execution order for OpenSpec changes.

Analyze these selected changes (marked with [x]).
Read the proposal files in the specified directories to understand their dependencies:

{change_list}

Your task:
1. Read each change's proposal.md at the given path to understand what it does
2. Identify dependencies between these changes
3. Group changes that can run in parallel (no dependencies on each other)
4. Order groups so dependencies are completed before dependents

Return ONLY valid JSON in this exact format:
{{
  "groups": [
    {{"id": 1, "changes": ["change-a", "change-b"], "depends_on": []}},
    {{"id": 2, "changes": ["change-c"], "depends_on": [1]}}
  ]
}}

Rules:
- Every change ID must appear exactly once
- Group IDs start at 1 and increment
- depends_on lists group IDs (not change IDs) that must complete first
- Groups with empty depends_on can start immediately
- Changes with no dependencies on each other should be in the same group
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

        Ok(result)
    }

    /// Validate JSON schema for analysis result.
    ///
    /// Checks that:
    /// - JSON is parseable
    /// - `groups` key exists
    /// - `groups` is an array
    fn validate_json_schema(&self, json_str: &str) -> Result<()> {
        // Parse as generic JSON value first
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| OrchestratorError::Parse(format!("Invalid JSON syntax: {}", e)))?;

        // Check for required `groups` key
        if !value.is_object() {
            return Err(OrchestratorError::Parse(
                "JSON root must be an object".to_string(),
            ));
        }

        let groups = value.get("groups").ok_or_else(|| {
            OrchestratorError::Parse("Missing required key 'groups' in JSON".to_string())
        })?;

        // Check that groups is an array
        if !groups.is_array() {
            return Err(OrchestratorError::Parse(
                "Key 'groups' must be an array".to_string(),
            ));
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

        for group in &result.groups {
            for change_id in &group.changes {
                // Check ID exists
                if !valid_ids.contains(change_id.as_str()) {
                    return Err(OrchestratorError::Parse(format!(
                        "Unknown change ID in response: {}",
                        change_id
                    )));
                }

                // Check for duplicates
                if seen_ids.contains(change_id.as_str()) {
                    return Err(OrchestratorError::Parse(format!(
                        "Duplicate change ID in response: {}",
                        change_id
                    )));
                }
                seen_ids.insert(change_id.as_str());
            }
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
    fn validate_dependency_graph(&self, groups: &[ParallelGroup]) -> Result<()> {
        let group_ids: HashSet<u32> = groups.iter().map(|g| g.id).collect();

        // Check all depends_on references are valid
        for group in groups {
            for dep_id in &group.depends_on {
                if !group_ids.contains(dep_id) {
                    return Err(OrchestratorError::Parse(format!(
                        "Invalid dependency reference: group {} depends on non-existent group {}",
                        group.id, dep_id
                    )));
                }
                if *dep_id == group.id {
                    return Err(OrchestratorError::Parse(format!(
                        "Self-dependency detected: group {} depends on itself",
                        group.id
                    )));
                }
            }
        }

        // Check for circular dependencies using DFS
        self.detect_cycles(groups)?;

        Ok(())
    }

    /// Detect cycles in the dependency graph using DFS
    fn detect_cycles(&self, groups: &[ParallelGroup]) -> Result<()> {
        let mut adjacency: HashMap<u32, Vec<u32>> = HashMap::new();
        for group in groups {
            adjacency.insert(group.id, group.depends_on.clone());
        }

        let mut visited: HashSet<u32> = HashSet::new();
        let mut rec_stack: HashSet<u32> = HashSet::new();

        for group in groups {
            if !visited.contains(&group.id)
                && self.has_cycle(group.id, &adjacency, &mut visited, &mut rec_stack)
            {
                return Err(OrchestratorError::Parse(
                    "Circular dependency detected in parallelization groups".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// DFS helper for cycle detection
    fn has_cycle(
        &self,
        node: u32,
        adjacency: &HashMap<u32, Vec<u32>>,
        visited: &mut HashSet<u32>,
        rec_stack: &mut HashSet<u32>,
    ) -> bool {
        visited.insert(node);
        rec_stack.insert(node);

        if let Some(deps) = adjacency.get(&node) {
            for &dep in deps {
                if !visited.contains(&dep) {
                    if self.has_cycle(dep, adjacency, visited, rec_stack) {
                        return true;
                    }
                } else if rec_stack.contains(&dep) {
                    return true;
                }
            }
        }

        rec_stack.remove(&node);
        false
    }

    /// Sort groups in topological order (dependencies first)
    fn topological_sort(&self, groups: Vec<ParallelGroup>) -> Result<Vec<ParallelGroup>> {
        if groups.is_empty() {
            return Ok(Vec::new());
        }

        // Build in-degree map
        let mut in_degree: HashMap<u32, usize> = HashMap::new();
        let mut group_map: HashMap<u32, ParallelGroup> = HashMap::new();
        let mut dependents: HashMap<u32, Vec<u32>> = HashMap::new();

        for group in groups {
            in_degree.insert(group.id, group.depends_on.len());
            for &dep_id in &group.depends_on {
                dependents.entry(dep_id).or_default().push(group.id);
            }
            group_map.insert(group.id, group);
        }

        // Kahn's algorithm
        let mut queue: VecDeque<u32> = in_degree
            .iter()
            .filter_map(|(&id, &deg)| if deg == 0 { Some(id) } else { None })
            .collect();

        let mut sorted: Vec<ParallelGroup> = Vec::new();

        while let Some(id) = queue.pop_front() {
            if let Some(group) = group_map.remove(&id) {
                sorted.push(group);
            }

            if let Some(deps) = dependents.get(&id) {
                for &dep_id in deps {
                    if let Some(deg) = in_degree.get_mut(&dep_id) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(dep_id);
                        }
                    }
                }
            }
        }

        // If not all groups were processed, there's a cycle (shouldn't happen after validation)
        if sorted.len() != in_degree.len() {
            warn!("Topological sort incomplete - cycle may exist");
        }

        Ok(sorted)
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

    #[test]
    fn test_extract_json_pure() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

        let json = r#"{"groups": [{"id": 1, "changes": ["a"], "depends_on": []}]}"#;
        let result = analyzer.extract_json(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_json_markdown() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

        let response = r#"Here's the analysis:

```json
{"groups": [{"id": 1, "changes": ["a"], "depends_on": []}]}
```

That's all."#;
        let result = analyzer.extract_json(response);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_change_ids_missing() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

        let changes = vec![create_test_change("a"), create_test_change("b")];
        let result = AnalysisResult {
            groups: vec![ParallelGroup {
                id: 1,
                changes: vec!["a".to_string()], // Missing "b"
                depends_on: Vec::new(),
            }],
            dependencies: HashMap::new(),
        };

        let validation = analyzer.validate_change_ids(&result, &changes);
        assert!(validation.is_err());
    }

    #[test]
    fn test_validate_change_ids_duplicate() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

        let changes = vec![create_test_change("a"), create_test_change("b")];
        let result = AnalysisResult {
            groups: vec![
                ParallelGroup {
                    id: 1,
                    changes: vec!["a".to_string()],
                    depends_on: Vec::new(),
                },
                ParallelGroup {
                    id: 2,
                    changes: vec!["a".to_string(), "b".to_string()], // Duplicate "a"
                    depends_on: vec![1],
                },
            ],
            dependencies: HashMap::new(),
        };

        let validation = analyzer.validate_change_ids(&result, &changes);
        assert!(validation.is_err());
    }

    #[test]
    fn test_validate_dependency_graph_valid() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

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
        ];

        let result = analyzer.validate_dependency_graph(&groups);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_dependency_graph_self_reference() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

        let groups = vec![ParallelGroup {
            id: 1,
            changes: vec!["a".to_string()],
            depends_on: vec![1], // Self-reference
        }];

        let result = analyzer.validate_dependency_graph(&groups);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_dependency_graph_cycle() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

        let groups = vec![
            ParallelGroup {
                id: 1,
                changes: vec!["a".to_string()],
                depends_on: vec![2], // Cycle: 1 -> 2 -> 1
            },
            ParallelGroup {
                id: 2,
                changes: vec!["b".to_string()],
                depends_on: vec![1],
            },
        ];

        let result = analyzer.validate_dependency_graph(&groups);
        assert!(result.is_err());
    }

    #[test]
    fn test_topological_sort_simple() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

        let groups = vec![
            ParallelGroup {
                id: 2,
                changes: vec!["b".to_string()],
                depends_on: vec![1],
            },
            ParallelGroup {
                id: 1,
                changes: vec!["a".to_string()],
                depends_on: Vec::new(),
            },
        ];

        let sorted = analyzer.topological_sort(groups).unwrap();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].id, 1); // Group 1 should come first
        assert_eq!(sorted[1].id, 2);
    }

    #[test]
    fn test_topological_sort_complex() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

        // Diamond dependency: 1 -> 2, 1 -> 3, 2 -> 4, 3 -> 4
        let groups = vec![
            ParallelGroup {
                id: 4,
                changes: vec!["d".to_string()],
                depends_on: vec![2, 3],
            },
            ParallelGroup {
                id: 3,
                changes: vec!["c".to_string()],
                depends_on: vec![1],
            },
            ParallelGroup {
                id: 2,
                changes: vec!["b".to_string()],
                depends_on: vec![1],
            },
            ParallelGroup {
                id: 1,
                changes: vec!["a".to_string()],
                depends_on: Vec::new(),
            },
        ];

        let sorted = analyzer.topological_sort(groups).unwrap();
        assert_eq!(sorted.len(), 4);
        assert_eq!(sorted[0].id, 1); // Group 1 must be first
        assert_eq!(sorted[3].id, 4); // Group 4 must be last

        // Groups 2 and 3 can be in either order
        let middle_ids: Vec<u32> = sorted[1..3].iter().map(|g| g.id).collect();
        assert!(middle_ids.contains(&2));
        assert!(middle_ids.contains(&3));
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
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

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

        // Check that selected changes are marked with [x]
        assert!(prompt.contains("[x] selected-a (openspec/changes/selected-a/)"));
        assert!(prompt.contains("[x] selected-c (openspec/changes/selected-c/)"));

        // Check that unselected change is NOT included
        assert!(!prompt.contains("unselected-b"));

        // Check that instruction mentions "marked with [x]"
        assert!(prompt.contains("marked with [x]"));

        // Check that instruction mentions reading proposal files
        assert!(prompt.contains("Read the proposal files in the specified directories"));
    }

    #[test]
    fn test_build_prompt_all_selected() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

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

        // All should be included with [x] marker
        assert!(prompt.contains("[x] change-1 (openspec/changes/change-1/)"));
        assert!(prompt.contains("[x] change-2 (openspec/changes/change-2/)"));
    }

    #[test]
    fn test_build_prompt_none_selected() {
        let agent = AgentRunner::new(crate::config::OrchestratorConfig::default());
        let analyzer = ParallelizationAnalyzer::new(agent);

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
        assert!(prompt.contains("Analyze these selected changes"));
    }
}
