//! Native OpenSpec utility commands for `cflx openspec`.
//!
//! Replaces the former Python helper `scripts/cflx.py` with native Rust
//! implementations for list, show, validate, and archive operations.

use crate::dependency_targets::{self, DependencyTargetClass};
use chrono::Local;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ─── Spec Promotion Engine ───────────────────────────────────────────────────
// Ported from skills/shared/cflx_spec_promotion.py

/// Parsed delta sections: ADDED, MODIFIED, REMOVED — each maps requirement name to block text.
#[derive(Debug, Default)]
struct DeltaSections {
    added: Vec<(String, String)>,
    modified: Vec<(String, String)>,
    removed: Vec<(String, String)>,
}

/// Split spec content into (preamble, [(name, full_block), ...]).
fn split_spec(content: &str) -> (String, Vec<(String, String)>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?m)^### Requirement:").unwrap());

    let mut blocks: Vec<(String, String)> = Vec::new();

    let parts: Vec<&str> = re.split(content).collect();
    // The regex splits at the start of "### Requirement:" so we need to re-attach
    let starts: Vec<_> = re.find_iter(content).collect();

    if parts.is_empty() {
        return (content.to_string(), blocks);
    }

    let preamble = parts[0].to_string();

    for (i, _start) in starts.iter().enumerate() {
        let block_body = if i < parts.len() - 1 {
            parts[i + 1]
        } else {
            ""
        };
        let full_block = format!("### Requirement:{}", block_body);

        // Extract the key from the heading line
        let heading_line = full_block.lines().next().unwrap_or("");
        let key = heading_line
            .strip_prefix("### Requirement:")
            .unwrap_or("")
            .trim()
            .to_string();

        if !key.is_empty() {
            blocks.push((key, full_block));
        }
    }

    (preamble, blocks)
}

/// Parse delta content into sections (ADDED, MODIFIED, REMOVED).
fn parse_delta_sections(delta: &str) -> DeltaSections {
    static SECTION_RE: OnceLock<Regex> = OnceLock::new();
    let section_re = SECTION_RE
        .get_or_init(|| Regex::new(r"(?m)^## (ADDED|MODIFIED|REMOVED) Requirements\s*$").unwrap());

    let matches: Vec<_> = section_re.find_iter(delta).collect();
    let caps: Vec<_> = section_re.captures_iter(delta).collect();

    let mut sections = DeltaSections::default();

    for (i, cap) in caps.iter().enumerate() {
        let section_type = cap.get(1).unwrap().as_str();
        let start = matches[i].end();
        let end = if i + 1 < matches.len() {
            matches[i + 1].start()
        } else {
            delta.len()
        };
        let section_content = &delta[start..end];
        let (_, blocks) = split_spec(section_content);

        match section_type {
            "ADDED" => sections.added.extend(blocks),
            "MODIFIED" => sections.modified.extend(blocks),
            "REMOVED" => sections.removed.extend(blocks),
            _ => {}
        }
    }

    sections
}

