//! Native OpenSpec utility commands for `cflx openspec`.
//!
//! Replaces the former Python helper `scripts/cflx.py` with native Rust
//! implementations for list, show, validate, and archive operations.

mod archive;
mod dependency_status;
mod model;
mod promotion;
mod rendering;
mod validation;

use crate::archive_layout;
use archive::ArchiveEngine;
use model::{ChangeInfo, DependencyStatusContext, ShowInfo, SpecInfo};
use regex::Regex;
use rendering::{
    render_changes_output, render_show_json_value, render_show_output, render_specs_output,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use validation::{count_requirements_in_spec, count_tasks, ValidationEngine};

struct OpenSpecManager {
    root_dir: PathBuf,
    changes_dir: PathBuf,
    archive_dir: PathBuf,
    specs_dir: PathBuf,
}

impl OpenSpecManager {
    fn new() -> Self {
        let root_dir = PathBuf::from(".");
        let changes_dir = root_dir.join("openspec/changes");
        let archive_dir = changes_dir.join("archive");
        let specs_dir = root_dir.join("openspec/specs");
        Self {
            root_dir,
            changes_dir,
            archive_dir,
            specs_dir,
        }
    }

    fn find_change_dir(&self, change_id: &str) -> Result<Option<PathBuf>, String> {
        // Check active changes
        let change_dir = self.changes_dir.join(change_id);
        if change_dir.exists() && change_dir.join("proposal.md").exists() {
            return Ok(Some(change_dir));
        }

        if let Some(error) = archive_layout::invalid_layout_error(change_id, &self.archive_dir) {
            return Err(error.message());
        }

        Ok(
            archive_layout::find_valid_archive_entry(change_id, &self.archive_dir)
                .filter(|candidate| candidate.join("proposal.md").exists()),
        )
    }

    fn list_changes(&self) -> Vec<ChangeInfo> {
        let mut changes = Vec::new();
        if !self.changes_dir.exists() {
            return changes;
        }

        let dependency_status_context = DependencyStatusContext::from_workspace(&self.root_dir);

        if let Ok(entries) = fs::read_dir(&self.changes_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name == "archive" || name.starts_with('.') {
                    continue;
                }
                if !path.join("proposal.md").exists() {
                    eprintln!(
                        "Warning: Ignoring invalid change directory '{}' (missing proposal.md)",
                        name
                    );
                    continue;
                }
                if let Some(mut info) = self.get_change_info(&path, false) {
                    info.dependency_statuses =
                        dependency_status_context.statuses_for(&info.dependencies);
                    changes.push(info);
                }
            }
        }

        changes.sort_by(|a, b| a.id.cmp(&b.id));
        changes
    }

    fn list_specs(&self) -> Vec<SpecInfo> {
        let mut specs = Vec::new();
        if !self.specs_dir.exists() {
            return specs;
        }

        if let Ok(entries) = fs::read_dir(&self.specs_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let spec_file = path.join("spec.md");
                if spec_file.exists() {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let rel_path = format!("openspec/specs/{}/spec.md", name);
                    let requirement_count = count_requirements_in_spec(&spec_file);
                    specs.push(SpecInfo {
                        name,
                        path: rel_path,
                        requirement_count,
                    });
                }
            }
        }

        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    fn get_change_info(&self, change_dir: &Path, archived: bool) -> Option<ChangeInfo> {
        let id = change_dir.file_name()?.to_string_lossy().to_string();
        let rel_path = if archived {
            format!("openspec/changes/archive/{}", id)
        } else {
            format!("openspec/changes/{}", id)
        };

        let proposal_path = change_dir.join("proposal.md");
        let dependencies = if archived {
            Vec::new()
        } else {
            crate::openspec::parse_proposal_metadata_from_file(&proposal_path).dependencies
        };

        let mut info = ChangeInfo {
            id,
            path: rel_path,
            title: None,
            tasks_completed: 0,
            tasks_total: 0,
            dependencies,
            dependency_statuses: Vec::new(),
        };

        // Extract title from proposal.md
        if let Ok(content) = fs::read_to_string(&proposal_path) {
            static TITLE_RE: OnceLock<Regex> = OnceLock::new();
            let re = TITLE_RE.get_or_init(|| Regex::new(r"(?m)^#\s+(.+)$").unwrap());
            if let Some(caps) = re.captures(&content) {
                info.title = Some(caps[1].trim().to_string());
            }
        }

        // Count tasks
        if let Ok(content) = fs::read_to_string(change_dir.join("tasks.md")) {
            let (completed, total) = count_tasks(&content);
            info.tasks_completed = completed;
            info.tasks_total = total;
        }

        Some(info)
    }

    fn show_change(&self, change_id: &str, deltas_only: bool) -> Result<Option<ShowInfo>, String> {
        let Some(change_dir) = self.find_change_dir(change_id)? else {
            return Ok(None);
        };
        let archived = change_dir.to_string_lossy().contains("/archive/");
        let rel_path = change_dir
            .strip_prefix(&self.root_dir)
            .unwrap_or(&change_dir)
            .to_string_lossy()
            .to_string();

        let mut info = ShowInfo {
            id: change_id.to_string(),
            path: rel_path,
            archived,
            proposal: None,
            tasks: None,
            tasks_completed: 0,
            tasks_total: 0,
            dependencies: Vec::new(),
            dependency_statuses: Vec::new(),
            design: None,
            specs: HashMap::new(),
        };

        // Read proposal
        let proposal_path = change_dir.join("proposal.md");
        if let Ok(content) = fs::read_to_string(&proposal_path) {
            info.proposal = Some(content);
        }

        if !archived && !deltas_only {
            info.dependencies =
                crate::openspec::parse_proposal_metadata_from_file(&proposal_path).dependencies;
            info.dependency_statuses = DependencyStatusContext::from_workspace(&self.root_dir)
                .statuses_for(&info.dependencies);
        }

        // Read tasks
        if let Ok(content) = fs::read_to_string(change_dir.join("tasks.md")) {
            let (completed, total) = count_tasks(&content);
            info.tasks_completed = completed;
            info.tasks_total = total;
            info.tasks = Some(content);
        }

        // Read design
        if let Ok(content) = fs::read_to_string(change_dir.join("design.md")) {
            info.design = Some(content);
        }

        // Read spec deltas
        let specs_dir = change_dir.join("specs");
        if specs_dir.exists() {
            if let Ok(entries) = fs::read_dir(&specs_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_dir() {
                        let spec_file = path.join("spec.md");
                        if spec_file.exists() {
                            if let Ok(content) = fs::read_to_string(&spec_file) {
                                let name = path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                info.specs.insert(name, content);
                            }
                        }
                    }
                }
            }
        }

        if deltas_only {
            return Ok(Some(ShowInfo {
                id: info.id,
                path: info.path,
                archived: info.archived,
                proposal: None,
                tasks: None,
                tasks_completed: 0,
                tasks_total: 0,
                dependencies: Vec::new(),
                dependency_statuses: Vec::new(),
                design: None,
                specs: info.specs,
            }));
        }

        Ok(Some(info))
    }
    fn validate_change(
        &self,
        change_id: Option<&str>,
        strict: bool,
        evidence_mode: &str,
    ) -> (bool, Vec<String>, Vec<String>) {
        ValidationEngine { manager: self }.validate_change(change_id, strict, evidence_mode)
    }

    fn archive_change(&self, change_id: &str, skip_specs: bool) -> Result<String, String> {
        ArchiveEngine { manager: self }.archive_change(change_id, skip_specs)
    }
}

