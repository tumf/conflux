//! Shared dependency classification context for the parallel scheduler.
//!
//! The scheduler has two dependency-sensitive phases: blocked-only analysis gating and
//! dispatch selection. This context centralizes the repository/reducer evidence used by
//! both phases so dependency target semantics cannot drift between them.

use std::collections::HashSet;
use std::path::PathBuf;

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::dependency_targets::{
    classify_dependency_target, collect_active_change_ids, collect_archived_change_ids,
    collect_rejected_change_ids, DependencyTargetClass,
};
use crate::error::{OrchestratorError, Result};
use crate::orchestration::state::OrchestratorState;
use crate::vcs::WorkspaceManager;

use super::{DependencyBlockerFingerprint, ParallelExecutor};

#[derive(Debug, Clone)]
pub(super) struct DependencyContext {
    repo_root: PathBuf,
    queued_ids: HashSet<String>,
    in_flight_ids: HashSet<String>,
    active_ids: HashSet<String>,
    archived_ids: HashSet<String>,
    rejected_ids: HashSet<String>,
    terminal_error_ids: HashSet<String>,
    effective_dependency_base: Option<String>,
}

impl DependencyContext {
    pub(super) fn from_executor(
        executor: &ParallelExecutor,
        queued_ids: impl IntoIterator<Item = impl AsRef<str>>,
        in_flight: &HashSet<String>,
    ) -> Self {
        Self::from_parts(
            executor.repo_root.clone(),
            queued_ids,
            in_flight,
            executor.shared_orchestrator_state.as_ref(),
        )
    }