/// Reconstruct a spec from preamble and requirement blocks.
fn reconstruct(preamble: &str, blocks: &[(String, String)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let trimmed_preamble = preamble.trim_end_matches('\n');
    if !trimmed_preamble.trim().is_empty() {
        parts.push(trimmed_preamble.to_string());
    }
    for (_, block) in blocks {
        parts.push(block.trim_end_matches('\n').to_string());
    }
    let mut result = parts.join("\n\n");
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Check if two block lists are equal (same keys and stripped content).
fn blocks_equal(b1: &[(String, String)], b2: &[(String, String)]) -> bool {
    if b1.len() != b2.len() {
        return false;
    }
    b1.iter()
        .zip(b2.iter())
        .all(|((k1, v1), (k2, v2))| k1 == k2 && v1.trim() == v2.trim())
}

/// Merge a change delta into a canonical spec.
///
/// Returns (result_content, errors). When errors is non-empty, the canonical
/// content is returned unchanged.
pub fn merge_spec_delta(canonical: &str, delta: &str) -> (String, Vec<String>) {
    let mut errors: Vec<String> = Vec::new();
    let sections = parse_delta_sections(delta);
    let (preamble, original_blocks) = split_spec(canonical);
    let original_dict: HashMap<&str, &str> = original_blocks
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // Validate MODIFIED targets
    for (key, _) in &sections.modified {
        if !original_dict.contains_key(key.as_str()) {
            errors.push(format!(
                "MODIFIED target not found in canonical spec: '### Requirement: {}'",
                key
            ));
        }
    }

    // Validate REMOVED targets
    for (key, _) in &sections.removed {
        if !original_dict.contains_key(key.as_str()) {
            errors.push(format!(
                "REMOVED target not found in canonical spec: '### Requirement: {}'",
                key
            ));
        }
    }

    if !errors.is_empty() {
        return (canonical.to_string(), errors);
    }

    let removed_keys: std::collections::HashSet<&str> =
        sections.removed.iter().map(|(k, _)| k.as_str()).collect();
    let modified_map: HashMap<&str, &str> = sections
        .modified
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // Apply REMOVED and MODIFIED to original blocks (preserve order)
    let mut result_blocks: Vec<(String, String)> = Vec::new();
    for (key, block) in &original_blocks {
        if removed_keys.contains(key.as_str()) {
            continue; // delete
        } else if let Some(new_block) = modified_map.get(key.as_str()) {
            result_blocks.push((key.clone(), new_block.to_string())); // replace
        } else {
            result_blocks.push((key.clone(), block.clone())); // keep
        }
    }

    // Append ADDED blocks at the end
    for (key, block) in &sections.added {
        result_blocks.push((key.clone(), block.clone()));
    }

    // Reject no-op promotions
    if blocks_equal(&original_blocks, &result_blocks) {
        errors
            .push("Archive promotion would produce no canonical diff (no-op archive)".to_string());
        return (canonical.to_string(), errors);
    }

    (reconstruct(&preamble, &result_blocks), Vec::new())
}

/// Convert a delta-format spec to canonical format for brand-new specs.
pub fn delta_to_canonical(delta: &str) -> Result<String, String> {
    let sections = parse_delta_sections(delta);
    let mut all_blocks: Vec<(String, String)> = Vec::new();
    all_blocks.extend(sections.added);
    all_blocks.extend(sections.modified);
    all_blocks.extend(sections.removed);

    if all_blocks.is_empty() {
        return Err(
            "Spec delta parse error: no canonical requirement blocks found for promotion"
                .to_string(),
        );
    }

    Ok(reconstruct("", &all_blocks))
}

/// Simulate spec promotion without writing files.
pub fn simulate_promotion(canonical: Option<&str>, delta: &str) -> (String, Vec<String>) {
    match canonical {
        None => match delta_to_canonical(delta) {
            Ok(canonicalized) => (canonicalized, Vec::new()),
            Err(err) => (delta.to_string(), vec![err]),
        },
        Some(canonical) => merge_spec_delta(canonical, delta),
    }
}

// ─── OpenSpec Manager ────────────────────────────────────────────────────────

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

    fn find_change_dir(&self, change_id: &str) -> Option<PathBuf> {
        // Check active changes
        let change_dir = self.changes_dir.join(change_id);
        if change_dir.exists() && change_dir.join("proposal.md").exists() {
            return Some(change_dir);
        }

        // Check archive (both direct and dated entries)
        if !self.archive_dir.exists() {
            return None;
        }

        fs::read_dir(&self.archive_dir)
            .ok()?
            .filter_map(|entry| entry.ok())
            .find_map(|entry| {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                let candidate = entry.path();
                if (name_str == change_id || name_str.ends_with(&format!("-{}", change_id)))
                    && candidate.join("proposal.md").exists()
                {
                    Some(candidate)
                } else {
                    None
                }
            })
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

    fn show_change(&self, change_id: &str, deltas_only: bool) -> Option<ShowInfo> {
        let change_dir = self.find_change_dir(change_id)?;
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
            return Some(ShowInfo {
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
            });
        }

        Some(info)
    }

    // ─── Validation ──────────────────────────────────────────────────────

    fn validate_change(
        &self,
        change_id: Option<&str>,
        strict: bool,
        evidence_mode: &str,
    ) -> (bool, Vec<String>, Vec<String>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if let Some(id) = change_id {
            let change_dir = match self.find_change_dir(id) {
                Some(d) => d,
                None => {
                    errors.push(format!("Change '{}' not found", id));
                    return (false, errors, warnings);
                }
            };
            let (e, w) = self.validate_change_dir(&change_dir, strict, evidence_mode);
            errors.extend(e);
            warnings.extend(w);
        } else {
            // Validate all changes
            if self.changes_dir.exists() {
                if let Ok(entries) = fs::read_dir(&self.changes_dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.is_dir() {
                            let name = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            if name == "archive" || name.starts_with('.') {
                                continue;
                            }
                            let (e, w) = self.validate_change_dir(&path, strict, evidence_mode);
                            errors.extend(e);
                            warnings.extend(w);
                        }
                    }
                }
            }
        }

        (errors.is_empty(), errors, warnings)
    }

    fn validate_change_dir(
        &self,
        change_dir: &Path,
        strict: bool,
        evidence_mode: &str,
    ) -> (Vec<String>, Vec<String>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let change_id = change_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let proposal_file = change_dir.join("proposal.md");
        let tasks_file = change_dir.join("tasks.md");

        // Check required files
        if !proposal_file.exists() {
            errors.push(format!("{}: Missing proposal.md", change_id));
        }
        if !tasks_file.exists() {
            errors.push(format!("{}: Missing tasks.md", change_id));
        }

        // Validate proposal structure
        if proposal_file.exists() {
            if let Ok(content) = fs::read_to_string(&proposal_file) {
                static HEADING_RE: OnceLock<Regex> = OnceLock::new();
                let re = HEADING_RE.get_or_init(|| Regex::new(r"(?m)^#\s+.+$").unwrap());
                if !re.is_match(&content) {
                    errors.push(format!("{}: proposal.md missing title heading", change_id));
                }

                // Validate Change Type field in strict mode
                if strict {
                    let change_type = extract_change_type(&content);
                    match change_type {
                        None => {
                            errors.push(format!(
                                "{}: proposal.md missing 'Change Type' field (must be one of: hybrid, implementation, spec-only)",
                                change_id
                            ));
                        }
                        Some(ref ct)
                            if ct != "spec-only" && ct != "implementation" && ct != "hybrid" =>
                        {
                            errors.push(format!(
                                "{}: proposal.md has invalid Change Type '{}' (must be one of: hybrid, implementation, spec-only)",
                                change_id, ct
                            ));
                        }
                        _ => {}
                    }
                }

                let dependency_diagnostics =
                    classify_proposal_dependency_targets(&change_id, &proposal_file);
                for diagnostic in dependency_diagnostics {
                    match diagnostic.classification {
                        DependencyTargetClass::Missing | DependencyTargetClass::Rejected => {
                            errors.push(diagnostic.message)
                        }
                        DependencyTargetClass::Archived => warnings.push(diagnostic.message),
                        DependencyTargetClass::Queued | DependencyTargetClass::InFlight => {}
                    }
                }
            }
        }

        // Validate tasks format
        if tasks_file.exists() {
            if let Ok(content) = fs::read_to_string(&tasks_file) {
                let proposal_content = fs::read_to_string(&proposal_file).ok();
                let change_type = proposal_content
                    .as_deref()
                    .and_then(extract_change_type)
                    .as_deref()
                    .map(str::to_string);
                let (te, tw) = validate_tasks_content(
                    &content,
                    &change_id,
                    strict,
                    evidence_mode,
                    change_type.as_deref(),
                    proposal_content.as_deref(),
                );
                errors.extend(te);
                warnings.extend(tw);
            }
        }

        // Validate spec deltas (strict mode)
        if strict {
            let specs_dir = change_dir.join("specs");
            let no_delta_marker = specs_dir.join(".no-delta");

            if !specs_dir.exists() || !specs_dir.is_dir() {
                errors.push(format!(
                    "{}: No spec deltas found (required in strict mode)",
                    change_id
                ));
                return (errors, warnings);
            }

            let has_no_delta_marker = no_delta_marker.exists() && no_delta_marker.is_file();
            let spec_delta_dirs: Vec<_> = fs::read_dir(&specs_dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .collect();

            if has_no_delta_marker && !spec_delta_dirs.is_empty() {
                errors.push(format!(
                    "{}: specs/.no-delta conflicts with existing spec delta directories",
                    change_id
                ));
                return (errors, warnings);
            }

            if has_no_delta_marker {
                // Intentional no-delta change
                return (errors, warnings);
            }

            if !spec_delta_dirs.is_empty() {
                let se = self.validate_specs_dir(&specs_dir, &change_id);
                errors.extend(se);

                // Archive-risk warning for spec-only proposals
                if proposal_file.exists() {
                    if let Ok(content) = fs::read_to_string(&proposal_file) {
                        if extract_change_type(&content).as_deref() == Some("spec-only") {
                            let rw = check_archive_risk(&specs_dir, &change_id);
                            warnings.extend(rw);
                        }
                    }
                }
            } else {
                errors.push(format!(
                    "{}: No spec deltas found (required in strict mode)",
                    change_id
                ));
            }
        }

        (errors, warnings)
    }

    fn validate_specs_dir(&self, specs_dir: &Path, change_id: &str) -> Vec<String> {
        let mut errors = Vec::new();

        static REQ_RE: OnceLock<Regex> = OnceLock::new();
        static SCENARIO_RE: OnceLock<Regex> = OnceLock::new();
        let req_re = REQ_RE.get_or_init(|| Regex::new(r"(?m)^### Requirement:").unwrap());
        let scenario_re = SCENARIO_RE.get_or_init(|| Regex::new(r"(?m)^#### Scenario:").unwrap());

        if let Ok(entries) = fs::read_dir(specs_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let spec_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let spec_file = path.join("spec.md");

                if !spec_file.exists() {
                    errors.push(format!("{}: Missing spec.md in {}", change_id, spec_name));
                    continue;
                }

                if let Ok(content) = fs::read_to_string(&spec_file) {
                    // Check for delta markers
                    let has_delta = content.contains("## ADDED Requirements")
                        || content.contains("## MODIFIED Requirements")
                        || content.contains("## REMOVED Requirements");

                    if !has_delta {
                        errors.push(format!(
                            "{}: {}/spec.md missing delta markers (ADDED/MODIFIED/REMOVED)",
                            change_id, spec_name
                        ));
                    }

                    // Check for scenarios
                    let has_reqs = req_re.is_match(&content);
                    let has_scenarios = scenario_re.is_match(&content);

                    if has_reqs && !has_scenarios {
                        errors.push(format!(
                            "{}: {}/spec.md has requirements but no scenarios",
                            change_id, spec_name
                        ));
                    }

                    errors.extend(
                        self.validate_delta_targets_against_canonical(
                            &spec_name, &content, change_id,
                        ),
                    );
                }
            }
        }

        errors
    }

    fn validate_delta_targets_against_canonical(
        &self,
        spec_name: &str,
        delta_content: &str,
        change_id: &str,
    ) -> Vec<String> {
        let canonical_spec = self.specs_dir.join(spec_name).join("spec.md");
        let canonical_content = if canonical_spec.exists() {
            fs::read_to_string(&canonical_spec).unwrap_or_default()
        } else {
            String::new()
        };

        let (_, promotion_errors) = merge_spec_delta(&canonical_content, delta_content);
        promotion_errors
            .into_iter()
            .filter(|err| {
                err.contains("MODIFIED target not found in canonical spec")
                    || err.contains("REMOVED target not found in canonical spec")
            })
            .map(|err| format!("{}: {}: {}", change_id, spec_name, err))
            .collect()
    }

    // ─── Archive ─────────────────────────────────────────────────────────

    fn archive_change(&self, change_id: &str, skip_specs: bool) -> Result<String, String> {
        let change_dir = self.changes_dir.join(change_id);

        if !change_dir.exists() {
            return Err(format!("Change '{}' not found", change_id));
        }

        if change_dir.to_string_lossy().contains("/archive/") {
            return Err(format!("Change '{}' is already archived", change_id));
        }

        // Validate before archiving
        let (is_valid, errors, warnings) = self.validate_change(Some(change_id), true, "error");
        if !is_valid {
            return Err(format!("Validation failed:\n{}", errors.join("\n")));
        }
        if !warnings.is_empty() {
            return Err(format!(
                "Validation warnings must be resolved before archive:\n{}",
                warnings.join("\n")
            ));
        }

        // Simulate spec promotion
        if !skip_specs {
            let sim_errors = self.simulate_spec_promotion(&change_dir);
            if !sim_errors.is_empty() {
                return Err(format!(
                    "Spec promotion simulation failed:\n{}",
                    sim_errors.join("\n")
                ));
            }
        }

        // Create archive directory
        fs::create_dir_all(&self.archive_dir)
            .map_err(|e| format!("Failed to create archive directory: {}", e))?;

        // Move to archive using dated destination (YYYY-MM-DD-<change_id>)
        let archive_name = format!("{}-{}", Local::now().format("%Y-%m-%d"), change_id);
        let archive_dest = self.archive_dir.join(&archive_name);
        if archive_dest.exists() {
            return Err(format!(
                "Archive destination already exists: {}",
                archive_dest.display()
            ));
        }

        fs::rename(&change_dir, &archive_dest)
            .map_err(|e| format!("Failed to move to archive: {}", e))?;

        // Update specs
        if !skip_specs {
            let specs_updated = self.update_specs_from_change(&archive_dest);
            Ok(format!(
                "Archived to openspec/changes/archive/{}\nSpecs updated: {:?}",
                archive_name, specs_updated
            ))
        } else {
            Ok(format!(
                "Archived to openspec/changes/archive/{}",
                archive_name
            ))
        }
    }

    fn simulate_spec_promotion(&self, change_dir: &Path) -> Vec<String> {
        let mut errors = Vec::new();
        let specs_dir = change_dir.join("specs");
        if !specs_dir.exists() {
            return errors;
        }

        if let Ok(entries) = fs::read_dir(&specs_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let spec_file = path.join("spec.md");
                if !spec_file.exists() {
                    continue;
                }

                let spec_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let canonical_spec = self.specs_dir.join(&spec_name).join("spec.md");
                let canonical_content = if canonical_spec.exists() {
                    Some(fs::read_to_string(&canonical_spec).unwrap_or_default())
                } else {
                    None
                };
                let delta_content = match fs::read_to_string(&spec_file) {
                    Ok(c) => c,
                    Err(e) => {
                        errors.push(format!("{}: Failed to read delta: {}", spec_name, e));
                        continue;
                    }
                };

                let (_, sim_errors) =
                    simulate_promotion(canonical_content.as_deref(), &delta_content);
                for err in sim_errors {
                    errors.push(format!("{}: {}", spec_name, err));
                }
            }
        }

        errors
    }

    fn update_specs_from_change(&self, change_dir: &Path) -> Vec<String> {
        let mut updated = Vec::new();
        let specs_dir = change_dir.join("specs");

        if !specs_dir.exists() {
            return updated;
        }

        if let Ok(entries) = fs::read_dir(&specs_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let spec_file = path.join("spec.md");
                if !spec_file.exists() {
                    continue;
                }

                let spec_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let canonical_dir = self.specs_dir.join(&spec_name);
                let canonical_spec = canonical_dir.join("spec.md");

                // Create parent dir
                let _ = fs::create_dir_all(&canonical_dir);

                let delta_content = match fs::read_to_string(&spec_file) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if canonical_spec.exists() {
                    let canonical_content = fs::read_to_string(&canonical_spec).unwrap_or_default();
                    let (merged, errors) = merge_spec_delta(&canonical_content, &delta_content);
                    if errors.is_empty() {
                        let _ = fs::write(&canonical_spec, merged);
                        updated.push(spec_name);
                    } else {
                        eprintln!(
                            "parse error/promotion error for spec '{}': {}",
                            spec_name,
                            errors.join("; ")
                        );
                    }
                } else {
                    match delta_to_canonical(&delta_content) {
                        Ok(canonicalized) => {
                            let _ = fs::write(&canonical_spec, canonicalized);
                            updated.push(spec_name);
                        }
                        Err(err) => {
                            eprintln!(
                                "parse error/promotion error for spec '{}': {}",
                                spec_name, err
                            );
                        }
                    }
                }
            }
        }

        updated
    }
}

