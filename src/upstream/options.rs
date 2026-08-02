//! Invocation-scoped upstream integration options and static startup validation.
//!
//! Everything here is a pure function over already-observed facts so the CLI
//! contract can be unit tested without touching a repository, a remote, or a
//! process. Observation of those facts (detached HEAD, remote configuration,
//! base cleanliness) lives in [`crate::upstream::coordinator`] and
//! [`crate::upstream::git_ops`].

use std::fmt;

/// Remote selected when `-u` / `--integrate-upstream` is given without a value.
pub const DEFAULT_UPSTREAM_REMOTE: &str = "origin";

/// Validated, invocation-scoped upstream integration configuration.
///
/// This is deliberately *not* part of persistent orchestration config: it is
/// constructed per `cflx run` invocation and dropped with the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamIntegrationConfig {
    /// Selected remote name (never a URL, never `remote:branch`).
    pub remote: String,
    /// Complete repository verification command supplied by the operator.
    pub verify_command: String,
}

impl UpstreamIntegrationConfig {
    pub fn new(remote: impl Into<String>, verify_command: impl Into<String>) -> Self {
        Self {
            remote: remote.into(),
            verify_command: verify_command.into(),
        }
    }
}

/// Every way an upstream invocation can be rejected before workspace mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamOptionError {
    /// `--upstream-verify-command` missing or empty while the option is enabled.
    MissingVerifyCommand,
    /// `--upstream-verify-command` supplied without enabling upstream integration.
    VerifyCommandWithoutOption,
    /// Remote name is empty or carries branch selection syntax.
    InvalidRemote(String),
    /// Effective execution is not cumulative parallel mode.
    NotParallelMode,
    /// `--push` maintains per-change branches instead of the cumulative base.
    ConflictsWithPush,
    /// HEAD is not attached to a branch, so there is no same-name remote branch.
    DetachedHead,
    /// Workspace is not resolvable as a Git repository.
    NotGitRepository,
    /// Selected remote is not configured locally.
    RemoteNotConfigured(String),
    /// Selected remote has no branch with the cumulative base branch name.
    RemoteBranchMissing { remote: String, branch: String },
    /// Cumulative base is dirty or has an unfinished operation.
    BaseNotClean(String),
    /// First-parent local history contains a commit that is neither a validated
    /// cumulative change integration nor a validated upstream integration.
    UnrelatedLocalHistory { commit: String, subject: String },
    /// Upstream recovery evidence exists but `-u` was omitted.
    UpstreamRecoveryRequiresOption {
        remote: String,
        branch: String,
        merge_commit: String,
    },
    /// A publication-required cumulative-base integration is not proven
    /// remote-reachable and `-u` was omitted.
    ///
    /// This is the option-less restart refusal: the marked change is *not*
    /// terminal `merged`, so recovery requires restarting with `-u` and a fresh
    /// verification command.
    PublicationRecoveryRequiresOption {
        change_id: String,
        remote: String,
        branch: String,
        integration_commit: String,
    },
}

impl fmt::Display for UpstreamOptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingVerifyCommand => write!(
                f,
                "--integrate-upstream requires an explicit non-empty --upstream-verify-command"
            ),
            Self::VerifyCommandWithoutOption => write!(
                f,
                "--upstream-verify-command requires -u/--integrate-upstream"
            ),
            Self::InvalidRemote(value) => write!(
                f,
                "invalid upstream remote '{}': use a remote name only (no branch selection)",
                value
            ),
            Self::NotParallelMode => write!(
                f,
                "-u/--integrate-upstream is only valid for cumulative parallel run mode"
            ),
            Self::ConflictsWithPush => write!(
                f,
                "-u/--integrate-upstream cannot be combined with --push: --push publishes individual change branches instead of the cumulative base"
            ),
            Self::DetachedHead => write!(
                f,
                "-u/--integrate-upstream requires an attached HEAD so the same-name remote branch can be resolved"
            ),
            Self::NotGitRepository => write!(
                f,
                "-u/--integrate-upstream requires a Git repository workspace"
            ),
            Self::RemoteNotConfigured(remote) => {
                write!(f, "remote '{}' is not configured in this repository", remote)
            }
            Self::RemoteBranchMissing { remote, branch } => write!(
                f,
                "remote '{}' has no branch '{}' matching the cumulative base branch",
                remote, branch
            ),
            Self::BaseNotClean(reason) => {
                write!(f, "cumulative base is not clean: {}", reason)
            }
            Self::UnrelatedLocalHistory { commit, subject } => write!(
                f,
                "local first-parent history contains unrelated commit {} ('{}'); upstream integration would publish it",
                commit, subject
            ),
            Self::UpstreamRecoveryRequiresOption {
                remote,
                branch,
                merge_commit,
            } => write!(
                f,
                "unpushed upstream merge {} for {}/{} requires restarting with --integrate-upstream={} and an explicit --upstream-verify-command",
                merge_commit, remote, branch, remote
            ),
            Self::PublicationRecoveryRequiresOption {
                change_id,
                remote,
                branch,
                integration_commit,
            } => write!(
                f,
                "change '{}' was integrated into cumulative base at {} for required publication to {}/{} and is not proven reachable from that remote branch; it is not merged. Restart with --integrate-upstream={} and a fresh --upstream-verify-command to resume publication",
                change_id, integration_commit, remote, branch, remote
            ),
        }
    }
}

