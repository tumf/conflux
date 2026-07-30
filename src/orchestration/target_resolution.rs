//! Repository-evidence resolution for explicit run targets.
//!
//! Explicit targets (positional IDs or `--change`) used to be validated against
//! the current active OpenSpec change list only. Once a requested change is
//! archived and integrated into base it leaves that list, so repeating the same
//! target set after an interruption rejected the already completed ID as
//! unknown before any resume logic could run.
//!
//! This module classifies each requested ID from repository evidence instead:
//!
//! 1. [`TargetClassification::Active`] - present in the active OpenSpec list
//! 2. [`TargetClassification::AlreadyCompleted`] - proven by the base branch tree
//! 3. [`TargetClassification::ResumableWorkspace`] - proven by a cflx-managed
//!    worktree whose own file/Git state identifies the change
//! 4. [`TargetClassification::Unknown`] - no evidence at all
//! 5. [`TargetClassification::EvidenceError`] - contradictory or unreadable evidence
//! 6. [`TargetClassification::ResumeRefused`] - worktree-only recovery under `--no-resume`
//!
//! Classification is read-only: it never creates, deletes, or mutates a
//! workspace, and it never consults commit subjects, logs, events, or a server
//! database.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::error::OrchestratorError;

#[allow(unused_imports)]
pub use crate::execution::state::{
    classify_base_completion, BaseCompletionEvidence, BaseEvidenceErrorKind,
};
#[allow(unused_imports)]
pub use crate::openspec::Change;

/// Classification of one requested explicit target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetClassification {
    /// Present in the current active OpenSpec change list.
    Active,
    /// Base-integrated: archived in the base branch tree, nothing left to do.
    AlreadyCompleted,
    /// Recoverable from an existing cflx-managed worktree.
    ResumableWorkspace,
    /// No active, completed, or valid managed-worktree evidence.
    Unknown,
    /// Contradictory or unreadable repository evidence.
    EvidenceError,
    /// Only worktree-recoverable, but `--no-resume` forbids implicit reuse.
    ResumeRefused,
}

impl TargetClassification {
    /// Whether this classification sends the target into normal scheduling.
    pub fn is_dispatchable(self) -> bool {
        matches!(self, Self::Active | Self::ResumableWorkspace)
    }

    /// Whether this classification rejects the invocation.
    pub fn is_failure(self) -> bool {
        matches!(
            self,
            Self::Unknown | Self::EvidenceError | Self::ResumeRefused
        )
    }

    /// Stable machine-readable label for output consumers.
    #[allow(dead_code)] // Consumed by output surfaces and classification tests.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::AlreadyCompleted => "already_completed",
            Self::ResumableWorkspace => "resumable_workspace",
            Self::Unknown => "unknown",
            Self::EvidenceError => "evidence_error",
            Self::ResumeRefused => "resume_refused",
        }
    }
}

/// One classified target, retaining its requested identity and evidence.
#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    /// The ID exactly as requested (trimmed).
    pub requested_id: String,
    pub classification: TargetClassification,
    /// Active metadata, or the change reconstructed from workspace evidence.
    pub change: Option<Change>,
    /// Managed workspace path backing a resumable/refused classification.
    pub workspace_path: Option<PathBuf>,
    /// Human-facing evidence detail; always present for failures.
    pub diagnostic: Option<String>,
}

impl ResolvedTarget {
    fn new(requested_id: &str, classification: TargetClassification) -> Self {
        Self {
            requested_id: requested_id.to_string(),
            classification,
            change: None,
            workspace_path: None,
            diagnostic: None,
        }
    }

    fn with_change(mut self, change: Change) -> Self {
        self.change = Some(change);
        self
    }

    fn with_workspace(mut self, path: PathBuf) -> Self {
        self.workspace_path = Some(path);
        self
    }

    fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostic = Some(diagnostic.into());
        self
    }
}

/// Ordered classification result for one explicit target set.
///
/// Order is the deduplicated request order throughout, so terminal reporting and
/// dispatch never reorder what the supervisor asked for.
#[derive(Debug, Clone, Default)]
pub struct TargetResolution {
    /// Classified targets in deduplicated request order.
    pub targets: Vec<ResolvedTarget>,
    /// IDs repeated in the request, in first-repeat order.
    pub duplicates: Vec<String>,
}