// ─── Helper types ────────────────────────────────────────────────────────────

struct ChangeInfo {
    id: String,
    path: String,
    title: Option<String>,
    tasks_completed: u32,
    tasks_total: u32,
    dependencies: Vec<String>,
    dependency_statuses: Vec<DependencyStatusInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DependencyStatusInfo {
    id: String,
    status: DependencyListStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyListStatus {
    Done,
    Running,
    Pending,
    Rejected,
    Missing,
}

impl DependencyListStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Running => "running",
            Self::Pending => "pending",
            Self::Rejected => "rejected",
            Self::Missing => "missing",
        }
    }
}

struct DependencyStatusContext {
    active_ids: HashSet<String>,
    in_flight_ids: HashSet<String>,
    archived_ids: HashSet<String>,
    rejected_ids: HashSet<String>,
}

impl DependencyStatusContext {
    fn from_workspace(root_dir: &Path) -> Self {
        Self {
            active_ids: collect_active_change_ids_from_root(root_dir),
            in_flight_ids: collect_in_flight_change_ids_from_root(root_dir),
            archived_ids: dependency_targets::collect_archived_change_ids(root_dir),
            rejected_ids: dependency_targets::collect_rejected_change_ids(root_dir),
        }
    }

    fn statuses_for(&self, dependencies: &[String]) -> Vec<DependencyStatusInfo> {
        dependencies
            .iter()
            .map(|dependency| DependencyStatusInfo {
                id: dependency.clone(),
                status: self.status_for(dependency),
            })
            .collect()
    }

