//! Real Git/hook implementation of [`WorktreeBackend`].
//!
//! This is the only place worktree operations touch a repository. Everything
//! above it — eligibility, the mutation guard, teardown ordering, hook
//! ordering — lives in [`super::service`] so it is the same for every frontend.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::config::OrchestratorConfig;
use crate::vcs::git::commands;

use super::service::{
    ConflictPolicy, DirtyState, MergeAttempt, WorktreeBackend, WorktreeFacts, WorktreeOpError,
    WorktreeOpResult,
};

/// Git-backed worktree operations for one repository.
pub struct GitWorktreeBackend {
    repo_root: PathBuf,
    config: Arc<OrchestratorConfig>,
    /// Event channel handed to the hook runner so `on_merged` output reaches the
    /// same observers as every other hook.
    hook_events: Option<tokio::sync::mpsc::Sender<crate::events::ExecutionEvent>>,
}

impl GitWorktreeBackend {
    /// Build a backend for `repo_root`.
    pub fn new(repo_root: PathBuf, config: Arc<OrchestratorConfig>) -> Self {
        Self {
            repo_root,
            config,
            hook_events: None,
        }
    }

    /// Route hook output to an orchestrator event channel.
    pub fn with_hook_events(
        mut self,
        tx: tokio::sync::mpsc::Sender<crate::events::ExecutionEvent>,
    ) -> Self {
        self.hook_events = Some(tx);
        self
    }
}

#[async_trait]
impl WorktreeBackend for GitWorktreeBackend {
    async fn observe(&self) -> WorktreeOpResult<Vec<WorktreeFacts>> {
        let enriched = super::get_worktrees(&self.repo_root)
            .await
            .map_err(|e| WorktreeOpError::Internal(format!("failed to list worktrees: {e}")))?;

        // One base-side read, not one per worktree: `MERGE_HEAD` is a property of
        // the repository root, and every worktree's eligibility depends on it.
        let base_merge_in_progress = commands::is_merge_in_progress(&self.repo_root)
            .await
            .unwrap_or(false);

        let mut facts = Vec::with_capacity(enriched.len());
        for worktree in enriched {
            let dirty = match commands::has_uncommitted_changes(&worktree.path).await {
                Ok((true, _)) => DirtyState::Dirty,
                Ok((false, _)) => DirtyState::Clean,
                Err(error) => {
                    debug!(
                        path = %worktree.path.display(),
                        "dirty state could not be determined: {error}"
                    );
                    DirtyState::Unknown
                }
            };
            facts.push(WorktreeFacts {
                identity: crate::parallel::acceptance_state::worktree_identity(&worktree.path),
                path: worktree.path,
                branch: worktree.branch,
                head: worktree.head,
                is_main: worktree.is_main,
                is_detached: worktree.is_detached,
                has_commits_ahead: worktree.has_commits_ahead,
                conflict_files: worktree
                    .merge_conflict
                    .map(|c| c.conflict_files)
                    .unwrap_or_default(),
                dirty,
                base_merge_in_progress,
            });
        }
        Ok(facts)
    }

    async fn base_head(&self) -> WorktreeOpResult<String> {
        commands::get_current_commit(&self.repo_root)
            .await
            .map_err(|e| WorktreeOpError::Internal(format!("failed to read base HEAD: {e}")))
    }

