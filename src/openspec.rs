use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use tracing::{debug, info, warn};

use crate::error::{OrchestratorError, Result};
use crate::task_parser;
use crate::tui::log_deduplicator;

/// Represents allowed proposal priority values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalPriority {
    High,
    Medium,
    Low,
}

/// Machine-readable metadata extracted from `proposal.md` frontmatter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalMetadata {
    pub change_type: Option<String>,
    pub priority: Option<ProposalPriority>,
    pub dependencies: Vec<String>,
    pub references: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifications: Vec<VerificationDeclaration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationDeclaration {
    pub id: Option<String>,
    pub requirement: Option<String>,
    pub phase: Option<String>,
    pub owner: Option<String>,
    pub trigger: Option<String>,
    pub automation: Option<String>,
    pub evidence: Option<String>,
    pub rerun: Option<String>,
    pub prerequisites: Option<Vec<String>>,
    /// Where the verification actually executes. Optional during migration.
    pub execution_class: Option<String>,
    /// Whether the verification may block Conflux completion. Optional during migration.
    pub completion_role: Option<String>,
}

/// Execution environment for a structured verification declaration.
///
/// Only `RepositoryLocal` evidence can be produced offline from the repository
/// before integration, so only that class may hold a change-blocking role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationExecutionClass {
    RepositoryLocal,
    RepositoryAutomation,
    DeployedService,
    PhysicalDevice,
    ExternalApproval,
    CredentialedExternal,
}

impl VerificationExecutionClass {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "repository-local" => Some(Self::RepositoryLocal),
            "repository-automation" => Some(Self::RepositoryAutomation),
            "deployed-service" => Some(Self::DeployedService),
            "physical-device" => Some(Self::PhysicalDevice),
            "external-approval" => Some(Self::ExternalApproval),
            "credentialed-external" => Some(Self::CredentialedExternal),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::RepositoryLocal => "repository-local",
            Self::RepositoryAutomation => "repository-automation",
            Self::DeployedService => "deployed-service",
            Self::PhysicalDevice => "physical-device",
            Self::ExternalApproval => "external-approval",
            Self::CredentialedExternal => "credentialed-external",
        }
    }

    /// Whether evidence for this class is produced from the repository itself.
    pub fn is_repository_local(self) -> bool {
        matches!(self, Self::RepositoryLocal)
    }
}

/// Whether a structured verification may keep a Conflux change incomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationCompletionRole {
    ChangeBlocking,
    OperationalObservation,
}

impl VerificationCompletionRole {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "change-blocking" => Some(Self::ChangeBlocking),
            "operational-observation" => Some(Self::OperationalObservation),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ChangeBlocking => "change-blocking",
            Self::OperationalObservation => "operational-observation",
        }
    }
}

/// Structural problem found in a single verification declaration's role metadata.
///
/// Message rendering belongs to the validator so the owning change ID stays with
/// the caller; this enum only carries the deterministic classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationRoleIssue {
    UnknownExecutionClass(String),
    UnknownCompletionRole(String),
    /// Exactly one of the two role fields is present.
    IncompleteRoleMetadata,
    /// `change-blocking` declared on a non-`pre-integration` phase.
    PostIntegrationChangeBlocking(String),
    /// `change-blocking` declared on a non-`repository-local` execution class.
    NonLocalChangeBlocking(VerificationExecutionClass),
    /// Neither role field is declared yet (migration warning, not an error).
    LegacyRoleMetadataMissing,
}

impl VerificationRoleIssue {
    /// Migration gaps are advisory; every other issue is a hard failure.
    pub fn is_warning(&self) -> bool {
        matches!(self, Self::LegacyRoleMetadataMissing)
    }
}

impl VerificationDeclaration {
    pub fn execution_class(&self) -> Option<VerificationExecutionClass> {
        self.execution_class
            .as_deref()
            .and_then(VerificationExecutionClass::parse)
    }

    pub fn completion_role(&self) -> Option<VerificationCompletionRole> {
        self.completion_role
            .as_deref()
            .and_then(VerificationCompletionRole::parse)
    }

    /// Whether this declaration participates in the new role model at all.
    pub fn declares_role_metadata(&self) -> bool {
        non_empty(self.execution_class.as_deref()).is_some()
            || non_empty(self.completion_role.as_deref()).is_some()
    }

    /// Whether this declaration is a valid repository-local completion gate.
    pub fn is_change_blocking(&self) -> bool {
        self.completion_role() == Some(VerificationCompletionRole::ChangeBlocking)
    }