    fn status_for(&self, dependency: &str) -> DependencyListStatus {
        match dependency_targets::classify_dependency_target(
            dependency,
            self.active_ids.iter().map(String::as_str),
            self.in_flight_ids.iter().map(String::as_str),
            &self.archived_ids,
            &self.rejected_ids,
        ) {
            DependencyTargetClass::Archived => DependencyListStatus::Done,
            DependencyTargetClass::InFlight => DependencyListStatus::Running,
            DependencyTargetClass::Queued => DependencyListStatus::Pending,
            DependencyTargetClass::Rejected => DependencyListStatus::Rejected,
            DependencyTargetClass::Missing => DependencyListStatus::Missing,
        }
    }
}

struct SpecInfo {
    name: String,
    path: String,
    requirement_count: usize,
}

struct ShowInfo {
    id: String,
    path: String,
    archived: bool,
    proposal: Option<String>,
    tasks: Option<String>,
    tasks_completed: u32,
    tasks_total: u32,
    dependencies: Vec<String>,
    dependency_statuses: Vec<DependencyStatusInfo>,
    design: Option<String>,
    specs: HashMap<String, String>,
}

// ─── Helper functions ────────────────────────────────────────────────────────

fn count_requirements_in_spec(spec_path: &Path) -> usize {
    static REQUIREMENT_RE: OnceLock<Regex> = OnceLock::new();
    let requirement_re =
        REQUIREMENT_RE.get_or_init(|| Regex::new(r"(?m)^### Requirement:").unwrap());

    match fs::read_to_string(spec_path) {
        Ok(content) => requirement_re.find_iter(&content).count(),
        Err(_) => 0,
    }
}