    async fn create(&self, path: &Path, branch: &str, base_commit: &str) -> WorktreeOpResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                WorktreeOpError::Internal(format!("failed to create workspace directory: {e}"))
            })?;
        }
        let path_str = path.to_str().ok_or_else(|| {
            WorktreeOpError::Internal("derived worktree path is not valid UTF-8".to_string())
        })?;
        commands::worktree_add(&self.repo_root, path_str, branch, base_commit)
            .await
            .map_err(|e| WorktreeOpError::Internal(format!("failed to create worktree: {e}")))?;

        // Setup is best-effort in every existing creation path; keep that.
        if let Err(error) = commands::run_worktree_setup(&self.repo_root, path).await {
            warn!(path = %path.display(), "worktree setup script failed: {error} (non-fatal)");
        }
        Ok(())
    }

    async fn remove(&self, path: &Path, skip_teardown: bool) -> WorktreeOpResult<()> {
        let path_str = path.to_str().ok_or_else(|| {
            WorktreeOpError::Internal("worktree path is not valid UTF-8".to_string())
        })?;
        commands::worktree_remove_with_options(
            &self.repo_root,
            path_str,
            commands::WorktreeRemoveOptions { skip_teardown },
        )
        .await
        .map_err(|e| WorktreeOpError::Internal(format!("failed to remove worktree: {e}")))
    }

    async fn delete_branch(&self, branch: &str) -> WorktreeOpResult<()> {
        commands::branch_delete(&self.repo_root, branch)
            .await
            .map_err(|e| WorktreeOpError::Internal(format!("failed to delete branch: {e}")))
    }

    async fn merge_into_base(
        &self,
        branch: &str,
        policy: ConflictPolicy,
    ) -> WorktreeOpResult<MergeAttempt> {
        match policy {
            ConflictPolicy::AbortOnConflict => {
                match commands::merge_branch(&self.repo_root, branch).await {
                    Ok(()) => Ok(MergeAttempt::Merged),
                    // `merge_branch` already aborted; the message carries the files.
                    Err(error) => Err(WorktreeOpError::Internal(error.to_string())),
                }
            }
            ConflictPolicy::PreserveConflict => {
                match commands::merge_branch_preserving_conflict(&self.repo_root, branch).await {
                    Ok(commands::PreservedMergeOutcome::Merged) => Ok(MergeAttempt::Merged),
                    Ok(commands::PreservedMergeOutcome::Conflict { files }) => {
                        Ok(MergeAttempt::Conflict { files })
                    }
                    Err(error) => Err(WorktreeOpError::Internal(format!(
                        "base merge failed: {error}"
                    ))),
                }
            }
        }
    }

    async fn run_on_merged(&self, change_id: &str, worktree_path: &Path) -> WorktreeOpResult<()> {
        let hooks_config = self.config.get_hooks();
        let hooks = match &self.hook_events {
            Some(tx) => crate::hooks::HookRunner::with_event_tx(
                hooks_config,
                self.repo_root.clone(),
                tx.clone(),
            ),
            None => crate::hooks::HookRunner::new(hooks_config, self.repo_root.clone()),
        };

        // Read from the backend's own repository rather than the process CWD, so
        // hook context is correct for whichever repository this backend serves.
        let (completed_tasks, total_tasks) =
            match crate::openspec::list_changes_native_from(&self.repo_root) {
                Ok(changes) => changes
                    .iter()
                    .find(|c| c.id == change_id)
                    .map(|c| (c.completed_tasks, c.total_tasks))
                    .unwrap_or((0, 0)),
                Err(error) => {
                    warn!("failed to fetch task counts for on_merged hook: {error}");
                    (0, 0)
                }
            };

        let context = crate::hooks::HookContext::new(0, 0, 0, false)
            .with_change(change_id, completed_tasks, total_tasks)
            .with_apply_count(0)
            .with_parallel_context(&worktree_path.to_string_lossy(), None);

        hooks
            .run_hook(crate::hooks::HookType::OnMerged, &context)
            .await
            .map_err(|e| WorktreeOpError::Internal(e.to_string()))
    }

    async fn change_is_eligible(&self, change_id: &str) -> WorktreeOpResult<()> {
        // `list_changes_native_from` skips `archive/` and anything without a
        // proposal, so presence in this list *is* "managed and not archived".
        let changes = crate::openspec::list_changes_native_from(&self.repo_root).map_err(|e| {
            WorktreeOpError::Internal(format!("failed to enumerate managed changes: {e}"))
        })?;
        if changes.iter().any(|change| change.id == change_id) {
            Ok(())
        } else {
            Err(WorktreeOpError::NotFound(format!(
                "'{change_id}' is not a managed, non-archived change eligible for a worktree"
            )))
        }
    }
}