impl TargetResolution {
    /// Requested IDs in deduplicated request order.
    pub fn requested_ids(&self) -> Vec<String> {
        self.targets
            .iter()
            .map(|t| t.requested_id.clone())
            .collect()
    }

    /// Classification recorded for a requested ID, if it was resolved.
    #[allow(dead_code)] // Typed accessor for terminal/output consumers.
    pub fn classification_of(&self, requested_id: &str) -> Option<TargetClassification> {
        self.targets
            .iter()
            .find(|t| t.requested_id == requested_id)
            .map(|t| t.classification)
    }

    /// Evidence detail recorded for a requested ID, if any.
    #[allow(dead_code)] // Typed accessor for terminal/output consumers.
    pub fn diagnostic_of(&self, requested_id: &str) -> Option<&str> {
        self.targets
            .iter()
            .find(|t| t.requested_id == requested_id)
            .and_then(|t| t.diagnostic.as_deref())
    }

    fn ids_with(&self, classification: TargetClassification) -> Vec<String> {
        self.targets
            .iter()
            .filter(|t| t.classification == classification)
            .map(|t| t.requested_id.clone())
            .collect()
    }

    /// IDs sent into scheduling (active + resumable), in request order.
    pub fn processed_ids(&self) -> Vec<String> {
        self.targets
            .iter()
            .filter(|t| t.classification.is_dispatchable())
            .map(|t| t.requested_id.clone())
            .collect()
    }

    /// IDs skipped because base evidence proves completion, in request order.
    pub fn already_completed_ids(&self) -> Vec<String> {
        self.ids_with(TargetClassification::AlreadyCompleted)
    }

    /// IDs registered for workspace resume, in request order.
    pub fn resumable_ids(&self) -> Vec<String> {
        self.ids_with(TargetClassification::ResumableWorkspace)
    }

    /// Active IDs, in request order.
    #[allow(dead_code)] // Typed accessor for terminal/output consumers.
    pub fn active_ids(&self) -> Vec<String> {
        self.ids_with(TargetClassification::Active)
    }

    /// IDs with no usable evidence, in request order.
    pub fn unknown_ids(&self) -> Vec<String> {
        self.ids_with(TargetClassification::Unknown)
    }

    /// IDs whose evidence was contradictory or unreadable, in request order.
    pub fn evidence_error_ids(&self) -> Vec<String> {
        self.ids_with(TargetClassification::EvidenceError)
    }

    /// IDs refused because only a workspace could recover them under `--no-resume`.
    pub fn resume_refused_ids(&self) -> Vec<String> {
        self.ids_with(TargetClassification::ResumeRefused)
    }

    /// Changes to dispatch (active + resumable), in request order.
    pub fn dispatch_changes(&self) -> Vec<Change> {
        self.targets
            .iter()
            .filter(|t| t.classification.is_dispatchable())
            .filter_map(|t| t.change.clone())
            .collect()
    }

    /// Whether any target rejects the invocation.
    pub fn has_failures(&self) -> bool {
        !self.duplicates.is_empty() || self.targets.iter().any(|t| t.classification.is_failure())
    }

    /// Aggregate every rejection into a single actionable diagnostic.
    ///
    /// All duplicate and unresolvable IDs are reported together so the caller
    /// fails once, before any workspace is created, deleted, or mutated.
    pub fn failure_error(&self) -> Option<OrchestratorError> {
        if !self.has_failures() {
            return None;
        }

        let mut parts = Vec::new();
        if !self.duplicates.is_empty() {
            parts.push(format!(
                "duplicate change IDs: {}",
                self.duplicates.join(", ")
            ));
        }

        let unknown = self.unknown_ids();
        if !unknown.is_empty() {
            parts.push(format!("unknown change IDs: {}", unknown.join(", ")));
        }

        let unreadable = self.evidence_error_ids();
        if !unreadable.is_empty() {
            parts.push(format!(
                "unusable change evidence: {}",
                self.describe_ids(&unreadable)
            ));
        }

        let refused = self.resume_refused_ids();
        if !refused.is_empty() {
            parts.push(format!(
                "workspace-only change IDs refused by --no-resume: {}",
                self.describe_ids(&refused)
            ));
        }

        Some(OrchestratorError::Parse(format!(
            "invalid run targets: {}",
            parts.join("; ")
        )))
    }