fn count_tasks(content: &str) -> (u32, u32) {
    static CHECKBOX_RE: OnceLock<Regex> = OnceLock::new();
    static CHECKED_RE: OnceLock<Regex> = OnceLock::new();
    let checkbox_re = CHECKBOX_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[[ x]\]").unwrap());
    let checked_re = CHECKED_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[x\]").unwrap());

    let excluded_sections = ["future work", "out of scope", "notes"];
    let mut in_excluded = false;
    let mut completed = 0u32;
    let mut total = 0u32;

    for line in content.lines() {
        if line.starts_with("##") {
            let section_name = line.trim_start_matches('#').trim().to_lowercase();
            in_excluded = excluded_sections.iter().any(|&s| section_name.contains(s));
            continue;
        }
        if in_excluded {
            continue;
        }
        if checkbox_re.is_match(line) {
            total += 1;
            if checked_re.is_match(line) {
                completed += 1;
            }
        }
    }

    (completed, total)
}

fn extract_change_type(proposal_content: &str) -> Option<String> {
    static CT_RE: OnceLock<Regex> = OnceLock::new();
    let re = CT_RE.get_or_init(|| {
        Regex::new(r"(?mi)^\*\*Change Type\*\*\s*:\s*(.+)$|^Change Type\s*:\s*(.+)$").unwrap()
    });

    re.captures(proposal_content).map(|caps| {
        let value = caps
            .get(1)
            .or_else(|| caps.get(2))
            .unwrap()
            .as_str()
            .trim()
            .trim_matches('`')
            .to_lowercase();
        value
    })
}

#[derive(Debug, Clone)]
struct DependencyTargetDiagnostic {
    classification: DependencyTargetClass,
    message: String,
}

fn classify_proposal_dependency_targets(
    change_id: &str,
    proposal_file: &Path,
) -> Vec<DependencyTargetDiagnostic> {
    let proposal_metadata = crate::openspec::parse_proposal_metadata_from_file(proposal_file);
    if proposal_metadata.dependencies.is_empty() {
        return Vec::new();
    }

    let active_ids = collect_active_change_ids();
    let in_flight_ids = collect_in_flight_change_ids();
    let archived_ids = collect_archived_change_ids();
    let rejected_ids = dependency_targets::collect_rejected_change_ids(Path::new("."));

    proposal_metadata
        .dependencies
        .into_iter()
        .map(|dependency| {
            let classification = dependency_targets::classify_dependency_target(
                &dependency,
                active_ids.iter().map(String::as_str),
                in_flight_ids.iter().map(String::as_str),
                &archived_ids,
                &rejected_ids,
            );

            let message = match classification {
                DependencyTargetClass::Queued => format!(
                    "{}: proposal dependency '{}' classified as queued (active change)",
                    change_id, dependency
                ),
                DependencyTargetClass::InFlight => format!(
                    "{}: proposal dependency '{}' classified as in-flight (workspace execution marker)",
                    change_id, dependency
                ),
                DependencyTargetClass::Archived => format!(
                    "{}: proposal dependency '{}' classified as archived dependency reference (warning: metadata should be reviewed after archive)",
                    change_id, dependency
                ),
                DependencyTargetClass::Rejected => format!(
                    "{}: proposal dependency '{}' is invalid: classified as rejected dependency target",
                    change_id, dependency
                ),
                DependencyTargetClass::Missing => format!(
                    "{}: proposal dependency '{}' is invalid: not found in active, in-flight, archived, or rejected change targets",
                    change_id, dependency
                ),
            };

            DependencyTargetDiagnostic {
                classification,
                message,
            }
        })
        .collect()
}

fn collect_active_change_ids() -> HashSet<String> {
    collect_active_change_ids_from_root(Path::new("."))
}

fn collect_active_change_ids_from_root(root_dir: &Path) -> HashSet<String> {
    let changes_dir = root_dir.join("openspec/changes");
    let Ok(entries) = fs::read_dir(changes_dir) else {
        return HashSet::new();
    };

    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "archive" || name.starts_with('.') {
                return None;
            }
            if !path.join("proposal.md").exists() || path.join("REJECTED.md").exists() {
                return None;
            }
            Some(name)
        })
        .collect()
}