/// `cflx openspec list` — list changes or specs.
pub fn cmd_list(show_specs: bool) -> Result<(), String> {
    let mgr = OpenSpecManager::new();

    if show_specs {
        let specs = mgr.list_specs();
        print!("{}", render_specs_output(&specs));
    } else {
        let changes = mgr.list_changes();
        print!("{}", render_changes_output(&changes));
    }

    Ok(())
}

/// `cflx openspec show` — show change details.
pub fn cmd_show(change_id: &str, json_output: bool, deltas_only: bool) -> Result<(), String> {
    let mgr = OpenSpecManager::new();
    let info = mgr
        .show_change(change_id, deltas_only)?
        .ok_or_else(|| format!("Change '{}' not found", change_id))?;

    if json_output {
        let json_value = render_show_json_value(&info);
        println!(
            "{}",
            serde_json::to_string_pretty(&json_value).unwrap_or_default()
        );
        return Ok(());
    }

    print!("{}", render_show_output(&info));

    Ok(())
}

/// `cflx openspec validate` — validate changes.
///
/// Returns (is_valid, exit_code).
pub fn cmd_validate(change_id: Option<&str>, strict: bool, evidence: &str) -> (bool, i32) {
    let mgr = OpenSpecManager::new();

    // Check obsolete artifacts
    check_obsolete_artifacts();

    let (is_valid, errors, warnings) = mgr.validate_change(change_id, strict, evidence);

    for warning in &warnings {
        eprintln!("\x1b[93m! {}\x1b[0m", warning);
    }

    if is_valid {
        println!("\x1b[92m\u{2713} Validation passed\x1b[0m");
        (true, 0)
    } else {
        eprintln!("\x1b[91m\u{2717} Validation failed:\x1b[0m");
        for error in &errors {
            eprintln!("  {}", error);
        }
        (false, 1)
    }
}

/// `cflx openspec archive` — archive a deployed change.
pub fn cmd_archive(change_id: &str, skip_specs: bool) -> Result<(), String> {
    let mgr = OpenSpecManager::new();
    let message = mgr.archive_change(change_id, skip_specs)?;
    println!("\x1b[92m\u{2713} {}\x1b[0m", message);
    Ok(())
}

