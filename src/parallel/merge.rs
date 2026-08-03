//! Merge operations for parallel execution.
//!
//! This module handles:
//! - Merge attempt logic (checking base branch state)
//! - Merge execution and conflict resolution
//! - Merge verification

use crate::error::{OrchestratorError, Result};
use crate::vcs::git::commands as git_commands;
use crate::vcs::{VcsBackend, VcsError};
use std::path::Path;
use std::path::PathBuf;

use super::conflict::{self, ResolveFailure};
use super::events::send_event;
use super::resolve_state::{self, GitResolveEvidence};
use super::AlreadyReportedFailureKind;
use super::MergeTaskOutcome;
use super::ParallelEvent;
use super::ParallelExecutor;
use super::PostArchiveAction;
use super::ResolveFailureClassification;

/// Typed failure of one background base-lane attempt.
///
/// Each variant states who already owns the change-scoped lifecycle event, so
/// the queue boundary never re-derives scope from message text or from
/// [`super::MergeResultOrigin`]. Anything that cannot prove change-local scope
/// converts into [`BaseLaneFailure::RunFatal`], including every
/// [`OrchestratorError`] that reaches this boundary through `?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BaseLaneFailure {
    /// Bounded conflict resolution exhausted; `ResolveFailed` already emitted.
    ResolveExhausted {
        attempts: u32,
        classification: ResolveFailureClassification,
        detail: String,
    },
    /// A typed change-scoped owner already reported this failure.
    AlreadyReported {
        kind: AlreadyReportedFailureKind,
        detail: String,
    },
    /// Base/repository truth is unsafe or unknown; the run has no safe continuation.
    RunFatal { detail: String },
}

impl BaseLaneFailure {
    /// Fail closed: an unproven failure is run-fatal, never change-local.
    pub(super) fn fatal(detail: impl Into<String>) -> Self {
        Self::RunFatal {
            detail: detail.into(),
        }
    }

    /// Attach change identity and produce the scheduler-visible outcome.
    pub(super) fn into_outcome(self, change_id: &str) -> MergeTaskOutcome {
        match self {
            Self::ResolveExhausted {
                attempts,
                classification,
                detail,
            } => MergeTaskOutcome::resolve_exhausted(change_id, attempts, classification, detail),
            Self::AlreadyReported { kind, detail } => {
                MergeTaskOutcome::already_reported(change_id, kind, detail)
            }
            Self::RunFatal { detail } => MergeTaskOutcome::run_fatal(detail),
        }
    }
}

impl std::fmt::Display for BaseLaneFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResolveExhausted { detail, .. }
            | Self::AlreadyReported { detail, .. }
            | Self::RunFatal { detail } => write!(f, "{}", detail),
        }
    }
}

impl From<OrchestratorError> for BaseLaneFailure {
    fn from(error: OrchestratorError) -> Self {
        Self::fatal(error.to_string())
    }
}

impl From<VcsError> for BaseLaneFailure {
    fn from(error: VcsError) -> Self {
        Self::fatal(error.to_string())
    }
}

impl From<ResolveFailure> for BaseLaneFailure {
    fn from(failure: ResolveFailure) -> Self {
        match failure {
            ResolveFailure::Exhausted {
                attempts,
                classification,
                detail,
            } => Self::ResolveExhausted {
                attempts,
                classification,
                detail,
            },
            ResolveFailure::Unclassified(error) => Self::fatal(error.to_string()),
        }
    }
}

/// Result alias for base-lane operations that classify their own failures.
pub(super) type BaseLaneResult<T> = std::result::Result<T, BaseLaneFailure>;

/// One admitted change carried through the merge/resolve chain in declared order.
///
/// The archive worktree path is known only here, at merge admission. Dropping
/// it and reconstructing paths later from `workspace_manager.workspaces()` is
/// exactly the failure this type exists to prevent: a preserved archived
/// worktree can be absent from that process-local list even though its path and
/// Git state still exist, and resolve would then silently skip every piece of
/// worktree evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequentialMergeItem {
    /// Workspace branch name integrated into the target.
    pub revision: String,
    /// OpenSpec change identifier.
    pub change_id: String,
    /// Archive worktree path supplied at merge admission.
    pub archive_path: PathBuf,
    /// Branch base recorded at admission, when it was available there.
    pub branch_base: Option<String>,
}

impl SequentialMergeItem {
    /// Build an ordered batch from the parallel merge inputs.
    ///
    /// Cardinality or order loss is rejected here, before any agent runs, so a
    /// truncated batch can never be silently resolved as if it were complete.
    pub fn batch(
        revisions: &[String],
        change_ids: &[String],
        archive_paths: &[PathBuf],
    ) -> std::result::Result<Vec<Self>, String> {
        if revisions.is_empty() {
            return Err("Sequential merge batch has no revisions".to_string());
        }
        if revisions.len() != change_ids.len() {
            return Err(format!(
                "Expected {} change_ids for {} revisions",
                revisions.len(),
                change_ids.len()
            ));
        }
        if revisions.len() != archive_paths.len() {
            return Err(format!(
                "Expected {} archive paths for {} revisions",
                revisions.len(),
                archive_paths.len()
            ));
        }

        let mut items = Vec::with_capacity(revisions.len());
        for ((revision, change_id), archive_path) in revisions
            .iter()
            .zip(change_ids.iter())
            .zip(archive_paths.iter())
        {
            if revision.trim().is_empty() {
                return Err("Sequential merge batch has an empty revision".to_string());
            }
            if change_id.trim().is_empty() {
                return Err(format!(
                    "Sequential merge batch has an empty change_id for revision '{}'",
                    revision
                ));
            }
            if archive_path.as_os_str().is_empty() {
                return Err(format!(
                    "Sequential merge batch has an empty archive path for change '{}'",
                    change_id
                ));
            }
            items.push(Self {
                revision: revision.clone(),
                change_id: change_id.clone(),
                archive_path: archive_path.clone(),
                branch_base: None,
            });
        }
        Ok(items)
    }

    /// Branch names in declared order.
    pub fn revisions(items: &[Self]) -> Vec<String> {
        items.iter().map(|item| item.revision.clone()).collect()
    }

    /// Change identifiers in declared order.
    pub fn change_ids(items: &[Self]) -> Vec<String> {
        items.iter().map(|item| item.change_id.clone()).collect()
    }
}

fn on_merged_failure_message(change_id: &str, error: &OrchestratorError) -> String {
    format!(
        "on_merged hook failed for '{}'; merged transition blocked: {}",
        change_id, error
    )
}

fn archive_completion_verification_root<'a>(
    repo_root: &'a Path,
    archive_path: &'a Path,
) -> &'a Path {
    if archive_path.exists() {
        archive_path
    } else {
        repo_root
    }
}