fn collect_in_flight_change_ids() -> HashSet<String> {
    collect_in_flight_change_ids_from_root(Path::new("."))
}

fn collect_in_flight_change_ids_from_root(root_dir: &Path) -> HashSet<String> {
    let state_file = root_dir.join(".conflux-inflight");
    let Ok(content) = fs::read_to_string(state_file) else {
        return HashSet::new();
    };

    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn collect_archived_change_ids() -> HashSet<String> {
    dependency_targets::collect_archived_change_ids(Path::new("."))
}

/// Evidence hint patterns
const EVIDENCE_HINTS: &[&str] = &[
    "src/",
    "tests/",
    "test/",
    "source path",
    "source paths",
    "test file",
    "test files",
    "runnable command",
    "runnable commands",
    "uv run ",
    "pytest",
    "make ",
    "python ",
    "python3 ",
    "cflx validate",
    ".py",
    ".ts",
    ".js",
    ".rs",
    ".go",
    ".toml",
    ".spec",
    ".test",
    "dockerfile",
    " --once",
    "npm test",
    "npm run ",
    "npx ",
    "yarn ",
    "pnpm ",
    "cargo test",
    "cargo build",
    "docker build",
    "go test",
];

const VERIFICATION_OWNERSHIP_MARKERS: &[&str] = &[
    "unit",
    "integration",
    "e2e",
    "manual",
    "benchmark",
    "not-testable",
];

fn has_repository_evidence_hint(verification_text: &str) -> bool {
    let normalized = verification_text.trim().to_lowercase();
    EVIDENCE_HINTS.iter().any(|hint| normalized.contains(hint))
}

fn has_verification_ownership_marker(verification_text: &str) -> bool {
    let normalized = verification_text.trim().to_lowercase();
    VERIFICATION_OWNERSHIP_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle_lower = needle.to_lowercase();
    haystack
        .char_indices()
        .map(|(idx, _)| idx)
        .find(|&idx| haystack[idx..].to_lowercase().starts_with(&needle_lower))
}

fn extract_inline_verification(task_text: &str) -> Option<String> {
    let marker = "(verification:";
    let start = find_case_insensitive(task_text, marker)?;
    let mut depth = 1usize;
    let mut in_backticks = false;
    let content_start = start + marker.len();
    let mut content_end = None;

    for (relative_idx, ch) in task_text[content_start..].char_indices() {
        match ch {
            '`' => in_backticks = !in_backticks,
            '(' if !in_backticks => depth += 1,
            ')' if !in_backticks => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    content_end = Some(content_start + relative_idx);
                    break;
                }
            }
            _ => {}
        }
    }

    let end = content_end.unwrap_or(task_text.len());
    Some(task_text[content_start..end].trim().to_string())
}

fn is_self_referential_final_validation_task(task_text: &str, change_id: &str) -> bool {
    let normalized = task_text.to_lowercase();
    let change_id = change_id.to_lowercase();

    let mentions_same_change_validation = normalized.contains("cflx openspec validate")
        && normalized.contains(&change_id)
        && (normalized.contains("--strict")
            || normalized.contains("--evidence")
            || normalized.contains("final")
            || normalized.contains("archive"));

    let asks_for_final_validation = normalized.contains("final openspec validation")
        || normalized.contains("final validation")
        || normalized.contains("archive validation")
        || normalized.contains("archive gate")
        || normalized.contains("archive readiness");

    mentions_same_change_validation && asks_for_final_validation
}

fn self_referential_final_validation_message(change_id: &str, line_num: usize) -> String {
    format!(
        "{}: tasks.md:{}: self-referential final OpenSpec validation checkbox detected. \
         Final OpenSpec validation must not be a checkbox task; move final validation to a \
         non-checkbox `## Final Validation` section because archive validation is the authoritative gate.",
        change_id, line_num
    )
}