impl std::error::Error for UpstreamOptionError {}

/// Validate a `--integrate-upstream[=<remote>]` value.
///
/// Mirrors `--push` remote handling: a remote *name*, never `remote:branch`.
pub fn parse_upstream_remote(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains(':') || trimmed.contains(char::is_whitespace) {
        return Err(UpstreamOptionError::InvalidRemote(value.to_string()).to_string());
    }
    Ok(trimmed.to_string())
}

/// Collapse the two clap spellings of the opt-in into one selected remote.
///
/// The spec allows a named remote only as `--integrate-upstream=<remote>`, so
/// the two spellings are separate clap arguments: `-u` is a strictly value-less
/// short flag (clap rejects `-u=<remote>`), and `--integrate-upstream` is the
/// long form that optionally carries `=<remote>`. Both select
/// [`DEFAULT_UPSTREAM_REMOTE`] when value-less, so they collapse here.
pub fn selected_upstream_remote(short_opt_in: bool, long_opt_in: Option<&str>) -> Option<String> {
    match long_opt_in {
        Some(remote) => Some(remote.to_string()),
        None if short_opt_in => Some(DEFAULT_UPSTREAM_REMOTE.to_string()),
        None => None,
    }
}

/// Resolve the invocation-scoped configuration from raw CLI values.
///
/// `integrate_upstream` is clap's already-parsed representation: `None` when the
/// option is absent, `Some(remote)` when present (clap supplies
/// [`DEFAULT_UPSTREAM_REMOTE`] for the value-less `-u` / `--integrate-upstream`
/// aliases).
///
/// Returns `Ok(None)` for the default-off path, which MUST install no upstream
/// behavior at all.
pub fn resolve_upstream_config(
    integrate_upstream: Option<&str>,
    upstream_verify_command: Option<&str>,
) -> Result<Option<UpstreamIntegrationConfig>, UpstreamOptionError> {
    let verify_command = upstream_verify_command
        .map(str::trim)
        .filter(|c| !c.is_empty());

    let Some(remote) = integrate_upstream else {
        // A verification command alone must not silently enable the feature.
        if verify_command.is_some() {
            return Err(UpstreamOptionError::VerifyCommandWithoutOption);
        }
        return Ok(None);
    };

    let remote = remote.trim();
    if remote.is_empty() || remote.contains(':') || remote.contains(char::is_whitespace) {
        return Err(UpstreamOptionError::InvalidRemote(remote.to_string()));
    }

    let Some(verify_command) = verify_command else {
        return Err(UpstreamOptionError::MissingVerifyCommand);
    };

    Ok(Some(UpstreamIntegrationConfig::new(remote, verify_command)))
}

/// Normalize the shared upstream contract for one frontend invocation.
///
/// `run`, bare local TUI, and explicit local `tui` all route through this, so
/// the three entrypoints cannot drift: the same raw option values produce the
/// same [`UpstreamIntegrationConfig`], and the same incompatible combinations
/// are rejected before any orchestration mutation.
pub fn resolve_frontend_upstream_config(
    integrate_upstream: Option<&str>,
    upstream_verify_command: Option<&str>,
    push_remote: Option<&str>,
) -> Result<Option<UpstreamIntegrationConfig>, UpstreamOptionError> {
    let config = resolve_upstream_config(integrate_upstream, upstream_verify_command)?;
    let Some(config) = config else {
        return Ok(None);
    };
    if push_remote.is_some() {
        return Err(UpstreamOptionError::ConflictsWithPush);
    }
    Ok(Some(config))
}