/// Check if the base branch is dirty (has uncommitted changes or merge in progress).
///
/// Returns `Ok(None)` if the base branch is clean, or `Ok(Some(reason))` with a description
/// of why the base branch is dirty.
pub async fn base_dirty_reason(repo_root: &Path) -> Result<Option<String>> {
    let is_git_repo = git_commands::check_git_repo(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if !is_git_repo {
        return Ok(None);
    }

    let merge_in_progress = git_commands::is_merge_in_progress(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if merge_in_progress {
        return Ok(Some("Merge in progress (MERGE_HEAD exists)".to_string()));
    }

    let (has_changes, status) = git_commands::has_uncommitted_changes(repo_root)
        .await
        .map_err(OrchestratorError::from_vcs_error)?;
    if has_changes {
        let trimmed = status.trim();
        let reason = if trimmed.is_empty() {
            "Working tree has uncommitted changes".to_string()
        } else {
            format!("Working tree has uncommitted changes:\n{}", trimmed)
        };
        return Ok(Some(reason));
    }

    Ok(None)
}

/// Deferred merge metadata.
#[derive(Debug)]
pub struct DeferredMerge {
    /// Human-readable reason for defer.
    pub reason: String,
    /// Whether deferral is auto-resumable (`ResolveWait`) or requires manual action (`MergeWait`).
    pub auto_resumable: bool,
}

/// Result of a merge attempt
#[derive(Debug)]
pub enum MergeAttempt {
    /// Merge succeeded, includes the merge revision
    Merged { revision: String },
    /// Merge deferred with explicit classification.
    Deferred(DeferredMerge),
}

#[derive(Debug)]
enum ArchiveVerificationStatus {
    Complete,
    Incomplete,
    Failed(String),
}

fn already_merged_revision() -> String {
    "already-merged-to-base".to_string()
}

fn archive_verification_outcome(
    change_id: &str,
    archive_path: &Path,
    status: ArchiveVerificationStatus,
    already_merged_to_base: bool,
) -> Option<MergeAttempt> {
    match status {
        ArchiveVerificationStatus::Complete => None,
        ArchiveVerificationStatus::Incomplete => {
            if already_merged_to_base {
                tracing::info!(
                    change_id = %change_id,
                    archive_path = %archive_path.display(),
                    "Suppressing archive-incomplete merge deferral because change is already integrated into base"
                );
                return Some(MergeAttempt::Merged {
                    revision: already_merged_revision(),
                });
            }

            let reason = format!(
                "Archive incomplete for '{}': worktree may be dirty, openspec/changes/{} may still exist, or archive entry may be missing",
                change_id, change_id
            );
            tracing::warn!("{}", reason);
            Some(MergeAttempt::Deferred(DeferredMerge::manual(reason)))
        }
        ArchiveVerificationStatus::Failed(error) => {
            if already_merged_to_base {
                tracing::info!(
                    change_id = %change_id,
                    archive_path = %archive_path.display(),
                    error = %error,
                    "Suppressing archive-verification merge deferral because change is already integrated into base"
                );
                return Some(MergeAttempt::Merged {
                    revision: already_merged_revision(),
                });
            }

            let reason = format!(
                "Failed to verify archive completion for '{}': {}",
                change_id, error
            );
            tracing::warn!("{}", reason);
            Some(MergeAttempt::Deferred(DeferredMerge::manual(reason)))
        }
    }
}

impl DeferredMerge {
    fn auto(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            auto_resumable: true,
        }
    }

    fn manual(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            auto_resumable: false,
        }
    }
}

pub(super) struct ActivePostArchiveMergeGuard {
    change_id: String,
    active: bool,
}

impl ActivePostArchiveMergeGuard {
    pub(super) fn acquire(change_id: impl Into<String>) -> Option<Self> {
        let change_id = change_id.into();
        let mut active = super::active_post_archive_merges()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !active.insert(change_id.clone()) {
            return None;
        }
        Some(Self {
            change_id,
            active: true,
        })
    }

    #[cfg(test)]
    pub(super) fn force_register_for_test(change_id: impl Into<String>) -> Self {
        let change_id = change_id.into();
        let mut active = super::active_post_archive_merges()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.insert(change_id.clone());
        Self {
            change_id,
            active: true,
        }
    }

    fn release(&mut self) {
        if !self.active {
            return;
        }
        let mut active = super::active_post_archive_merges()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.remove(&self.change_id);
        self.active = false;
    }
}

impl Drop for ActivePostArchiveMergeGuard {
    fn drop(&mut self) {
        self.release();
    }
}

impl ParallelExecutor {
    pub(super) async fn is_change_already_merged_to_base(&self, change_id: &str) -> bool {
        let original_branch = match self
            .workspace_manager
            .ensure_original_branch_initialized()
            .await
        {
            Ok(branch) => branch,
            Err(error) => {
                tracing::warn!(
                    change_id = %change_id,
                    "Failed to determine base branch before post-archive merge idempotency check: {}",
                    error
                );
                return false;
            }
        };

        match crate::execution::state::is_merged_to_base(
            change_id,
            &self.repo_root,
            &original_branch,
        )
        .await
        {
            Ok(true) => true,
            Ok(false) => false,
            Err(error) => {
                tracing::warn!(
                    change_id = %change_id,
                    base_branch = %original_branch,
                    "Failed to check whether change is already merged to base: {}",
                    error
                );
                false
            }
        }
    }

    pub(super) fn is_post_archive_merge_active_for(change_id: &str) -> bool {
        super::active_post_archive_merges()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(change_id)
    }

