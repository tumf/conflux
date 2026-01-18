//! Spec delta parsing and conflict detection module
//!
//! This module provides functionality to:
//! - Parse spec delta files from changes
//! - Detect conflicts between spec deltas across multiple changes
//! - Generate human-readable and JSON output

use crate::error::{OrchestratorError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Represents a delta operation type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeltaType {
    Added,
    Modified,
    Removed,
    Renamed { from: String },
}

/// Represents a requirement delta in a spec file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementDelta {
    /// Name of the requirement
    pub name: String,
    /// Type of operation
    pub delta_type: DeltaType,
    /// Content of the requirement (if applicable)
    pub content: Option<String>,
    /// Source change ID
    pub change_id: String,
    /// Spec file path (relative to change)
    pub spec_path: PathBuf,
}

/// Represents a conflict between two requirement deltas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// Requirement name
    pub requirement_name: String,
    /// First delta involved in conflict
    pub delta1: RequirementDelta,
    /// Second delta involved in conflict
    pub delta2: RequirementDelta,
    /// Conflict reason
    pub reason: ConflictReason,
}

/// Reason for conflict
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictReason {
    /// Same requirement modified with different content
    ContentMismatch,
    /// One change removes, another modifies/adds
    RemoveConflict,
    /// Conflicting rename operations
    RenameConflict,
}

/// Parse all spec delta files from a change directory
pub fn parse_change_deltas(change_id: &str) -> Result<Vec<RequirementDelta>> {
    let change_path = Path::new("openspec/changes").join(change_id);
    if !change_path.exists() {
        return Err(OrchestratorError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Change directory not found: {}", change_id),
        )));
    }

    let specs_path = change_path.join("specs");
    if !specs_path.exists() {
        // No specs directory means no deltas
        return Ok(Vec::new());
    }

    let mut deltas = Vec::new();
    collect_deltas_recursive(&specs_path, change_id, &mut deltas)?;
    Ok(deltas)
}

/// Recursively collect deltas from spec files
fn collect_deltas_recursive(
    dir: &Path,
    change_id: &str,
    deltas: &mut Vec<RequirementDelta>,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_deltas_recursive(&path, change_id, deltas)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
            if let Some(file_deltas) = parse_spec_file(&path, change_id)? {
                deltas.extend(file_deltas);
            }
        }
    }

    Ok(())
}

/// Parse a single spec file and extract requirement deltas
fn parse_spec_file(path: &Path, change_id: &str) -> Result<Option<Vec<RequirementDelta>>> {
    let content = fs::read_to_string(path)?;
    let mut deltas = Vec::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Check for section headers
        let delta_type = if line == "## ADDED Requirements" {
            Some(DeltaType::Added)
        } else if line == "## MODIFIED Requirements" {
            Some(DeltaType::Modified)
        } else if line == "## REMOVED Requirements" {
            Some(DeltaType::Removed)
        } else if line.starts_with("## RENAMED Requirements") {
            // For RENAMED, we need to extract the "from" name
            // Format: ### Requirement: NewName (from OldName)
            None // We'll handle this specially
        } else {
            None
        };

        if let Some(dtype) = delta_type {
            // Parse requirements under this section
            i += 1;
            while i < lines.len() {
                let req_line = lines[i].trim();
                if req_line.starts_with("## ") {
                    // New section
                    break;
                }

                if req_line.starts_with("### Requirement:") {
                    let req_name = req_line
                        .trim_start_matches("### Requirement:")
                        .trim()
                        .to_string();

                    // Collect content until next requirement or section
                    let mut content_lines = Vec::new();
                    i += 1;
                    while i < lines.len() {
                        let content_line = lines[i];
                        if content_line.trim().starts_with("### Requirement:")
                            || content_line.trim().starts_with("## ")
                        {
                            break;
                        }
                        content_lines.push(content_line);
                        i += 1;
                    }

                    let content = if matches!(dtype, DeltaType::Removed) {
                        None
                    } else {
                        Some(content_lines.join("\n"))
                    };

                    deltas.push(RequirementDelta {
                        name: req_name,
                        delta_type: dtype.clone(),
                        content,
                        change_id: change_id.to_string(),
                        spec_path: path.to_path_buf(),
                    });

                    continue;
                }

                i += 1;
            }
            continue;
        }

        // Handle RENAMED section specially
        if line.starts_with("## RENAMED Requirements") {
            i += 1;
            while i < lines.len() {
                let req_line = lines[i].trim();
                if req_line.starts_with("## ") {
                    break;
                }

                if req_line.starts_with("### Requirement:") {
                    // Format: ### Requirement: NewName (from OldName)
                    let req_text = req_line.trim_start_matches("### Requirement:").trim();
                    if let Some(from_pos) = req_text.find("(from ") {
                        let new_name = req_text[..from_pos].trim().to_string();
                        let from_text = &req_text[from_pos + 6..]; // Skip "(from "
                        let old_name = from_text.trim_end_matches(')').trim().to_string();

                        // Collect content
                        let mut content_lines = Vec::new();
                        i += 1;
                        while i < lines.len() {
                            let content_line = lines[i];
                            if content_line.trim().starts_with("### Requirement:")
                                || content_line.trim().starts_with("## ")
                            {
                                break;
                            }
                            content_lines.push(content_line);
                            i += 1;
                        }

                        deltas.push(RequirementDelta {
                            name: new_name,
                            delta_type: DeltaType::Renamed { from: old_name },
                            content: Some(content_lines.join("\n")),
                            change_id: change_id.to_string(),
                            spec_path: path.to_path_buf(),
                        });

                        continue;
                    }
                }

                i += 1;
            }
            continue;
        }

        i += 1;
    }

    if deltas.is_empty() {
        Ok(None)
    } else {
        Ok(Some(deltas))
    }
}