    /// Whether a change-blocking role is declared on non-local evidence.
    ///
    /// This is the hazardous composition that turns operational release
    /// acceptance into a graph-wide implementation stop.
    pub fn is_non_local_change_blocker(&self) -> bool {
        self.is_change_blocking()
            && (self.phase.as_deref() != Some("pre-integration")
                || !self
                    .execution_class()
                    .is_some_and(VerificationExecutionClass::is_repository_local))
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// Evaluate role metadata for one declaration without touching the filesystem.
pub fn evaluate_verification_role(
    declaration: &VerificationDeclaration,
) -> Vec<VerificationRoleIssue> {
    let mut issues = Vec::new();
    let execution_class = non_empty(declaration.execution_class.as_deref());
    let completion_role = non_empty(declaration.completion_role.as_deref());

    match (execution_class, completion_role) {
        (None, None) => return vec![VerificationRoleIssue::LegacyRoleMetadataMissing],
        (Some(_), None) | (None, Some(_)) => {
            issues.push(VerificationRoleIssue::IncompleteRoleMetadata)
        }
        (Some(_), Some(_)) => {}
    }

    let parsed_class =
        execution_class.map(|value| (value, VerificationExecutionClass::parse(value)));
    if let Some((raw, None)) = parsed_class {
        issues.push(VerificationRoleIssue::UnknownExecutionClass(
            raw.to_string(),
        ));
    }
    let parsed_role =
        completion_role.map(|value| (value, VerificationCompletionRole::parse(value)));
    if let Some((raw, None)) = parsed_role {
        issues.push(VerificationRoleIssue::UnknownCompletionRole(
            raw.to_string(),
        ));
    }

    if matches!(
        parsed_role,
        Some((_, Some(VerificationCompletionRole::ChangeBlocking)))
    ) {
        match declaration.phase.as_deref() {
            Some("pre-integration") => {}
            Some(phase) => issues.push(VerificationRoleIssue::PostIntegrationChangeBlocking(
                phase.to_string(),
            )),
            None => issues.push(VerificationRoleIssue::PostIntegrationChangeBlocking(
                "<missing phase>".to_string(),
            )),
        }
        if let Some((_, Some(class))) = parsed_class {
            if !class.is_repository_local() {
                issues.push(VerificationRoleIssue::NonLocalChangeBlocking(class));
            }
        }
    }

    issues
}

/// Collect declaration IDs that are valid repository-local completion gates.
pub fn change_blocking_verification_ids(declarations: &[VerificationDeclaration]) -> Vec<String> {
    declarations
        .iter()
        .filter(|declaration| {
            declaration.is_change_blocking() && !declaration.is_non_local_change_blocker()
        })
        .filter_map(|declaration| non_empty(declaration.id.as_deref()).map(str::to_string))
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProposalFrontmatterParse {
    pub metadata: Option<ProposalFrontmatterMetadata>,
    pub diagnostics: Vec<String>,
}

fn proposal_priority_label(priority: ProposalPriority) -> String {
    match priority {
        ProposalPriority::High => "high",
        ProposalPriority::Medium => "medium",
        ProposalPriority::Low => "low",
    }
    .to_string()
}

fn warnings_to_strings(warnings: &[ProposalFrontmatterWarning]) -> Vec<String> {
    warnings
        .iter()
        .map(|warning| warning.message.clone())
        .collect()
}

fn frontmatter_metadata_to_metadata(metadata: ProposalFrontmatterMetadata) -> ProposalMetadata {
    ProposalMetadata {
        change_type: metadata.change_type,
        priority: metadata
            .priority
            .as_deref()
            .and_then(|priority| match priority {
                "high" => Some(ProposalPriority::High),
                "medium" => Some(ProposalPriority::Medium),
                "low" => Some(ProposalPriority::Low),
                _ => None,
            }),
        dependencies: metadata.dependencies.unwrap_or_default(),
        references: metadata.references,
        verifications: metadata.verifications,
        warnings: warnings_to_strings(&metadata.warnings),
    }
}

fn read_proposal_from_path(path: &Path) -> ProposalReadResult {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            debug!(proposal = %path.display(), error = %error, "Failed to read proposal metadata source");
            return ProposalReadResult {
                metadata: None,
                body_dependencies: Vec::new(),
            };
        }
    };

    let (frontmatter, body) = split_frontmatter(&content);
    let metadata =
        frontmatter.and_then(|frontmatter| parse_frontmatter_metadata(frontmatter, path));
    let body_dependencies = parse_body_dependencies(body, path);

    ProposalReadResult {
        metadata,
        body_dependencies,
    }
}

pub fn read_proposal(change_id: &str) -> ProposalReadResult {
    let proposal_path = Path::new("openspec/changes")
        .join(change_id)
        .join("proposal.md");
    read_proposal_from_path(&proposal_path)
}

/// Parse proposal metadata from a `proposal.md` file.
///
/// Supports optional YAML frontmatter. Dependencies in frontmatter take precedence,
/// but the legacy `## Dependencies` section remains supported as a fallback.
pub fn parse_proposal_metadata_from_file(path: &Path) -> ProposalMetadata {
    let proposal = read_proposal_from_path(path);

    if let Some(metadata) = proposal.metadata {
        let mut parsed = frontmatter_metadata_to_metadata(metadata);
        if parsed.dependencies.is_empty() {
            parsed.dependencies = proposal.body_dependencies;
        }
        parsed
    } else {
        ProposalMetadata {
            change_type: None,
            priority: None,
            dependencies: proposal.body_dependencies,
            references: Vec::new(),
            verifications: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// Read proposal dependencies for scheduler dispatch gates.
///
/// Unlike the display-oriented metadata parser, this fails when proposal metadata
/// cannot be read or decoded. Dispatch must not treat unavailable dependency
/// declarations as an empty dependency list.
pub fn parse_proposal_dependencies_strict_from_file(path: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(path)?;
    let (frontmatter, body) = split_frontmatter(&content);
    if content.starts_with("---\n") && frontmatter.is_none() {
        return Err(OrchestratorError::Parse(format!(
            "Failed to find closing frontmatter delimiter in {}",
            path.display()
        )));
    }

    let mut dependencies = match frontmatter {
        Some(frontmatter) => {
            let parsed = parse_frontmatter_metadata_strict(frontmatter, path);
            if !parsed.diagnostics.is_empty() {
                return Err(OrchestratorError::Parse(parsed.diagnostics.join("; ")));
            }
            parsed
                .metadata
                .and_then(|metadata| metadata.dependencies)
                .unwrap_or_default()
        }
        None => Vec::new(),
    };
    if dependencies.is_empty() {
        dependencies = parse_body_dependencies(body, path);
    }
    Ok(dependencies)
}

#[cfg_attr(not(test), allow(dead_code))]
fn parse_proposal_metadata(content: &str, path: &Path) -> ProposalMetadata {
    let (frontmatter, body) = split_frontmatter(content);
    let mut metadata = frontmatter
        .and_then(|frontmatter| parse_frontmatter_metadata(frontmatter, path))
        .map(frontmatter_metadata_to_metadata)
        .unwrap_or_default();

    if metadata.dependencies.is_empty() {
        metadata.dependencies = parse_body_dependencies(body, path);
    }

    metadata
}

fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    if !content.starts_with("---\n") {
        return (None, content);
    }

    let remainder = &content[4..];
    if let Some(end) = remainder.find("\n---\n") {
        let frontmatter = &remainder[..end];
        let body = &remainder[end + 5..];
        return (Some(frontmatter), body);
    }

    (None, content)
}

fn parse_frontmatter_metadata(
    frontmatter: &str,
    path: &Path,
) -> Option<ProposalFrontmatterMetadata> {
    let parsed = parse_frontmatter_metadata_strict(frontmatter, path);
    parsed.metadata
}

pub fn parse_proposal_frontmatter_strict(content: &str, path: &Path) -> ProposalFrontmatterParse {
    split_frontmatter(content)
        .0
        .map(|frontmatter| parse_frontmatter_metadata_strict(frontmatter, path))
        .unwrap_or_default()
}

pub fn parse_frontmatter_metadata_strict(
    frontmatter: &str,
    path: &Path,
) -> ProposalFrontmatterParse {
    let value: Value = match serde_yaml::from_str(frontmatter) {
        Ok(value) => value,
        Err(error) => {
            let diagnostic = format!("Failed to parse proposal frontmatter YAML: {error}");
            warn!(proposal = %path.display(), error = %error, "Failed to parse proposal frontmatter YAML");
            return ProposalFrontmatterParse {
                metadata: None,
                diagnostics: vec![diagnostic],
            };
        }
    };

    let mut warnings = Vec::new();
    let mut diagnostics = Vec::new();
    if let Value::Mapping(mapping) = &value {
        let known_keys: HashSet<&str> = [
            "change_type",
            "priority",
            "dependencies",
            "references",
            "verifications",
        ]
        .into_iter()
        .collect();

        for key in mapping.keys() {
            if let Some(key) = key.as_str() {
                if !known_keys.contains(key) {
                    let warning = format!("Unknown proposal frontmatter key: {}", key);
                    warn!(proposal = %path.display(), key = key, warning = %warning, "Unknown proposal frontmatter key detected");
                    warnings.push(ProposalFrontmatterWarning {
                        key: key.to_string(),
                        message: warning,
                    });
                }
            }
        }

        if let Some(Value::Sequence(verifications)) =
            mapping.get(Value::String("verifications".to_string()))
        {
            let known_verification_keys: HashSet<&str> = [
                "id",
                "requirement",
                "phase",
                "owner",
                "trigger",
                "automation",
                "evidence",
                "rerun",
                "prerequisites",
                "execution_class",
                "completion_role",
            ]
            .into_iter()
            .collect();
            for (index, verification) in verifications.iter().enumerate() {
                if let Value::Mapping(verification) = verification {
                    for key in verification.keys() {
                        if let Some(key) = key.as_str() {
                            if !known_verification_keys.contains(key) {
                                diagnostics.push(format!(
                                    "Unknown verification declaration key at index {}: {}",
                                    index, key
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    let raw: RawProposalFrontmatter = match serde_yaml::from_value(value) {
        Ok(raw) => raw,
        Err(error) => {
            let diagnostic = format!("Failed to decode proposal frontmatter fields: {error}");
            warn!(proposal = %path.display(), error = %error, "Failed to decode proposal frontmatter fields");
            return ProposalFrontmatterParse {
                metadata: None,
                diagnostics: vec![diagnostic],
            };
        }
    };

    let mut dependencies = Vec::new();
    if let Some(items) = raw.dependencies {
        for item in items {
            let dep_id = extract_dependency_id(item.trim());
            if !dep_id.is_empty() {
                dependencies.push(dep_id);
            }
        }
    }

    let metadata = ProposalFrontmatterMetadata {
        change_type: raw.change_type,
        priority: raw.priority.map(proposal_priority_label),
        dependencies: (!dependencies.is_empty()).then_some(dependencies),
        references: raw.references.unwrap_or_default(),
        verifications: raw.verifications.unwrap_or_default(),
        warnings,
    };

    ProposalFrontmatterParse {
        metadata: (!metadata.is_empty()).then_some(metadata),
        diagnostics,
    }
}

/// Represents a change from openspec list
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub id: String,
    pub completed_tasks: u32,
    pub total_tasks: u32,
    #[allow(dead_code)]
    pub last_modified: String,
    /// Dependencies on other changes (parsed from proposal.md)
    pub dependencies: Vec<String>,
    /// Optional proposal metadata from `proposal.md` frontmatter.
    pub metadata: ProposalMetadata,
}

impl Change {
    /// Calculate progress percentage
    pub fn progress_percent(&self) -> f32 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        (self.completed_tasks as f32 / self.total_tasks as f32) * 100.0
    }

    /// Check if all tasks are completed
    pub fn is_complete(&self) -> bool {
        self.completed_tasks == self.total_tasks && self.total_tasks > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalFrontmatterWarning {
    pub key: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalFrontmatterMetadata {
    pub change_type: Option<String>,
    pub priority: Option<String>,
    pub dependencies: Option<Vec<String>>,
    pub references: Vec<String>,
    pub verifications: Vec<VerificationDeclaration>,
    pub warnings: Vec<ProposalFrontmatterWarning>,
}

impl ProposalFrontmatterMetadata {
    fn is_empty(&self) -> bool {
        self.change_type.is_none()
            && self.priority.is_none()
            && self.dependencies.is_none()
            && self.references.is_empty()
            && self.verifications.is_empty()
            && self.warnings.is_empty()
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawProposalFrontmatter {
    change_type: Option<String>,
    priority: Option<ProposalPriority>,
    dependencies: Option<Vec<String>>,
    references: Option<Vec<String>>,
    verifications: Option<Vec<VerificationDeclaration>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalReadResult {
    pub metadata: Option<ProposalFrontmatterMetadata>,
    pub body_dependencies: Vec<String>,
}

impl ProposalReadResult {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn dependencies_for_analysis(&self) -> Vec<String> {
        self.metadata
            .as_ref()
            .and_then(|metadata| metadata.dependencies.clone())
            .unwrap_or_else(|| self.body_dependencies.clone())
    }
}

/// Parse dependencies from the legacy body `## Dependencies` section.
///
/// Supports formats like:
/// - `- feature-base`
/// - `- [feature-base](../feature-base/proposal.md)`
/// - `- feature-base: description`
fn parse_body_dependencies(content: &str, path: &Path) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut in_deps_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for Dependencies section header
        if trimmed.starts_with("## Dependencies") {
            in_deps_section = true;
            continue;
        }

        // Exit section on next header
        if in_deps_section && trimmed.starts_with("## ") {
            break;
        }

        // Parse bullet points in Dependencies section
        if in_deps_section && trimmed.starts_with("- ") {
            let item = trimmed.trim_start_matches("- ").trim();

            // Skip "None" or empty items
            if item.is_empty() || item.eq_ignore_ascii_case("none") {
                continue;
            }

            // Extract change ID from various formats
            let dep_id = extract_dependency_id(item);
            if !dep_id.is_empty() {
                dependencies.push(dep_id);
            }
        }
    }

    if !dependencies.is_empty() {
        info!(proposal = %path.display(), dependencies = ?dependencies, "Parsed proposal dependencies from body section");
    }

    dependencies
}

fn parse_dependencies_from_base(base_path: &Path, change_id: &str) -> ProposalMetadata {
    let proposal_path = base_path
        .join("openspec/changes")
        .join(change_id)
        .join("proposal.md");

    let mut metadata = parse_proposal_metadata_from_file(&proposal_path);
    metadata
        .dependencies
        .retain(|dependency| dependency != change_id);
    metadata
}

/// Extract a dependency ID from a bullet point item.
///
/// Handles formats like:
/// - `feature-base` -> `feature-base`
/// - `[feature-base](../feature-base/proposal.md)` -> `feature-base`
/// - `feature-base: some description` -> `feature-base`
/// - `feature-base (optional)` -> `feature-base`
fn extract_dependency_id(item: &str) -> String {
    // Handle markdown link format: [id](path)
    if item.starts_with('[') {
        if let Some(end) = item.find(']') {
            return item[1..end].trim().to_string();
        }
    }

    // Handle inline code format: `id`
    if let Some(stripped) = item.strip_prefix('`') {
        if let Some(end) = stripped.find('`') {
            return stripped[..end].trim().to_string();
        }
    }

    // Handle plain text with optional suffix (colon, parenthesis)
    // Note: Don't split on '-' as it's common in change IDs like "feature-base"
    item.split(&[':', '('][..])
        .next()
        .unwrap_or(item)
        .trim()
        .to_string()
}

/// List changes by directly reading the openspec/changes directory.
///
/// This is the native implementation that avoids calling the external
/// `openspec list --json` command. It reads the directory structure and
/// parses tasks.md files to get accurate task progress.
pub fn list_changes_native() -> Result<Vec<Change>> {
    list_changes_native_from(Path::new("."))
}

pub fn list_changes_native_from(base_path: &Path) -> Result<Vec<Change>> {
    let changes_dir = base_path.join("openspec/changes");

    if !changes_dir.exists() {
        debug!("Changes directory does not exist: {:?}", changes_dir);
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(changes_dir).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to read changes directory: {}", e))
    })?;

    let mut changes = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| {
            OrchestratorError::ConfigLoad(format!("Failed to read directory entry: {}", e))
        })?;

        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        // Skip special directories like 'archive'
        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if dir_name == "archive" || dir_name.starts_with('.') {
            continue;
        }

        // Skip changes without proposal.md
        let proposal_path = path.join("proposal.md");
        if !proposal_path.exists() {
            debug!("Skipping change '{}' - no proposal.md found", dir_name);
            continue;
        }

        // Rejected marker committed on base means this change is terminal and must
        // not be re-queued in subsequent runs.
        if path.join("REJECTED.md").exists() {
            debug!(
                "Skipping change '{}' from active list - REJECTED.md marker exists",
                dir_name
            );
            continue;
        }

        // Parse tasks.md for this change
        let tasks_path = path.join("tasks.md");
        let (completed_tasks, total_tasks) =
            match task_parser::parse_file(&tasks_path, Some(dir_name)) {
                Ok(progress) => (progress.completed, progress.total),
                Err(_) => {
                    // If tasks.md doesn't exist or can't be parsed, use 0/0
                    debug!("Could not parse tasks for change '{}', using 0/0", dir_name);
                    (0, 0)
                }
            };

        // Parse dependencies and metadata from proposal.md
        let metadata = parse_dependencies_from_base(base_path, dir_name);
        let dependencies = metadata.dependencies.clone();

        changes.push(Change {
            id: dir_name.to_string(),
            completed_tasks,
            total_tasks,
            last_modified: String::new(), // Not used in native implementation
            dependencies,
            metadata,
        });
    }

    // Sort by id for consistent ordering
    changes.sort_by(|a, b| a.id.cmp(&b.id));

    if log_deduplicator::should_log_change_count(changes.len()) {
        debug!("Found {} changes via native parsing", changes.len());
    }
    Ok(changes)
}

/// List change directories that are marked as rejected (`REJECTED.md`) for
/// read-only UI surfaces.
///
/// Unlike [`list_changes_native`], this function intentionally returns only
/// marker-bearing rows and is not used for execution candidate discovery.
pub fn list_rejected_changes_native_from(base_path: &Path) -> Result<Vec<Change>> {
    let changes_dir = base_path.join("openspec/changes");

    if !changes_dir.exists() {
        debug!("Changes directory does not exist: {:?}", changes_dir);
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(changes_dir).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to read changes directory: {}", e))
    })?;

    let mut changes = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| {
            OrchestratorError::ConfigLoad(format!("Failed to read directory entry: {}", e))
        })?;

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if dir_name == "archive" || dir_name.starts_with('.') {
            continue;
        }

        if !path.join("proposal.md").exists() || !path.join("REJECTED.md").exists() {
            continue;
        }

        let tasks_path = path.join("tasks.md");
        let (completed_tasks, total_tasks) =
            match task_parser::parse_file(&tasks_path, Some(dir_name)) {
                Ok(progress) => (progress.completed, progress.total),
                Err(_) => {
                    debug!(
                        "Could not parse tasks for rejected change '{}', using 0/0",
                        dir_name
                    );
                    (0, 0)
                }
            };

        let metadata = parse_dependencies_from_base(base_path, dir_name);
        let dependencies = metadata.dependencies.clone();

        changes.push(Change {
            id: dir_name.to_string(),
            completed_tasks,
            total_tasks,
            last_modified: String::new(),
            dependencies,
            metadata,
        });
    }

    changes.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoggingConfig;
    use crate::tui::log_deduplicator;
    use std::env;
    use std::fs;
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    static LOG_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn test_change_progress() {
        let change = Change {
            id: "test".to_string(),
            completed_tasks: 0,
            total_tasks: 0,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        };
        assert_eq!(change.progress_percent(), 0.0);
        assert!(!change.is_complete());
    }

    #[test]
    fn test_change_is_complete() {
        let change = Change {
            id: "test".to_string(),
            completed_tasks: 5,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        };
        assert_eq!(change.progress_percent(), 100.0);
        assert!(change.is_complete());
    }

    // ====================
    // list_changes_native tests
    // ====================

    // Note: These tests run in the actual project directory which has openspec/changes
    // The function relies on relative paths, so these tests validate real behavior

    #[test]
    fn test_list_changes_native_returns_ok() {
        // Test that the function returns Ok when run in a valid openspec project
        // This test verifies the basic functionality works in the real project structure
        let result = list_changes_native();
        // The function should always return Ok (empty vec if dir doesn't exist)
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_changes_native_excludes_archive() {
        // The result should not include "archive" as a change ID
        let result = list_changes_native().unwrap();
        assert!(
            !result.iter().any(|c| c.id == "archive"),
            "archive directory should be excluded"
        );
    }

    #[test]
    fn test_list_changes_native_excludes_hidden() {
        // The result should not include any hidden directories (starting with .)
        let result = list_changes_native().unwrap();
        assert!(
            !result.iter().any(|c| c.id.starts_with('.')),
            "hidden directories should be excluded"
        );
    }

    #[test]
    fn test_list_changes_native_sorted_by_id() {
        // The result should be sorted by change ID
        let result = list_changes_native().unwrap();
        if result.len() > 1 {
            let mut sorted = result.clone();
            sorted.sort_by(|a, b| a.id.cmp(&b.id));
            assert_eq!(result, sorted, "changes should be sorted by ID");
        }
    }

    #[test]
    fn test_list_changes_native_parses_task_counts() {
        // If there are any changes, verify they have valid task counts
        let result = list_changes_native().unwrap();
        for change in &result {
            // completed_tasks should never exceed total_tasks
            assert!(
                change.completed_tasks <= change.total_tasks,
                "completed_tasks ({}) should not exceed total_tasks ({}) for change '{}'",
                change.completed_tasks,
                change.total_tasks,
                change.id
            );
        }
    }

    #[test]
    fn test_list_changes_native_integration() {
        // Integration test: verify the function works with the actual project structure
        // This test runs in the project root where openspec/changes exists
        let result = list_changes_native();
        assert!(result.is_ok(), "list_changes_native should succeed");

        let changes = result.unwrap();
        // Verify each change has a non-empty ID
        for change in &changes {
            assert!(!change.id.is_empty(), "change ID should not be empty");
        }
    }

    #[test]
    fn test_list_changes_native_excludes_rejected_marker_changes() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("change-rejected");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(change_dir.join("proposal.md"), "# proposal").unwrap();
        fs::write(change_dir.join("tasks.md"), "- [ ] pending task").unwrap();
        fs::write(
            change_dir.join("REJECTED.md"),
            "# REJECTED\n\n- reason: blocked\n",
        )
        .unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let result = list_changes_native().unwrap();

        env::set_current_dir(original_dir).unwrap();

        assert!(
            result.iter().all(|change| change.id != "change-rejected"),
            "changes with REJECTED.md marker must be excluded from active change list"
        );
    }

    #[test]
    fn test_list_changes_and_rejected_native_from_use_explicit_base_when_cwd_differs() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let repo_dir = TempDir::new().unwrap();
        let other_dir = TempDir::new().unwrap();
        let base = repo_dir.path().join("openspec").join("changes");

        let active_dir = base.join("change-active");
        fs::create_dir_all(&active_dir).unwrap();
        fs::write(active_dir.join("proposal.md"), "# proposal").unwrap();
        fs::write(active_dir.join("tasks.md"), "- [ ] active task").unwrap();

        let rejected_dir = base.join("change-rejected");
        fs::create_dir_all(&rejected_dir).unwrap();
        fs::write(rejected_dir.join("proposal.md"), "# proposal").unwrap();
        fs::write(rejected_dir.join("tasks.md"), "- [x] rejected task").unwrap();
        fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED").unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(other_dir.path()).unwrap();

        let active = list_changes_native_from(repo_dir.path()).unwrap();
        let rejected = list_rejected_changes_native_from(repo_dir.path()).unwrap();

        env::set_current_dir(original_dir).unwrap();

        assert_eq!(
            active.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["change-active"]
        );
        assert_eq!(
            rejected.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["change-rejected"]
        );
        assert_eq!(
            (rejected[0].completed_tasks, rejected[0].total_tasks),
            (1, 1)
        );
    }

    #[test]
    fn test_list_rejected_changes_native_returns_only_marker_rows() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().join("openspec").join("changes");

        let rejected_dir = base.join("change-rejected");
        fs::create_dir_all(&rejected_dir).unwrap();
        fs::write(rejected_dir.join("proposal.md"), "# proposal").unwrap();
        fs::write(rejected_dir.join("tasks.md"), "- [ ] pending task").unwrap();
        fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED").unwrap();

        let active_dir = base.join("change-active");
        fs::create_dir_all(&active_dir).unwrap();
        fs::write(active_dir.join("proposal.md"), "# proposal").unwrap();
        fs::write(active_dir.join("tasks.md"), "- [ ] pending task").unwrap();

        let result = list_rejected_changes_native_from(temp_dir.path()).unwrap();

        assert_eq!(result.len(), 1, "only rejected rows should be returned");
        assert_eq!(result[0].id, "change-rejected");
    }

    #[test]
    fn test_read_proposal_prefers_frontmatter_dependencies() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("change-a");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "---\ndependencies:\n  - frontmatter-dep\npriority: high\nreferences:\n  - src/analyzer.rs\n---\n# Change: Sample\n\n## Dependencies\n\n- body-dep\n",
        )
        .unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let proposal = read_proposal("change-a");

        env::set_current_dir(original_dir).unwrap();

        let metadata = proposal
            .metadata
            .as_ref()
            .expect("metadata should be present");
        assert_eq!(metadata.priority.as_deref(), Some("high"));
        assert_eq!(
            metadata.dependencies,
            Some(vec!["frontmatter-dep".to_string()])
        );
        assert_eq!(metadata.references, vec!["src/analyzer.rs".to_string()]);
        assert!(metadata.warnings.is_empty());
        assert_eq!(proposal.body_dependencies, vec!["body-dep".to_string()]);
        assert_eq!(
            proposal.dependencies_for_analysis(),
            vec!["frontmatter-dep".to_string()]
        );
    }

    #[test]
    fn test_read_proposal_falls_back_to_body_dependencies_without_frontmatter_field() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("change-a");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "---\npriority: low\n---\n# Change: Sample\n\n## Dependencies\n\n- body-dep\n",
        )
        .unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let proposal = read_proposal("change-a");

        env::set_current_dir(original_dir).unwrap();

        let metadata = proposal
            .metadata
            .as_ref()
            .expect("metadata should be present");
        assert_eq!(metadata.priority.as_deref(), Some("low"));
        assert_eq!(metadata.dependencies, None);
        assert_eq!(
            proposal.dependencies_for_analysis(),
            vec!["body-dep".to_string()]
        );
    }

    #[test]
    fn test_strict_dependency_metadata_rejects_unreadable_or_invalid_frontmatter() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.path().join("missing.md");
        assert!(parse_proposal_dependencies_strict_from_file(&missing).is_err());

        let invalid = temp_dir.path().join("invalid.md");
        fs::write(&invalid, "---\ndependencies: [broken\n---\n# Change\n").unwrap();
        assert!(parse_proposal_dependencies_strict_from_file(&invalid).is_err());
    }

    #[test]
    fn test_read_proposal_warns_on_unknown_frontmatter_keys() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("change-a");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "---\ndependencies:\n  - frontmatter-dep\nowner: tumf\n---\n# Change: Sample\n",
        )
        .unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let proposal = read_proposal("change-a");