    /// Stable ordered human-facing summary lines.
    ///
    /// Output consumers read the classification arrays directly instead of
    /// parsing diagnostics, so requested / processed / already-completed /
    /// pending groups stay distinguishable.
    pub fn report_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "Explicit targets requested: {}",
            self.requested_ids().join(", ")
        )];

        let processed = self.processed_ids();
        if !processed.is_empty() {
            lines.push(format!("  to process: {}", processed.join(", ")));
        }
        let resumable = self.resumable_ids();
        if !resumable.is_empty() {
            lines.push(format!("  resumable workspaces: {}", resumable.join(", ")));
        }
        let completed = self.already_completed_ids();
        if !completed.is_empty() {
            lines.push(format!(
                "  already completed (skipped): {}",
                completed.join(", ")
            ));
        }
        let pending = self.pending_ids();
        if !pending.is_empty() {
            lines.push(format!("  unresolved: {}", self.describe_ids(&pending)));
        }
        if !self.duplicates.is_empty() {
            lines.push(format!("  duplicates: {}", self.duplicates.join(", ")));
        }

        lines
    }

    /// IDs that could not be resolved into work, in request order.
    pub fn pending_ids(&self) -> Vec<String> {
        self.targets
            .iter()
            .filter(|t| t.classification.is_failure())
            .map(|t| t.requested_id.clone())
            .collect()
    }

    fn describe_ids(&self, ids: &[String]) -> String {
        ids.iter()
            .map(|id| {
                match self
                    .targets
                    .iter()
                    .find(|t| &t.requested_id == id)
                    .and_then(|t| t.diagnostic.as_deref())
                {
                    Some(detail) => format!("{} ({})", id, detail),
                    None => id.clone(),
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Managed-worktree resume evidence for one requested change.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceResumeEvidence {
    /// The worktree's own file/Git state identifies the requested change.
    Resumable { path: PathBuf, change: Box<Change> },
    /// No usable workspace evidence (absent, name-only, or malformed).
    NotResumable { detail: String },
    /// The workspace could not be read.
    EvidenceError { detail: String },
}

/// Read-only repository evidence source used by the resolver.
///
/// Injecting this keeps the resolver's decision table unit-testable without a
/// real repository, worktree, or Git process.
#[async_trait]
pub trait TargetEvidence: Send + Sync {
    /// Base branch tree evidence for base-integrated completion.
    async fn base_completion(&self, change_id: &str) -> BaseCompletionEvidence;
    /// cflx-managed worktree evidence for resume routing.
    async fn workspace_resume(&self, change_id: &str) -> WorkspaceResumeEvidence;
}

/// Invocation options affecting classification.
#[derive(Debug, Clone, Copy, Default)]
pub struct TargetResolutionOptions {
    /// `--no-resume`: existing workspaces must not be implicitly reused.
    pub no_resume: bool,
}

/// Classify an explicit target set from repository evidence.
///
/// Evidence precedence per target:
///
/// 1. active list membership (an active change always wins over a candidate worktree)
/// 2. base branch tree completion
/// 3. cflx-managed worktree state
/// 4. otherwise unknown
///
/// Contradictory or unreadable evidence fails safely instead of degrading to
/// "unknown" or "completed".
pub async fn resolve_explicit_targets(
    requested: &[String],
    active_changes: &[Change],
    evidence: &dyn TargetEvidence,
    options: TargetResolutionOptions,
) -> TargetResolution {
    let mut resolution = TargetResolution::default();
    let mut seen: HashSet<String> = HashSet::new();

    for requested_id in requested {
        let id = requested_id.trim();
        if id.is_empty() {
            continue;
        }
        if !seen.insert(id.to_string()) {
            resolution.duplicates.push(id.to_string());
            continue;
        }

        // 1. Active changes take precedence over any candidate worktree so that
        //    ordinary work never degrades into workspace-only resume routing.
        if let Some(change) = active_changes.iter().find(|c| c.id == id) {
            resolution.targets.push(
                ResolvedTarget::new(id, TargetClassification::Active).with_change(change.clone()),
            );
            continue;
        }

        // 2. Base-integrated completion, proven only by the base branch tree.
        match evidence.base_completion(id).await {
            BaseCompletionEvidence::Completed => {
                resolution.targets.push(
                    ResolvedTarget::new(id, TargetClassification::AlreadyCompleted)
                        .with_diagnostic(BaseCompletionEvidence::Completed.describe()),
                );
                continue;
            }
            evidence @ BaseCompletionEvidence::Contradictory { .. } => {
                resolution.targets.push(
                    ResolvedTarget::new(id, TargetClassification::EvidenceError)
                        .with_diagnostic(evidence.describe()),
                );
                continue;
            }
            evidence @ BaseCompletionEvidence::EvidenceError { .. } => {
                resolution.targets.push(
                    ResolvedTarget::new(id, TargetClassification::EvidenceError)
                        .with_diagnostic(evidence.describe()),
                );
                continue;
            }
            BaseCompletionEvidence::NotCompleted => {}
        }

        // 3. Managed worktree evidence. A matching name alone is never enough.
        match evidence.workspace_resume(id).await {
            WorkspaceResumeEvidence::Resumable { path, change } => {
                if options.no_resume {
                    // Refuse rather than delete the workspace implicitly.
                    resolution.targets.push(
                        ResolvedTarget::new(id, TargetClassification::ResumeRefused)
                            .with_workspace(path)
                            .with_diagnostic(
                                "recoverable only from an existing workspace; \
                                 rerun without --no-resume or remove the workspace explicitly",
                            ),
                    );
                } else {
                    resolution.targets.push(
                        ResolvedTarget::new(id, TargetClassification::ResumableWorkspace)
                            .with_change(*change)
                            .with_workspace(path),
                    );
                }
            }
            WorkspaceResumeEvidence::EvidenceError { detail } => {
                resolution.targets.push(
                    ResolvedTarget::new(id, TargetClassification::EvidenceError)
                        .with_diagnostic(detail),
                );
            }
            WorkspaceResumeEvidence::NotResumable { detail } => {
                resolution.targets.push(
                    ResolvedTarget::new(id, TargetClassification::Unknown).with_diagnostic(detail),
                );
            }
        }
    }

    resolution
}

/// An explicit target set whose classification is deferred to a later
/// repository-evidence boundary.
///
/// An enabled real `-u` run must complete its initial upstream base-lane
/// checkpoint before classifying, because that checkpoint can integrate the very
/// archive that proves a requested target is already completed. The plan carries
/// the requested set to that boundary and records the result for terminal
/// consumers.
#[derive(Clone)]
pub struct ExplicitTargetPlan {
    requested: Vec<String>,
    base_branch: String,
    options: TargetResolutionOptions,
    resolved: std::sync::Arc<tokio::sync::Mutex<Option<TargetResolution>>>,
}

impl ExplicitTargetPlan {
    pub fn new(
        requested: Vec<String>,
        base_branch: String,
        options: TargetResolutionOptions,
    ) -> Self {
        Self {
            requested,
            base_branch,
            options,
            resolved: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub fn requested(&self) -> &[String] {
        &self.requested
    }

    /// Classify against current repository evidence and record the result.
    ///
    /// This is read-only with respect to workspaces: it resolves before any
    /// change-worktree creation or reuse registration and never mutates one.
    pub async fn resolve(&self, repo_root: &Path) -> TargetResolution {
        let active = crate::openspec::list_changes_native_from(repo_root).unwrap_or_default();
        let evidence =
            RepositoryTargetEvidence::new(repo_root.to_path_buf(), self.base_branch.clone());
        let resolution =
            resolve_explicit_targets(&self.requested, &active, &evidence, self.options).await;
        *self.resolved.lock().await = Some(resolution.clone());
        resolution
    }

    /// The recorded resolution, if the deferred boundary was reached.
    pub async fn resolved(&self) -> Option<TargetResolution> {
        self.resolved.lock().await.clone()
    }
}

impl std::fmt::Debug for ExplicitTargetPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExplicitTargetPlan")
            .field("requested", &self.requested)
            .field("base_branch", &self.base_branch)
            .field("options", &self.options)
            .finish()
    }
}

/// Real repository evidence: base branch tree plus cflx-managed worktrees.
pub struct RepositoryTargetEvidence {
    repo_root: PathBuf,
    base_branch: String,
}

impl RepositoryTargetEvidence {
    pub fn new(repo_root: PathBuf, base_branch: String) -> Self {
        Self {
            repo_root,
            base_branch,
        }
    }
}

#[async_trait]
impl TargetEvidence for RepositoryTargetEvidence {
    async fn base_completion(&self, change_id: &str) -> BaseCompletionEvidence {
        classify_base_completion(change_id, &self.repo_root, &self.base_branch).await
    }

    async fn workspace_resume(&self, change_id: &str) -> WorkspaceResumeEvidence {
        // Read-only discovery: never cleans up or replaces a candidate worktree.
        let path = match crate::vcs::git::get_worktree_path_for_change(&self.repo_root, change_id)
            .await
        {
            Ok(Some(path)) => path,
            Ok(None) => {
                return WorkspaceResumeEvidence::NotResumable {
                    detail: "no active change directory, base archive entry, or managed workspace"
                        .to_string(),
                }
            }
            Err(err) => {
                return WorkspaceResumeEvidence::EvidenceError {
                    detail: format!("managed workspace discovery failed: {}", err),
                }
            }
        };

        workspace_resume_evidence(change_id, &path, &self.base_branch).await
    }
}

/// Validate one candidate worktree as resume evidence for `change_id`.
///
/// A matching workspace/branch name is deliberately insufficient: the worktree
/// must exist on disk, carry its own change evidence (active change directory or
/// archive entry with a proposal), and be routable by the existing workspace
/// state detection.
pub async fn workspace_resume_evidence(
    change_id: &str,
    workspace_path: &Path,
    base_branch: &str,
) -> WorkspaceResumeEvidence {
    if !workspace_path.is_dir() {
        return WorkspaceResumeEvidence::NotResumable {
            detail: format!(
                "managed workspace path '{}' does not exist; workspace name alone is not resume evidence",
                workspace_path.display()
            ),
        };
    }

    let Some(change) = change_from_workspace(change_id, workspace_path) else {
        return WorkspaceResumeEvidence::NotResumable {
            detail: format!(
                "managed workspace '{}' contains no proposal for '{}'; workspace name alone is not resume evidence",
                workspace_path.display(),
                change_id
            ),
        };
    };

    // The existing resume detector must be able to route this workspace.
    match crate::execution::state::detect_workspace_state(change_id, workspace_path, base_branch)
        .await
    {
        Ok(_state) => WorkspaceResumeEvidence::Resumable {
            path: workspace_path.to_path_buf(),
            change: Box::new(change),
        },
        Err(err) => WorkspaceResumeEvidence::EvidenceError {
            detail: format!(
                "managed workspace '{}' state is unreadable: {}",
                workspace_path.display(),
                err
            ),
        },
    }
}

/// Reconstruct a change from a workspace's own files.
///
/// Prefers the active change directory, then a not-yet-integrated archive entry,
/// which is what an interrupted archive step leaves behind.
fn change_from_workspace(change_id: &str, workspace_path: &Path) -> Option<Change> {
    let active_dir = workspace_path.join("openspec/changes").join(change_id);
    if active_dir.join("proposal.md").is_file() {
        if let Ok(changes) = crate::openspec::list_changes_native_from(workspace_path) {
            if let Some(change) = changes.into_iter().find(|c| c.id == change_id) {
                return Some(change);
            }
        }
    }

    let archive_entry =
        find_archive_entry(change_id, &workspace_path.join("openspec/changes/archive"))?;
    let proposal_path = archive_entry.join("proposal.md");
    if !proposal_path.is_file() {
        return None;
    }

    let (completed_tasks, total_tasks) =
        crate::task_parser::parse_file(&archive_entry.join("tasks.md"), Some(change_id))
            .map(|progress| (progress.completed, progress.total))
            .unwrap_or((0, 0));
    let metadata = crate::openspec::parse_proposal_metadata_from_file(&proposal_path);
    let dependencies = metadata.dependencies.clone();

    Some(Change {
        id: change_id.to_string(),
        completed_tasks,
        total_tasks,
        last_modified: String::new(),
        dependencies,
        metadata,
    })
}

/// Find an exact or date-prefixed archive entry directory.
fn find_archive_entry(change_id: &str, archive_dir: &Path) -> Option<PathBuf> {
    if !archive_dir.is_dir() {
        return None;
    }

    std::fs::read_dir(archive_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            (name == change_id || name.ends_with(&format!("-{change_id}"))) && entry.path().is_dir()
        })
        .map(|entry| entry.path())
}