/// Detect conflicts between deltas from different changes
pub fn detect_conflicts(all_deltas: &[RequirementDelta]) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    // Group deltas by requirement name
    let mut by_name: HashMap<String, Vec<&RequirementDelta>> = HashMap::new();
    for delta in all_deltas {
        by_name.entry(delta.name.clone()).or_default().push(delta);
    }

    // Check for conflicts within each requirement name
    for (req_name, deltas) in by_name {
        if deltas.len() < 2 {
            continue;
        }

        // Check all pairs of deltas for this requirement
        for i in 0..deltas.len() {
            for j in (i + 1)..deltas.len() {
                let d1 = deltas[i];
                let d2 = deltas[j];

                // Skip if same change (shouldn't happen)
                if d1.change_id == d2.change_id {
                    continue;
                }

                // Check for conflicts
                if let Some(reason) = check_conflict_pair(d1, d2) {
                    conflicts.push(Conflict {
                        requirement_name: req_name.clone(),
                        delta1: d1.clone(),
                        delta2: d2.clone(),
                        reason,
                    });
                }
            }
        }
    }

    // Check for rename conflicts (renamed from the same source)
    let mut rename_sources: HashMap<String, Vec<&RequirementDelta>> = HashMap::new();
    for delta in all_deltas {
        if let DeltaType::Renamed { from } = &delta.delta_type {
            rename_sources.entry(from.clone()).or_default().push(delta);
        }
    }

    for (from_name, deltas) in rename_sources {
        if deltas.len() < 2 {
            continue;
        }

        // Multiple renames from the same source
        for i in 0..deltas.len() {
            for j in (i + 1)..deltas.len() {
                let d1 = deltas[i];
                let d2 = deltas[j];

                if d1.change_id == d2.change_id {
                    continue;
                }

                conflicts.push(Conflict {
                    requirement_name: from_name.clone(),
                    delta1: d1.clone(),
                    delta2: d2.clone(),
                    reason: ConflictReason::RenameConflict,
                });
            }
        }
    }

    conflicts
}

/// Check if two deltas conflict with each other
fn check_conflict_pair(d1: &RequirementDelta, d2: &RequirementDelta) -> Option<ConflictReason> {
    match (&d1.delta_type, &d2.delta_type) {
        // Both removed: no conflict (same intention)
        (DeltaType::Removed, DeltaType::Removed) => None,

        // One removed, one modified/added: conflict
        (DeltaType::Removed, DeltaType::Modified)
        | (DeltaType::Removed, DeltaType::Added)
        | (DeltaType::Modified, DeltaType::Removed)
        | (DeltaType::Added, DeltaType::Removed) => Some(ConflictReason::RemoveConflict),

        // Both added or both modified: check content
        (DeltaType::Added, DeltaType::Added) | (DeltaType::Modified, DeltaType::Modified) => {
            if d1.content != d2.content {
                Some(ConflictReason::ContentMismatch)
            } else {
                None
            }
        }

        // Added vs Modified: check content
        (DeltaType::Added, DeltaType::Modified) | (DeltaType::Modified, DeltaType::Added) => {
            if d1.content != d2.content {
                Some(ConflictReason::ContentMismatch)
            } else {
                None
            }
        }

        // Renamed: already handled separately
        _ => None,
    }
}