    fn from_parts(
        repo_root: PathBuf,
        queued_ids: impl IntoIterator<Item = impl AsRef<str>>,
        in_flight: &HashSet<String>,
        shared_orchestrator_state: Option<&std::sync::Arc<RwLock<OrchestratorState>>>,
    ) -> Self {
        let terminal_error_ids = shared_orchestrator_state
            .and_then(|state| state.try_read().ok())
            .map(|state| {
                state
                    .initial_change_ids()
                    .iter()
                    .filter(|id| state.is_terminal_error_change(id))
                    .cloned()
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();

        let queued_ids = queued_ids
            .into_iter()
            .map(|id| id.as_ref().to_string())
            .collect::<HashSet<_>>();
        let in_flight_ids = in_flight.iter().cloned().collect::<HashSet<_>>();
        let active_ids = collect_active_change_ids(&repo_root);
        let archived_ids = collect_archived_change_ids(&repo_root);
        let rejected_ids = collect_rejected_change_ids(&repo_root);

        debug!(
            queued = queued_ids.len(),
            in_flight = in_flight_ids.len(),
            active = active_ids.len(),
            archived = archived_ids.len(),
            rejected = rejected_ids.len(),
            terminal_error = terminal_error_ids.len(),
            "Built dependency classification context"
        );

        Self {
            repo_root,
            queued_ids,
            in_flight_ids,
            active_ids,
            archived_ids,
            rejected_ids,
            terminal_error_ids,
            effective_dependency_base: None,
        }
    }

    pub(super) fn classify(&self, dep_id: &str) -> DependencyTargetClass {
        let class = classify_dependency_target(
            dep_id,
            self.queued_ids.iter().map(String::as_str),
            self.in_flight_ids.iter().map(String::as_str),
            self.active_ids.iter().map(String::as_str),
            &self.archived_ids,
            &self.rejected_ids,
        );

        if matches!(
            class,
            DependencyTargetClass::Missing | DependencyTargetClass::Rejected
        ) {
            return class;
        }

        if self.terminal_error_ids.contains(dep_id) {
            DependencyTargetClass::Error
        } else {
            class
        }
    }

    pub(super) fn is_terminal_error_change(&self, change_id: &str) -> bool {
        self.terminal_error_ids.contains(change_id)
    }

    pub(super) fn is_blocked(
        &self,
        dependencies: &[String],
    ) -> Option<DependencyBlockerFingerprint> {
        let blockers = dependencies
            .iter()
            .filter_map(|dep_id| {
                let class = self.classify(dep_id);
                if matches!(class, DependencyTargetClass::Archived) {
                    None
                } else {
                    Some((dep_id.clone(), class.as_str().to_string()))
                }
            })
            .collect::<Vec<_>>();

        if blockers.is_empty() {
            None
        } else {
            Some(blockers)
        }
    }

    pub(super) async fn effective_dependency_base(
        &mut self,
        workspace_manager: &dyn WorkspaceManager,
    ) -> Result<&str> {
        if self.effective_dependency_base.is_none() {
            let original_branch = workspace_manager
                .ensure_original_branch_initialized()
                .await
                .map_err(OrchestratorError::from_vcs_error)?;

            let effective_base = match crate::vcs::git::commands::get_current_branch(
                &self.repo_root,
            )
            .await
            {
                Ok(Some(current_branch)) if current_branch != original_branch => {
                    debug!(
                        original_branch = %original_branch,
                        effective_dependency_base = %current_branch,
                        "Using current integration branch as effective dependency base"
                    );
                    current_branch
                }
                Ok(Some(_)) | Ok(None) => original_branch,
                Err(err) => {
                    warn!(
                        error = %err,
                        original_branch = %original_branch,
                        "Failed to determine current branch for dependency base; using original branch"
                    );
                    original_branch
                }
            };

            self.effective_dependency_base = Some(effective_base);
        }

        Ok(self
            .effective_dependency_base
            .as_deref()
            .expect("effective dependency base initialized above"))
    }

    pub(super) async fn is_dependency_resolved_with_base(
        &mut self,
        dep_id: &str,
        workspace_manager: &dyn WorkspaceManager,
    ) -> Result<(bool, String)> {
        let effective_base = self
            .effective_dependency_base(workspace_manager)
            .await?
            .to_string();

        match crate::execution::state::is_merged_to_base(dep_id, &self.repo_root, &effective_base)
            .await
        {
            Ok(is_merged) => Ok((is_merged, effective_base)),
            Err(e) => {
                warn!(
                    dependency = %dep_id,
                    effective_dependency_base = %effective_base,
                    error = %e,
                    "Failed to check if dependency is merged to effective base; assuming not resolved"
                );
                Ok((false, effective_base))
            }
        }
    }

    pub(super) fn blocker_fingerprint(
        change_id: &str,
        blockers: &[(String, DependencyTargetClass)],
    ) -> DependencyBlockerFingerprint {
        let mut fingerprint = blockers
            .iter()
            .map(|(dep_id, class)| (dep_id.clone(), class.as_str().to_string()))
            .collect::<Vec<_>>();
        fingerprint.sort();
        fingerprint.insert(0, ("change_id".to_string(), change_id.to_string()));
        fingerprint
    }

    pub(super) fn log_archived_dependency_check(change_id: &str, dep_id: &str) {
        debug!(
            change_id = %change_id,
            dependency = %dep_id,
            "Archived dependency evidence found; verifying base-branch merge before dispatch"
        );
    }

    pub(super) fn log_dependency_resolved(
        change_id: &str,
        dep_id: &str,
        class: DependencyTargetClass,
        effective_base: &str,
    ) {
        if matches!(class, DependencyTargetClass::Archived) {
            debug!(
                change_id = %change_id,
                dependency = %dep_id,
                effective_dependency_base = %effective_base,
                "Archived dependency is merged into effective dependency base"
            );
        }
    }

    pub(super) fn log_dependency_unresolved(
        change_id: &str,
        dep_id: &str,
        class: DependencyTargetClass,
        effective_base: &str,
    ) {
        if matches!(class, DependencyTargetClass::Archived) {
            info!(
                change_id = %change_id,
                dependency = %dep_id,
                effective_dependency_base = %effective_base,
                "Archived dependency is not merged into effective dependency base; dispatch remains blocked"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_change(root: &std::path::Path, id: &str) {
        let change_dir = root.join("openspec/changes").join(id);
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# Change\n").unwrap();
    }

    #[test]
    fn context_classifies_from_single_collected_evidence_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        write_change(temp_dir.path(), "active-a");
        let archive_dir = temp_dir
            .path()
            .join("openspec/changes/archive/2026-06-17-archived-a");
        std::fs::create_dir_all(&archive_dir).unwrap();
        std::fs::write(archive_dir.join("proposal.md"), "# Archived\n").unwrap();
        write_change(temp_dir.path(), "rejected-a");
        std::fs::write(
            temp_dir
                .path()
                .join("openspec/changes/rejected-a/REJECTED.md"),
            "# REJECTED\n",
        )
        .unwrap();

        let in_flight = HashSet::from(["flight-a".to_string()]);
        let context = DependencyContext::from_parts(
            temp_dir.path().to_path_buf(),
            ["queued-a"],
            &in_flight,
            None,
        );

        assert_eq!(context.classify("queued-a"), DependencyTargetClass::Queued);
        assert_eq!(
            context.classify("flight-a"),
            DependencyTargetClass::InFlight
        );
        assert_eq!(
            context.classify("active-a"),
            DependencyTargetClass::ActiveButNotQueued
        );
        assert_eq!(
            context.classify("archived-a"),
            DependencyTargetClass::Archived
        );
        assert_eq!(
            context.classify("rejected-a"),
            DependencyTargetClass::Rejected
        );
        assert_eq!(
            context.classify("missing-a"),
            DependencyTargetClass::Missing
        );
    }
}