fn validate_tasks_content(
    content: &str,
    change_id: &str,
    strict: bool,
    evidence_mode: &str,
    change_type: Option<&str>,
    _proposal_content: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    static CHECKBOX_RE: OnceLock<Regex> = OnceLock::new();
    static CHECKBOX_DETAIL_RE: OnceLock<Regex> = OnceLock::new();
    static BARE_TASK_RE: OnceLock<Regex> = OnceLock::new();
    static VERIFICATION_CONTINUATION_RE: OnceLock<Regex> = OnceLock::new();

    let checkbox_re = CHECKBOX_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[[ x]\]").unwrap());
    let checkbox_detail_re =
        CHECKBOX_DETAIL_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[([ x])\]\s+(.*)$").unwrap());
    let bare_task_re = BARE_TASK_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s+[^\[]").unwrap());
    let verification_continuation_re = VERIFICATION_CONTINUATION_RE
        .get_or_init(|| Regex::new(r"^\s{2,}(?i:verification:)\s*(.+)$").unwrap());

    let excluded_sections = ["future work", "out of scope", "notes"];
    let mut in_excluded = false;

    let is_behavior_change = matches!(change_type, Some("implementation" | "hybrid"));

    let mut last_checkbox_line_num: Option<usize> = None;
    let mut pending_behavior_task_without_verification: Option<usize> = None;

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;

        if line.starts_with("##") {
            let section_name = line.trim_start_matches('#').trim().to_lowercase();
            in_excluded = excluded_sections.iter().any(|&s| section_name.contains(s));
            continue;
        }

        // Check for checkboxes in excluded sections
        if in_excluded && checkbox_re.is_match(line) {
            errors.push(format!(
                "{}: tasks.md:{}: Checkbox found in excluded section (should be removed)",
                change_id, line_num
            ));
            continue;
        }

        if let Some(caps) = checkbox_detail_re.captures(line) {
            last_checkbox_line_num = Some(line_num);
            if !in_excluded {
                let task_text = caps.get(2).map_or("", |m| m.as_str()).trim();

                let inline_verification = extract_inline_verification(task_text);
                let continuation_verification = verification_continuation_re
                    .captures(line)
                    .and_then(|vcaps| vcaps.get(1).map(|m| m.as_str().trim().to_string()));
                let verification_text = inline_verification
                    .or(continuation_verification)
                    .filter(|v| !v.is_empty());

                if strict && is_self_referential_final_validation_task(task_text, change_id) {
                    errors.push(self_referential_final_validation_message(
                        change_id, line_num,
                    ));
                    pending_behavior_task_without_verification = None;
                } else if strict && evidence_mode != "off" && is_behavior_change {
                    if let Some(vtext) = verification_text {
                        if !has_repository_evidence_hint(&vtext) {
                            let msg = format!(
                                "{}: tasks.md:{}: Verification note should cite repository-verifiable evidence such as source paths, tests, or runnable commands",
                                change_id, line_num
                            );
                            if evidence_mode == "error" {
                                errors.push(msg);
                            } else if evidence_mode == "warn" {
                                warnings.push(msg);
                            }
                        }
                        if !has_verification_ownership_marker(&vtext) {
                            let msg = format!(
                                "{}: tasks.md:{}: Verification ownership missing for behavior-changing task (expected one of: unit, integration, e2e, manual, benchmark, not-testable)",
                                change_id, line_num
                            );
                            if evidence_mode == "error" {
                                errors.push(msg);
                            } else if evidence_mode == "warn" {
                                warnings.push(msg);
                            }
                        }
                        pending_behavior_task_without_verification = None;
                    } else {
                        pending_behavior_task_without_verification = Some(line_num);
                    }
                }
            }
            continue;
        }

        if let Some(caps) = verification_continuation_re.captures(line) {
            if !in_excluded {
                if let Some(prev_line_num) = last_checkbox_line_num {
                    let vtext = caps.get(1).map_or("", |m| m.as_str()).trim();
                    if strict && evidence_mode != "off" && !vtext.is_empty() {
                        if !has_repository_evidence_hint(vtext) {
                            let msg = format!(
                                "{}: tasks.md:{}: Verification note should cite repository-verifiable evidence such as source paths, tests, or runnable commands",
                                change_id, prev_line_num
                            );
                            if evidence_mode == "error" {
                                errors.push(msg);
                            } else if evidence_mode == "warn" {
                                warnings.push(msg);
                            }
                        }
                        if !has_verification_ownership_marker(vtext) {
                            let msg = format!(
                                "{}: tasks.md:{}: Verification ownership missing for behavior-changing task (expected one of: unit, integration, e2e, manual, benchmark, not-testable)",
                                change_id, prev_line_num
                            );
                            if evidence_mode == "error" {
                                errors.push(msg);
                            } else if evidence_mode == "warn" {
                                warnings.push(msg);
                            }
                        }
                    }
                    if pending_behavior_task_without_verification == Some(prev_line_num) {
                        pending_behavior_task_without_verification = None;
                    }
                }
            }
            continue;
        }

        // Check for tasks without checkboxes in active sections
        if !in_excluded && bare_task_re.is_match(line) && !line.trim().is_empty() {
            let trimmed = line.trim();
            if !trimmed.starts_with("##")
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("---")
                && !trimmed.starts_with("```")
            {
                errors.push(format!(
                    "{}: tasks.md:{}: Possible task without checkbox: {}",
                    change_id,
                    line_num,
                    truncate_for_display(trimmed, 50)
                ));
            }
        }
    }

    if strict && evidence_mode != "off" {
        if let Some(line_num) = pending_behavior_task_without_verification {
            let msg = format!(
                "{}: tasks.md:{}: Behavior-bearing task missing '(verification: ...)' note",
                change_id, line_num
            );
            if evidence_mode == "error" {
                errors.push(msg);
            } else if evidence_mode == "warn" {
                warnings.push(msg);
            }
        }
    }

    (errors, warnings)
}

fn check_archive_risk(specs_dir: &Path, change_id: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut has_added = false;
    let mut has_risky = false;

    if let Ok(entries) = fs::read_dir(specs_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let spec_file = path.join("spec.md");
            if !spec_file.exists() {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&spec_file) {
                if content.contains("## ADDED Requirements") {
                    has_added = true;
                }
                if content.contains("## MODIFIED Requirements")
                    || content.contains("## REMOVED Requirements")
                {
                    has_risky = true;
                }
            }
        }
    }

    if has_risky && !has_added {
        warnings.push(format!(
            "{}: ARCHIVE-RISK WARNING — spec-only proposal relies solely on \
             MODIFIED/REMOVED deltas. These require successful canonical promotion rather \
             than simple append behavior. Verify canonical promotion behavior before acceptance.",
            change_id
        ));
    }

    warnings
}

// ─── Public command entry points ─────────────────────────────────────────────

fn render_specs_output(specs: &[SpecInfo]) -> String {
    let mut output = String::from("\n\x1b[1mSpecifications:\x1b[0m\n\n");
    for spec in specs {
        output.push_str(&format!("  \x1b[96m{}\x1b[0m\n", spec.name));
        output.push_str(&format!("    Path: {}\n", spec.path));
        output.push_str(&format!("    Requirements: {}\n\n", spec.requirement_count));
    }
    output
}

