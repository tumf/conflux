//! Startup wiring for the opt-in upstream integration option.
//!
//! Two entry points exist, and both run before any workspace mutation:
//!
//! - [`prepare_upstream_integration`] validates an enabled invocation. Dry-run
//!   stops after the static, offline checks; a real run additionally performs the
//!   initial fetch and validates the first-parent integration spine.
//! - [`ensure_no_unpushed_upstream_recovery`] refuses an option-less cumulative
//!   parallel run while trailer-identified upstream recovery evidence exists. It
//!   is offline by construction, so the default-off path gains no fetch.

use std::fmt;
use std::path::Path;

use super::coordinator::{
    publication_recovery_refusal, scan_pending_publications, scan_unpushed_upstream_merges,
    upstream_recovery_refusal, validate_initial_fetch,
};
use super::git_ops::GitUpstreamOps;
use super::options::{
    validate_static_preconditions, StaticPreconditions, UpstreamIntegrationConfig,
    UpstreamOptionError,
};
use super::ports::{UpstreamGit, UpstreamPortError};

/// Validated invocation-scoped runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamRuntime {
    pub config: UpstreamIntegrationConfig,
    /// Cumulative base branch; also the selected remote's branch name.
    pub branch: String,
}

/// Startup rejection, either an invalid invocation or an unusable boundary.
#[derive(Debug)]
pub enum UpstreamStartupError {
    Invalid(UpstreamOptionError),
    Port(UpstreamPortError),
}