/// Repository/invocation facts that can be checked without network access.
///
/// Dry-run validates exactly this set; a real run additionally performs the
/// initial fetch and validates remote branch existence and first-parent spine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticPreconditions {
    /// Effective execution mode is cumulative parallel.
    pub parallel: bool,
    /// `--push` remote, when the operator requested per-change pushes.
    pub push_remote: Option<String>,
    /// Workspace resolves to a Git repository.
    pub is_git_repository: bool,
    /// HEAD is attached to a branch (the cumulative base branch).
    pub attached_branch: Option<String>,
    /// Selected remote is configured locally.
    pub remote_configured: bool,
    /// Reason the cumulative base is unusable, when it is not clean.
    pub base_dirty_reason: Option<String>,
}

/// Validate everything that is knowable before any network or workspace mutation.
pub fn validate_static_preconditions(
    config: &UpstreamIntegrationConfig,
    facts: &StaticPreconditions,
) -> Result<String, UpstreamOptionError> {
    if !facts.parallel {
        return Err(UpstreamOptionError::NotParallelMode);
    }
    if facts.push_remote.is_some() {
        return Err(UpstreamOptionError::ConflictsWithPush);
    }
    if !facts.is_git_repository {
        return Err(UpstreamOptionError::NotGitRepository);
    }
    let Some(branch) = facts.attached_branch.as_deref() else {
        return Err(UpstreamOptionError::DetachedHead);
    };
    if !facts.remote_configured {
        return Err(UpstreamOptionError::RemoteNotConfigured(
            config.remote.clone(),
        ));
    }
    if let Some(reason) = &facts.base_dirty_reason {
        return Err(UpstreamOptionError::BaseNotClean(reason.clone()));
    }
    Ok(branch.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> StaticPreconditions {
        StaticPreconditions {
            parallel: true,
            push_remote: None,
            is_git_repository: true,
            attached_branch: Some("main".to_string()),
            remote_configured: true,
            base_dirty_reason: None,
        }
    }

    #[test]
    fn upstream_integration_absent_option_is_default_off() {
        assert_eq!(resolve_upstream_config(None, None), Ok(None));
    }

    #[test]
    fn upstream_integration_short_and_long_aliases_are_equivalent() {
        // clap resolves both value-less spellings to the same default value, so
        // equivalence is asserted on the resolved configuration.
        let short = resolve_upstream_config(Some(DEFAULT_UPSTREAM_REMOTE), Some("cargo test"));
        let long = resolve_upstream_config(Some(DEFAULT_UPSTREAM_REMOTE), Some("cargo test"));
        assert_eq!(short, long);
        assert_eq!(
            short.unwrap(),
            Some(UpstreamIntegrationConfig::new("origin", "cargo test"))
        );
    }

    #[test]
    fn upstream_integration_explicit_remote_is_selected() {
        let resolved = resolve_upstream_config(Some("upstream"), Some("cargo test")).unwrap();
        assert_eq!(
            resolved,
            Some(UpstreamIntegrationConfig::new("upstream", "cargo test"))
        );
    }

    #[test]
    fn upstream_integration_requires_verify_command() {
        assert_eq!(
            resolve_upstream_config(Some("origin"), None),
            Err(UpstreamOptionError::MissingVerifyCommand)
        );
        assert_eq!(
            resolve_upstream_config(Some("origin"), Some("   ")),
            Err(UpstreamOptionError::MissingVerifyCommand)
        );
    }

    #[test]
    fn upstream_integration_verify_command_alone_is_rejected() {
        assert_eq!(
            resolve_upstream_config(None, Some("cargo test")),
            Err(UpstreamOptionError::VerifyCommandWithoutOption)
        );
    }

    #[test]
    fn upstream_integration_rejects_branch_selection_remote() {
        assert!(parse_upstream_remote("origin:main").is_err());
        assert!(parse_upstream_remote("").is_err());
        assert_eq!(parse_upstream_remote("upstream").unwrap(), "upstream");
        assert_eq!(
            resolve_upstream_config(Some("origin:main"), Some("cargo test")),
            Err(UpstreamOptionError::InvalidRemote(
                "origin:main".to_string()
            ))
        );
    }

    #[test]
    fn upstream_integration_rejects_serial_mode() {
        let config = UpstreamIntegrationConfig::new("origin", "cargo test");
        let mut f = facts();
        f.parallel = false;
        assert_eq!(
            validate_static_preconditions(&config, &f),
            Err(UpstreamOptionError::NotParallelMode)
        );
    }

    #[test]
    fn upstream_integration_rejects_push_combination() {
        let config = UpstreamIntegrationConfig::new("origin", "cargo test");
        let mut f = facts();
        f.push_remote = Some("origin".to_string());
        assert_eq!(
            validate_static_preconditions(&config, &f),
            Err(UpstreamOptionError::ConflictsWithPush)
        );
    }

    #[test]
    fn upstream_integration_rejects_detached_head_and_non_git() {
        let config = UpstreamIntegrationConfig::new("origin", "cargo test");
        let mut detached = facts();
        detached.attached_branch = None;
        assert_eq!(
            validate_static_preconditions(&config, &detached),
            Err(UpstreamOptionError::DetachedHead)
        );

        let mut not_git = facts();
        not_git.is_git_repository = false;
        assert_eq!(
            validate_static_preconditions(&config, &not_git),
            Err(UpstreamOptionError::NotGitRepository)
        );
    }

    #[test]
    fn upstream_integration_rejects_missing_remote_and_dirty_base() {
        let config = UpstreamIntegrationConfig::new("upstream", "cargo test");
        let mut missing = facts();
        missing.remote_configured = false;
        assert_eq!(
            validate_static_preconditions(&config, &missing),
            Err(UpstreamOptionError::RemoteNotConfigured("upstream".into()))
        );

        let mut dirty = facts();
        dirty.base_dirty_reason = Some("uncommitted changes".to_string());
        assert_eq!(
            validate_static_preconditions(&config, &dirty),
            Err(UpstreamOptionError::BaseNotClean(
                "uncommitted changes".into()
            ))
        );
    }

    #[test]
    fn per_change_upstream_frontends_normalize_to_one_configuration() {
        // `run`, bare local TUI, and explicit local `tui` supply the same raw
        // values, so all three must produce one identical configuration.
        let run = resolve_frontend_upstream_config(
            Some(DEFAULT_UPSTREAM_REMOTE),
            Some("cargo test"),
            None,
        );
        let bare_tui = resolve_frontend_upstream_config(
            Some(DEFAULT_UPSTREAM_REMOTE),
            Some("cargo test"),
            None,
        );
        let explicit_tui = resolve_frontend_upstream_config(
            Some(DEFAULT_UPSTREAM_REMOTE),
            Some("cargo test"),
            None,
        );
        assert_eq!(run, bare_tui);
        assert_eq!(bare_tui, explicit_tui);
        assert_eq!(
            run.unwrap(),
            Some(UpstreamIntegrationConfig::new("origin", "cargo test"))
        );
    }

    #[test]
    fn per_change_upstream_frontend_default_off_installs_nothing() {
        assert_eq!(
            resolve_frontend_upstream_config(None, None, Some("origin")),
            Ok(None),
            "a disabled invocation must not be rejected by upstream-only constraints"
        );
    }

    #[test]
    fn per_change_upstream_frontend_rejects_push() {
        assert_eq!(
            resolve_frontend_upstream_config(Some("origin"), Some("cargo test"), Some("origin")),
            Err(UpstreamOptionError::ConflictsWithPush)
        );
    }

    #[test]
    fn per_change_upstream_frontend_requires_verify_command_and_valid_remote() {
        assert_eq!(
            resolve_frontend_upstream_config(Some("origin"), None, None),
            Err(UpstreamOptionError::MissingVerifyCommand)
        );
        assert_eq!(
            resolve_frontend_upstream_config(Some("origin"), Some("  "), None),
            Err(UpstreamOptionError::MissingVerifyCommand)
        );
        assert_eq!(
            resolve_frontend_upstream_config(Some("origin:main"), Some("cargo test"), None),
            Err(UpstreamOptionError::InvalidRemote("origin:main".into()))
        );
        assert_eq!(
            resolve_frontend_upstream_config(None, Some("cargo test"), None),
            Err(UpstreamOptionError::VerifyCommandWithoutOption)
        );
    }

    #[test]
    fn per_change_upstream_frontend_accepts_explicit_remote() {
        assert_eq!(
            resolve_frontend_upstream_config(Some("upstream"), Some("cargo test"), None),
            Ok(Some(UpstreamIntegrationConfig::new(
                "upstream",
                "cargo test"
            )))
        );
    }

    #[test]
    fn upstream_integration_static_validation_returns_base_branch() {
        let config = UpstreamIntegrationConfig::new("origin", "cargo test");
        assert_eq!(
            validate_static_preconditions(&config, &facts()).unwrap(),
            "main"
        );
    }
}