/// Format conflicts for human-readable output
pub fn format_conflicts_human(conflicts: &[Conflict]) -> String {
    if conflicts.is_empty() {
        return "No conflicts detected.".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!("Found {} conflict(s):\n\n", conflicts.len()));

    for (idx, conflict) in conflicts.iter().enumerate() {
        output.push_str(&format!("Conflict {}:\n", idx + 1));
        output.push_str(&format!("  Requirement: {}\n", conflict.requirement_name));
        output.push_str(&format!("  Reason: {:?}\n", conflict.reason));
        output.push_str(&format!(
            "  Change 1: {} ({:?})\n",
            conflict.delta1.change_id, conflict.delta1.delta_type
        ));
        output.push_str(&format!(
            "  Change 2: {} ({:?})\n",
            conflict.delta2.change_id, conflict.delta2.delta_type
        ));
        output.push('\n');
    }

    output
}

/// Format conflicts for JSON output
pub fn format_conflicts_json(conflicts: &[Conflict]) -> Result<String> {
    serde_json::to_string_pretty(conflicts).map_err(OrchestratorError::Json)
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn test_parse_spec_file_added_section() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"# Test Spec

## ADDED Requirements

### Requirement: New Feature

This is a new feature.
Additional line.

### Requirement: Another Feature

Another feature content.
"#;
        temp_file.write_all(content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let deltas = parse_spec_file(temp_file.path(), "test-change")
            .unwrap()
            .unwrap();

        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].name, "New Feature");
        assert_eq!(deltas[0].delta_type, DeltaType::Added);
        assert!(deltas[0]
            .content
            .as_ref()
            .unwrap()
            .contains("This is a new feature"));

        assert_eq!(deltas[1].name, "Another Feature");
        assert_eq!(deltas[1].delta_type, DeltaType::Added);
    }

    #[test]
    fn test_parse_spec_file_modified_section() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"# Test Spec

## MODIFIED Requirements

### Requirement: Existing Feature

Modified content.
"#;
        temp_file.write_all(content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let deltas = parse_spec_file(temp_file.path(), "test-change")
            .unwrap()
            .unwrap();

        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].name, "Existing Feature");
        assert_eq!(deltas[0].delta_type, DeltaType::Modified);
    }

    #[test]
    fn test_parse_spec_file_removed_section() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"# Test Spec

## REMOVED Requirements

### Requirement: Old Feature
"#;
        temp_file.write_all(content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let deltas = parse_spec_file(temp_file.path(), "test-change")
            .unwrap()
            .unwrap();

        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].name, "Old Feature");
        assert_eq!(deltas[0].delta_type, DeltaType::Removed);
        assert_eq!(deltas[0].content, None);
    }

    #[test]
    fn test_parse_spec_file_renamed_section() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"# Test Spec

## RENAMED Requirements

### Requirement: NewName (from OldName)

Renamed content.
"#;
        temp_file.write_all(content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let deltas = parse_spec_file(temp_file.path(), "test-change")
            .unwrap()
            .unwrap();

        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].name, "NewName");
        assert!(matches!(&deltas[0].delta_type, DeltaType::Renamed { from } if from == "OldName"));
    }

    #[test]
    fn test_parse_spec_file_multiple_sections() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        let content = r#"# Test Spec

## ADDED Requirements

### Requirement: Feature A

Content A.

## MODIFIED Requirements

### Requirement: Feature B

Content B.

## REMOVED Requirements

