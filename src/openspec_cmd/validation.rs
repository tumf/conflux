use crate::dependency_targets::DependencyTargetClass;
use crate::openspec::{VerificationCompletionRole, VerificationDeclaration, VerificationRoleIssue};
use crate::openspec_cmd::dependency_status::classify_proposal_dependency_targets;
use crate::openspec_cmd::promotion::merge_spec_delta;
use crate::openspec_cmd::rendering::truncate_for_display;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path};
use std::process::Command;
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
                Ok(Some(d)) => d,
                Ok(None) => {
                    errors.push(format!("Change '{}' not found", id));
                    return (false, errors, warnings);
                }
                Err(error) => {
                    errors.push(error);
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
                    match change_type.as_deref() {
                        None => errors.push(format!(
                            "{}: proposal.md missing 'Change Type' field (must be one of: hybrid, implementation, spec-only)",
                            change_id
                        )),
                        Some(ct) if !matches!(ct, "spec-only" | "implementation" | "hybrid") => {
                            errors.push(format!(
                                "{}: proposal.md has invalid Change Type '{}' (must be one of: hybrid, implementation, spec-only)",
                                change_id, ct
                            ));
                        }
                        _ => {}
                    }
                    let (verification_errors, verification_warnings) =
                        validate_verification_declarations(
                            &content,
                            &proposal_file,
                            &self.manager.root_dir,
                            &change_id,
                            change_type.as_deref(),
                        );
                    errors.extend(verification_errors);
                    warnings.extend(verification_warnings);

                    let (gate_errors, gate_warnings) = validate_release_gate_dependencies(
                        &self.manager.root_dir,
                        &change_id,
                        &content,
                        &proposal_file,
                    );
                    errors.extend(gate_errors);
                    warnings.extend(gate_warnings);
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
                        | DependencyTargetClass::Resolving
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

/// Classification of a top-level `tasks.md` section.
///
/// Task counting and task validation share this contract so a section can never
/// be counted as active work by one and narrative metadata by the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskSectionKind {
    /// Implementation work. Task-like bullets MUST use checkboxes.
    Active,
    /// Human-facing metadata (Final Validation, Implementation Blocker, Future
    /// Work, Out of Scope, Notes, Acceptance Notes). Ordinary prose and list
    /// metadata are permitted; checkboxes are not.
    Narrative,
    /// Runtime-owned acceptance follow-up. The runtime writes and clears these
    /// sections, so they keep their own parsing and validation behavior.
    RuntimeAcceptanceFollowUp,
}

impl TaskSectionKind {
    /// Whether checkbox tasks in this section participate in progress totals.
    fn counts_tasks(self) -> bool {
        !matches!(self, TaskSectionKind::Narrative)
    }
}

/// Narrative section markers matched as substrings of a lowercased heading.
///
/// `Recovered Acceptance Notes` and `Acceptance Notes` both match `notes`, and
/// `Implementation Blocker #3` matches `implementation blocker`.
const NARRATIVE_SECTION_MARKERS: &[&str] = &[
    "future work",
    "out of scope",
    "notes",
    "final validation",
    "implementation blocker",
];

/// Classify a top-level `tasks.md` heading line (`## ...`).
///
/// Runtime ownership wins over narrative matching so runtime-owned sections keep
/// their dedicated behavior regardless of wording overlap.
pub(crate) fn classify_task_section(heading_line: &str) -> TaskSectionKind {
    let section_name = heading_line.trim_start_matches('#').trim().to_lowercase();

    if section_name == "current acceptance follow-up"
        || (section_name.starts_with("acceptance #")
            && section_name.ends_with(" failure follow-up"))
    {
        return TaskSectionKind::RuntimeAcceptanceFollowUp;
    }

    if NARRATIVE_SECTION_MARKERS
        .iter()
        .any(|marker| section_name.contains(marker))
    {
        return TaskSectionKind::Narrative;
    }

    TaskSectionKind::Active
}