    /// Handle merge attempt and cleanup after successful archive.
    ///
    /// # Arguments
    /// * `workspace_result` - Result from archived workspace
    ///
    /// The returned [`MergeTaskOutcome`] is exhaustive by design: this is the
    /// classification boundary, so every failure leaves here already knowing
    /// whether its lifecycle event was owned elsewhere or the run is unsafe.
    pub(super) async fn handle_merge_and_cleanup(
        &mut self,
        workspace_result: super::types::WorkspaceResult,
    ) -> MergeTaskOutcome {
        let revisions = vec![workspace_result.workspace_name.clone()];
        let change_ids = vec![workspace_result.change_id.clone()];

        // Find workspace path for archive verification.
        // First check in-memory list, then fall back to filesystem discovery
        // (needed when merge runs in a spawned executor with an empty workspace list).
        let workspace_path = self
            .workspace_manager
            .workspaces()
            .iter()
            .find(|workspace| workspace.name == workspace_result.workspace_name)
            .map(|workspace| workspace.path.clone());
        let workspace_path = match workspace_path {
            Some(p) => Some(p),
            None => {
                match self
                    .workspace_manager
                    .find_existing_workspace(&workspace_result.change_id)
                    .await
                {
                    Ok(Some(info)) => Some(info.path),
                    Ok(None) => None,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to discover workspace for '{}': {}",
                            workspace_result.change_id,
                            e
                        );
                        None
                    }
                }
            }
        };

        if let Some(path) = workspace_path {
            let archive_paths = vec![path];

            if let PostArchiveAction::PushToRemote { remote } = &self.post_archive_action {
                return self
                    .push_archived_change_and_cleanup(
                        &workspace_result,
                        remote.clone(),
                        &archive_paths[0],
                    )
                    .await;
            }

            tracing::info!(
                "Merging archived {} (workspace: {})",
                workspace_result.change_id,
                workspace_result.workspace_name
            );

            match self
                .attempt_merge(&revisions, &change_ids, &archive_paths)
                .await
            {
                Ok(MergeAttempt::Merged { revision }) => {
                    // With upstream publication enabled, `attempt_merge` already
                    // ran `on_merged` and emitted change-scoped publication
                    // events while it owned the base lane. Local merge is not
                    // terminal there, so no `MergeCompleted` is sent.
                    let publication_owned_completion = self.upstream_enabled();

                    // Run on_merged hook before merged status transition (MergeCompleted event)
                    let merge_hooks = if publication_owned_completion {
                        None
                    } else {
                        self.hooks.as_ref()
                    };
                    if let Some(hooks) = merge_hooks {
                        // Fetch actual task counts from change data
                        let (completed_tasks, total_tasks) =
                            match crate::openspec::list_changes_native() {
                                Ok(changes) => changes
                                    .iter()
                                    .find(|c| c.id == workspace_result.change_id)
                                    .map(|c| (c.completed_tasks, c.total_tasks))
                                    .unwrap_or((0, 0)),
                                Err(e) => {
                                    tracing::warn!(
                                        "Failed to fetch task counts for on_merged hook: {}",
                                        e
                                    );
                                    (0, 0)
                                }
                            };

                        // Find workspace path
                        let workspace_path = self
                            .workspace_manager
                            .workspaces()
                            .iter()
                            .find(|w| w.name == workspace_result.workspace_name)
                            .map(|w| w.path.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let hook_context = crate::hooks::HookContext::new(
                            0, // changes_processed not easily available here
                            0, // total_changes not easily available here
                            0, // remaining_changes not easily available here
                            false,
                        )
                        .with_change(&workspace_result.change_id, completed_tasks, total_tasks)
                        .with_apply_count(0)
                        .with_parallel_context(&workspace_path, None);

                        if let Err(e) = hooks
                            .run_hook(crate::hooks::HookType::OnMerged, &hook_context)
                            .await
                        {
                            let message =
                                on_merged_failure_message(&workspace_result.change_id, &e);
                            tracing::error!("{}", message);
                            send_event(
                                &self.event_tx,
                                ParallelEvent::HookFailed {
                                    change_id: workspace_result.change_id.clone(),
                                    hook_type: crate::hooks::HookType::OnMerged.to_string(),
                                    error: e.to_string(),
                                },
                            )
                            .await;
                            send_event(
                                &self.event_tx,
                                ParallelEvent::ResolveFailed {
                                    change_id: workspace_result.change_id.clone(),
                                    error: message.clone(),
                                },
                            )
                            .await;
                            // `HookFailed` is the typed owner of this failure;
                            // the queue boundary must not promote it again.
                            return MergeTaskOutcome::already_reported(
                                &workspace_result.change_id,
                                AlreadyReportedFailureKind::Hook,
                                message,
                            );
                        }
                    }

                    // Send MergeCompleted after on_merged hook (triggers merged status transition)
                    if !publication_owned_completion {
                        send_event(
                            &self.event_tx,
                            ParallelEvent::MergeCompleted {
                                change_id: workspace_result.change_id.clone(),
                                revision: revision.clone(),
                            },
                        )
                        .await;
                    }

                    // Merge succeeded, cleanup workspace
                    send_event(
                        &self.event_tx,
                        ParallelEvent::CleanupStarted {
                            workspace: workspace_result.workspace_name.clone(),
                        },
                    )
                    .await;

                    if let Err(err) = self
                        .workspace_manager
                        .cleanup_workspace(&workspace_result.workspace_name)
                        .await
                    {
                        tracing::warn!(
                            "Failed to cleanup worktree '{}' after merge: {}",
                            workspace_result.workspace_name,
                            err
                        );
                    } else {
                        send_event(
                            &self.event_tx,
                            ParallelEvent::CleanupCompleted {
                                workspace: workspace_result.workspace_name.clone(),
                            },
                        )
                        .await;
                    }
                    MergeTaskOutcome::Merged
                }
                Ok(MergeAttempt::Deferred(deferred)) => {
                    let reason = deferred.reason.clone();
                    let auto_resumable = deferred.auto_resumable;
                    if auto_resumable {
                        self.resolve_wait_changes
                            .insert(workspace_result.change_id.clone());
                        self.merge_wait_changes.remove(&workspace_result.change_id);
                    } else {
                        self.merge_wait_changes
                            .insert(workspace_result.change_id.clone());
                        self.resolve_wait_changes
                            .remove(&workspace_result.change_id);
                    }

                    let workspace_status = if auto_resumable {
                        crate::vcs::WorkspaceStatus::Resolving
                    } else {
                        crate::vcs::WorkspaceStatus::MergeWait
                    };
                    tracing::info!(
                        change_id = %workspace_result.change_id,
                        workspace = %workspace_result.workspace_name,
                        auto_resumable,
                        status = ?workspace_status,
                        "Classifying deferred post-archive merge for reducer/display synchronization"
                    );
                    // Auto-resumable deferrals are scheduler-owned retry work. They must
                    // not publish manual MergeWait evidence; only concrete manual
                    // deferrals are exposed as MergeWait.
                    self.workspace_manager.update_workspace_status(
                        &workspace_result.workspace_name,
                        workspace_status.clone(),
                    );

                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeDeferred {
                            change_id: workspace_result.change_id.clone(),
                            reason: reason.clone(),
                            auto_resumable,
                        },
                    )
                    .await;

                    send_event(
                        &self.event_tx,
                        ParallelEvent::WorkspaceStatusUpdated {
                            change_id: workspace_result.change_id.clone(),
                            workspace_name: workspace_result.workspace_name.clone(),
                            status: workspace_status,
                        },
                    )
                    .await;
                    MergeTaskOutcome::deferred(reason, auto_resumable)
                }
                Err(failure) => {
                    // No generic global Error here. The failure already carries
                    // its own scope: an exhausted resolve and an already-reported
                    // publication/hook failure each have a change-scoped owner,
                    // and only a run-fatal outcome may reach the single global
                    // Error owner at the queue/orchestration boundary.
                    tracing::error!(
                        change_id = %workspace_result.change_id,
                        workspace = %workspace_result.workspace_name,
                        outcome = ?failure,
                        "Background base-lane attempt failed"
                    );
                    failure.into_outcome(&workspace_result.change_id)
                }
            }
        } else {
            let reason = format!(
                "Workspace '{}' not found after archive completion, skipping merge",
                workspace_result.workspace_name
            );
            tracing::warn!("{}", reason);
            MergeTaskOutcome::deferred(reason, false)
        }
    }

    async fn push_archived_change_and_cleanup(
        &mut self,
        workspace_result: &super::types::WorkspaceResult,
        remote: String,
        archive_path: &Path,
    ) -> MergeTaskOutcome {
        use crate::execution::archive::is_archive_commit_complete;

        let branch = workspace_result.workspace_name.clone();
        let verification_root = archive_completion_verification_root(&self.repo_root, archive_path);
        match is_archive_commit_complete(&workspace_result.change_id, Some(verification_root)).await
        {
            Ok(true) => {}
            Ok(false) => {
                let reason = format!(
                    "Archive incomplete for '{}': push skipped and workspace preserved",
                    workspace_result.change_id
                );
                tracing::warn!(%reason);
                return MergeTaskOutcome::deferred(reason, false);
            }
            Err(error) => {
                let reason = format!(
                    "Failed to verify archive completion for '{}': {}",
                    workspace_result.change_id, error
                );
                tracing::warn!(%reason);
                return MergeTaskOutcome::deferred(reason, false);
            }
        }

        send_event(
            &self.event_tx,
            ParallelEvent::PushStarted {
                change_id: workspace_result.change_id.clone(),
                remote: remote.clone(),
                branch: branch.clone(),
            },
        )
        .await;

        tracing::info!(
            change_id = %workspace_result.change_id,
            remote = %remote,
            branch = %branch,
            "Pushing archived change branch"
        );

        if let Err(error) =
            git_commands::push_same_named_branch(&remote, &branch, archive_path).await
        {
            let message = format!(
                "Failed to push archived {} branch '{}' to remote '{}': {}",
                workspace_result.change_id, branch, remote, error
            );
            tracing::error!(%message);
            send_event(
                &self.event_tx,
                ParallelEvent::PushFailed {
                    change_id: workspace_result.change_id.clone(),
                    remote,
                    branch,
                    error: message.clone(),
                },
            )
            .await;
            // `PushFailed` owns this change transition and its explicit retry;
            // crossing the shared boundary must not promote it to run-fatal.
            return MergeTaskOutcome::already_reported(
                &workspace_result.change_id,
                AlreadyReportedFailureKind::Push,
                message,
            );
        }

        send_event(
            &self.event_tx,
            ParallelEvent::PushCompleted {
                change_id: workspace_result.change_id.clone(),
                remote,
                branch,
            },
        )
        .await;

        send_event(
            &self.event_tx,
            ParallelEvent::CleanupStarted {
                workspace: workspace_result.workspace_name.clone(),
            },
        )
        .await;
        if let Err(err) = self
            .workspace_manager
            .cleanup_workspace(&workspace_result.workspace_name)
            .await
        {
            tracing::warn!(
                "Failed to cleanup worktree '{}' after push: {}",
                workspace_result.workspace_name,
                err
            );
        } else {
            send_event(
                &self.event_tx,
                ParallelEvent::CleanupCompleted {
                    workspace: workspace_result.workspace_name.clone(),
                },
            )
            .await;
        }
        MergeTaskOutcome::Merged
    }

    /// Attempt one base-lane integration for the admitted batch.
    ///
    /// Failures are classified here, at the boundary that still knows what was
    /// mutated and which lifecycle event was already emitted. `?` converts an
    /// unclassified [`OrchestratorError`] into [`BaseLaneFailure::RunFatal`], so
    /// a repository or base query that fails before a change-scoped transition
    /// fails closed rather than being guessed as change-local.
    pub(super) async fn attempt_merge(
        &self,
        revisions: &[String],
        change_ids: &[String],
        archive_paths: &[PathBuf],
    ) -> BaseLaneResult<MergeAttempt> {
        use crate::execution::archive::is_archive_commit_complete;

        let auto_resolve_count = self
            .auto_resolve_count
            .load(std::sync::atomic::Ordering::SeqCst);
        let manual_resolve_count = self
            .manual_resolve_count
            .as_ref()
            .map(|counter| counter.load(std::sync::atomic::Ordering::SeqCst))
            .unwrap_or(0);
        if auto_resolve_count.saturating_add(manual_resolve_count) > 0 {
            return Ok(MergeAttempt::Deferred(DeferredMerge::auto(
                "Resolve in progress for another change",
            )));
        }

        let Ok(_merge_guard) = super::global_merge_lock().try_lock() else {
            return Ok(MergeAttempt::Deferred(DeferredMerge::auto(
                "Merge lane busy; retry when current base operation completes",
            )));
        };

        if let Some(reason) = base_dirty_reason(&self.repo_root).await? {
            return Ok(MergeAttempt::Deferred(DeferredMerge::manual(reason)));
        }

        // The ordered batch is built before any agent runs so cardinality or
        // order loss fails here rather than surfacing later as skipped
        // worktree evidence.
        let batch = SequentialMergeItem::batch(revisions, change_ids, archive_paths)
            .map_err(OrchestratorError::GitCommand)?;

        let primary_change_id = change_ids
            .first()
            .map(String::as_str)
            .unwrap_or("<unknown>");

        // Only one completed result may occupy the base lane through local
        // integration, verification, publication, and remote confirmation. While
        // a prior result's publication is unconfirmed, a later result waits here
        // rather than entering cumulative base; independent worktree apply and
        // acceptance are unaffected because they never take this lane.
        //
        // This is checked before archive verification so a waiting result does no
        // base work at all while the lane is owed to another change.
        if self.upstream_enabled() {
            if let Some(blocking) = self.blocking_publication_change(primary_change_id).await {
                return Ok(MergeAttempt::Deferred(DeferredMerge::auto(format!(
                    "Waiting for upstream publication of '{}' to be remotely confirmed",
                    blocking
                ))));
            }
        }

        // Repository evidence, not process memory, decides whether this change
        // still needs local integration: a marked-but-unpublished change is
        // already in cumulative base and owes only publication. Its archive
        // evidence is already integrated, so re-verifying the (possibly removed)
        // archive worktree would prove nothing.
        let resume_publication_only =
            self.upstream_enabled() && self.has_pending_publication_for(primary_change_id).await;

        // Verify that all changes are actually archived before attempting merge.
        // A duplicate post-archive merge can race with a successful merge+cleanup path:
        // the stale path may see the archived worktree as dirty/incomplete even though
        // the archive evidence is already integrated into base. In that case base git
        // state is authoritative and the duplicate task is idempotent success.
        //
        // Also, the archive worktree can be cleaned up before a deferred retry runs, so
        // never invoke `git status` from a missing archive path. Use the workspace-local
        // archive path only while it exists; otherwise fall back to the stable base
        // repository root for repository-visible evidence.
        for (change_id, archive_path) in change_ids
            .iter()
            .zip(archive_paths.iter())
            .filter(|_| !resume_publication_only)
        {
            let verification_root =
                archive_completion_verification_root(&self.repo_root, archive_path.as_path());
            if verification_root == self.repo_root.as_path() && !archive_path.exists() {
                tracing::warn!(
                    change_id = %change_id,
                    stale_archive_path = %archive_path.display(),
                    repo_root = %self.repo_root.display(),
                    "Archive verification path is stale; using stable repository root"
                );
            }

            let status = match is_archive_commit_complete(change_id, Some(verification_root)).await
            {
                Ok(true) => ArchiveVerificationStatus::Complete,
                Ok(false) => ArchiveVerificationStatus::Incomplete,
                Err(error) => ArchiveVerificationStatus::Failed(error.to_string()),
            };
            let already_merged_to_base = match status {
                ArchiveVerificationStatus::Complete => false,
                ArchiveVerificationStatus::Incomplete | ArchiveVerificationStatus::Failed(_) => {
                    self.is_change_already_merged_to_base(change_id).await
                }
            };
            if let Some(outcome) = archive_verification_outcome(
                change_id,
                archive_path,
                status,
                already_merged_to_base,
            ) {
                return Ok(outcome);
            }
        }

        // Deterministic checkpoint boundary: immediately before a completed
        // result enters cumulative base. The base lane is already owned and the
        // base is already known clean here, so the checkpoint may start; results
        // that arrive while it runs stay queued behind its single fetch.
        if self.upstream_enabled() && !resume_publication_only {
            self.run_upstream_checkpoint(
                crate::upstream::checkpoint::CheckpointTrigger::BeforeBaseIntegration,
                change_ids.first().map(String::as_str),
                true,
            )
            .await?;
        }

        let revision = if resume_publication_only {
            tracing::info!(
                change_id = %primary_change_id,
                "Resuming upstream publication for an already integrated change"
            );
            git_commands::get_current_commit(&self.repo_root)
                .await
                .map_err(OrchestratorError::from_vcs_error)?
        } else {
            for change_id in change_ids {
                send_event(
                    &self.event_tx,
                    ParallelEvent::ResolveStarted {
                        change_id: change_id.clone(),
                        command: format!(
                            "merge archived change into base branch ({} revision(s))",
                            revisions.len()
                        ),
                    },
                )
                .await;
            }

            self.merge_and_resolve(&batch).await?
        };

        // The base lane is still owned here. Publication runs inside it so the
        // next completed result cannot enter cumulative base before this one's
        // published revision is known.
        if self.upstream_enabled() {
            self.publish_base_integration(
                primary_change_id,
                revisions.first().map(String::as_str),
                resume_publication_only,
            )
            .await?;
        }

        Ok(MergeAttempt::Merged { revision })
    }

    /// Complete the opted-in base-lane sequence for one integrated change.
    ///
    /// Ordering is the contract: durable publication identity, then `on_merged`,
    /// then complete verification, then native push and remote confirmation. A
    /// failure at any step leaves the change unpublished, emits no
    /// `PushCompleted`, and keeps the lane closed to later results.
    ///
    /// `resuming` means the marker commit already exists, so local integration
    /// and its `on_merged` hook already happened. Resumption therefore re-runs
    /// only the unproven part — verification, publication, and confirmation —
    /// and never fires `on_merged` a second time for the same integration.
    ///
    /// Every failure path here reports through an existing typed change-scoped
    /// owner — `PushFailed`, or `HookFailed` for the `on_merged` step — so the
    /// shared base-lane boundary sees `AlreadyReported` and never promotes an
    /// unpublished-but-resumable change to a run-fatal outcome.
    pub(super) async fn publish_base_integration(
        &self,
        change_id: &str,
        workspace_name: Option<&str>,
        resuming: bool,
    ) -> BaseLaneResult<()> {
        let (remote, branch) = self
            .upstream_identity()
            .await
            .unwrap_or_else(|| ("origin".to_string(), "HEAD".to_string()));

        let already_reported = |kind: AlreadyReportedFailureKind, detail: String| {
            BaseLaneFailure::AlreadyReported { kind, detail }
        };

        // Durable identity first: after this commit exists, process loss cannot
        // make the change look like ordinary terminal `merged` history.
        if !resuming {
            if let Err(err) = self.record_publication_intent(change_id).await {
                self.report_publication_failure(change_id, &remote, &branch, &err.to_string())
                    .await;
                return Err(already_reported(
                    AlreadyReportedFailureKind::Push,
                    err.to_string(),
                ));
            }
        }

        // Publication progress, deliberately *not* `MergeCompleted`: local merge
        // is not the opted-in terminal success.
        send_event(
            &self.event_tx,
            ParallelEvent::PushStarted {
                change_id: change_id.to_string(),
                remote: remote.clone(),
                branch: branch.clone(),
            },
        )
        .await;

        if !resuming {
            if let Err(err) = self
                .run_on_merged_for_publication(change_id, workspace_name)
                .await
            {
                self.report_publication_failure(change_id, &remote, &branch, &err.to_string())
                    .await;
                // `HookFailed` is emitted by `run_on_merged_for_publication`
                // itself, so the hook owner keeps this failure.
                return Err(already_reported(
                    AlreadyReportedFailureKind::Hook,
                    err.to_string(),
                ));
            }
        }

        if let Err(err) = self.run_upstream_base_result_verification(change_id).await {
            self.report_publication_failure(change_id, &remote, &branch, &err.to_string())
                .await;
            return Err(already_reported(
                AlreadyReportedFailureKind::Push,
                err.to_string(),
            ));
        }

        match self.publish_completed_change(change_id).await {
            Ok(super::upstream_lane::PublicationLaneOutcome::Confirmed { head }) => {
                tracing::info!(
                    change_id = %change_id,
                    head = %head,
                    remote = %remote,
                    branch = %branch,
                    "Remote observation confirmed cumulative publication"
                );
                send_event(
                    &self.event_tx,
                    ParallelEvent::PushCompleted {
                        change_id: change_id.to_string(),
                        remote,
                        branch,
                    },
                )
                .await;
                Ok(())
            }
            Ok(super::upstream_lane::PublicationLaneOutcome::Unpublished { reason }) => {
                self.report_publication_failure(change_id, &remote, &branch, &reason)
                    .await;
                Err(already_reported(AlreadyReportedFailureKind::Push, reason))
            }
            Err(err) => {
                self.report_publication_failure(change_id, &remote, &branch, &err.to_string())
                    .await;
                Err(already_reported(
                    AlreadyReportedFailureKind::Push,
                    err.to_string(),
                ))
            }
        }
    }

    /// Run `on_merged` for an opted-in publication.
    ///
    /// The hook keeps its existing meaning of successful local base integration
    /// and still runs before publication, so its failure prevents push and final
    /// success.
    async fn run_on_merged_for_publication(
        &self,
        change_id: &str,
        workspace_name: Option<&str>,
    ) -> Result<()> {
        let Some(hooks) = self.hooks.as_ref() else {
            return Ok(());
        };

        let (completed_tasks, total_tasks) = match crate::openspec::list_changes_native() {
            Ok(changes) => changes
                .iter()
                .find(|c| c.id == change_id)
                .map(|c| (c.completed_tasks, c.total_tasks))
                .unwrap_or((0, 0)),
            Err(e) => {
                tracing::warn!("Failed to fetch task counts for on_merged hook: {}", e);
                (0, 0)
            }
        };

        let workspace_path = workspace_name
            .and_then(|name| {
                self.workspace_manager
                    .workspaces()
                    .iter()
                    .find(|w| w.name == name)
                    .map(|w| w.path.to_string_lossy().to_string())
            })
            .unwrap_or_default();

        let hook_context = crate::hooks::HookContext::new(0, 0, 0, false)
            .with_change(change_id, completed_tasks, total_tasks)
            .with_apply_count(0)
            .with_parallel_context(&workspace_path, None);

        if let Err(e) = hooks
            .run_hook(crate::hooks::HookType::OnMerged, &hook_context)
            .await
        {
            let message = on_merged_failure_message(change_id, &e);
            tracing::error!("{}", message);
            send_event(
                &self.event_tx,
                ParallelEvent::HookFailed {
                    change_id: change_id.to_string(),
                    hook_type: crate::hooks::HookType::OnMerged.to_string(),
                    error: e.to_string(),
                },
            )
            .await;
            return Err(OrchestratorError::GitCommand(message));
        }
        Ok(())
    }

    /// Report a publication failure as recoverable, resumable work.
    ///
    /// No `PushCompleted` is emitted, so the change never reaches terminal
    /// `pushed`; `PushFailed` projects into the existing recoverable error flow
    /// whose explicit retry (F5 or the local web control) resumes publication.
    async fn report_publication_failure(
        &self,
        change_id: &str,
        remote: &str,
        branch: &str,
        reason: &str,
    ) {
        tracing::error!(
            change_id = %change_id,
            remote = %remote,
            branch = %branch,
            reason = %reason,
            "Upstream publication did not complete; change remains unpublished"
        );
        send_event(
            &self.event_tx,
            ParallelEvent::PushFailed {
                change_id: change_id.to_string(),
                remote: remote.to_string(),
                branch: branch.to_string(),
                error: format!("upstream publication incomplete: {}", reason),
            },
        )
        .await;
    }

    #[cfg(test)]
    pub async fn resolve_merge_for_change(&mut self, change_id: &str) -> Result<()> {
        let workspace_info = self
            .workspace_manager
            .find_existing_workspace(change_id)
            .await
            .map_err(OrchestratorError::from_vcs_error)?
            .ok_or_else(|| OrchestratorError::ChangeNotFound(change_id.to_string()))?;
        let workspace = self
            .workspace_manager
            .reuse_workspace(&workspace_info)
            .await
            .map_err(OrchestratorError::from_vcs_error)?;

        let revisions = vec![workspace.name.clone()];
        let change_ids = vec![change_id.to_string()];

        // ResolveStarted event will be sent from within conflict resolution functions with command string

        let archive_paths = vec![workspace.path.clone()];
        match self
            .attempt_merge(&revisions, &change_ids, &archive_paths)
            .await
            .map_err(|failure| OrchestratorError::GitCommand(failure.to_string()))?
        {
            MergeAttempt::Merged { revision } => {
                // Run on_merged hook before merged status transition (MergeCompleted event)
                if let Some(ref hooks) = self.hooks {
                    // Fetch actual task counts from change data
                    let (completed_tasks, total_tasks) =
                        match crate::openspec::list_changes_native() {
                            Ok(changes) => changes
                                .iter()
                                .find(|c| c.id == *change_id)
                                .map(|c| (c.completed_tasks, c.total_tasks))
                                .unwrap_or((0, 0)),
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to fetch task counts for on_merged hook: {}",
                                    e
                                );
                                (0, 0)
                            }
                        };

                    let hook_context = crate::hooks::HookContext::new(
                        0, // changes_processed not easily available here
                        0, // total_changes not easily available here
                        0, // remaining_changes not easily available here
                        false,
                    )
                    .with_change(change_id, completed_tasks, total_tasks)
                    .with_apply_count(0)
                    .with_parallel_context(&workspace.path.to_string_lossy(), None);

                    if let Err(e) = hooks
                        .run_hook(crate::hooks::HookType::OnMerged, &hook_context)
                        .await
                    {
                        let message = on_merged_failure_message(change_id, &e);
                        tracing::error!("{}", message);
                        send_event(
                            &self.event_tx,
                            ParallelEvent::HookFailed {
                                change_id: change_id.to_string(),
                                hook_type: crate::hooks::HookType::OnMerged.to_string(),
                                error: e.to_string(),
                            },
                        )
                        .await;
                        send_event(
                            &self.event_tx,
                            ParallelEvent::ResolveFailed {
                                change_id: change_id.to_string(),
                                error: message,
                            },
                        )
                        .await;
                        return Ok(());
                    }
                }

                // Send MergeCompleted after on_merged hook (triggers merged status transition)
                send_event(
                    &self.event_tx,
                    ParallelEvent::MergeCompleted {
                        change_id: change_id.to_string(),
                        revision: revision.clone(),
                    },
                )
                .await;

                send_event(
                    &self.event_tx,
                    ParallelEvent::CleanupStarted {
                        workspace: workspace.name.clone(),
                    },
                )
                .await;
                if let Err(err) = self
                    .workspace_manager
                    .cleanup_workspace(&workspace.name)
                    .await
                {
                    tracing::warn!(
                        "Failed to cleanup worktree '{}' after merge: {}",
                        workspace.name,
                        err
                    );
                } else {
                    send_event(
                        &self.event_tx,
                        ParallelEvent::CleanupCompleted {
                            workspace: workspace.name.clone(),
                        },
                    )
                    .await;
                }

                // Send ResolveCompleted to update TUI status
                send_event(
                    &self.event_tx,
                    ParallelEvent::ResolveCompleted {
                        change_id: change_id.to_string(),
                        worktree_change_ids: None,
                    },
                )
                .await;

                // A resolve just completed: auto-resumable deferred changes may now be
                // unblocked (the base was dirty because this resolve was in progress).
                self.retry_deferred_merges().await;

                Ok(())
            }
            MergeAttempt::Deferred(deferred) => {
                if deferred.auto_resumable {
                    // Auto-resumable: another merge/resolve is in progress.
                    // Track as deferred so retry_deferred_merges picks it up.
                    self.resolve_wait_changes.insert(change_id.to_string());

                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeDeferred {
                            change_id: change_id.to_string(),
                            reason: deferred.reason.clone(),
                            auto_resumable: true,
                        },
                    )
                    .await;
                } else {
                    // Manual intervention required (e.g. uncommitted changes).
                    send_event(
                        &self.event_tx,
                        ParallelEvent::ResolveFailed {
                            change_id: change_id.to_string(),
                            error: deferred.reason.clone(),
                        },
                    )
                    .await;
                }
                Err(OrchestratorError::GitCommand(deferred.reason))
            }
        }
    }

    /// Merge the ordered batch and resolve any conflicts.
    pub(super) async fn merge_and_resolve(
        &self,
        items: &[SequentialMergeItem],
    ) -> BaseLaneResult<String> {
        let change_ids_vec = SequentialMergeItem::change_ids(items);
        let shared_stagger_state = self.shared_stagger_state.clone();
        let auto_resolve_count = self.auto_resolve_count.clone();
        self.merge_and_resolve_with(items, |revisions, details| {
            let change_ids_clone = change_ids_vec.clone();
            let shared_stagger_state_clone = shared_stagger_state.clone();
            let auto_resolve_count_clone = auto_resolve_count.clone();
            async move {
                conflict::resolve_conflicts_with_retry(
                    self.workspace_manager.as_ref(),
                    &self.config,
                    &self.event_tx,
                    &revisions,
                    &change_ids_clone,
                    &details,
                    self.max_conflict_retries,
                    shared_stagger_state_clone,
                    auto_resolve_count_clone,
                )
                .await
            }
        })
        .await
    }

    pub(super) async fn merge_and_resolve_with<'a, F, Fut>(
        &'a self,
        items: &'a [SequentialMergeItem],
        mut resolve_conflicts: F,
    ) -> BaseLaneResult<String>
    where
        F: FnMut(Vec<String>, String) -> Fut,
        Fut: std::future::Future<Output = std::result::Result<(), ResolveFailure>> + Send + 'a,
    {
        let max_attempts = self.max_conflict_retries.max(1);
        let revisions = SequentialMergeItem::revisions(items);
        let revisions = revisions.as_slice();
        let change_ids = SequentialMergeItem::change_ids(items);
        let change_ids = change_ids.as_slice();

        send_event(
            &self.event_tx,
            ParallelEvent::MergeStarted {
                revisions: revisions.to_vec(),
            },
        )
        .await;

        if matches!(
            self.workspace_manager.backend_type(),
            VcsBackend::Git | VcsBackend::Auto
        ) {
            let base_revision = self.workspace_manager.get_current_revision().await?;
            let target_branch = self
                .workspace_manager
                .ensure_original_branch_initialized()
                .await
                .map_err(OrchestratorError::from_vcs_error)?;

            conflict::resolve_merges_with_retry(conflict::ResolveMergesWithRetryArgs {
                workspace_manager: self.workspace_manager.as_ref(),
                config: &self.config,
                event_tx: &self.event_tx,
                items,
                target_branch: target_branch.as_str(),
                base_revision: base_revision.as_str(),
                max_retries: max_attempts,
                shared_stagger_state: self.shared_stagger_state.clone(),
                auto_resolve_count: self.auto_resolve_count.clone(),
                // An opted-in change's terminal success is remote confirmation,
                // not this local merge, so the resolve path must not emit the
                // merged-finalizing per-change completion. `publish_base_integration`
                // owns the change-scoped events from here on.
                publication_owns_completion: self.upstream_enabled(),
            })
            .await?;

            // Verification decides whether the base actually integrated the
            // batch. A failure here leaves base truth unknown, which is exactly
            // the case the outcome contract requires to fail closed.
            self.verify_merge_commits(&base_revision, items)
                .await
                .map_err(|error| {
                    BaseLaneFailure::fatal(format!(
                        "Post-merge verification left base integration truth unknown: {}",
                        error
                    ))
                })?;

            let merge_revision = self.workspace_manager.get_current_revision().await?;

            // Note: MergeCompleted event is sent by the caller after running on_merged hook.
            // This ensures on_merged executes before the merged status transition.
            return Ok(merge_revision);
        }

        for attempt in 1..=max_attempts {
            tracing::info!(
                "Merge attempt {}/{} for revisions: {}",
                attempt,
                max_attempts,
                revisions.join(", ")
            );

            let merge_result = self.workspace_manager.merge_workspaces(revisions).await;

            match merge_result {
                Ok(merge_revision) => {
                    if attempt > 1 {
                        tracing::info!("Merge succeeded after {} attempts", attempt);
                    }

                    // Note: MergeCompleted event is sent by the caller after running on_merged hook.
                    // This ensures on_merged executes before the merged status transition.
                    return Ok(merge_revision);
                }
                Err(VcsError::Conflict { details, .. }) => {
                    let conflict_files =
                        conflict::detect_conflicts(self.workspace_manager.as_ref()).await?;
                    tracing::warn!(
                        "Merge conflict detected on attempt {}/{}",
                        attempt,
                        max_attempts
                    );
                    send_event(
                        &self.event_tx,
                        ParallelEvent::MergeConflict {
                            files: conflict_files,
                        },
                    )
                    .await;

                    if attempt >= max_attempts {
                        // One authoritative change-scoped owner for the bounded
                        // exhaustion, emitted through the same helper the Git
                        // path uses so the two cannot drift.
                        return Err(conflict::fail_resolve(
                            &self.event_tx,
                            change_ids,
                            max_attempts,
                            ResolveFailureClassification::UnresolvedConflict,
                            format!(
                                "Merge conflict unresolved after {} attempts: {}",
                                max_attempts, details
                            ),
                        )
                        .await
                        .into());
                    }

                    tracing::info!(
                        "Resolving merge conflicts (attempt {}/{}).",
                        attempt,
                        max_attempts
                    );

                    // ResolveStarted event will be sent from within conflict resolution functions with command string

                    if let Err(err) = resolve_conflicts(revisions.to_vec(), details.clone()).await {
                        tracing::warn!(
                            "Conflict resolution failed on attempt {}/{}: {}",
                            attempt,
                            max_attempts,
                            err
                        );

                        // No second `ResolveFailed` here: a bounded exhaustion
                        // already emitted its authoritative per-change event, and
                        // an unclassified failure established no change-scoped
                        // transition to duplicate.
                        return Err(err.into());
                    }
                    tracing::info!("Conflict resolution completed, retrying merge");

                    // Note: ResolveCompleted will be sent when the merge succeeds
                }
                Err(e) => return Err(OrchestratorError::from(e).into()),
            }
        }

        // Fallback: should not normally reach here. An unexplained exit leaves
        // base truth unproven, so it fails closed.
        Err(BaseLaneFailure::fatal(
            "Merge failed: exhausted all attempts without success or error".to_string(),
        ))
    }

    /// Verify final integration of every batch item.
    ///
    /// Delegates to the same verifier the resolve retry classifier uses, so the
    /// two callers cannot drift into different notions of "integrated".
    pub(super) async fn verify_merge_commits(
        &self,
        base_revision: &str,
        items: &[SequentialMergeItem],
    ) -> Result<()> {
        if !matches!(
            self.workspace_manager.backend_type(),
            VcsBackend::Git | VcsBackend::Auto
        ) {
            return Ok(());
        }

        let evidence = GitResolveEvidence::new(self.workspace_manager.repo_root());
        resolve_state::verify_final_integration(&evidence, items, base_revision)
            .await
            .map_err(OrchestratorError::GitCommand)
    }
}

