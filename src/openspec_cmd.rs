//! Native OpenSpec utility commands for `cflx openspec`.
//!
//! Replaces the former Python helper `scripts/cflx.py` with native Rust
//! implementations for list, show, validate, and archive operations.

use chrono::Local;
use regex::Regex;
use std::collections::HashMap;
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
pub fn delta_to_canonical(delta: &str) -> String {
    let sections = parse_delta_sections(delta);
    let mut all_blocks: Vec<(String, String)> = Vec::new();
    all_blocks.extend(sections.added);
    all_blocks.extend(sections.modified);
    all_blocks.extend(sections.removed);

    if all_blocks.is_empty() {
        // Fallback: strip section markers when no requirement blocks were parsed
        static FALLBACK_RE: OnceLock<Regex> = OnceLock::new();
        let re = FALLBACK_RE.get_or_init(|| {
            Regex::new(r"(?m)^## (?:ADDED|MODIFIED|REMOVED) Requirements\s*\n").unwrap()
        });
        return re.replace_all(delta, "## Requirements\n").to_string();
    }

    reconstruct("", &all_blocks)
}

/// Simulate spec promotion without writing files.
pub fn simulate_promotion(canonical: Option<&str>, delta: &str) -> (String, Vec<String>) {
    match canonical {
        None => (delta_to_canonical(delta), Vec::new()),
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
                if let Some(info) = self.get_change_info(&path, false) {
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

        let mut info = ChangeInfo {
            id,
            path: rel_path,
            title: None,
            tasks_completed: 0,
            tasks_total: 0,
        };

        // Extract title from proposal.md
        if let Ok(content) = fs::read_to_string(change_dir.join("proposal.md")) {
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
            design: None,
            specs: HashMap::new(),
        };

        // Read proposal
        if let Ok(content) = fs::read_to_string(change_dir.join("proposal.md")) {
            info.proposal = Some(content);
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
                }
            }
        }

        errors
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
                    let (merged, _) = merge_spec_delta(&canonical_content, &delta_content);
                    let _ = fs::write(&canonical_spec, merged);
                } else {
                    let _ = fs::write(&canonical_spec, delta_to_canonical(&delta_content));
                }

                updated.push(spec_name);
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

/// Behavior-bearing task keywords
const BEHAVIOR_TASK_KEYWORDS: &[&str] = &[
    "add ",
    "implement ",
    "create ",
    "update ",
    "modify ",
    "introduce ",
    "wire ",
    "integrate ",
    "expose ",
    "persist ",
    "support ",
    "build ",
];

