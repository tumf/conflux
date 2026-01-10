//! Parallelization analyzer for identifying independent changes.
//!
//! Uses LLM-based analysis to determine which changes can be executed
//! in parallel and what dependencies exist between them.

use crate::agent::AgentRunner;
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

        // Log dependencies for each change
        for c in changes {
            if c.dependencies.is_empty() {
                info!("  - {}: no dependencies", c.id);
            } else {
                info!("  - {}: depends on [{}]", c.id, c.dependencies.join(", "));
            }
        }

        debug!("Analysis prompt: {}", prompt);

        // Call LLM for analysis
        let response = self.agent.analyze_dependencies(&prompt).await?;
        debug!("LLM response: {}", response);

        // Parse JSON response
        let result = self.parse_response(&response, changes)?;

        // Validate dependency graph (no circular dependencies)
        self.validate_dependency_graph(&result.groups)?;

        // Return groups in topological order
        let sorted = self.topological_sort(result.groups)?;

        info!("Analysis complete: {} groups identified", sorted.len());

        Ok(sorted)
    }

    /// Build the prompt for parallelization analysis
    fn build_parallelization_prompt(&self, changes: &[Change]) -> String {
        let change_list: String = changes
            .iter()
            .map(|c| {
                let deps_str = if c.dependencies.is_empty() {
                    "none".to_string()
                } else {
                    c.dependencies.join(", ")
                };
                format!(
                    "- {}: {}/{} tasks completed, depends on: [{}]",
                    c.id, c.completed_tasks, c.total_tasks, deps_str
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Analyze the following OpenSpec changes and group them for parallel execution.

Changes to analyze:
{change_list}

Instructions:
1. RESPECT EXPLICIT DEPENDENCIES: Changes with declared dependencies MUST wait for those dependencies to complete
2. Group changes that have no dependencies on each other for parallel execution
3. Changes with the same dependencies can be grouped together
4. A change can only start after ALL its dependencies are complete

Return JSON in this exact format:
{{
  "groups": [
    {{"id": 1, "changes": ["change-id-1", "change-id-2"], "depends_on": []}},
    {{"id": 2, "changes": ["change-id-3"], "depends_on": [1]}}
  ]
}}

Rules:
- Every change ID must appear exactly once
- Group IDs start at 1 and increase
- depends_on references group IDs, not change IDs
- Groups with empty depends_on can start immediately (no dependencies)
- A change with dependency X must be in a group that depends_on the group containing X
- Return valid JSON only, no additional text"#
        )
    }

    /// Parse LLM response into AnalysisResult
    fn parse_response(&self, response: &str, changes: &[Change]) -> Result<AnalysisResult> {
        // Try to extract JSON from response (may have surrounding text)
        let json_str = self.extract_json(response)?;

        // Parse JSON
        let result: AnalysisResult = serde_json::from_str(&json_str).map_err(|e| {
            OrchestratorError::Parse(format!("Failed to parse parallelization response: {}", e))
        })?;

        // Validate all change IDs exist
        self.validate_change_ids(&result, changes)?;

        Ok(result)
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
}