pub(super) fn count_tasks(content: &str) -> (u32, u32) {
    static CHECKBOX_RE: OnceLock<Regex> = OnceLock::new();
    static CHECKED_RE: OnceLock<Regex> = OnceLock::new();
    let checkbox_re = CHECKBOX_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[[ x]\]").unwrap());
    let checked_re = CHECKED_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[x\]").unwrap());

    let mut section = TaskSectionKind::Active;
    let mut completed = 0u32;
    let mut total = 0u32;
    let mut fences = crate::task_parser::FenceTracker::default();

    for line in content.lines() {
        // Fenced literals (e.g. recovered acceptance notes) are inert content.
        if fences.observe(line) {
            continue;
        }
        if line.starts_with("##") {
            // Runtime-owned follow-up subsections stay runtime-owned until the
            // next top-level section, mirroring task validation.
            if line.starts_with("###") && section == TaskSectionKind::RuntimeAcceptanceFollowUp {
                continue;
            }
            section = classify_task_section(line);
            continue;
        }
        if !section.counts_tasks() {
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

fn validate_verification_declarations(
    proposal_content: &str,
    proposal_file: &Path,
    root_dir: &Path,
    change_id: &str,
    change_type: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let parsed =
        crate::openspec::parse_proposal_frontmatter_strict(proposal_content, proposal_file);
    let mut errors = parsed
        .diagnostics
        .into_iter()
        .map(|diagnostic| {
            format!(
                "{}: proposal.md verification metadata: {}",
                change_id, diagnostic
            )
        })
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    let verifications = parsed
        .metadata
        .map(|metadata| metadata.verifications)
        .unwrap_or_default();
    let mut ids = HashSet::new();
    let mut has_pre_integration = false;

    for verification in &verifications {
        let label = verification.id.as_deref().unwrap_or("<missing id>");
        for (field, value) in [
            ("id", verification.id.as_deref()),
            ("requirement", verification.requirement.as_deref()),
            ("phase", verification.phase.as_deref()),
            ("owner", verification.owner.as_deref()),
            ("trigger", verification.trigger.as_deref()),
            ("automation", verification.automation.as_deref()),
            ("evidence", verification.evidence.as_deref()),
            ("rerun", verification.rerun.as_deref()),
        ] {
            if value.is_none_or(|value| value.trim().is_empty()) {
                errors.push(format!(
                    "{}: verification '{}': missing non-empty {}",
                    change_id, label, field
                ));
            }
        }
        match verification.prerequisites.as_deref() {
            None => errors.push(format!(
                "{}: verification '{}': missing prerequisites list",
                change_id, label
            )),
            Some(prerequisites) if prerequisites.iter().any(|item| item.trim().is_empty()) => {
                errors.push(format!(
                    "{}: verification '{}': prerequisites must not contain empty values",
                    change_id, label
                ));
            }
            Some(_) => {}
        }
        if let Some(id) = verification
            .id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
        {
            if !ids.insert(id) {
                errors.push(format!("{}: duplicate verification id '{}'", change_id, id));
            }
        }
        match verification.phase.as_deref() {
            Some("pre-integration") => {
                has_pre_integration = true;
                if verification.owner.as_deref() != Some("conflux-acceptance") {
                    errors.push(format!(
                        "{}: verification '{}': pre-integration requires owner: conflux-acceptance",
                        change_id, label
                    ));
                }
            }
            Some("post-integration") => {
                if verification.owner.as_deref() != Some("repository-automation") {
                    errors.push(format!("{}: verification '{}': post-integration requires owner: repository-automation", change_id, label));
                }
            }
            Some(phase) => errors.push(format!(
                "{}: verification '{}': invalid phase '{}'",
                change_id, label, phase
            )),
            None => {}
        }
        if let Some(automation) = verification.automation.as_deref() {
            errors.extend(validate_automation_path(
                root_dir, change_id, label, automation,
            ));
        }
        for issue in crate::openspec::evaluate_verification_role(verification) {
            let message = verification_role_issue_message(change_id, label, verification, &issue);
            if issue.is_warning() {
                warnings.push(message);
            } else {
                errors.push(message);
            }
        }
    }

    if matches!(change_type, Some("implementation" | "hybrid")) && !has_pre_integration {
        errors.push(format!("{}: implementation and hybrid proposals require at least one pre-integration verification with repository-verifiable evidence", change_id));
    }
    (errors, warnings)
}

/// Render a role-metadata issue as an owning-proposal diagnostic.
///
/// Migration guidance is derived from the structured `phase` field rather than
/// prose so natural-language content never controls the classification.
fn verification_role_issue_message(
    change_id: &str,
    label: &str,
    verification: &VerificationDeclaration,
    issue: &VerificationRoleIssue,
) -> String {
    match issue {
        VerificationRoleIssue::UnknownExecutionClass(value) => format!(
            "{}: verification '{}': invalid execution_class '{}' (must be one of: {})",
            change_id, label, value, EXECUTION_CLASS_VALUES
        ),
        VerificationRoleIssue::UnknownCompletionRole(value) => format!(
            "{}: verification '{}': invalid completion_role '{}' (must be one of: change-blocking, operational-observation)",
            change_id, label, value
        ),
        VerificationRoleIssue::IncompleteRoleMetadata => format!(
            "{}: verification '{}': execution_class and completion_role must be declared together",
            change_id, label
        ),
        VerificationRoleIssue::PostIntegrationChangeBlocking(phase) => format!(
            "{}: verification '{}': completion_role: change-blocking requires phase: pre-integration (found '{}'); declare completion_role: {} instead",
            change_id,
            label,
            phase,
            VerificationCompletionRole::OperationalObservation.as_str()
        ),
        VerificationRoleIssue::NonLocalChangeBlocking(class) => format!(
            "{}: verification '{}': completion_role: change-blocking requires execution_class: repository-local (found '{}'); non-local outcomes must be completion_role: {} and must not block acceptance, archive, or merge",
            change_id,
            label,
            class.as_str(),
            VerificationCompletionRole::OperationalObservation.as_str()
        ),
        VerificationRoleIssue::LegacyRoleMetadataMissing => {
            let suggestion = match verification.phase.as_deref() {
                Some("post-integration") => {
                    "add execution_class: repository-automation (or another non-local class) and completion_role: operational-observation"
                }
                Some("pre-integration") => {
                    "add execution_class: repository-local and completion_role: change-blocking"
                }
                _ => "add execution_class and completion_role",
            };
            format!(
                "{}: verification '{}': legacy declaration without execution_class/completion_role; {} during migration",
                change_id, label, suggestion
            )
        }
    }
}

const EXECUTION_CLASS_VALUES: &str = "repository-local, repository-automation, deployed-service, physical-device, external-approval, credentialed-external";

/// Structured release-gate facts about one dependency target proposal.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ReleaseGateTarget {
    /// The target's own frontmatter could not be decoded.
    pub(super) metadata_unreadable: bool,
    /// `(verification id, human-readable non-local reason)` pairs.
    pub(super) non_local_blockers: Vec<(String, String)>,
}

/// Extract release-gate facts from already-parsed declarations.
pub(super) fn release_gate_target_facts(
    verifications: &[VerificationDeclaration],
) -> ReleaseGateTarget {
    ReleaseGateTarget {
        metadata_unreadable: false,
        non_local_blockers: verifications
            .iter()
            .filter(|declaration| declaration.is_non_local_change_blocker())
            .map(|declaration| {
                let reason = match (declaration.execution_class(), declaration.phase.as_deref()) {
                    (Some(class), _) if !class.is_repository_local() => {
                        format!("execution class '{}'", class.as_str())
                    }
                    (_, Some(phase)) if phase != "pre-integration" => {
                        format!("phase '{}'", phase)
                    }
                    _ => "missing repository-local pre-integration metadata".to_string(),
                };
                (verification_label(declaration), reason)
            })
            .collect(),
    }
}

fn verification_label(declaration: &VerificationDeclaration) -> String {
    declaration
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .unwrap_or("<missing id>")
        .to_string()
}

/// Classify one dependency edge from already-collected structured facts.
///
/// Pure decision logic: no filesystem, process, or network access, so the
/// release-gate rule can be verified in isolation from repository layout.
pub(super) fn classify_release_gate_edge(
    dependent: &str,
    dependent_is_observational: bool,
    dependent_in_flight: bool,
    target_id: &str,
    target: &ReleaseGateTarget,
) -> (Vec<String>, Vec<String>) {
    if target.metadata_unreadable {
        return (
            Vec::new(),
            vec![format!(
                "{}: proposal dependency '{}' has unreadable verification metadata; resolve the verification metadata errors reported on '{}' before this dependency edge can be classified",
                dependent, target_id, target_id
            )],
        );
    }

    // Release observations never block Conflux completion, so an observational
    // dependent cannot be stalled by the target's observational outcomes.
    if dependent_is_observational {
        return (Vec::new(), Vec::new());
    }

    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for (verification_id, reason) in &target.non_local_blockers {
        let message = format!(
            "{}: proposal dependency '{}' declares non-local change-blocking verification '{}' ({}); a repository-local implementation change must not depend on non-local release acceptance. Remedy: split '{}' into a repository-local implementation change plus a separate operational-observation release change, or remove this hard dependency",
            dependent, target_id, verification_id, reason, target_id
        );
        if dependent_in_flight {
            warnings.push(format!(
                "{} (in-flight operation is not interrupted; enforcement applies at the next dispatch eligibility decision)",
                message
            ));
        } else {
            errors.push(message);
        }
    }
    (errors, warnings)
}

/// Render reverse-dependency impact warnings for a target that declares a
/// non-local change blocker.
pub(super) fn reverse_impact_warnings(
    target_id: &str,
    blocker_ids: &[String],
    queued_dependents: &[String],
    in_flight_dependents: &[String],
) -> Vec<String> {
    if blocker_ids.is_empty() {
        return Vec::new();
    }

    let mut warnings = Vec::new();
    if !queued_dependents.is_empty() {
        warnings.push(format!(
            "{}: non-local change-blocking verification(s) [{}] affect queued dependents: {}. Those dependents become ineligible at their next dispatch eligibility decision",
            target_id,
            blocker_ids.join(", "),
            queued_dependents.join(", ")
        ));
    }
    if !in_flight_dependents.is_empty() {
        warnings.push(format!(
            "{}: in-flight dependents are not interrupted by this verification metadata change: {}",
            target_id,
            in_flight_dependents.join(", ")
        ));
    }
    warnings
}

/// Read a dependency target's structured release-gate facts.
///
/// Only valid structured declarations participate; prose is never consulted.
fn read_release_gate_target(root_dir: &Path, target_id: &str) -> Option<ReleaseGateTarget> {
    let target_proposal = root_dir
        .join("openspec/changes")
        .join(target_id)
        .join("proposal.md");
    let content = fs::read_to_string(&target_proposal).ok()?;
    let parsed = crate::openspec::parse_proposal_frontmatter_strict(&content, &target_proposal);
    if !parsed.diagnostics.is_empty() {
        return Some(ReleaseGateTarget {
            metadata_unreadable: true,
            non_local_blockers: Vec::new(),
        });
    }

    let verifications = parsed.metadata.map(|m| m.verifications).unwrap_or_default();
    Some(release_gate_target_facts(&verifications))
}

/// Whether a change is a correctly modeled release-observation change.
///
/// Such changes never block Conflux completion, so they may legitimately form
/// ordered chains among themselves and behind implementation changes.
pub(super) fn is_observational_release_change(verifications: &[VerificationDeclaration]) -> bool {
    let declared: Vec<_> = verifications
        .iter()
        .filter(|declaration| declaration.declares_role_metadata())
        .collect();
    !declared.is_empty()
        && declared.iter().all(|declaration| {
            declaration.completion_role()
                == Some(crate::openspec::VerificationCompletionRole::OperationalObservation)
        })
}

/// Reject hard dependency edges that turn non-local release acceptance into an
/// implementation-graph bottleneck, and report reverse-dependency impact.
///
/// Both directions are derived only from valid structured verification
/// declarations; the classification never reads natural-language content and
/// never contacts an external system.
fn validate_release_gate_dependencies(
    root_dir: &Path,
    change_id: &str,
    proposal_content: &str,
    proposal_file: &Path,
) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let parsed =
        crate::openspec::parse_proposal_frontmatter_strict(proposal_content, proposal_file);
    let own_verifications = parsed.metadata.map(|m| m.verifications).unwrap_or_default();
    let dependent_is_observational = is_observational_release_change(&own_verifications);

    let dependencies =
        crate::openspec::parse_proposal_metadata_from_file(proposal_file).dependencies;
    let archived_ids = crate::dependency_targets::collect_archived_change_ids(root_dir);
    let active_ids = super::dependency_status::collect_active_change_ids_from_root(root_dir);
    let in_flight_ids = super::dependency_status::collect_in_flight_change_ids_from_root(root_dir);
    // An in-flight dependent keeps running; the new classification only takes
    // effect at its next dispatch eligibility decision.
    let dependent_in_flight = in_flight_ids.contains(change_id);

    for dependency in &dependencies {
        if dependency == change_id || archived_ids.contains(dependency) {
            continue;
        }
        if !active_ids.contains(dependency) && !in_flight_ids.contains(dependency) {
            // Missing/rejected targets already have dedicated diagnostics.
            continue;
        }
        let Some(target) = read_release_gate_target(root_dir, dependency) else {
            continue;
        };

        let (edge_errors, edge_warnings) = classify_release_gate_edge(
            change_id,
            dependent_is_observational,
            dependent_in_flight,
            dependency,
            &target,
        );
        errors.extend(edge_errors);
        warnings.extend(edge_warnings);
    }

    // Reverse-dependency impact: this change is the target that gained a blocker.
    let own_blockers: Vec<String> = own_verifications
        .iter()
        .filter(|declaration| declaration.is_non_local_change_blocker())
        .map(verification_label)
        .collect();
    if !own_blockers.is_empty() {
        let (queued, in_flight) =
            collect_dependents(root_dir, change_id, &active_ids, &in_flight_ids);
        warnings.extend(reverse_impact_warnings(
            change_id,
            &own_blockers,
            &queued,
            &in_flight,
        ));
    }

    (errors, warnings)
}

/// Collect active dependents of `target_id`, split into queued and in-flight.
fn collect_dependents(
    root_dir: &Path,
    target_id: &str,
    active_ids: &HashSet<String>,
    in_flight_ids: &HashSet<String>,
) -> (Vec<String>, Vec<String>) {
    let mut queued = Vec::new();
    let mut in_flight = Vec::new();

    for candidate in active_ids.iter().chain(in_flight_ids.iter()) {
        if candidate == target_id || queued.contains(candidate) || in_flight.contains(candidate) {
            continue;
        }
        let proposal = root_dir
            .join("openspec/changes")
            .join(candidate)
            .join("proposal.md");
        if !proposal.exists() {
            continue;
        }
        if !crate::openspec::parse_proposal_metadata_from_file(&proposal)
            .dependencies
            .iter()
            .any(|dependency| dependency == target_id)
        {
            continue;
        }
        if in_flight_ids.contains(candidate) {
            in_flight.push(candidate.clone());
        } else {
            queued.push(candidate.clone());
        }
    }

    queued.sort();
    in_flight.sort();
    (queued, in_flight)
}

fn validate_automation_path(
    root_dir: &Path,
    change_id: &str,
    id: &str,
    automation: &str,
) -> Vec<String> {
    let path = Path::new(automation);
    if automation.trim().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return vec![format!(
            "{}: verification '{}': unsafe automation path '{}'",
            change_id, id, automation
        )];
    }
    let candidate = root_dir.join(path);
    let canonical_root = match root_dir.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return vec![format!(
                "{}: verification '{}': cannot resolve repository root",
                change_id, id
            )]
        }
    };
    let canonical_candidate = match candidate.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return vec![format!(
                "{}: verification '{}': automation path '{}' does not exist",
                change_id, id, automation
            )]
        }
    };
    if !canonical_candidate.starts_with(&canonical_root) {
        return vec![format!(
            "{}: verification '{}': automation path '{}' escapes repository",
            change_id, id, automation
        )];
    }
    if !canonical_candidate.is_file() {
        return vec![format!(
            "{}: verification '{}': automation path '{}' is not a regular file",
            change_id, id, automation
        )];
    }
    match Command::new("git")
        .args(["ls-files", "--error-unmatch", "--", automation])
        .current_dir(root_dir)
        .output()
    {
        Ok(output) if output.status.success() => Vec::new(),
        Ok(_) => vec![format!(
            "{}: verification '{}': automation path '{}' is not tracked by git",
            change_id, id, automation
        )],
        Err(error) => vec![format!(
            "{}: verification '{}': cannot verify tracked automation path '{}': {}",
            change_id, id, automation, error
        )],
    }
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