/// Evidence hint patterns
const EVIDENCE_HINTS: &[&str] = &[
    "src/",
    "tests/",
    "test/",
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
    ".spec",
    ".test",
    " --once",
    "npm test",
    "npm run ",
    "npx ",
    "yarn ",
    "pnpm ",
    "cargo test",
    "cargo build",
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

const ARTIFACT_HEAVY_TASK_KEYWORDS: &[&str] = &["define ", "document ", "describe "];

const EXECUTABLE_SURFACE_HINTS: &[&str] = &[
    " cli",
    " api",
    "workflow",
    " job",
    " worker",
    " background process",
    " command",
    " webhook",
    " endpoint",
];

const EXECUTABLE_VERIFICATION_HINTS: &[&str] = &[
    "curl ",
    "http",
    "api",
    "cli",
    "command",
    "cflx openspec",
    "cargo run",
    "npm run",
    "pytest",
    "go test",
    "cargo test",
];

const RUNTIME_BEHAVIOR_HINTS: &[&str] = &[
    "handler",
    "webhook",
    "persist",
    "persistence",
    "notification",
    "command",
    "job",
    "worker",
    "queue",
    "background process",
];

fn looks_like_behavior_task(task_text: &str) -> bool {
    let normalized = task_text.trim().to_lowercase();
    BEHAVIOR_TASK_KEYWORDS
        .iter()
        .any(|kw| normalized.contains(kw))
}

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

fn looks_like_artifact_heavy_task(task_text: &str) -> bool {
    let normalized = task_text.trim().to_lowercase();
    ARTIFACT_HEAVY_TASK_KEYWORDS
        .iter()
        .any(|kw| normalized.contains(kw))
}

fn proposal_mentions_executable_surface(proposal_content: &str) -> bool {
    let normalized = format!(" {}", proposal_content.trim().to_lowercase());
    EXECUTABLE_SURFACE_HINTS
        .iter()
        .any(|hint| normalized.contains(hint))
}

fn verification_mentions_executable_surface(verification_text: &str) -> bool {
    let normalized = verification_text.trim().to_lowercase();
    EXECUTABLE_VERIFICATION_HINTS
        .iter()
        .any(|hint| normalized.contains(hint))
}

fn proposal_mentions_runtime_behavior(proposal_content: &str) -> bool {
    let normalized = proposal_content.trim().to_lowercase();
    RUNTIME_BEHAVIOR_HINTS
        .iter()
        .any(|hint| normalized.contains(hint))
}

fn validate_tasks_content(
    content: &str,
    change_id: &str,
    strict: bool,
    evidence_mode: &str,
    change_type: Option<&str>,
    proposal_content: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    static CHECKBOX_RE: OnceLock<Regex> = OnceLock::new();
    static CHECKBOX_DETAIL_RE: OnceLock<Regex> = OnceLock::new();
    static BARE_TASK_RE: OnceLock<Regex> = OnceLock::new();
    static VERIFICATION_RE: OnceLock<Regex> = OnceLock::new();
    static VERIFICATION_CONTINUATION_RE: OnceLock<Regex> = OnceLock::new();

    let checkbox_re = CHECKBOX_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[[ x]\]").unwrap());
    let checkbox_detail_re =
        CHECKBOX_DETAIL_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[([ x])\]\s+(.*)$").unwrap());
    let bare_task_re = BARE_TASK_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s+[^\[]").unwrap());
    let verification_re = VERIFICATION_RE
        .get_or_init(|| Regex::new(r"(?i)\(verification:\s*(.+?)\)\s*[.。]?$").unwrap());
    let verification_continuation_re = VERIFICATION_CONTINUATION_RE
        .get_or_init(|| Regex::new(r"^\s{2,}(?i:verification:)\s*(.+)$").unwrap());

    let excluded_sections = ["future work", "out of scope", "notes"];
    let mut in_excluded = false;

    let is_behavior_change = matches!(change_type, Some("implementation" | "hybrid"));
    let proposal_text = proposal_content.unwrap_or_default();
    let proposal_has_executable_surface = proposal_mentions_executable_surface(proposal_text);
    let proposal_has_runtime_behavior = proposal_mentions_runtime_behavior(proposal_text);

    let mut behavior_task_count = 0usize;
    let mut artifact_task_count = 0usize;
    let mut has_executable_runnable_verification = false;
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
                let is_behavior_task = looks_like_behavior_task(task_text);
                let is_artifact_task = looks_like_artifact_heavy_task(task_text);
                if is_behavior_task {
                    behavior_task_count += 1;
                }
                if is_artifact_task {
                    artifact_task_count += 1;
                }

                let inline_verification = verification_re
                    .captures(task_text)
                    .and_then(|vcaps| vcaps.get(1).map(|m| m.as_str().trim().to_string()));
                let continuation_verification = verification_continuation_re
                    .captures(line)
                    .and_then(|vcaps| vcaps.get(1).map(|m| m.as_str().trim().to_string()));
                let verification_text = inline_verification
                    .or(continuation_verification)
                    .filter(|v| !v.is_empty());

                if let Some(vtext) = &verification_text {
                    if verification_mentions_executable_surface(vtext) {
                        has_executable_runnable_verification = true;
                    }
                }

                if strict && evidence_mode != "off" && is_behavior_task {
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
                    if verification_mentions_executable_surface(vtext) {
                        has_executable_runnable_verification = true;
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

    if strict && evidence_mode != "off" && is_behavior_change {
        if artifact_task_count > 0 && artifact_task_count >= behavior_task_count.max(1) {
            let msg = format!(
                "{}: tasks.md: Artifact-oriented tasks dominate or match behavior-changing tasks",
                change_id
            );
            if evidence_mode == "error" {
                errors.push(msg);
            } else if evidence_mode == "warn" {
                warnings.push(msg);
            }
        }

        if proposal_has_executable_surface && !has_executable_runnable_verification {
            let msg = format!(
                "{}: tasks.md: Executable-surface behavior lacks runnable verification coverage",
                change_id
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

/// `cflx openspec list` — list changes or specs.
pub fn cmd_list(show_specs: bool) -> Result<(), String> {
    let mgr = OpenSpecManager::new();

    if show_specs {
        let specs = mgr.list_specs();
        print!("{}", render_specs_output(&specs));
    } else {
        let changes = mgr.list_changes();
        println!("\n\x1b[1mChanges:\x1b[0m\n");
        for change in &changes {
            println!("  \x1b[92m[ACTIVE]\x1b[0m \x1b[1m{}\x1b[0m", change.id);
            if let Some(ref title) = change.title {
                println!("    Title: {}", title);
            }
            if change.tasks_total > 0 {
                let progress = format!("{}/{}", change.tasks_completed, change.tasks_total);
                if change.tasks_completed == change.tasks_total {
                    println!("    Tasks: \x1b[92m{}\x1b[0m", progress);
                } else {
                    println!("    Tasks: {}", progress);
                }
            }
            println!("    Path: {}", change.path);
            println!();
        }
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

/// `cflx openspec show` — show change details.
pub fn cmd_show(change_id: &str, json_output: bool, deltas_only: bool) -> Result<(), String> {
    let mgr = OpenSpecManager::new();
    let info = mgr
        .show_change(change_id, deltas_only)
        .ok_or_else(|| format!("Change '{}' not found", change_id))?;

    if json_output {
        let mut map = serde_json::Map::new();
        map.insert("id".to_string(), serde_json::Value::String(info.id));
        map.insert("path".to_string(), serde_json::Value::String(info.path));
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
        if !info.specs.is_empty() {
            let specs_map: serde_json::Map<String, serde_json::Value> = info
                .specs
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            map.insert("specs".to_string(), serde_json::Value::Object(specs_map));
        }

        let json_value = serde_json::Value::Object(map);
        println!(
            "{}",
            serde_json::to_string_pretty(&json_value).unwrap_or_default()
        );
        return Ok(());
    }

    // Human-readable output
    println!("\n\x1b[1mChange: {}\x1b[0m", info.id);
    println!("Path: {}", info.path);
    println!(
        "Status: {}",
        if info.archived { "ARCHIVED" } else { "ACTIVE" }
    );

    if info.tasks.is_some() {
        println!("Tasks: {}/{}", info.tasks_completed, info.tasks_total);
    }

    if let Some(ref proposal) = info.proposal {
        println!("\n\x1b[1mProposal:\x1b[0m");
        println!("{}", truncate_for_display(proposal, 500));
    }

    if !info.specs.is_empty() {
        println!("\n\x1b[1mSpec Deltas:\x1b[0m");
        for (name, content) in &info.specs {
            println!("\n  \x1b[96m{}:\x1b[0m", name);
            println!("  {}", truncate_for_display(content, 300));
        }
    }

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
        let result = delta_to_canonical(delta);
        assert!(result.contains("### Requirement: Feature A"));
        assert!(!result.contains("## ADDED"));
    }

    #[test]
    fn test_delta_to_canonical_fallback() {
        let delta = "## ADDED Requirements\n\nSome content without requirement blocks.\n";
        let result = delta_to_canonical(delta);
        assert!(result.contains("## Requirements"));
        assert!(!result.contains("## ADDED"));
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
    fn test_validate_tasks_with_standalone_verification_hint() {
        let content =
            "- [ ] Add a new feature\n  verification: unit - cargo test covers the feature\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);
        assert!(errors.is_empty());
        assert!(warnings.is_empty());
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
    fn test_validate_tasks_with_weak_verification() {
        let content = "- [ ] Add a new feature (verification: manual review)\n";
        let (errors, warnings) =
            validate_tasks_content(content, "test", true, "warn", Some("implementation"), None);
        assert!(errors.is_empty());
        // "manual review" lacks repository-verifiable hints
        assert!(!warnings.is_empty());
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

    #[test]
    fn test_warns_artifact_heavy_tasks_dominate() {
        let content =
            "- [ ] Define API contract (verification: manual - documented in docs/api.md)\n";
        let (errors, warnings) = validate_tasks_content(
            content,
            "test",
            true,
            "warn",
            Some("implementation"),
            Some("# Change\n\n**Change Type**: implementation\n"),
        );
        assert!(errors.is_empty());
        assert!(warnings
            .iter()
            .any(|w| w.contains("Artifact-oriented tasks dominate")));
    }

    #[test]
    fn test_warns_executable_surface_without_runnable_verification() {
        let content =
            "- [ ] Implement queue worker support (verification: manual - reviewer walkthrough)\n";
        let proposal = "# Change\n\n**Change Type**: implementation\n\n## Problem\nAPI workflow must trigger worker job\n";
        let (errors, warnings) = validate_tasks_content(
            content,
            "test",
            true,
            "warn",
            Some("implementation"),
            Some(proposal),
        );
        assert!(errors.is_empty());
        assert!(warnings
            .iter()
            .any(|w| w.contains("Executable-surface behavior lacks runnable verification")));
    }
}

#[cfg(test)]
mod openspec_list_show_tests {
    use super::*;
    use std::env;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
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

    #[test]
    fn test_list_changes_excludes_archived_entries() {
        let _guard = cwd_lock().lock().unwrap();
        let original_cwd = env::current_dir().unwrap();
        let temp = TempDir::new().unwrap();

        env::set_current_dir(temp.path()).unwrap();

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

        env::set_current_dir(original_cwd).unwrap();
    }

    #[test]
    fn test_list_specs_includes_requirement_counts() {
        let _guard = cwd_lock().lock().unwrap();
        let original_cwd = env::current_dir().unwrap();
        let temp = TempDir::new().unwrap();

        env::set_current_dir(temp.path()).unwrap();

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

        env::set_current_dir(original_cwd).unwrap();
    }

    #[test]
    fn test_show_change_resolves_archived_entry() {
        let _guard = cwd_lock().lock().unwrap();
        let original_cwd = env::current_dir().unwrap();
        let temp = TempDir::new().unwrap();

        env::set_current_dir(temp.path()).unwrap();

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

        env::set_current_dir(original_cwd).unwrap();
    }

    #[test]
    fn test_show_change_resolves_dated_archived_entry() {
        let _guard = cwd_lock().lock().unwrap();
        let original_cwd = env::current_dir().unwrap();
        let temp = TempDir::new().unwrap();

        env::set_current_dir(temp.path()).unwrap();

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

        env::set_current_dir(original_cwd).unwrap();
    }

    #[test]
    fn test_archive_change_creates_dated_destination_and_message() {
        let _guard = cwd_lock().lock().unwrap();
        let original_cwd = env::current_dir().unwrap();
        let temp = TempDir::new().unwrap();

        env::set_current_dir(temp.path()).unwrap();

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

        env::set_current_dir(original_cwd).unwrap();
    }

    #[test]
    fn test_archive_change_rejects_existing_dated_destination() {
        let _guard = cwd_lock().lock().unwrap();
        let original_cwd = env::current_dir().unwrap();
        let temp = TempDir::new().unwrap();

        env::set_current_dir(temp.path()).unwrap();

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

        env::set_current_dir(original_cwd).unwrap();
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