fn format_dependency_statuses(dependency_statuses: &[DependencyStatusInfo]) -> String {
    dependency_statuses
        .iter()
        .map(|dependency| format!("{} [{}]", dependency.id, dependency.status.label()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_changes_output(changes: &[ChangeInfo]) -> String {
    let mut output = String::from("\n\x1b[1mChanges:\x1b[0m\n\n");
    for change in changes {
        output.push_str(&format!(
            "  \x1b[92m[ACTIVE]\x1b[0m \x1b[1m{}\x1b[0m\n",
            change.id
        ));
        if let Some(ref title) = change.title {
            output.push_str(&format!("    Title: {}\n", title));
        }
        if change.tasks_total > 0 {
            let progress = format!("{}/{}", change.tasks_completed, change.tasks_total);
            if change.tasks_completed == change.tasks_total {
                output.push_str(&format!("    Tasks: \x1b[92m{}\x1b[0m\n", progress));
            } else {
                output.push_str(&format!("    Tasks: {}\n", progress));
            }
        }
        if !change.dependency_statuses.is_empty() {
            output.push_str(&format!(
                "    Dependencies: {}\n",
                format_dependency_statuses(&change.dependency_statuses)
            ));
        }
        output.push_str(&format!("    Path: {}\n\n", change.path));
    }
    output
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

fn truncate_for_display(content: &str, max_chars: usize) -> String {
    let truncated: String = content.chars().take(max_chars).collect();
    if content.chars().count() > max_chars {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

fn render_show_json_value(info: &ShowInfo) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("id".to_string(), serde_json::Value::String(info.id.clone()));
    map.insert(
        "path".to_string(),
        serde_json::Value::String(info.path.clone()),
    );
    map.insert(
        "archived".to_string(),
        serde_json::Value::Bool(info.archived),
    );

    if let Some(ref proposal) = info.proposal {
        map.insert(
            "proposal".to_string(),
            serde_json::Value::String(proposal.clone()),
        );
    }
    if let Some(ref tasks) = info.tasks {
        map.insert(
            "tasks".to_string(),
            serde_json::Value::String(tasks.clone()),
        );
        map.insert(
            "tasks_completed".to_string(),
            serde_json::Value::Number(info.tasks_completed.into()),
        );
        map.insert(
            "tasks_total".to_string(),
            serde_json::Value::Number(info.tasks_total.into()),
        );
    }
    if let Some(ref design) = info.design {
        map.insert(
            "design".to_string(),
            serde_json::Value::String(design.clone()),
        );
    }
    if !info.dependencies.is_empty() {
        let dependencies = info
            .dependency_statuses
            .iter()
            .map(|dependency| {
                let mut dep_map = serde_json::Map::new();
                dep_map.insert(
                    "id".to_string(),
                    serde_json::Value::String(dependency.id.clone()),
                );
                dep_map.insert(
                    "status".to_string(),
                    serde_json::Value::String(dependency.status.label().to_string()),
                );
                serde_json::Value::Object(dep_map)
            })
            .collect::<Vec<_>>();
        map.insert(
            "dependencies".to_string(),
            serde_json::Value::Array(dependencies),
        );
    }
    if !info.specs.is_empty() {
        let specs_map: serde_json::Map<String, serde_json::Value> = info
            .specs
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        map.insert("specs".to_string(), serde_json::Value::Object(specs_map));
    }

    serde_json::Value::Object(map)
}

fn render_show_output(info: &ShowInfo) -> String {
    let mut output = String::new();
    output.push_str(&format!("\n\x1b[1mChange: {}\x1b[0m\n", info.id));
    output.push_str(&format!("Path: {}\n", info.path));
    output.push_str(&format!(
        "Status: {}\n",
        if info.archived { "ARCHIVED" } else { "ACTIVE" }
    ));

    if info.tasks.is_some() {
        output.push_str(&format!(
            "Tasks: {}/{}\n",
            info.tasks_completed, info.tasks_total
        ));
    }

    if !info.dependency_statuses.is_empty() {
        output.push_str(&format!(
            "Dependencies: {}\n",
            format_dependency_statuses(&info.dependency_statuses)
        ));
    }

    if let Some(ref proposal) = info.proposal {
        output.push_str("\n\x1b[1mProposal:\x1b[0m\n");
        output.push_str(&format!("{}\n", truncate_for_display(proposal, 500)));
    }

    if !info.specs.is_empty() {
        output.push_str("\n\x1b[1mSpec Deltas:\x1b[0m\n");
        for (name, content) in &info.specs {
            output.push_str(&format!("\n  \x1b[96m{}:\x1b[0m\n", name));
            output.push_str(&format!("  {}\n", truncate_for_display(content, 300)));
        }
    }

    output
}

/// `cflx openspec show` — show change details.
pub fn cmd_show(change_id: &str, json_output: bool, deltas_only: bool) -> Result<(), String> {
    let mgr = OpenSpecManager::new();
    let info = mgr
        .show_change(change_id, deltas_only)
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
    use super::*;

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

    fn create_strict_valid_change(dir: &Path, proposal_title: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("proposal.md"),
            format!(
                "# {}\n\n**Change Type**: implementation\n\n## Problem\narchive behavior update\n",
                proposal_title
            ),
        )
        .unwrap();
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
            "# Strict validation fixture\n\n**Change Type**: implementation\n\n## Problem\nvalidator fixture\n",
        )
        .unwrap();
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
            .expect("dated archived change should resolve via show");

        assert!(info.archived);
        assert_eq!(info.id, "archived-change");
        assert!(info
            .path
            .contains("openspec/changes/archive/2026-04-28-archived-change"));
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
