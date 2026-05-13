use crate::dependency_targets::DependencyTargetClass;
use crate::openspec_cmd::dependency_status::classify_proposal_dependency_targets;
use crate::openspec_cmd::promotion::merge_spec_delta;
use crate::openspec_cmd::rendering::truncate_for_display;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

pub(super) struct ValidationEngine<'a> {
    pub(super) manager: &'a super::OpenSpecManager,
}

impl<'a> ValidationEngine<'a> {
    pub(super) fn validate_change(
        &self,
        change_id: Option<&str>,
        strict: bool,
        evidence_mode: &str,
    ) -> (bool, Vec<String>, Vec<String>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if let Some(id) = change_id {
            let change_dir = match self.manager.find_change_dir(id) {
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
            if self.manager.changes_dir.exists() {
                if let Ok(entries) = fs::read_dir(&self.manager.changes_dir) {
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
                        DependencyTargetClass::Queued
                        | DependencyTargetClass::InFlight
                        | DependencyTargetClass::ActiveButNotQueued => {}
                        DependencyTargetClass::Error => unreachable!(
                            "proposal dependency classification cannot produce terminal-error state"
                        ),
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
        let canonical_spec = self.manager.specs_dir.join(spec_name).join("spec.md");
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
}

pub(super) fn count_requirements_in_spec(spec_path: &Path) -> usize {
    static REQUIREMENT_RE: OnceLock<Regex> = OnceLock::new();
    let requirement_re =
        REQUIREMENT_RE.get_or_init(|| Regex::new(r"(?m)^### Requirement:").unwrap());

    match fs::read_to_string(spec_path) {
        Ok(content) => requirement_re.find_iter(&content).count(),
        Err(_) => 0,
    }
}

pub(super) fn count_tasks(content: &str) -> (u32, u32) {
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

pub(super) fn extract_change_type(proposal_content: &str) -> Option<String> {
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

pub(super) fn has_repository_evidence_hint(verification_text: &str) -> bool {
    let normalized = verification_text.trim().to_lowercase();
    EVIDENCE_HINTS.iter().any(|hint| normalized.contains(hint))
}

pub(super) fn has_verification_ownership_marker(verification_text: &str) -> bool {
    let normalized = verification_text.trim().to_lowercase();
    VERIFICATION_OWNERSHIP_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

pub(super) fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let needle_lower = needle.to_lowercase();
    haystack
        .char_indices()
        .map(|(idx, _)| idx)
        .find(|&idx| haystack[idx..].to_lowercase().starts_with(&needle_lower))
}

pub(super) fn extract_inline_verification(task_text: &str) -> Option<String> {
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

pub(super) fn is_self_referential_final_validation_task(task_text: &str, change_id: &str) -> bool {
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

pub(super) fn self_referential_final_validation_message(
    change_id: &str,
    line_num: usize,
) -> String {
    format!(
        "{}: tasks.md:{}: self-referential final OpenSpec validation checkbox detected. \
         Final OpenSpec validation must not be a checkbox task; move final validation to a \
         non-checkbox `## Final Validation` section because archive validation is the authoritative gate.",
        change_id, line_num
    )
}

pub(super) fn validate_tasks_content(
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

pub(super) fn check_archive_risk(specs_dir: &Path, change_id: &str) -> Vec<String> {
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