        env::set_current_dir(original_dir).unwrap();

        let metadata = proposal
            .metadata
            .as_ref()
            .expect("metadata should be present");
        assert_eq!(
            metadata.dependencies,
            Some(vec!["frontmatter-dep".to_string()])
        );
        assert_eq!(metadata.warnings.len(), 1);
        assert_eq!(metadata.warnings[0].key, "owner");
        assert!(metadata.warnings[0]
            .message
            .contains("Unknown proposal frontmatter key: owner"));
        assert_eq!(
            proposal.dependencies_for_analysis(),
            vec!["frontmatter-dep".to_string()]
        );
    }

    #[test]
    fn test_list_changes_native_excludes_without_proposal() {
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        // Test that changes without proposal.md are excluded
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec").join("changes");
        fs::create_dir_all(&changes_dir).unwrap();

        // Create change-a WITH proposal.md
        let change_a_dir = changes_dir.join("change-a");
        fs::create_dir_all(&change_a_dir).unwrap();
        fs::File::create(change_a_dir.join("proposal.md")).unwrap();
        let mut tasks_a = fs::File::create(change_a_dir.join("tasks.md")).unwrap();
        writeln!(tasks_a, "- [x] Task 1\n- [ ] Task 2").unwrap();

        // Create change-b WITHOUT proposal.md
        let change_b_dir = changes_dir.join("change-b");
        fs::create_dir_all(&change_b_dir).unwrap();
        let mut tasks_b = fs::File::create(change_b_dir.join("tasks.md")).unwrap();
        writeln!(tasks_b, "- [ ] Task 1").unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let result = list_changes_native().unwrap();

        env::set_current_dir(original_dir).unwrap();

        // Should only include change-a (has proposal.md), not change-b (no proposal.md)
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "change-a");
        assert_eq!(result[0].completed_tasks, 1);
        assert_eq!(result[0].total_tasks, 2);
    }

    #[test]
    fn test_parse_proposal_metadata_prefers_frontmatter_dependencies() {
        let proposal = r#"---
change_type: hybrid
priority: high
dependencies:
  - frontmatter-change
references:
  - src/openspec.rs
---

# Change: Example

## Dependencies
- body-change
"#;

        let metadata = parse_proposal_metadata(proposal, Path::new("proposal.md"));

        assert_eq!(metadata.priority, Some(ProposalPriority::High));
        assert_eq!(
            metadata.dependencies,
            vec!["frontmatter-change".to_string()]
        );
        assert_eq!(metadata.references, vec!["src/openspec.rs".to_string()]);
        assert!(metadata.warnings.is_empty());
    }

    #[test]
    fn test_parse_verification_declarations_and_strict_diagnostics() {
        let proposal = r#"---
change_type: implementation
verifications:
  - id: local
    requirement: parser behavior
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull request
    automation: Cargo.toml
    evidence: cargo test
    rerun: cargo test
    prerequisites: []
  - id: deployed
    requirement: deployment behavior
    phase: post-integration
    owner: repository-automation
    trigger: default branch integration
    automation: Cargo.toml
    evidence: CI result
    rerun: rerun CI
    prerequisites:
      - deployment approval
---
# Change: Example
"#;
        let parsed = parse_proposal_frontmatter_strict(proposal, Path::new("proposal.md"));
        assert!(parsed.diagnostics.is_empty());
        let metadata = parsed.metadata.expect("metadata");
        assert_eq!(metadata.verifications.len(), 2);
        assert!(metadata.warnings.is_empty());
        assert_eq!(metadata.verifications[0].id.as_deref(), Some("local"));
        assert_eq!(
            metadata.verifications[1].prerequisites,
            Some(vec!["deployment approval".to_string()])
        );

        let malformed = parse_proposal_frontmatter_strict(
            "---\nverifications: [\n---\n# Change",
            Path::new("proposal.md"),
        );
        assert!(malformed.metadata.is_none());
        assert!(!malformed.diagnostics.is_empty());

        let unknown_nested = parse_proposal_frontmatter_strict(
            "---\nverifications:\n  - id: local\n    unexpected: value\n---\n# Change",
            Path::new("proposal.md"),
        );
        assert_eq!(unknown_nested.metadata.unwrap().verifications.len(), 1);
        assert_eq!(
            unknown_nested.diagnostics,
            vec!["Unknown verification declaration key at index 0: unexpected"]
        );
    }

    #[test]
    fn test_parse_verification_declarations_preserves_duplicate_invalid_and_empty_values() {
        let parsed = parse_proposal_frontmatter_strict(
            "---\nverifications:\n  - id: duplicate\n    phase: invalid\n    prerequisites: []\n  - id: duplicate\n    phase: pre-integration\n    prerequisites: []\n---\n# Change",
            Path::new("proposal.md"),
        );

        assert!(parsed.diagnostics.is_empty());
        let declarations = &parsed.metadata.unwrap().verifications;
        assert_eq!(declarations.len(), 2);
        assert_eq!(declarations[0].id.as_deref(), Some("duplicate"));
        assert_eq!(declarations[0].phase.as_deref(), Some("invalid"));
        assert_eq!(declarations[0].prerequisites, Some(Vec::new()));
        assert_eq!(declarations[1].id.as_deref(), Some("duplicate"));
    }

    fn declaration(
        phase: &str,
        execution_class: &str,
        completion_role: &str,
    ) -> VerificationDeclaration {
        VerificationDeclaration {
            id: Some("check".to_string()),
            phase: (!phase.is_empty()).then(|| phase.to_string()),
            execution_class: (!execution_class.is_empty()).then(|| execution_class.to_string()),
            completion_role: (!completion_role.is_empty()).then(|| completion_role.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_parse_verification_role_fields_from_frontmatter() {
        let parsed = parse_proposal_frontmatter_strict(
            "---\nverifications:\n  - id: local\n    phase: pre-integration\n    execution_class: repository-local\n    completion_role: change-blocking\n    prerequisites: []\n---\n# Change",
            Path::new("proposal.md"),
        );

        assert!(parsed.diagnostics.is_empty());
        let declarations = parsed.metadata.unwrap().verifications;
        assert_eq!(
            declarations[0].execution_class(),
            Some(VerificationExecutionClass::RepositoryLocal)
        );
        assert_eq!(
            declarations[0].completion_role(),
            Some(VerificationCompletionRole::ChangeBlocking)
        );
        assert!(declarations[0].is_change_blocking());
        assert!(!declarations[0].is_non_local_change_blocker());
        assert_eq!(
            change_blocking_verification_ids(&declarations),
            vec!["local".to_string()]
        );
    }

    #[test]
    fn test_legacy_verification_declaration_reports_migration_issue_only() {
        let legacy = declaration("pre-integration", "", "");

        assert!(!legacy.declares_role_metadata());
        let issues = evaluate_verification_role(&legacy);
        assert_eq!(
            issues,
            vec![VerificationRoleIssue::LegacyRoleMetadataMissing]
        );
        assert!(issues.iter().all(VerificationRoleIssue::is_warning));
        assert!(change_blocking_verification_ids(&[legacy]).is_empty());
    }

    #[test]
    fn test_evaluate_verification_role_rejects_inconsistent_combinations() {
        assert_eq!(
            evaluate_verification_role(&declaration(
                "post-integration",
                "repository-automation",
                "change-blocking"
            )),
            vec![
                VerificationRoleIssue::PostIntegrationChangeBlocking(
                    "post-integration".to_string()
                ),
                VerificationRoleIssue::NonLocalChangeBlocking(
                    VerificationExecutionClass::RepositoryAutomation
                )
            ]
        );
        assert_eq!(
            evaluate_verification_role(&declaration(
                "pre-integration",
                "physical-device",
                "change-blocking"
            )),
            vec![VerificationRoleIssue::NonLocalChangeBlocking(
                VerificationExecutionClass::PhysicalDevice
            )]
        );
        assert_eq!(
            evaluate_verification_role(&declaration("pre-integration", "repository-local", "")),
            vec![VerificationRoleIssue::IncompleteRoleMetadata]
        );
        assert_eq!(
            evaluate_verification_role(&declaration("pre-integration", "on-my-laptop", "blocking")),
            vec![
                VerificationRoleIssue::UnknownExecutionClass("on-my-laptop".to_string()),
                VerificationRoleIssue::UnknownCompletionRole("blocking".to_string()),
            ]
        );
    }

    #[test]
    fn test_operational_observation_declarations_are_never_change_blocking() {
        for class in [
            "repository-automation",
            "deployed-service",
            "physical-device",
            "external-approval",
            "credentialed-external",
        ] {
            let observation = declaration("post-integration", class, "operational-observation");
            assert!(
                evaluate_verification_role(&observation).is_empty(),
                "{class}"
            );
            assert!(!observation.is_change_blocking());
            assert!(!observation.is_non_local_change_blocker());
        }
    }

    #[test]
    fn test_non_local_change_blocker_detection_covers_missing_phase_and_class() {
        assert!(
            declaration("", "repository-local", "change-blocking").is_non_local_change_blocker()
        );
        assert!(declaration("pre-integration", "", "change-blocking").is_non_local_change_blocker());
        assert!(
            declaration("pre-integration", "deployed-service", "change-blocking")
                .is_non_local_change_blocker()
        );
    }

    #[test]
    fn test_parse_proposal_metadata_falls_back_to_body_dependencies() {
        let proposal = r#"---
change_type: implementation
priority: medium
references:
  - tests/test_demo.py
---

# Change: Example

## Dependencies
- [body-change](../body-change/proposal.md)
- another-change: note
"#;

        let metadata = parse_proposal_metadata(proposal, Path::new("proposal.md"));

        assert_eq!(metadata.priority, Some(ProposalPriority::Medium));
        assert_eq!(
            metadata.dependencies,
            vec!["body-change".to_string(), "another-change".to_string()]
        );
        assert_eq!(metadata.references, vec!["tests/test_demo.py".to_string()]);
    }

    #[test]
    fn test_parse_proposal_metadata_warns_on_unknown_frontmatter_keys() {
        let proposal = r#"---
change_type: spec-only
priority: low
owner: tumf
references: []
---

# Change: Example
"#;

        let metadata = parse_proposal_metadata(proposal, Path::new("proposal.md"));

        assert_eq!(metadata.priority, Some(ProposalPriority::Low));
        assert_eq!(metadata.warnings.len(), 1);
        assert!(metadata.warnings[0].contains("owner"));
    }

    #[test]
    fn test_list_changes_native_suppresses_repetitive_logs() {
        let _cwd_lock = crate::test_support::cwd_lock().lock().unwrap();
        let log_lock = LOG_TEST_MUTEX.get_or_init(|| Mutex::new(()));
        let _guard = log_lock.lock().expect("log mutex poisoned");

        let temp_dir = TempDir::new().unwrap();
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("sample-change");
        fs::create_dir_all(&change_dir).unwrap();
        // Create proposal.md (required for change to be included)
        fs::File::create(change_dir.join("proposal.md")).unwrap();
        let mut tasks_file = fs::File::create(change_dir.join("tasks.md")).unwrap();
        writeln!(tasks_file, "- [ ] Task 1").unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        log_deduplicator::configure_logging(LoggingConfig {
            suppress_repetitive_debug: true,
            summary_interval_secs: 0,
        });

        let _ = list_changes_native();
        let _ = list_changes_native();

        let should_log_progress = log_deduplicator::should_log_task_progress("sample-change", 0, 1);
        let should_log_count = log_deduplicator::should_log_change_count(1);

        env::set_current_dir(original_dir).unwrap();

        assert!(!should_log_progress);
        assert!(!should_log_count);
    }
}