/// Check for obsolete OpenSpec artifacts and print warnings.
fn check_obsolete_artifacts() {
    let obsolete = [
        (
            "openspec/AGENTS.md",
            "openspec/AGENTS.md is obsolete; Conflux skills embed all required conventions",
        ),
        (
            "openspec/project.md",
            "openspec/project.md is obsolete; use .cflx.jsonc for project configuration",
        ),
    ];

    for (path, message) in &obsolete {
        if Path::new(path).exists() {
            eprintln!("\x1b[93m! OBSOLETE: {}\x1b[0m", message);
        }
    }

    let agents_md = Path::new("AGENTS.md");
    if agents_md.exists() {
        if let Ok(content) = fs::read_to_string(agents_md) {
            if content.contains("<!-- OPENSPEC:START -->") {
                eprintln!(
                    "\x1b[93m! OBSOLETE: AGENTS.md contains <!-- OPENSPEC:START --> markers; \
                     these inline OpenSpec instructions are obsolete and should be removed\x1b[0m"
                );
            }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod spec_promotion_tests {
    use crate::openspec_cmd::promotion::{
        delta_to_canonical, merge_spec_delta, parse_delta_sections, simulate_promotion, split_spec,
    };

    #[test]
    fn test_split_spec_empty() {
        let (preamble, blocks) = split_spec("");
        assert!(preamble.is_empty());
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_split_spec_with_blocks() {
        let content = "# Spec\n\n### Requirement: Feature A\n\nContent A.\n\n### Requirement: Feature B\n\nContent B.\n";
        let (preamble, blocks) = split_spec(content);
        assert!(preamble.contains("# Spec"));
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "Feature A");
        assert_eq!(blocks[1].0, "Feature B");
    }

    #[test]
    fn test_parse_delta_sections() {
        let delta = "## ADDED Requirements\n\n### Requirement: New Feature\n\nNew content.\n\n## MODIFIED Requirements\n\n### Requirement: Old Feature\n\nUpdated content.\n\n## REMOVED Requirements\n\n### Requirement: Dead Feature\n\nRemoved.\n";
        let sections = parse_delta_sections(delta);
        assert_eq!(sections.added.len(), 1);
        assert_eq!(sections.added[0].0, "New Feature");
        assert_eq!(sections.modified.len(), 1);
        assert_eq!(sections.modified[0].0, "Old Feature");
        assert_eq!(sections.removed.len(), 1);
        assert_eq!(sections.removed[0].0, "Dead Feature");
    }

    #[test]
    fn test_merge_spec_delta_added() {
        let canonical = "# Spec\n\n### Requirement: Existing\n\nExisting content.\n";
        let delta = "## ADDED Requirements\n\n### Requirement: New Feature\n\nNew content.\n";
        let (result, errors) = merge_spec_delta(canonical, delta);
        assert!(errors.is_empty());
        assert!(result.contains("### Requirement: Existing"));
        assert!(result.contains("### Requirement: New Feature"));
    }

    #[test]
    fn test_merge_spec_delta_modified() {
        let canonical = "# Spec\n\n### Requirement: Feature A\n\nOld content.\n";
        let delta = "## MODIFIED Requirements\n\n### Requirement: Feature A\n\nNew content.\n";
        let (result, errors) = merge_spec_delta(canonical, delta);
        assert!(errors.is_empty());
        assert!(result.contains("New content"));
        assert!(!result.contains("Old content"));
    }

    #[test]
    fn test_merge_spec_delta_removed() {
        let canonical = "# Spec\n\n### Requirement: Feature A\n\nContent A.\n\n### Requirement: Feature B\n\nContent B.\n";
        let delta = "## REMOVED Requirements\n\n### Requirement: Feature A\n\nContent A.\n";
        let (result, errors) = merge_spec_delta(canonical, delta);
        assert!(errors.is_empty());
        assert!(!result.contains("Feature A"));
        assert!(result.contains("Feature B"));
    }

    #[test]
    fn test_merge_spec_delta_modified_target_missing() {
        let canonical = "# Spec\n\n### Requirement: Feature A\n\nContent A.\n";
        let delta = "## MODIFIED Requirements\n\n### Requirement: NonExistent\n\nNew content.\n";
        let (_, errors) = merge_spec_delta(canonical, delta);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("MODIFIED target not found"));
    }

    #[test]
    fn test_merge_spec_delta_removed_target_missing() {
        let canonical = "# Spec\n\n### Requirement: Feature A\n\nContent A.\n";
        let delta = "## REMOVED Requirements\n\n### Requirement: NonExistent\n\nContent.\n";
        let (_, errors) = merge_spec_delta(canonical, delta);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("REMOVED target not found"));
    }

    #[test]
    fn test_merge_spec_delta_noop_rejection() {
        let canonical = "### Requirement: Feature A\n\nContent A.\n";
        let delta = "## ADDED Requirements\n";
        let (_, errors) = merge_spec_delta(canonical, delta);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("no-op archive"));
    }

    #[test]
    fn test_delta_to_canonical() {
        let delta = "## ADDED Requirements\n\n### Requirement: Feature A\n\nContent A.\n";
        let result =
            delta_to_canonical(delta).expect("delta should parse into canonical requirements");
        assert!(result.contains("### Requirement: Feature A"));
        assert!(!result.contains("## ADDED"));
    }

    #[test]
    fn test_delta_to_canonical_parse_error() {
        let delta = "## ADDED Requirements\n\nSome content without requirement blocks.\n";
        let err = delta_to_canonical(delta).expect_err("malformed delta must fail closed");
        assert!(err.contains("parse error"));
    }

    #[test]
    fn test_simulate_promotion_new_spec() {
        let delta = "## ADDED Requirements\n\n### Requirement: Feature A\n\nContent A.\n";
        let (result, errors) = simulate_promotion(None, delta);
        assert!(errors.is_empty());
        assert!(result.contains("Feature A"));
    }

    #[test]
    fn test_simulate_promotion_existing_spec() {
        let canonical = "### Requirement: Existing\n\nExisting content.\n";
        let delta = "## ADDED Requirements\n\n### Requirement: New\n\nNew content.\n";
        let (result, errors) = simulate_promotion(Some(canonical), delta);
        assert!(errors.is_empty());
        assert!(result.contains("Existing"));
        assert!(result.contains("New"));
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::openspec_cmd::validation::{extract_change_type, validate_tasks_content};

    #[test]
    fn test_count_tasks_basic() {
        let content = "- [x] Task 1\n- [ ] Task 2\n- [x] Task 3\n";
        let (completed, total) = count_tasks(content);
        assert_eq!(completed, 2);
        assert_eq!(total, 3);
    }

    #[test]
    fn test_count_tasks_excludes_future_work() {
        let content =
            "## Implementation\n- [x] Task 1\n- [ ] Task 2\n## Future Work\n- [ ] Future task\n";
        let (completed, total) = count_tasks(content);
        assert_eq!(completed, 1);
        assert_eq!(total, 2);
    }

    #[test]
    fn test_extract_change_type_bold() {
        let content = "# Change\n\n**Change Type**: hybrid\n";
        assert_eq!(extract_change_type(content), Some("hybrid".to_string()));
    }

    #[test]
    fn test_extract_change_type_plain() {
        let content = "# Change\n\nChange Type: spec-only\n";
        assert_eq!(extract_change_type(content), Some("spec-only".to_string()));
    }

    #[test]
    fn test_extract_change_type_missing() {
        let content = "# Change\n\nNo type here.\n";
        assert_eq!(extract_change_type(content), None);
    }

    #[test]
    fn test_validate_tasks_checkbox_in_excluded() {
        let content = "## Future Work\n- [ ] Should not have checkbox\n";
        let (errors, _) = validate_tasks_content(content, "test", false, "off", None, None);
        assert!(errors.iter().any(|e| e.contains("excluded section")));
    }

    #[test]
    fn test_validate_tasks_bare_task() {
        let content = "## Implementation\n- Some task without checkbox\n";
        let (errors, _) = validate_tasks_content(content, "test", false, "off", None, None);
        assert!(errors
            .iter()
            .any(|e| e.contains("Possible task without checkbox")));
    }

    #[test]
    fn test_validate_tasks_bare_task_utf8_safe_preview_boundary() {
        let content =
            "## Implementation\n- UTF8§12345678901234567890123456789012345678901234567890 task\n";
        let (errors, _) = validate_tasks_content(content, "test", false, "off", None, None);

        let warning = errors
            .iter()
            .find(|e| e.contains("Possible task without checkbox"))
            .expect("bare-task warning should be present");

        assert!(warning.contains("Possible task without checkbox"));
        assert!(warning.contains("UTF8§"));
        assert!(warning.contains("..."));
    }

    #[test]
    fn test_validate_tasks_bare_task_long_preview_still_truncated() {
        let content =
            "## Implementation\n- abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789\n";
        let (errors, _) = validate_tasks_content(content, "test", false, "off", None, None);

        let warning = errors
            .iter()
            .find(|e| e.contains("Possible task without checkbox"))
            .expect("bare-task warning should be present");

        assert!(warning.contains("Possible task without checkbox"));
        assert!(warning.contains("..."));
    }

    #[test]
    fn test_validate_tasks_evidence_warn() {
        let content = "- [ ] Add a new feature for users\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);
        assert!(errors.is_empty());
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("Behavior-bearing task missing"));
    }

    #[test]
    fn test_validate_tasks_evidence_error() {
        let content = "- [ ] Add a new feature for users\n";
        let (errors, _) =
            validate_tasks_content(content, "test", true, "error", Some("implementation"), None);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("Behavior-bearing task missing"));
    }

    #[test]
    fn test_validate_tasks_with_verification_hint() {
        let content =
            "- [ ] Add a new feature (verification: unit - cargo test covers the feature)\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);
        assert!(errors.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_validate_tasks_accepts_generic_evidence_vocabulary_with_ownership() {
        let content = "- [ ] Update source evidence wording (verification: unit - source paths document the changed implementation)\n- [ ] Update test evidence wording (verification: unit - test files cover the validator behavior)\n- [ ] Update command evidence wording (verification: manual - runnable command is provided by the task note)\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "error", Some("implementation"), None);

        assert!(
            errors.is_empty(),
            "generic repository evidence vocabulary should pass: {errors:?}"
        );
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn test_validate_tasks_rejects_generic_evidence_vocabulary_without_ownership() {
        let content = "- [ ] Update source evidence wording (verification: source paths document the changed implementation)\n- [ ] Update test evidence wording (verification: test files cover the validator behavior)\n- [ ] Update command evidence wording (verification: runnable command is provided by the task note)\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);

        assert!(errors.is_empty());
        assert!(
            warnings
                .iter()
                .filter(|warning| warning.contains("Verification ownership missing"))
                .count()
                >= 3,
            "ownership-free generic evidence notes should still warn: {warnings:?}"
        );
        assert!(
            !warnings.iter().any(|warning| warning
                .contains("Verification note should cite repository-verifiable evidence")),
            "generic evidence vocabulary itself should be recognized: {warnings:?}"
        );
    }

    #[test]
    fn test_validate_tasks_with_standalone_verification_hint() {
        let content =
            "- [ ] Add a new feature\n  verification: unit - cargo test covers the feature\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);
        assert!(errors.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_validate_tasks_accepts_inline_verification_before_completion_prose() {
        let content = "- [ ] Update validator parsing (verification: manual - inspect src/openspec_cmd.rs and run cargo test openspec_cmd --lib) Completion condition: additional prose after the verification note remains ordinary task text.\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "error", Some("implementation"), None);

        assert!(
            errors.is_empty(),
            "inline verification before completion prose should pass: {errors:?}"
        );
        assert!(
            warnings.is_empty(),
            "inline verification before completion prose should not warn: {warnings:?}"
        );
    }

    #[test]
    fn test_extract_inline_verification_tolerates_parentheses_before_evidence() {
        let content = "- [ ] Update verification note parsing (verification: manual - run (`cflx openspec validate fixture --strict`) after inspecting src/openspec_cmd.rs) Completion condition: parser keeps command evidence.\n";
        let (errors, warnings) = validate_tasks_content(
            content,
            "alpha",
            true,
            "error",
            Some("implementation"),
            None,
        );

        assert!(
            errors.is_empty(),
            "parenthesized command should not truncate verification evidence: {errors:?}"
        );
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn test_extract_inline_verification_tolerates_backticked_parentheses_before_evidence() {
        let content = "- [ ] Update verification note parsing (verification: manual - reviewed command `printf \"done ) still command\"` then inspected src/openspec_cmd.rs and ran cflx openspec validate fixture --strict) Completion condition: evidence after backticked parenthesis is preserved.\n";
        let (errors, warnings) = validate_tasks_content(
            content,
            "alpha",
            true,
            "error",
            Some("implementation"),
            None,
        );

        assert!(
            errors.is_empty(),
            "backticked inner parenthesis should not truncate verification evidence: {errors:?}"
        );
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn test_standalone_verification_line_utf8_no_panic_and_findings() {
        let content = "## Implementation\n- [ ] Add a new feature\n  verification: 手動確認のみ\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);
        assert!(errors.is_empty());
        assert!(!warnings.is_empty());
        assert!(warnings
            .iter()
            .any(|w| w.contains("Verification note should cite repository-verifiable evidence")));
        assert!(warnings
            .iter()
            .any(|w| w.contains("Verification ownership missing")));
    }

    #[test]
    fn test_cflx_proposal_skill_final_validation_uses_non_checkbox_section() {
        let skill = include_str!("../skills/cflx-proposal/SKILL.md");
        let final_validation_pos = skill
            .find("## Final Validation")
            .expect("cflx-proposal skill should document a Final Validation section");
        let before_final_validation = &skill[..final_validation_pos];

        assert!(skill.contains("cflx openspec validate <id> --archive-gate"));
        assert!(skill.contains("Do not create final OpenSpec validation as a checkbox"));
        assert!(
            !before_final_validation.contains("- [ ] Final OpenSpec validation")
                && !before_final_validation.contains("- [ ] Record final OpenSpec validation"),
            "final OpenSpec validation guidance must not be modeled as a checkbox task"
        );
    }

    #[test]
    fn test_validate_tasks_with_weak_verification() {
        let content = "- [ ] Add a new feature (verification: manual review)\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);
        assert!(errors.is_empty());
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Verification note should cite repository-verifiable evidence")),
            "narrative-only manual review should still produce an evidence finding: {warnings:?}"
        );
    }

    #[test]
    fn test_rejects_self_referential_final_validation_checkbox() {
        let content = "- [ ] Record final OpenSpec validation before archive (verification: manual - run `cflx openspec validate alpha --strict --evidence warn`)\n";
        let (errors, warnings) = validate_tasks_content(
            content,
            "alpha",
            true,
            "error",
            Some("implementation"),
            None,
        );

        assert!(warnings.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("self-referential final OpenSpec validation checkbox"));
        assert!(errors[0].contains("non-checkbox `## Final Validation` section"));
    }

    #[test]
    fn test_allows_non_checkbox_final_validation_section() {
        let content = "## Implementation Tasks\n- [ ] Implement feature (verification: unit - cargo test openspec_cmd --lib)\n\n## Final Validation\n\nExpected archive gate: `cflx openspec validate alpha --strict --evidence warn` exits 0.\n";
        let (errors, warnings) = validate_tasks_content(
            content,
            "alpha",
            true,
            "error",
            Some("implementation"),
            None,
        );

        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn test_preserves_ordinary_repository_evidence() {
        let content = "- [ ] Rust verification (verification: unit - cargo test openspec_cmd --lib)\n- [ ] Frontend verification (verification: integration - npm run test)\n- [ ] Go verification (verification: integration - go test ./...)\n- [ ] Path verification (verification: unit - src/openspec_cmd.rs and tests/fixtures cover this)\n";
        let (errors, warnings) = validate_tasks_content(
            content,
            "alpha",
            true,
            "error",
            Some("implementation"),
            None,
        );

        assert!(
            errors.is_empty(),
            "ordinary evidence should pass: {errors:?}"
        );
        assert!(
            warnings.is_empty(),
            "ordinary evidence should not warn: {warnings:?}"
        );
    }

    #[test]
    fn test_validate_tasks_accepts_common_repository_artifacts_and_build_commands() {
        let content = "- [ ] Add Docker packaging evidence (verification: manual - Dockerfile documents the runtime image)\n- [ ] Add TOML configuration evidence (verification: unit - Cargo.toml and .toml configuration fixtures cover the change)\n- [ ] Add container build evidence (verification: integration - docker build validates the repository build artifact)\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "error", Some("implementation"), None);

        assert!(
            errors.is_empty(),
            "common repository artifacts and commands should pass: {errors:?}"
        );
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn test_accepts_observed_archive_gate_manual_note_shape() {
        let content = "- [x] Task 9: Complete archive gate verification for workspace persistence. (verification: manual - implemented in src/workspace/persistence.rs and tests/workspace_persistence_tests.rs; ran `cflx openspec validate add-s3-workspace-persistence --strict`) Completion condition: archive readiness evidence is repository-verifiable.\n";
        let (errors, warnings) = validate_tasks_content(
            content,
            "current-change",
            true,
            "error",
            Some("implementation"),
            None,
        );

        assert!(
            !errors.iter().any(|e| e.contains("Verification note should cite repository-verifiable evidence")),
            "observed manual note should retain repository evidence: {errors:?}"
        );
        assert!(
            !errors
                .iter()
                .any(|e| e.contains("Verification ownership missing")),
            "observed manual note should retain ownership marker: {errors:?}"
        );
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    }

    #[test]
    fn test_warns_missing_verification_ownership() {
        let content = "- [ ] Implement handler update (verification: cargo test -- --nocapture)\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);
        assert!(errors.is_empty());
        assert!(warnings
            .iter()
            .any(|w| w.contains("Verification ownership missing")));
    }
}

#[cfg(test)]
mod openspec_list_show_tests {
    use super::*;
    use crate::openspec_cmd::model::{DependencyListStatus, DependencyStatusInfo};
    use chrono::Local;
    use std::collections::HashMap;
    use std::env;
    use std::sync::MutexGuard;
    use tempfile::TempDir;

    struct CwdTestGuard {
        _lock: MutexGuard<'static, ()>,
        original_cwd: PathBuf,
    }

    impl CwdTestGuard {
        fn enter(path: &Path) -> Self {
            let lock = cwd_lock()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let original_cwd = env::current_dir().unwrap();
            env::set_current_dir(path).unwrap();
            Self {
                _lock: lock,
                original_cwd,
            }
        }
    }

    impl Drop for CwdTestGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.original_cwd).unwrap();
        }
    }

    fn cwd_lock() -> &'static std::sync::Mutex<()> {
        crate::test_support::cwd_lock()
    }

    fn create_change(dir: &Path, proposal_title: &str, tasks: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("proposal.md"),
            format!("# {}\n\nbody\n", proposal_title),
        )
        .unwrap();
        fs::write(dir.join("tasks.md"), tasks).unwrap();
    }

    fn create_change_with_frontmatter_dependencies(
        dir: &Path,
        proposal_title: &str,
        dependencies: &[&str],
        tasks: &str,
    ) {
        fs::create_dir_all(dir).unwrap();
        let deps_yaml = if dependencies.is_empty() {
            "[]".to_string()
        } else {
            let mut lines = String::new();
            for dep in dependencies {
                lines.push_str(&format!("\n  - {}", dep));
            }
            lines
        };
        let proposal = if dependencies.is_empty() {
            format!(
                "---\ndependencies: []\n---\n\n# {}\n\nbody\n",
                proposal_title
            )
        } else {
            format!(
                "---\ndependencies:{}\n---\n\n# {}\n\nbody\n",
                deps_yaml, proposal_title
            )
        };
        fs::write(dir.join("proposal.md"), proposal).unwrap();
        fs::write(dir.join("tasks.md"), tasks).unwrap();
    }

    fn create_change_with_body_dependencies(
        dir: &Path,
        proposal_title: &str,
        dependencies: &[&str],
        tasks: &str,
    ) {
        fs::create_dir_all(dir).unwrap();
        let mut proposal = format!("# {}\n\nbody\n\n## Dependencies\n", proposal_title);
        for dependency in dependencies {
            proposal.push_str(&format!("- {}\n", dependency));
        }
        fs::write(dir.join("proposal.md"), proposal).unwrap();
        fs::write(dir.join("tasks.md"), tasks).unwrap();
    }

    fn track_file(root: &Path, path: &str) {
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["add", "--", path])
            .current_dir(root)
            .status()
            .unwrap();
    }

    fn create_strict_valid_change(dir: &Path, proposal_title: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("proposal.md"),
            format!(
                "---\nverifications:\n  - id: local\n    requirement: archive behavior\n    phase: pre-integration\n    owner: conflux-acceptance\n    trigger: pull request\n    automation: Cargo.toml\n    evidence: cargo test\n    rerun: cargo test\n    prerequisites: []\n---\n# {}\n\n**Change Type**: implementation\n\n## Problem\narchive behavior update\n",
                proposal_title
            ),
        )
        .unwrap();
        fs::write("Cargo.toml", "[package]\nname = \"fixture\"\n").unwrap();
        track_file(&env::current_dir().unwrap(), "Cargo.toml");
        fs::write(
            dir.join("tasks.md"),
            "- [ ] 1. archive destination update (verification: unit - cargo test src::openspec_cmd::openspec_list_show_tests -- --nocapture)\n",
        )
        .unwrap();

        let spec_dir = dir.join("specs/archive");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(
            spec_dir.join("spec.md"),
            "## ADDED Requirements\n\n### Requirement: Archive naming\n\n#### Scenario: Dated destination\n- WHEN archive runs\n- THEN destination uses dated prefix\n",
        )
        .unwrap();
    }

    fn create_strict_change_with_spec_delta(dir: &Path, spec_name: &str, delta: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("proposal.md"),
            "---\nverifications:\n  - id: local\n    requirement: validator behavior\n    phase: pre-integration\n    owner: conflux-acceptance\n    trigger: pull request\n    automation: Cargo.toml\n    evidence: cargo test\n    rerun: cargo test\n    prerequisites: []\n---\n# Strict validation fixture\n\n**Change Type**: implementation\n\n## Problem\nvalidator fixture\n",
        )
        .unwrap();
        fs::write("Cargo.toml", "[package]\nname = \"fixture\"\n").unwrap();
        track_file(&env::current_dir().unwrap(), "Cargo.toml");
        fs::write(
            dir.join("tasks.md"),
            "- [ ] 1. validator update (verification: unit - cargo test openspec_cmd --lib)\n",
        )
        .unwrap();
        let spec_dir = dir.join("specs").join(spec_name);
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(spec_dir.join("spec.md"), delta).unwrap();
    }

    fn create_canonical_spec(root: &Path, spec_name: &str, content: &str) {
        let spec_dir = root.join("openspec/specs").join(spec_name);
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(spec_dir.join("spec.md"), content).unwrap();
    }

    #[test]
    fn test_list_changes_excludes_archived_entries() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let active_dir = temp.path().join("openspec/changes/active-change");
        let archived_dir = temp
            .path()
            .join("openspec/changes/archive/2026-04-27-archived-change");

        create_change(&active_dir, "Active Change", "- [x] done\n- [ ] pending\n");
        create_change(&archived_dir, "Archived Change", "- [x] archived\n");

        let mgr = OpenSpecManager::new();
        let changes = mgr.list_changes();

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].id, "active-change");
        assert!(changes[0].path.contains("openspec/changes/active-change"));
    }

    #[test]
    fn test_list_change_records_include_frontmatter_dependencies() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let dependent_dir = temp.path().join("openspec/changes/dependent-change");
        let dependency_dir = temp.path().join("openspec/changes/base-change");
        create_change_with_frontmatter_dependencies(
            &dependent_dir,
            "Dependent Change",
            &["base-change"],
            "- [ ] pending\n",
        );
        create_change(&dependency_dir, "Base Change", "- [ ] pending\n");

        let mgr = OpenSpecManager::new();
        let changes = mgr.list_changes();
        let dependent = changes
            .iter()
            .find(|change| change.id == "dependent-change")
            .expect("dependent change should be listed");

        assert_eq!(dependent.dependencies, vec!["base-change".to_string()]);
        assert_eq!(
            dependent.dependency_statuses,
            vec![DependencyStatusInfo {
                id: "base-change".to_string(),
                status: DependencyListStatus::Pending,
            }]
        );
    }

    #[test]
    fn test_list_change_dependency_statuses_cover_workspace_states() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let dependent_dir = temp.path().join("openspec/changes/dependent-change");
        create_change_with_frontmatter_dependencies(
            &dependent_dir,
            "Dependent Change",
            &[
                "pending-dep",
                "running-dep",
                "done-dep",
                "rejected-dep",
                "missing-dep",
            ],
            "- [ ] pending\n",
        );
        create_change(
            &temp.path().join("openspec/changes/pending-dep"),
            "Pending Dep",
            "- [ ] pending\n",
        );
        create_change(
            &temp.path().join("openspec/changes/running-dep"),
            "Running Dep",
            "- [ ] running\n",
        );
        fs::write(temp.path().join(".conflux-inflight"), "running-dep\n").unwrap();
        create_change(
            &temp
                .path()
                .join("openspec/changes/archive/2026-05-08-done-dep"),
            "Done Dep",
            "- [x] done\n",
        );
        create_change(
            &temp.path().join("openspec/changes/rejected-dep"),
            "Rejected Dep",
            "- [ ] rejected\n",
        );
        fs::write(
            temp.path()
                .join("openspec/changes/rejected-dep/REJECTED.md"),
            "# REJECTED\n",
        )
        .unwrap();

        let mgr = OpenSpecManager::new();
        let changes = mgr.list_changes();
        let dependent = changes
            .iter()
            .find(|change| change.id == "dependent-change")
            .expect("dependent change should be listed");

        assert_eq!(
            dependent.dependency_statuses,
            vec![
                DependencyStatusInfo {
                    id: "pending-dep".to_string(),
                    status: DependencyListStatus::Pending,
                },
                DependencyStatusInfo {
                    id: "running-dep".to_string(),
                    status: DependencyListStatus::Running,
                },
                DependencyStatusInfo {
                    id: "done-dep".to_string(),
                    status: DependencyListStatus::Done,
                },
                DependencyStatusInfo {
                    id: "rejected-dep".to_string(),
                    status: DependencyListStatus::Rejected,
                },
                DependencyStatusInfo {
                    id: "missing-dep".to_string(),
                    status: DependencyListStatus::Missing,
                },
            ]
        );
    }

    #[test]
    fn test_render_changes_output_shows_dependencies_only_when_present() {
        let changes = vec![
            ChangeInfo {
                id: "dependent-change".to_string(),
                path: "openspec/changes/dependent-change".to_string(),
                title: Some("Dependent Change".to_string()),
                tasks_completed: 0,
                tasks_total: 1,
                dependencies: vec!["done-dep".to_string(), "running-dep".to_string()],
                dependency_statuses: vec![
                    DependencyStatusInfo {
                        id: "done-dep".to_string(),
                        status: DependencyListStatus::Done,
                    },
                    DependencyStatusInfo {
                        id: "running-dep".to_string(),
                        status: DependencyListStatus::Running,
                    },
                ],
            },
            ChangeInfo {
                id: "independent-change".to_string(),
                path: "openspec/changes/independent-change".to_string(),
                title: Some("Independent Change".to_string()),
                tasks_completed: 0,
                tasks_total: 1,
                dependencies: Vec::new(),
                dependency_statuses: Vec::new(),
            },
        ];

        let rendered = render_changes_output(&changes);

        assert!(rendered.contains("    Dependencies: done-dep [done], running-dep [running]\n"));
        let independent_block = rendered
            .split("\x1b[1mindependent-change\x1b[0m")
            .nth(1)
            .expect("independent change block should render");
        assert!(!independent_block.contains("Dependencies:"));
    }

    #[test]
    fn test_body_dependencies_fallback_appears_in_list_output() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let dependent_dir = temp.path().join("openspec/changes/body-dependent");
        let dependency_dir = temp.path().join("openspec/changes/body-dep");
        create_change_with_body_dependencies(
            &dependent_dir,
            "Body Dependent",
            &["body-dep"],
            "- [ ] pending\n",
        );
        create_change(&dependency_dir, "Body Dep", "- [ ] pending\n");

        let mgr = OpenSpecManager::new();
        let changes = mgr.list_changes();
        let rendered = render_changes_output(&changes);

        let dependent = changes
            .iter()
            .find(|change| change.id == "body-dependent")
            .expect("body-dependent change should be listed");
        assert_eq!(dependent.dependencies, vec!["body-dep".to_string()]);
        assert!(rendered.contains("    Dependencies: body-dep [pending]\n"));
    }

    #[test]
    fn test_list_specs_includes_requirement_counts() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let foo_dir = temp.path().join("openspec/specs/foo-spec");
        fs::create_dir_all(&foo_dir).unwrap();
        fs::write(
            foo_dir.join("spec.md"),
            "# Foo\n\n### Requirement: One\n\nBody\n\n### Requirement: Two\n\nBody\n",
        )
        .unwrap();

        let empty_dir = temp.path().join("openspec/specs/empty-spec");
        fs::create_dir_all(&empty_dir).unwrap();
        fs::write(
            empty_dir.join("spec.md"),
            "# Empty\n\nNo requirements here.\n",
        )
        .unwrap();

        let mgr = OpenSpecManager::new();
        let specs = mgr.list_specs();

        let foo = specs.iter().find(|s| s.name == "foo-spec").unwrap();
        assert_eq!(foo.requirement_count, 2);

        let empty = specs.iter().find(|s| s.name == "empty-spec").unwrap();
        assert_eq!(empty.requirement_count, 0);

        let rendered = render_specs_output(&specs);
        assert!(rendered.contains("  \x1b[96mempty-spec\x1b[0m"));
        assert!(rendered.contains("    Path: openspec/specs/empty-spec/spec.md"));
        assert!(rendered.contains("    Requirements: 0"));
        assert!(rendered.contains("  \x1b[96mfoo-spec\x1b[0m"));
        assert!(rendered.contains("    Path: openspec/specs/foo-spec/spec.md"));
        assert!(rendered.contains("    Requirements: 2"));
        assert!(!rendered.contains("Dependencies:"));
    }

    #[test]
    fn test_show_change_dependency_statuses_cover_workspace_states() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let dependent_dir = temp.path().join("openspec/changes/dependent-change");
        create_change_with_frontmatter_dependencies(
            &dependent_dir,
            "Dependent Change",
            &[
                "pending-dep",
                "running-dep",
                "done-dep",
                "rejected-dep",
                "missing-dep",
            ],
            "- [ ] pending\n",
        );
        create_change(
            &temp.path().join("openspec/changes/pending-dep"),
            "Pending Dep",
            "- [ ] pending\n",
        );
        create_change(
            &temp.path().join("openspec/changes/running-dep"),
            "Running Dep",
            "- [ ] running\n",
        );
        fs::write(temp.path().join(".conflux-inflight"), "running-dep\n").unwrap();
        create_change(
            &temp
                .path()
                .join("openspec/changes/archive/2026-05-08-done-dep"),
            "Done Dep",
            "- [x] done\n",
        );
        create_change(
            &temp.path().join("openspec/changes/rejected-dep"),
            "Rejected Dep",
            "- [ ] rejected\n",
        );
        fs::write(
            temp.path()
                .join("openspec/changes/rejected-dep/REJECTED.md"),
            "# REJECTED\n",
        )
        .unwrap();

        let mgr = OpenSpecManager::new();
        let info = mgr
            .show_change("dependent-change", false)
            .expect("show should not error")
            .expect("dependent change should resolve via show");

        assert_eq!(
            info.dependencies,
            vec![
                "pending-dep".to_string(),
                "running-dep".to_string(),
                "done-dep".to_string(),
                "rejected-dep".to_string(),
                "missing-dep".to_string(),
            ]
        );
        assert_eq!(
            info.dependency_statuses,
            vec![
                DependencyStatusInfo {
                    id: "pending-dep".to_string(),
                    status: DependencyListStatus::Pending,
                },
                DependencyStatusInfo {
                    id: "running-dep".to_string(),
                    status: DependencyListStatus::Running,
                },
                DependencyStatusInfo {
                    id: "done-dep".to_string(),
                    status: DependencyListStatus::Done,
                },
                DependencyStatusInfo {
                    id: "rejected-dep".to_string(),
                    status: DependencyListStatus::Rejected,
                },
                DependencyStatusInfo {
                    id: "missing-dep".to_string(),
                    status: DependencyListStatus::Missing,
                },
            ]
        );
    }

    #[test]
    fn test_render_show_output_shows_dependencies_only_when_present() {
        let dependent = ShowInfo {
            id: "dependent-change".to_string(),
            path: "openspec/changes/dependent-change".to_string(),
            archived: false,
            proposal: Some("# Dependent Change\n".to_string()),
            tasks: Some("- [ ] pending\n".to_string()),
            tasks_completed: 0,
            tasks_total: 1,
            dependencies: vec!["feature-a".to_string()],
            dependency_statuses: vec![DependencyStatusInfo {
                id: "feature-a".to_string(),
                status: DependencyListStatus::Pending,
            }],
            design: None,
            specs: HashMap::new(),
        };
        let independent = ShowInfo {
            id: "independent-change".to_string(),
            path: "openspec/changes/independent-change".to_string(),
            archived: false,
            proposal: Some("# Independent Change\n".to_string()),
            tasks: Some("- [ ] pending\n".to_string()),
            tasks_completed: 0,
            tasks_total: 1,
            dependencies: Vec::new(),
            dependency_statuses: Vec::new(),
            design: None,
            specs: HashMap::new(),
        };

        let dependent_output = render_show_output(&dependent);
        let independent_output = render_show_output(&independent);

        assert!(dependent_output.contains("Dependencies: feature-a [pending]\n"));
        assert!(!independent_output.contains("Dependencies:"));
    }

    #[test]
    fn test_render_show_json_includes_structured_dependency_statuses() {
        let info = ShowInfo {
            id: "dependent-change".to_string(),
            path: "openspec/changes/dependent-change".to_string(),
            archived: false,
            proposal: Some("# Dependent Change\n".to_string()),
            tasks: Some("- [ ] pending\n".to_string()),
            tasks_completed: 0,
            tasks_total: 1,
            dependencies: vec!["feature-a".to_string()],
            dependency_statuses: vec![DependencyStatusInfo {
                id: "feature-a".to_string(),
                status: DependencyListStatus::Pending,
            }],
            design: None,
            specs: HashMap::new(),
        };

        let json = render_show_json_value(&info);
        let dependencies = json
            .get("dependencies")
            .and_then(|value| value.as_array())
            .expect("dependencies should be a JSON array");

        assert_eq!(dependencies.len(), 1);
        assert_eq!(
            dependencies[0].get("id").and_then(|value| value.as_str()),
            Some("feature-a")
        );
        assert_eq!(
            dependencies[0]
                .get("status")
                .and_then(|value| value.as_str()),
            Some("pending")
        );
    }

    #[test]
    fn test_show_change_deltas_only_omits_dependency_statuses() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let dependent_dir = temp.path().join("openspec/changes/dependent-change");
        create_change_with_frontmatter_dependencies(
            &dependent_dir,
            "Dependent Change",
            &[
                "pending-dep",
                "running-dep",
                "done-dep",
                "rejected-dep",
                "missing-dep",
            ],
            "- [ ] pending\n",
        );
        create_change(
            &temp.path().join("openspec/changes/pending-dep"),
            "Pending Dep",
            "- [ ] pending\n",
        );
        create_change(
            &temp.path().join("openspec/changes/running-dep"),
            "Running Dep",
            "- [ ] running\n",
        );
        fs::write(temp.path().join(".conflux-inflight"), "running-dep\n").unwrap();
        create_change(
            &temp
                .path()
                .join("openspec/changes/archive/2026-05-08-done-dep"),
            "Done Dep",
            "- [x] done\n",
        );
        create_change(
            &temp.path().join("openspec/changes/rejected-dep"),
            "Rejected Dep",
            "- [ ] rejected\n",
        );
        fs::write(
            temp.path()
                .join("openspec/changes/rejected-dep/REJECTED.md"),
            "# REJECTED\n",
        )
        .unwrap();

        let mgr = OpenSpecManager::new();
        let info = mgr
            .show_change("dependent-change", true)
            .expect("show should not error")
            .expect("dependent change should resolve via deltas-only show");
        let json = render_show_json_value(&info);
        let output = render_show_output(&info);

        assert!(info.proposal.is_none());
        assert!(info.tasks.is_none());
        assert!(info.dependencies.is_empty());
        assert!(info.dependency_statuses.is_empty());
        assert!(!json.as_object().unwrap().contains_key("dependencies"));
        assert!(!output.contains("Dependencies:"));
    }

    #[test]
    fn test_show_change_resolves_archived_entry() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let archived_dir = temp.path().join("openspec/changes/archive/archived-change");
        create_change(&archived_dir, "Archived Change", "- [x] archived\n");

        let mgr = OpenSpecManager::new();
        let info = mgr
            .show_change("archived-change", false)
            .expect("show should not error")
            .expect("archived change should resolve via show");

        assert!(info.archived);
        assert_eq!(info.id, "archived-change");
        assert!(info
            .path
            .contains("openspec/changes/archive/archived-change"));
    }

    #[test]
    fn test_show_change_resolves_dated_archived_entry() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let archived_dir = temp
            .path()
            .join("openspec/changes/archive/2026-04-28-archived-change");
        create_change(&archived_dir, "Archived Change", "- [x] archived\n");

        let mgr = OpenSpecManager::new();
        let info = mgr
            .show_change("archived-change", false)
            .expect("show should not error")
            .expect("dated archived change should resolve via show");

        assert!(info.archived);
        assert_eq!(info.id, "archived-change");
        assert!(info
            .path
            .contains("openspec/changes/archive/2026-04-28-archived-change"));
    }

    #[test]
    fn test_show_change_rejects_nested_archived_entry() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let archived_dir = temp
            .path()
            .join("openspec/changes/archive/2026-07-09/archived-change");
        create_change(&archived_dir, "Archived Change", "- [x] archived\n");

        let mgr = OpenSpecManager::new();
        let Err(err) = mgr.show_change("archived-change", false) else {
            panic!("nested archive layout should fail");
        };
        assert!(err.contains("Invalid archive layout"));
        assert!(err.contains("2026-07-09/archived-change"));
    }

    #[test]
    fn test_strict_validate_accepts_matching_modified_target() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        create_canonical_spec(
            temp.path(),
            "target-check",
            "# Target Check\n\n### Requirement: Existing Feature\n\nOld behavior.\n",
        );
        create_strict_change_with_spec_delta(
            &temp.path().join("openspec/changes/valid-modified-target"),
            "target-check",
            "## MODIFIED Requirements\n\n### Requirement: Existing Feature\n\nUpdated behavior.\n\n#### Scenario: Existing feature updates\n- WHEN validation runs\n- THEN the matching canonical target is accepted\n",
        );

        let mgr = OpenSpecManager::new();
        let (is_valid, errors, _warnings) =
            mgr.validate_change(Some("valid-modified-target"), true, "off");

        assert!(is_valid, "matching modified target should pass: {errors:?}");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_strict_validate_rejects_missing_modified_target() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        create_canonical_spec(
            temp.path(),
            "target-check",
            "# Target Check\n\n### Requirement: Existing Feature\n\nOld behavior.\n",
        );
        create_strict_change_with_spec_delta(
            &temp.path().join("openspec/changes/missing-modified-target"),
            "target-check",
            "## MODIFIED Requirements\n\n### Requirement: Missing Feature\n\nUpdated behavior.\n\n#### Scenario: Missing feature updates\n- WHEN validation runs\n- THEN the missing target is rejected\n",
        );

        let mgr = OpenSpecManager::new();
        let (is_valid, errors, _warnings) =
            mgr.validate_change(Some("missing-modified-target"), true, "off");

        assert!(!is_valid);
        assert!(
            errors.iter().any(|error| {
                error.contains("target-check")
                    && error.contains("MODIFIED target not found in canonical spec")
                    && error.contains("### Requirement: Missing Feature")
            }),
            "missing modified target diagnostic should include capability and heading: {errors:?}"
        );
    }

    #[test]
    fn test_strict_validate_rejects_missing_removed_target() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        create_canonical_spec(
            temp.path(),
            "target-check",
            "# Target Check\n\n### Requirement: Existing Feature\n\nOld behavior.\n",
        );
        create_strict_change_with_spec_delta(
            &temp.path().join("openspec/changes/missing-removed-target"),
            "target-check",
            "## REMOVED Requirements\n\n### Requirement: Missing Feature\n\nRemoved behavior.\n\n#### Scenario: Missing feature removal\n- WHEN validation runs\n- THEN the missing target is rejected\n",
        );

        let mgr = OpenSpecManager::new();
        let (is_valid, errors, _warnings) =
            mgr.validate_change(Some("missing-removed-target"), true, "off");

        assert!(!is_valid);
        assert!(
            errors.iter().any(|error| {
                error.contains("target-check")
                    && error.contains("REMOVED target not found in canonical spec")
                    && error.contains("### Requirement: Missing Feature")
            }),
            "missing removed target diagnostic should include capability and heading: {errors:?}"
        );
    }

    #[test]
    fn test_strict_validate_allows_added_only_delta_without_canonical_target() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        create_strict_change_with_spec_delta(
            &temp.path().join("openspec/changes/added-only-target"),
            "brand-new-capability",
            "## ADDED Requirements\n\n### Requirement: New Feature\n\nNew behavior.\n\n#### Scenario: New feature\n- WHEN validation runs\n- THEN no canonical target is required\n",
        );

        let mgr = OpenSpecManager::new();
        let (is_valid, errors, _warnings) =
            mgr.validate_change(Some("added-only-target"), true, "off");

        assert!(is_valid, "added-only delta should pass: {errors:?}");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_strict_validation_enforces_verification_contracts() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());
        let change_dir = temp.path().join("openspec/changes/verification-contract");
        create_strict_valid_change(&change_dir, "Verification Contract");
        let mgr = OpenSpecManager::new();
        assert!(
            mgr.validate_change(Some("verification-contract"), true, "off")
                .0
        );

        let invalid = "---\nverifications:\n  - id: duplicate\n    requirement: x\n    phase: post-integration\n    owner: conflux-acceptance\n    trigger: x\n    automation: ../outside\n    evidence: x\n    rerun: x\n    prerequisites: []\n  - id: duplicate\n    requirement: x\n    phase: invalid\n    owner: repository-automation\n    trigger: x\n    automation: missing\n    evidence: x\n    rerun: x\n    prerequisites: []\n---\n# Invalid\n\n**Change Type**: implementation\n";
        fs::write(change_dir.join("proposal.md"), invalid).unwrap();
        let (valid, errors, _) = mgr.validate_change(Some("verification-contract"), true, "off");
        assert!(!valid);
        assert!(errors
            .iter()
            .any(|error| error.contains("duplicate verification id")));
        assert!(errors
            .iter()
            .any(|error| error.contains("post-integration requires owner")));
        assert!(errors.iter().any(|error| error.contains("invalid phase")));
        assert!(errors
            .iter()
            .any(|error| error.contains("unsafe automation path")));
        assert!(errors.iter().any(|error| error.contains("does not exist")));
    }

    #[test]
    fn test_strict_validation_rejects_untracked_automation() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());
        let change_dir = temp.path().join("openspec/changes/untracked-automation");
        create_strict_valid_change(&change_dir, "Untracked Automation");
        fs::write("untracked.sh", "#!/bin/sh\n").unwrap();
        let proposal = fs::read_to_string(change_dir.join("proposal.md")).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            proposal.replace("automation: Cargo.toml", "automation: untracked.sh"),
        )
        .unwrap();

        let (valid, errors, _) =
            OpenSpecManager::new().validate_change(Some("untracked-automation"), true, "off");

        assert!(!valid);
        assert!(errors
            .iter()
            .any(|error| error.contains("is not tracked by git")));
    }

    #[test]
    fn test_strict_validation_allows_spec_only_without_verifications() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());
        let change_dir = temp.path().join("openspec/changes/spec-only-contract");
        fs::create_dir_all(change_dir.join("specs/example")).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "# Spec Only\n\n**Change Type**: spec-only\n",
        )
        .unwrap();
        fs::write(change_dir.join("tasks.md"), "- [ ] document behavior\n").unwrap();
        fs::write(change_dir.join("specs/example/spec.md"), "## ADDED Requirements\n\n### Requirement: Example\n\n#### Scenario: Example\n- WHEN read\n- THEN valid\n").unwrap();
        assert!(
            OpenSpecManager::new()
                .validate_change(Some("spec-only-contract"), true, "off")
                .0
        );
    }

    #[test]
    fn test_archive_gate_validation_rejects_missing_delta_target() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        create_canonical_spec(
            temp.path(),
            "target-check",
            "# Target Check\n\n### Requirement: Existing Feature\n\nOld behavior.\n",
        );
        create_strict_change_with_spec_delta(
            &temp.path().join("openspec/changes/archive-gate-missing-target"),
            "target-check",
            "## MODIFIED Requirements\n\n### Requirement: Missing Feature\n\nUpdated behavior.\n\n#### Scenario: Missing feature updates\n- WHEN archive gate validation runs\n- THEN the missing target is rejected before archive\n",
        );

        let mgr = OpenSpecManager::new();
        let (is_valid, errors, _warnings) =
            mgr.validate_change(Some("archive-gate-missing-target"), true, "error");

        assert!(!is_valid);
        assert!(
            errors.iter().any(|error| {
                error.contains("MODIFIED target not found in canonical spec")
                    && error.contains("### Requirement: Missing Feature")
            }),
            "archive-gate-equivalent validation should fail before archive: {errors:?}"
        );
    }

    #[test]
    fn test_archive_change_surfaces_missing_delta_target_during_validation() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        create_canonical_spec(
            temp.path(),
            "target-check",
            "# Target Check\n\n### Requirement: Existing Feature\n\nOld behavior.\n",
        );
        create_strict_change_with_spec_delta(
            &temp.path().join("openspec/changes/archive-missing-target"),
            "target-check",
            "## REMOVED Requirements\n\n### Requirement: Missing Feature\n\nRemoved behavior.\n\n#### Scenario: Missing feature removal\n- WHEN archive runs\n- THEN validation fails before promotion simulation\n",
        );

        let mgr = OpenSpecManager::new();
        let err = mgr
            .archive_change("archive-missing-target", false)
            .expect_err("archive should stop at validation for missing canonical target");

        assert!(err.contains("Validation failed"));
        assert!(err.contains("REMOVED target not found in canonical spec"));
        assert!(err.contains("### Requirement: Missing Feature"));
    }

    #[test]
    fn test_archive_change_creates_dated_destination_and_message() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let change_id = "archive-target";
        let change_dir = temp.path().join("openspec/changes").join(change_id);
        create_strict_valid_change(&change_dir, "Archive Target");

        let expected_verifications =
            crate::openspec::parse_proposal_metadata_from_file(&change_dir.join("proposal.md"))
                .verifications;
        let mgr = OpenSpecManager::new();
        let message = mgr
            .archive_change(change_id, true)
            .expect("archive should succeed with dated destination");

        let expected_prefix = format!(
            "Archived to openspec/changes/archive/{}-{}",
            Local::now().format("%Y-%m-%d"),
            change_id
        );
        assert!(message.starts_with(&expected_prefix));

        let archive_dest = temp.path().join(format!(
            "openspec/changes/archive/{}-{}",
            Local::now().format("%Y-%m-%d"),
            change_id
        ));
        assert!(archive_dest.exists());
        assert!(!change_dir.exists());
        let metadata =
            crate::openspec::parse_proposal_metadata_from_file(&archive_dest.join("proposal.md"));
        assert_eq!(metadata.verifications, expected_verifications);
    }

    #[test]
    fn test_archive_change_rejects_existing_dated_destination() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let change_id = "already-archived";
        let change_dir = temp.path().join("openspec/changes").join(change_id);
        create_strict_valid_change(&change_dir, "Already Archived");

        let existing_dest = temp.path().join(format!(
            "openspec/changes/archive/{}-{}",
            Local::now().format("%Y-%m-%d"),
            change_id
        ));
        fs::create_dir_all(&existing_dest).unwrap();

        let mgr = OpenSpecManager::new();
        let err = mgr
            .archive_change(change_id, true)
            .expect_err("archive should fail when dated destination already exists");

        assert!(err.contains("Archive destination already exists"));
        assert!(change_dir.exists());
    }

    #[test]
    fn test_validate_change_classifies_archived_dependency_as_warning() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let active_change = temp.path().join("openspec/changes/active-change");
        create_change_with_frontmatter_dependencies(
            &active_change,
            "Active Change",
            &["archived-dep"],
            "- [ ] 1. task (verification: integration - cargo test)",
        );

        let archived_change = temp
            .path()
            .join("openspec/changes/archive/2026-04-29-archived-dep");
        create_change(&archived_change, "Archived Dep", "- [x] done\n");

        let mgr = OpenSpecManager::new();
        let (is_valid, errors, warnings) = mgr.validate_change(Some("active-change"), false, "off");

        assert!(is_valid);
        assert!(errors.is_empty());
        assert!(warnings
            .iter()
            .any(|w| w.contains("classified as archived dependency reference")));
    }

    #[test]
    fn test_validate_change_reports_missing_dependency_as_error() {
        let temp = TempDir::new().unwrap();
        let _guard = CwdTestGuard::enter(temp.path());

        let active_change = temp.path().join("openspec/changes/active-change");
        create_change_with_frontmatter_dependencies(
            &active_change,
            "Active Change",
            &["missing-dep"],
            "- [ ] 1. task (verification: integration - cargo test)",
        );

        let mgr = OpenSpecManager::new();
        let (is_valid, errors, warnings) = mgr.validate_change(Some("active-change"), false, "off");

        assert!(!is_valid);
        assert!(errors
            .iter()
            .any(|e| e.contains("missing-dep") && e.contains("invalid")));
        assert!(warnings.is_empty());
    }
}

#[cfg(test)]
mod cmd_integration_tests {
    use super::*;

    #[test]
    fn test_cmd_list_runs_without_panic() {
        // Just verify it doesn't panic in the project directory
        let _ = cmd_list(false);
        let _ = cmd_list(true);
    }

    #[test]
    fn test_cmd_show_not_found() {
        let result = cmd_show("nonexistent-change-xyz", false, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_cmd_validate_all() {
        // Should run without panic in the project directory
        let (_, _) = cmd_validate(None, false, "off");
    }

    #[test]
    fn test_cmd_validate_nonexistent() {
        let (is_valid, _) = cmd_validate(Some("nonexistent-xyz"), false, "off");
        assert!(!is_valid);
    }
}