#[cfg(test)]
pub async fn resolve_deferred_merge(
    repo_root: PathBuf,
    config: crate::config::OrchestratorConfig,
    change_id: &str,
) -> Result<()> {
    let mut executor = ParallelExecutor::new(repo_root, config, None);
    executor.resolve_merge_for_change(change_id).await
}

#[cfg(test)]
mod tests {
    use super::{
        archive_completion_verification_root, archive_verification_outcome,
        ArchiveVerificationStatus, DeferredMerge, MergeAttempt,
    };
    use std::path::Path;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;

    use crate::parallel::merge_lock_test_mutex;

    async fn init_test_git_repo(path: &Path) {
        tokio::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .output()
            .await
            .expect("git init");
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(path)
            .output()
            .await
            .expect("git config email");
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .output()
            .await
            .expect("git config name");
        std::fs::write(path.join("README.md"), "base\n").expect("write readme");
        tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .await
            .expect("git add");
        tokio::process::Command::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(path)
            .output()
            .await
            .expect("git commit");
    }

    #[test]
    fn test_archive_incomplete_after_base_integration_is_idempotent_merged() {
        let outcome = archive_verification_outcome(
            "change-a",
            Path::new("/tmp/worktree-change-a"),
            ArchiveVerificationStatus::Incomplete,
            true,
        );

        match outcome {
            Some(MergeAttempt::Merged { revision }) => {
                assert_eq!(revision, "already-merged-to-base");
            }
            other => panic!(
                "already-integrated archive-incomplete duplicate must be idempotent merged, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_archive_incomplete_without_base_integration_remains_manual_deferred() {
        let outcome = archive_verification_outcome(
            "change-a",
            Path::new("/tmp/worktree-change-a"),
            ArchiveVerificationStatus::Incomplete,
            false,
        );

        match outcome {
            Some(MergeAttempt::Deferred(deferred)) => {
                assert!(!deferred.auto_resumable);
                assert!(deferred
                    .reason
                    .contains("Archive incomplete for 'change-a'"));
            }
            other => panic!(
                "non-integrated archive-incomplete workspace must remain manual deferred, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_archive_verification_failure_after_base_integration_is_idempotent_merged() {
        let outcome = archive_verification_outcome(
            "change-a",
            Path::new("/tmp/worktree-change-a"),
            ArchiveVerificationStatus::Failed("worktree vanished".to_string()),
            true,
        );

        assert!(matches!(outcome, Some(MergeAttempt::Merged { .. })));
    }

    #[tokio::test]
    async fn test_attempt_merge_dirty_base_remains_manual_deferred() {
        let _test_guard = merge_lock_test_mutex().lock().await;
        let temp = tempfile::TempDir::new().expect("tempdir");
        tokio::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(temp.path())
            .output()
            .await
            .expect("git init");
        tokio::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp.path())
            .output()
            .await
            .expect("git config user.email");
        tokio::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp.path())
            .output()
            .await
            .expect("git config user.name");
        std::fs::write(temp.path().join("README.md"), "base").expect("write readme");
        tokio::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(temp.path())
            .output()
            .await
            .expect("git add");
        tokio::process::Command::new("git")
            .args(["commit", "-m", "Base"])
            .current_dir(temp.path())
            .output()
            .await
            .expect("git commit");

        std::fs::write(temp.path().join("dirty.txt"), "dirty").expect("write dirty file");

        let config = crate::config::OrchestratorConfig {
            apply_command: Some("echo apply".to_string()),
            archive_command: Some("echo archive".to_string()),
            ..Default::default()
        };
        let executor =
            crate::parallel::ParallelExecutor::new(temp.path().to_path_buf(), config, None);

        let result = executor
            .attempt_merge(
                &["dummy-revision".to_string()],
                &["change-a".to_string()],
                &[temp.path().to_path_buf()],
            )
            .await;

        match result.expect("attempt merge should return deferred") {
            MergeAttempt::Deferred(deferred) => {
                assert!(!deferred.auto_resumable);
                assert!(
                    deferred
                        .reason
                        .contains("Working tree has uncommitted changes"),
                    "expected dirty-base manual deferral, got {}",
                    deferred.reason
                );
            }
            other => panic!("dirty base must remain manual deferred, got {:?}", other),
        }
    }

    #[test]
    fn test_auto_deferred_sets_auto_resumable_true() {
        let deferred = DeferredMerge::auto("Resolve in progress for another change");
        assert!(deferred.auto_resumable);
        assert_eq!(deferred.reason, "Resolve in progress for another change");
    }

    #[test]
    fn test_manual_deferred_sets_auto_resumable_false() {
        let deferred = DeferredMerge::manual("Working tree has uncommitted changes");
        assert!(!deferred.auto_resumable);
        assert_eq!(deferred.reason, "Working tree has uncommitted changes");
    }

    #[tokio::test]
    async fn attempt_merge_defers_without_waiting_for_global_merge_lock() {
        let _test_guard = merge_lock_test_mutex().lock().await;
        let temp = TempDir::new().expect("repo tempdir");
        init_test_git_repo(temp.path()).await;
        let _guard = super::super::global_merge_lock()
            .try_lock()
            .expect("test should acquire global merge lock");

        let config = crate::config::OrchestratorConfig {
            apply_command: Some("echo apply".to_string()),
            archive_command: Some("echo archive".to_string()),
            ..Default::default()
        };
        let executor =
            crate::parallel::ParallelExecutor::new(temp.path().to_path_buf(), config, None);

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            executor.attempt_merge(
                &["dummy-revision".to_string()],
                &["change-a".to_string()],
                &[temp.path().to_path_buf()],
            ),
        )
        .await
        .expect("attempt_merge must not await a busy global merge lock")
        .expect("busy lock should be represented as a deferred merge");

        match result {
            MergeAttempt::Deferred(deferred) => {
                assert!(deferred.auto_resumable);
                assert!(
                    deferred.reason.contains("Merge lane busy"),
                    "expected merge-lane-busy reason, got {}",
                    deferred.reason
                );
            }
            other => panic!("busy merge lane must defer, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn attempt_merge_checks_resolve_counters_before_global_merge_lock() {
        let _test_guard = merge_lock_test_mutex().lock().await;
        let temp = TempDir::new().expect("repo tempdir");
        init_test_git_repo(temp.path()).await;
        let _guard = super::super::global_merge_lock()
            .try_lock()
            .expect("test should acquire global merge lock");

        let config = crate::config::OrchestratorConfig {
            apply_command: Some("echo apply".to_string()),
            archive_command: Some("echo archive".to_string()),
            ..Default::default()
        };
        let executor =
            crate::parallel::ParallelExecutor::new(temp.path().to_path_buf(), config, None);
        executor.auto_resolve_count.store(1, Ordering::SeqCst);

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            executor.attempt_merge(
                &["dummy-revision".to_string()],
                &["change-a".to_string()],
                &[temp.path().to_path_buf()],
            ),
        )
        .await
        .expect("attempt_merge must check resolve counters before the lock")
        .expect("active resolve should be represented as a deferred merge");

        match result {
            MergeAttempt::Deferred(deferred) => {
                assert!(deferred.auto_resumable);
                assert_eq!(deferred.reason, "Resolve in progress for another change");
            }
            other => panic!("active resolve must defer, got {:?}", other),
        }
    }

    #[test]
    fn archive_verification_root_falls_back_to_repo_root_for_deleted_archive_path() {
        let repo_root = TempDir::new().expect("repo tempdir");
        let missing_archive_path = repo_root.path().join("deleted-worktree");

        let root = archive_completion_verification_root(repo_root.path(), &missing_archive_path);

        assert_eq!(root, repo_root.path());
    }

    #[test]
    fn archive_verification_root_uses_existing_archive_path() {
        let repo_root = TempDir::new().expect("repo tempdir");
        let archive_path = TempDir::new().expect("archive tempdir");

        let root = archive_completion_verification_root(repo_root.path(), archive_path.path());

        assert_eq!(root, archive_path.path());
    }
}