### Requirement: Feature C
"#;
        temp_file.write_all(content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let deltas = parse_spec_file(temp_file.path(), "test-change")
            .unwrap()
            .unwrap();

        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].name, "Feature A");
        assert_eq!(deltas[1].name, "Feature B");
        assert_eq!(deltas[2].name, "Feature C");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_mismatch_conflict() {
        let d1 = RequirementDelta {
            name: "Test Requirement".to_string(),
            delta_type: DeltaType::Modified,
            content: Some("Content A".to_string()),
            change_id: "change1".to_string(),
            spec_path: PathBuf::from("specs/test/spec.md"),
        };

        let d2 = RequirementDelta {
            name: "Test Requirement".to_string(),
            delta_type: DeltaType::Modified,
            content: Some("Content B".to_string()),
            change_id: "change2".to_string(),
            spec_path: PathBuf::from("specs/test/spec.md"),
        };

        let reason = check_conflict_pair(&d1, &d2);
        assert_eq!(reason, Some(ConflictReason::ContentMismatch));
    }

    #[test]
    fn test_remove_conflict() {
        let d1 = RequirementDelta {
            name: "Test Requirement".to_string(),
            delta_type: DeltaType::Removed,
            content: None,
            change_id: "change1".to_string(),
            spec_path: PathBuf::from("specs/test/spec.md"),
        };

        let d2 = RequirementDelta {
            name: "Test Requirement".to_string(),
            delta_type: DeltaType::Modified,
            content: Some("New content".to_string()),
            change_id: "change2".to_string(),
            spec_path: PathBuf::from("specs/test/spec.md"),
        };

        let reason = check_conflict_pair(&d1, &d2);
        assert_eq!(reason, Some(ConflictReason::RemoveConflict));
    }

    #[test]
    fn test_no_conflict_same_content() {
        let d1 = RequirementDelta {
            name: "Test Requirement".to_string(),
            delta_type: DeltaType::Modified,
            content: Some("Same content".to_string()),
            change_id: "change1".to_string(),
            spec_path: PathBuf::from("specs/test/spec.md"),
        };

        let d2 = RequirementDelta {
            name: "Test Requirement".to_string(),
            delta_type: DeltaType::Modified,
            content: Some("Same content".to_string()),
            change_id: "change2".to_string(),
            spec_path: PathBuf::from("specs/test/spec.md"),
        };

        let reason = check_conflict_pair(&d1, &d2);
        assert_eq!(reason, None);
    }

    #[test]
    fn test_detect_conflicts() {
        let deltas = vec![
            RequirementDelta {
                name: "Req1".to_string(),
                delta_type: DeltaType::Modified,
                content: Some("Content A".to_string()),
                change_id: "change1".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
            RequirementDelta {
                name: "Req1".to_string(),
                delta_type: DeltaType::Modified,
                content: Some("Content B".to_string()),
                change_id: "change2".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
        ];

        let conflicts = detect_conflicts(&deltas);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].requirement_name, "Req1");
    }

    #[test]
    fn test_detect_rename_conflict() {
        let deltas = vec![
            RequirementDelta {
                name: "NewName1".to_string(),
                delta_type: DeltaType::Renamed {
                    from: "OldName".to_string(),
                },
                content: Some("Content 1".to_string()),
                change_id: "change1".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
            RequirementDelta {
                name: "NewName2".to_string(),
                delta_type: DeltaType::Renamed {
                    from: "OldName".to_string(),
                },
                content: Some("Content 2".to_string()),
                change_id: "change2".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
        ];

        let conflicts = detect_conflicts(&deltas);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].requirement_name, "OldName");
        assert!(matches!(
            conflicts[0].reason,
            ConflictReason::RenameConflict
        ));
    }

    #[test]
    fn test_no_conflict_different_requirements() {
        let deltas = vec![
            RequirementDelta {
                name: "Req1".to_string(),
                delta_type: DeltaType::Modified,
                content: Some("Content A".to_string()),
                change_id: "change1".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
            RequirementDelta {
                name: "Req2".to_string(),
                delta_type: DeltaType::Modified,
                content: Some("Content B".to_string()),
                change_id: "change2".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
        ];

        let conflicts = detect_conflicts(&deltas);
        assert_eq!(conflicts.len(), 0);
    }

    #[test]
    fn test_no_conflict_both_removed() {
        let deltas = vec![
            RequirementDelta {
                name: "Req1".to_string(),
                delta_type: DeltaType::Removed,
                content: None,
                change_id: "change1".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
            RequirementDelta {
                name: "Req1".to_string(),
                delta_type: DeltaType::Removed,
                content: None,
                change_id: "change2".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
        ];

        let conflicts = detect_conflicts(&deltas);
        assert_eq!(conflicts.len(), 0);
    }

    #[test]
    fn test_format_conflicts_human() {
        let conflict = Conflict {
            requirement_name: "Test Req".to_string(),
            delta1: RequirementDelta {
                name: "Test Req".to_string(),
                delta_type: DeltaType::Modified,
                content: Some("Content A".to_string()),
                change_id: "change1".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
            delta2: RequirementDelta {
                name: "Test Req".to_string(),
                delta_type: DeltaType::Modified,
                content: Some("Content B".to_string()),
                change_id: "change2".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
            reason: ConflictReason::ContentMismatch,
        };

        let output = format_conflicts_human(&[conflict]);
        assert!(output.contains("Found 1 conflict"));
        assert!(output.contains("Test Req"));
        assert!(output.contains("change1"));
        assert!(output.contains("change2"));
    }

    #[test]
    fn test_format_conflicts_json() {
        let conflict = Conflict {
            requirement_name: "Test Req".to_string(),
            delta1: RequirementDelta {
                name: "Test Req".to_string(),
                delta_type: DeltaType::Modified,
                content: Some("Content A".to_string()),
                change_id: "change1".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
            delta2: RequirementDelta {
                name: "Test Req".to_string(),
                delta_type: DeltaType::Modified,
                content: Some("Content B".to_string()),
                change_id: "change2".to_string(),
                spec_path: PathBuf::from("specs/test/spec.md"),
            },
            reason: ConflictReason::ContentMismatch,
        };

        let json = format_conflicts_json(&[conflict]).unwrap();
        assert!(json.contains("Test Req"));
        assert!(json.contains("change1"));
        assert!(json.contains("ContentMismatch"));
    }
}