impl fmt::Display for UpstreamStartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(err) => write!(f, "{}", err),
            Self::Port(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for UpstreamStartupError {}

impl From<UpstreamOptionError> for UpstreamStartupError {
    fn from(err: UpstreamOptionError) -> Self {
        Self::Invalid(err)
    }
}

impl From<UpstreamPortError> for UpstreamStartupError {
    fn from(err: UpstreamPortError) -> Self {
        Self::Port(err)
    }
}

/// Observed repository facts required by the static precondition check.
async fn observe_static_preconditions(
    git: &dyn UpstreamGit,
    config: &UpstreamIntegrationConfig,
    parallel: bool,
    push_remote: Option<String>,
    is_git_repository: bool,
) -> Result<StaticPreconditions, UpstreamPortError> {
    // Ordering matters: repository-dependent observations are skipped entirely
    // when the workspace is not a Git repository.
    if !is_git_repository {
        return Ok(StaticPreconditions {
            parallel,
            push_remote,
            is_git_repository: false,
            attached_branch: None,
            remote_configured: false,
            base_dirty_reason: None,
        });
    }

    let attached_branch = git.current_branch().await?;
    let remote_configured = git.remote_configured(&config.remote).await?;
    let base_dirty_reason = if !git.is_working_tree_clean().await? {
        Some("cumulative base has uncommitted changes".to_string())
    } else {
        let state = git.merge_repository_state().await?;
        (state.merge_head_present || state.has_unmerged_entries)
            .then(|| "cumulative base has an unfinished merge".to_string())
    };

    Ok(StaticPreconditions {
        parallel,
        push_remote,
        is_git_repository: true,
        attached_branch,
        remote_configured,
        base_dirty_reason,
    })
}

/// Validate an enabled upstream invocation before any workspace mutation.
///
/// `dry_run` stops after the static, offline checks, so remote branch existence
/// and initial-fetch ancestry are intentionally deferred to a real run.
pub async fn prepare_upstream_integration(
    config: UpstreamIntegrationConfig,
    repo_root: &Path,
    parallel: bool,
    push_remote: Option<String>,
    is_git_repository: bool,
    dry_run: bool,
) -> Result<UpstreamRuntime, UpstreamStartupError> {
    let git = GitUpstreamOps::new(repo_root);
    let facts =
        observe_static_preconditions(&git, &config, parallel, push_remote, is_git_repository)
            .await?;
    let branch = validate_static_preconditions(&config, &facts)?;

    if dry_run {
        // No network fetch, merge, resolve command, verification command, or push.
        return Ok(UpstreamRuntime { config, branch });
    }

    match validate_initial_fetch(&git, &config.remote, &branch).await? {
        Ok(_validation) => Ok(UpstreamRuntime { config, branch }),
        Err(err) => Err(UpstreamStartupError::Invalid(err)),
    }
}

/// Refuse an option-less cumulative parallel run while opted-in publication work
/// is reachable from cumulative HEAD.
///
/// Two kinds of evidence block an option-less start, and both are offline: the
/// trailers name the remote and branch, and reachability is checked against the
/// local remote-tracking ref.
///
/// 1. A publication-required integration marker. This is checked first because
///    it is the case that would otherwise be *misreported* as terminal `merged`:
///    the change is locally integrated but still owes a confirmed push.
/// 2. An unpushed Conflux upstream merge, as before.
pub async fn ensure_no_unpushed_upstream_recovery(
    repo_root: &Path,
) -> Result<(), UpstreamStartupError> {
    let git = GitUpstreamOps::new(repo_root);

    let pending_publications = scan_pending_publications(&git).await?;
    if let Some(err) = publication_recovery_refusal(&pending_publications) {
        return Err(UpstreamStartupError::Invalid(err));
    }

    let evidence = scan_unpushed_upstream_merges(&git).await?;
    match upstream_recovery_refusal(&evidence) {
        Some(err) => Err(UpstreamStartupError::Invalid(err)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::classify::MergeRepositoryState;
    use crate::upstream::ports::PortResult;
    use crate::upstream::spine::SpineCommit;
    use async_trait::async_trait;

    /// Minimal double covering only the observations the static check uses.
    struct StaticFactsGit {
        branch: Option<String>,
        remote_configured: bool,
        clean: bool,
        merge_state: MergeRepositoryState,
    }

    #[async_trait]
    impl UpstreamGit for StaticFactsGit {
        async fn remote_configured(&self, _remote: &str) -> PortResult<bool> {
            Ok(self.remote_configured)
        }
        async fn current_branch(&self) -> PortResult<Option<String>> {
            Ok(self.branch.clone())
        }
        async fn fetch(&self, _r: &str, _b: &str) -> PortResult<()> {
            unreachable!("static precondition observation must not fetch")
        }
        async fn fetched_sha(&self, _r: &str, _b: &str) -> PortResult<Option<String>> {
            unreachable!("static precondition observation must not resolve remote refs")
        }
        async fn head_sha(&self) -> PortResult<String> {
            Ok("head".into())
        }
        async fn is_ancestor(&self, _a: &str, _d: &str) -> PortResult<bool> {
            Ok(true)
        }
        async fn merge_base(&self, _a: &str, _b: &str) -> PortResult<String> {
            Ok("base".into())
        }
        async fn merge_no_ff(
            &self,
            _sha: &str,
            _message: &str,
        ) -> PortResult<crate::upstream::ports::MergeCommandResult> {
            unreachable!("static precondition observation must not merge")
        }
        async fn commit_empty(&self, _message: &str) -> PortResult<String> {
            unreachable!("static precondition observation must not commit")
        }
        async fn merge_repository_state(&self) -> PortResult<MergeRepositoryState> {
            Ok(self.merge_state)
        }
        async fn is_working_tree_clean(&self) -> PortResult<bool> {
            Ok(self.clean)
        }
        async fn status_porcelain_v2(&self) -> PortResult<String> {
            Ok(String::new())
        }
        async fn commit_message(&self, _sha: &str) -> PortResult<String> {
            Ok(String::new())
        }
        async fn commit_parents(&self, _sha: &str) -> PortResult<Vec<String>> {
            Ok(Vec::new())
        }
        async fn first_parent_commits(
            &self,
            _from: Option<&str>,
            _to: &str,
            _limit: Option<usize>,
        ) -> PortResult<Vec<SpineCommit>> {
            Ok(Vec::new())
        }
        async fn local_ref_sha(&self, _reference: &str) -> PortResult<Option<String>> {
            Ok(None)
        }
        async fn push_porcelain(
            &self,
            _r: &str,
            _b: &str,
        ) -> PortResult<crate::upstream::ports::PushCommandResult> {
            unreachable!("static precondition observation must not push")
        }
        async fn ls_remote_sha(&self, _r: &str, _b: &str) -> PortResult<Option<String>> {
            unreachable!("static precondition observation must not reach the network")
        }
    }

    fn git(clean: bool, merge_state: MergeRepositoryState) -> StaticFactsGit {
        StaticFactsGit {
            branch: Some("main".into()),
            remote_configured: true,
            clean,
            merge_state,
        }
    }

    #[tokio::test]
    async fn upstream_integration_observes_static_facts_without_network() {
        let config = UpstreamIntegrationConfig::new("origin", "cargo test");
        let facts = observe_static_preconditions(
            &git(true, MergeRepositoryState::default()),
            &config,
            true,
            None,
            true,
        )
        .await
        .unwrap();
        assert_eq!(
            validate_static_preconditions(&config, &facts).unwrap(),
            "main"
        );
    }

    #[tokio::test]
    async fn upstream_integration_reports_dirty_base_and_unfinished_merge() {
        let config = UpstreamIntegrationConfig::new("origin", "cargo test");

        let dirty = observe_static_preconditions(
            &git(false, MergeRepositoryState::default()),
            &config,
            true,
            None,
            true,
        )
        .await
        .unwrap();
        assert!(dirty
            .base_dirty_reason
            .as_deref()
            .unwrap()
            .contains("uncommitted"));

        let unfinished = observe_static_preconditions(
            &git(
                true,
                MergeRepositoryState {
                    merge_head_present: true,
                    has_unmerged_entries: false,
                },
            ),
            &config,
            true,
            None,
            true,
        )
        .await
        .unwrap();
        assert!(unfinished
            .base_dirty_reason
            .as_deref()
            .unwrap()
            .contains("unfinished merge"));
    }

    #[tokio::test]
    async fn upstream_integration_skips_repository_observation_for_non_git_workspace() {
        let config = UpstreamIntegrationConfig::new("origin", "cargo test");
        let facts = observe_static_preconditions(
            &git(true, MergeRepositoryState::default()),
            &config,
            true,
            None,
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            validate_static_preconditions(&config, &facts),
            Err(UpstreamOptionError::NotGitRepository)
        );
    }
}