/// Deterministic task-format validation for a `tasks.md` body.
///
/// This is the format-only subset of [`validate_tasks_content`]: it reports
/// active-section bullets that look like tasks but lack checkboxes, checkboxes
/// inside narrative non-task sections, and self-referential final-validation
/// checkboxes. Evidence/verification-ownership policy is intentionally excluded
/// so the runtime pre-accept gate stays independent of proposal metadata and
/// evidence mode.
pub(crate) fn validate_task_format(content: &str, change_id: &str) -> Vec<String> {
    let (errors, _) = validate_tasks_content(content, change_id, false, "off", None, None);
    errors
}

/// Extract a `verification-id: <id>` reference from a task or continuation line.
pub(super) fn extract_verification_id_reference(text: &str) -> Option<String> {
    static VERIFICATION_ID_RE: OnceLock<Regex> = OnceLock::new();
    let re = VERIFICATION_ID_RE
        .get_or_init(|| Regex::new(r"(?i)verification-id\s*:\s*([A-Za-z0-9._-]+)").unwrap());
    re.captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Structured verification linkage requirements for one proposal's task list.
///
/// The requirement only activates once the proposal has migrated to the role
/// model; legacy declarations stay compatible and are handled by migration
/// warnings on the proposal itself.
struct VerificationLinkage {
    declared_ids: HashSet<String>,
    change_blocking_ids: HashSet<String>,
}

impl VerificationLinkage {
    fn from_proposal(proposal_content: Option<&str>) -> Option<Self> {
        let parsed = crate::openspec::parse_proposal_frontmatter_strict(
            proposal_content?,
            Path::new("proposal.md"),
        );
        let verifications = parsed.metadata.map(|m| m.verifications).unwrap_or_default();
        if !verifications
            .iter()
            .any(VerificationDeclaration::declares_role_metadata)
        {
            return None;
        }
        Some(Self {
            declared_ids: verifications
                .iter()
                .filter_map(|declaration| declaration.id.clone())
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect(),
            change_blocking_ids: crate::openspec::change_blocking_verification_ids(&verifications)
                .into_iter()
                .collect(),
        })
    }

    fn evaluate(&self, change_id: &str, line_num: usize, reference: &str) -> Option<String> {
        if self.change_blocking_ids.contains(reference) {
            return None;
        }
        if !self.declared_ids.contains(reference) {
            return Some(format!(
                "{}: tasks.md:{}: verification-id '{}' is not declared in proposal.md verifications",
                change_id, line_num, reference
            ));
        }
        Some(format!(
            "{}: tasks.md:{}: verification-id '{}' is not a change-blocking verification; move the non-local outcome to Future Work or a separate release-observation change",
            change_id, line_num, reference
        ))
    }

    fn missing_reference_message(change_id: &str, line_num: usize) -> String {
        format!(
            "{}: tasks.md:{}: active implementation checkbox must reference a change-blocking verification via 'verification-id: <id>'",
            change_id, line_num
        )
    }
}

pub(super) fn validate_tasks_content(
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
    static VERIFICATION_CONTINUATION_RE: OnceLock<Regex> = OnceLock::new();

    let checkbox_re = CHECKBOX_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[[ x]\]").unwrap());
    let checkbox_detail_re =
        CHECKBOX_DETAIL_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s*\[([ x])\]\s+(.*)$").unwrap());
    let bare_task_re = BARE_TASK_RE.get_or_init(|| Regex::new(r"^\s*[-*]\s+[^\[]").unwrap());
    let verification_continuation_re = VERIFICATION_CONTINUATION_RE
        .get_or_init(|| Regex::new(r"^\s{2,}(?i:verification:)\s*(.+)$").unwrap());

    let mut section = TaskSectionKind::Active;

    let is_behavior_change = matches!(change_type, Some("implementation" | "hybrid"));

    let mut last_checkbox_line_num: Option<usize> = None;
    let mut pending_behavior_task_without_verification: Option<usize> = None;
    let mut fences = crate::task_parser::FenceTracker::default();

    // Linkage is enforced only for behavior-changing proposals that already
    // declare structured verification roles.
    let linkage = (strict && is_behavior_change)
        .then(|| VerificationLinkage::from_proposal(proposal_content))
        .flatten();
    let mut pending_verification_id_line: Option<usize> = None;

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;

        // Fenced literals (e.g. recovered acceptance notes) are untrusted historical
        // content, never headings, tasks, or verification declarations.
        if fences.observe(line) {
            continue;
        }

        if line.starts_with("##") {
            // Runtime-owned follow-up subsections (e.g. `### External blockers`)
            // stay excluded until the next top-level section.
            if line.starts_with("###") && section == TaskSectionKind::RuntimeAcceptanceFollowUp {
                continue;
            }
            section = classify_task_section(line);
            continue;
        }

        // Runtime-owned acceptance findings are not implementation tasks and have no
        // implementation verification contract. The runtime removes them after PASS.
        if section == TaskSectionKind::RuntimeAcceptanceFollowUp {
            continue;
        }

        let in_excluded = section == TaskSectionKind::Narrative;

        // Check for checkboxes in narrative (non-task) sections
        if in_excluded && checkbox_re.is_match(line) {
            errors.push(format!(
                "{}: tasks.md:{}: Checkbox found in excluded section (should be removed)",
                change_id, line_num
            ));
            continue;
        }

        if let Some(caps) = checkbox_detail_re.captures(line) {
            // A new checkbox ends the previous checkbox's continuation window.
            if let Some(unresolved) = pending_verification_id_line.take() {
                errors.push(VerificationLinkage::missing_reference_message(
                    change_id, unresolved,
                ));
            }
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
                    continue;
                }

                if let Some(linkage) = linkage.as_ref() {
                    match extract_verification_id_reference(task_text) {
                        Some(reference) => {
                            errors.extend(linkage.evaluate(change_id, line_num, &reference))
                        }
                        // The reference may still arrive on a continuation line.
                        None => pending_verification_id_line = Some(line_num),
                    }
                }

                if strict && evidence_mode != "off" && is_behavior_change {
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
                    if let (Some(linkage), Some(unresolved)) =
                        (linkage.as_ref(), pending_verification_id_line)
                    {
                        if unresolved == prev_line_num {
                            if let Some(reference) = extract_verification_id_reference(vtext) {
                                errors.extend(linkage.evaluate(
                                    change_id,
                                    prev_line_num,
                                    &reference,
                                ));
                                pending_verification_id_line = None;
                            }
                        }
                    }
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

    if let Some(unresolved) = pending_verification_id_line {
        errors.push(VerificationLinkage::missing_reference_message(
            change_id, unresolved,
        ));
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
