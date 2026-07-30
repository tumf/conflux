//! Conflux-owned upstream integration and finalization workflow.
//!
//! The coordinator is the single workflow controller for the opted-in upstream
//! lane. It performs fetch, ancestry classification, `--no-ff` merge, complete
//! verification, ancestry re-checks, native non-force push, and remote
//! confirmation itself. The bounded `resolve_command` agent is a repair worker
//! only: it is invoked exclusively through the textual and semantic repair
//! predicates, it never selects Git operations, and it never executes the push.
//!
//! Every decision is recomputed from repository evidence. Nothing in this module
//! treats a prior command's success, an emitted event, or a log line as durable
//! proof.

use std::sync::Arc;

use super::checkpoint::{
    BaseLaneState, CheckpointDecision, CheckpointScheduler, CheckpointTrigger,
};
use super::classify::{
    classify_merge_outcome, classify_push_confirmation, classify_push_failure,
    classify_upstream_revision, parse_porcelain_v2, parse_push_porcelain, MergeOutcomeClass,
    PushConfirmation, PushFailureClass, UpstreamRevisionClass,
};
use super::options::{UpstreamIntegrationConfig, UpstreamOptionError};
use super::ports::{
    PortResult, RepairCause, RepairRequest, UpstreamEvent, UpstreamGit, UpstreamObserver,
    UpstreamPortError, UpstreamRepairAgent, UpstreamVerifier,
};
use super::spine::{validate_spine, SpineValidation};
use super::trailers::{format_upstream_merge_message, parse_upstream_trailers, UpstreamTrailers};

/// Bounded number of pre-push race/repair cycles before the run stalls.
const MAX_FINALIZE_ATTEMPTS: u32 = 5;

/// Bounded first-parent walk for the offline recovery scan.
const RECOVERY_SCAN_LIMIT: usize = 500;

/// Explicit outcome of the parallel scheduler.
///
/// Only [`SchedulerOutcome::DrainedSuccessfully`] may enter finalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerOutcome {
    DrainedSuccessfully,
    BlockedOrStalled,
    Cancelled,
}

/// Result of one checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamStepOutcome {
    /// Ancestry proved the fetched revision is already integrated.
    NoOp { fetched_sha: String },
    /// The fetched revision was integrated and cumulative HEAD passed verification.
    Integrated { merge_sha: String },
    /// The checkpoint did not run because the base lane was unsafe or already owned.
    Deferred { reason: String },
    /// Repair or verification could not converge; the run is resumable.
    Stalled { reason: String },
}

/// Result of finalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizeOutcome {
    /// One successful native push, confirmed against the remote.
    Completed { pushed_head: String },
    /// Nothing recognized was unpushed; no verification, merge, or push ran.
    NoWork,
    /// A non-drained scheduler outcome; finalization was not entered.
    Skipped { reason: String },
    /// Finalization could not converge; resumable.
    Stalled { reason: String },
}

/// Unpushed upstream merge evidence recovered from the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamRecoveryEvidence {
    pub merge_sha: String,
    pub trailers: UpstreamTrailers,
}

/// Result of the real run's initial fetch and identity validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialValidation {
    pub branch: String,
    pub fetched_sha: String,
    pub spine: SpineValidation,
}

/// Bounded offline scan for unpushed trailer-identified upstream merges.
///
/// This runs with **no** network access and with no selected remote, so it is
/// usable from the default-off startup path: the trailers themselves name the
/// remote and branch, and reachability is checked against the local
/// remote-tracking ref. A merge whose remote-tracking ref is missing counts as
/// unpushed, because nothing locally proves it was published.
pub async fn scan_unpushed_upstream_merges(
    git: &dyn UpstreamGit,
) -> PortResult<Vec<UpstreamRecoveryEvidence>> {
    let head = git.head_sha().await?;
    let commits = git
        .first_parent_commits(None, &head, Some(RECOVERY_SCAN_LIMIT))
        .await?;

    let mut evidence = Vec::new();
    for commit in commits {
        // Identity requires a real merge commit plus a complete trailer set.
        if commit.parents.len() < 2 {
            continue;
        }
        let Some(trailers) = parse_upstream_trailers(&commit.message) else {
            continue;
        };
        if !commit.parents[1..].contains(&trailers.sha) {
            continue;
        }

        let tracking_ref = format!("refs/remotes/{}/{}", trailers.remote, trailers.branch);
        let incorporated = match git.local_ref_sha(&tracking_ref).await? {
            Some(remote_sha) => git.is_ancestor(&commit.sha, &remote_sha).await?,
            None => false,
        };
        if !incorporated {
            evidence.push(UpstreamRecoveryEvidence {
                merge_sha: commit.sha,
                trailers,
            });
        }
    }

    Ok(evidence)
}

/// Refuse an option-less cumulative parallel run while upstream recovery evidence exists.
///
/// The diagnostic names the trailer-associated remote so the operator can restart
/// with the same selection and a newly supplied verification command.
pub fn upstream_recovery_refusal(
    evidence: &[UpstreamRecoveryEvidence],
) -> Option<UpstreamOptionError> {
    evidence.first().map(
        |found| UpstreamOptionError::UpstreamRecoveryRequiresOption {
            remote: found.trailers.remote.clone(),
            branch: found.trailers.branch.clone(),
            merge_commit: found.merge_sha.clone(),
        },
    )
}

/// Perform the real run's initial fetch and first-parent identity validation.
///
/// Exposed as a free function so startup can validate with only the Git port,
/// before any coordinator (and therefore any verification or repair capability)
/// is constructed.
pub async fn validate_initial_fetch(
    git: &dyn UpstreamGit,
    remote: &str,
    branch: &str,
) -> PortResult<Result<InitialValidation, UpstreamOptionError>> {
    if !git.remote_configured(remote).await? {
        return Ok(Err(UpstreamOptionError::RemoteNotConfigured(
            remote.to_string(),
        )));
    }
    if git.current_branch().await?.is_none() {
        return Ok(Err(UpstreamOptionError::DetachedHead));
    }

    git.fetch(remote, branch).await?;
    let Some(fetched_sha) = git.fetched_sha(remote, branch).await? else {
        return Ok(Err(UpstreamOptionError::RemoteBranchMissing {
            remote: remote.to_string(),
            branch: branch.to_string(),
        }));
    };

    let head = git.head_sha().await?;
    let merge_base = git.merge_base(&fetched_sha, &head).await?;
    let commits = git
        .first_parent_commits(Some(&merge_base), &head, None)
        .await?;
    let spine = validate_spine(&commits, remote, branch);

    if let Some((commit, reason)) = spine.rejected.clone() {
        return Ok(Err(UpstreamOptionError::UnrelatedLocalHistory {
            commit,
            subject: reason,
        }));
    }

    Ok(Ok(InitialValidation {
        branch: branch.to_string(),
        fetched_sha,
        spine,
    }))
}

/// Conflux-owned upstream integration workflow controller.
pub struct UpstreamCoordinator {
    config: UpstreamIntegrationConfig,
    branch: String,
    git: Arc<dyn UpstreamGit>,
    verifier: Arc<dyn UpstreamVerifier>,
    repair_agent: Arc<dyn UpstreamRepairAgent>,
    observer: Arc<dyn UpstreamObserver>,
    scheduler: CheckpointScheduler,
    /// At most one successful push per run; never retried after confirmation.
    pushed_head: Option<String>,
}

impl UpstreamCoordinator {
    pub fn new(
        config: UpstreamIntegrationConfig,
        branch: impl Into<String>,
        git: Arc<dyn UpstreamGit>,
        verifier: Arc<dyn UpstreamVerifier>,
        repair_agent: Arc<dyn UpstreamRepairAgent>,
        observer: Arc<dyn UpstreamObserver>,
    ) -> Self {
        Self {
            config,
            branch: branch.into(),
            git,
            verifier,
            repair_agent,
            observer,
            scheduler: CheckpointScheduler::new(),
            pushed_head: None,
        }
    }

    pub fn config(&self) -> &UpstreamIntegrationConfig {
        &self.config
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn pushed_head(&self) -> Option<&str> {
        self.pushed_head.as_deref()
    }

    /// Batched completed results waiting behind the active checkpoint.
    pub fn queued_results(&self) -> Vec<String> {
        self.scheduler.queued_results()
    }

    // ── Startup ────────────────────────────────────────────────────────────

    /// Perform the real run's initial fetch and first-parent identity validation.
    ///
    /// This runs before any worktree dispatch or base mutation, so a rejected
    /// invocation never leaves the workspace changed.
    pub async fn validate_initial_fetch(
        &self,
    ) -> PortResult<Result<InitialValidation, UpstreamOptionError>> {
        validate_initial_fetch(self.git.as_ref(), &self.config.remote, &self.branch).await
    }

    // ── Checkpoints ────────────────────────────────────────────────────────

    /// Run a base-lane checkpoint at a deterministic boundary.
    ///
    /// A batched request shares the active checkpoint's single fetch and is
    /// reported as deferred to its caller; the owning checkpoint publishes the
    /// integration result.
    pub async fn checkpoint(
        &mut self,
        trigger: CheckpointTrigger,
        lane: &BaseLaneState,
        pending_result: Option<&str>,
    ) -> PortResult<UpstreamStepOutcome> {
        let generation = match self.scheduler.request(trigger, lane, pending_result) {
            CheckpointDecision::Start { generation } => generation,
            CheckpointDecision::Batched { generation } => {
                return Ok(UpstreamStepOutcome::Deferred {
                    reason: format!("batched behind active checkpoint {}", generation),
                });
            }
            CheckpointDecision::Deferred { reason } => {
                self.observer
                    .observe(UpstreamEvent::CheckpointDeferred {
                        reason: reason.clone(),
                    })
                    .await;
                return Ok(UpstreamStepOutcome::Deferred { reason });
            }
        };

        self.observer
            .observe(UpstreamEvent::CheckpointStarted {
                remote: self.config.remote.clone(),
                branch: self.branch.clone(),
                trigger: format!("{:?}", trigger),
            })
            .await;

        let outcome = self.run_checkpoint_body().await;
        self.scheduler.release(generation);
        outcome
    }

    async fn run_checkpoint_body(&mut self) -> PortResult<UpstreamStepOutcome> {
        let remote = self.config.remote.clone();
        let branch = self.branch.clone();

        self.git.fetch(&remote, &branch).await?;
        let Some(fetched_sha) = self.git.fetched_sha(&remote, &branch).await? else {
            return Ok(UpstreamStepOutcome::Stalled {
                reason: format!("remote '{}' has no branch '{}'", remote, branch),
            });
        };
        let local_before = self.git.head_sha().await?;

        self.observer
            .observe(UpstreamEvent::FetchCompleted {
                remote: remote.clone(),
                branch: branch.clone(),
                fetched_sha: fetched_sha.clone(),
                local_sha: local_before.clone(),
            })
            .await;

        let class = classify_upstream_revision(
            self.git.is_ancestor(&fetched_sha, &local_before).await?,
            self.git.is_ancestor(&local_before, &fetched_sha).await?,
        );

        if class == UpstreamRevisionClass::AlreadyIntegrated {
            // Ancestry-proven no-op: no merge, no agent, no reverification.
            self.observer
                .observe(UpstreamEvent::NoOp {
                    fetched_sha: fetched_sha.clone(),
                })
                .await;
            return Ok(UpstreamStepOutcome::NoOp { fetched_sha });
        }

        self.integrate_revision(&fetched_sha, &local_before).await
    }

    /// Integrate a fetched revision that is not contained in cumulative base.
    ///
    /// Both strictly remote-ahead and diverged histories take the same
    /// `--no-ff` path so the merge commit always exists as crash-recovery
    /// evidence.
    async fn integrate_revision(
        &mut self,
        fetched_sha: &str,
        local_before: &str,
    ) -> PortResult<UpstreamStepOutcome> {
        self.observer
            .observe(UpstreamEvent::IntegrationStarted {
                fetched_sha: fetched_sha.to_string(),
            })
            .await;

        let message = format_upstream_merge_message(&self.config.remote, &self.branch, fetched_sha);
        let result = self.git.merge_no_ff(fetched_sha, &message).await?;

        match classify_merge_outcome(result.exit_success, result.state) {
            MergeOutcomeClass::Completed => {}
            MergeOutcomeClass::Conflicted => {
                if let Some(stall) = self.run_textual_repair(fetched_sha, local_before).await? {
                    return Ok(stall);
                }
            }
            MergeOutcomeClass::CommandFailure => {
                // Repository state establishes no repairable merge; an agent must
                // not be started merely because output text resembles a conflict.
                let reason = format!(
                    "upstream merge of {} failed as a command error with no repairable merge state",
                    fetched_sha
                );
                self.observer
                    .observe(UpstreamEvent::Stalled {
                        reason: reason.clone(),
                    })
                    .await;
                return Ok(UpstreamStepOutcome::Stalled { reason });
            }
        }

        let merge_sha = self.git.head_sha().await?;
        if !self.validate_identity(&merge_sha, fetched_sha).await? {
            let reason = format!(
                "upstream merge {} does not carry valid identity trailers for {}/{}",
                merge_sha, self.config.remote, self.branch
            );
            return Ok(UpstreamStepOutcome::Stalled { reason });
        }

        self.observer
            .observe(UpstreamEvent::IntegrationCompleted {
                merge_sha: merge_sha.clone(),
            })
            .await;

        // Every actual upstream tree change runs the complete verification gate
        // before the base lane may reopen.
        match self
            .verify_with_semantic_repair(&merge_sha, fetched_sha, &merge_sha)
            .await?
        {
            None => Ok(UpstreamStepOutcome::Integrated { merge_sha }),
            Some(stall) => Ok(stall),
        }
    }

    /// Bounded textual repair. Returns `Some(stalled)` when it cannot converge.
    async fn run_textual_repair(
        &mut self,
        fetched_sha: &str,
        local_before: &str,
    ) -> PortResult<Option<UpstreamStepOutcome>> {
        let max_attempts = self.repair_agent.max_attempts();
        for attempt in 1..=max_attempts {
            self.observer
                .observe(UpstreamEvent::Resolving {
                    cause: "textual".to_string(),
                    attempt,
                })
                .await;

            let request = RepairRequest {
                cause: RepairCause::TextualConflict,
                remote: self.config.remote.clone(),
                branch: self.branch.clone(),
                local_revision_before: local_before.to_string(),
                fetched_sha: fetched_sha.to_string(),
                conflict_files: Vec::new(),
                status: self.git.status_porcelain_v2().await.unwrap_or_default(),
                verify_command: self.config.verify_command.clone(),
                verify_output_tail: String::new(),
                push_diagnostics: String::new(),
            };
            // Command exit status is recorded but never establishes success.
            let _ = self.repair_agent.repair(&request).await?;

            if self.textual_repair_converged(fetched_sha).await? {
                return Ok(None);
            }
        }

        let reason = format!(
            "upstream textual repair did not converge for {} within {} attempts",
            fetched_sha, max_attempts
        );
        self.observer
            .observe(UpstreamEvent::Stalled {
                reason: reason.clone(),
            })
            .await;
        Ok(Some(UpstreamStepOutcome::Stalled { reason }))
    }

    /// Textual repair goal: repository state, never agent narrative.
    async fn textual_repair_converged(&self, fetched_sha: &str) -> PortResult<bool> {
        let state = self.git.merge_repository_state().await?;
        if state.merge_head_present || state.has_unmerged_entries {
            return Ok(false);
        }
        let head = self.git.head_sha().await?;
        if !self.git.is_ancestor(fetched_sha, &head).await? {
            return Ok(false);
        }
        self.validate_identity(&head, fetched_sha).await
    }

    /// Run the complete verification command, allowing bounded semantic repair.
    ///
    /// Returns `None` on success; `Some(stalled)` when the bounded cycle is
    /// exhausted. Every repair attempt is followed by a mandatory rerun.
    async fn verify_with_semantic_repair(
        &mut self,
        repair_start_head: &str,
        fetched_sha: &str,
        identity_commit: &str,
    ) -> PortResult<Option<UpstreamStepOutcome>> {
        let identity_message_before = self.git.commit_message(identity_commit).await?;
        let max_attempts = self.repair_agent.max_attempts();

        self.observer
            .observe(UpstreamEvent::Reverifying {
                command: self.config.verify_command.clone(),
            })
            .await;
        let mut outcome = self.verifier.verify().await?;

        let mut attempt = 0;
        while !outcome.success {
            self.observer
                .observe(UpstreamEvent::VerificationFailed {
                    output_tail: outcome.output_tail.clone(),
                })
                .await;

            if attempt >= max_attempts {
                break;
            }
            attempt += 1;

            self.observer
                .observe(UpstreamEvent::Resolving {
                    cause: "semantic".to_string(),
                    attempt,
                })
                .await;

            let request = RepairRequest {
                cause: RepairCause::SemanticVerification,
                remote: self.config.remote.clone(),
                branch: self.branch.clone(),
                local_revision_before: repair_start_head.to_string(),
                fetched_sha: fetched_sha.to_string(),
                conflict_files: Vec::new(),
                status: self.git.status_porcelain_v2().await.unwrap_or_default(),
                verify_command: self.config.verify_command.clone(),
                verify_output_tail: outcome.output_tail.clone(),
                push_diagnostics: String::new(),
            };
            let _ = self.repair_agent.repair(&request).await?;

            if !self
                .semantic_repair_state_converged(
                    repair_start_head,
                    fetched_sha,
                    identity_commit,
                    &identity_message_before,
                )
                .await?
            {
                let reason = format!(
                    "upstream semantic repair violated forward-only convergence after attempt {}",
                    attempt
                );
                self.observer
                    .observe(UpstreamEvent::Stalled {
                        reason: reason.clone(),
                    })
                    .await;
                return Ok(Some(UpstreamStepOutcome::Stalled { reason }));
            }

            // A repair attempt is only complete after a successful rerun of the
            // complete verification command.
            self.observer
                .observe(UpstreamEvent::Reverifying {
                    command: self.config.verify_command.clone(),
                })
                .await;
            outcome = self.verifier.verify().await?;
        }

        if outcome.success {
            return Ok(None);
        }

        let reason = format!(
            "complete verification command did not pass within {} repair attempts",
            max_attempts
        );
        self.observer
            .observe(UpstreamEvent::Stalled {
                reason: reason.clone(),
            })
            .await;
        Ok(Some(UpstreamStepOutcome::Stalled { reason }))
    }

    /// Semantic repair goal predicate. Amend/rebase/reset/push are detectable
    /// here as a violated ancestry, dirty state, or changed identity commit.
    async fn semantic_repair_state_converged(
        &self,
        repair_start_head: &str,
        fetched_sha: &str,
        identity_commit: &str,
        identity_message_before: &str,
    ) -> PortResult<bool> {
        let head = self.git.head_sha().await?;
        if !self.git.is_ancestor(repair_start_head, &head).await? {
            return Ok(false);
        }
        if !self.git.is_working_tree_clean().await? {
            return Ok(false);
        }
        let state = self.git.merge_repository_state().await?;
        if state.merge_head_present || state.has_unmerged_entries {
            return Ok(false);
        }
        if !self.git.is_ancestor(fetched_sha, &head).await? {
            return Ok(false);
        }
        if !self.git.is_ancestor(identity_commit, &head).await? {
            return Ok(false);
        }
        if self.git.commit_message(identity_commit).await? != identity_message_before {
            return Ok(false);
        }
        self.validate_identity(identity_commit, fetched_sha).await
    }

    /// Whether `commit` is a valid upstream merge for the selected identity.
    async fn validate_identity(&self, commit: &str, fetched_sha: &str) -> PortResult<bool> {
        let message = self.git.commit_message(commit).await?;
        let parents = self.git.commit_parents(commit).await?;
        Ok(super::trailers::validate_upstream_merge(
            &message,
            &parents,
            &self.config.remote,
            &self.branch,
        )
        .map(|trailers| trailers.sha == fetched_sha)
        .unwrap_or(false))
    }

    // ── Completed change results ───────────────────────────────────────────

    /// Run the complete verification command after a completed change result
    /// merged into cumulative base.
    ///
    /// The base lane stays closed until this succeeds, so a worktree accepted on
    /// an older base cannot silently unblock later base-dependent dispatch.
    pub async fn verify_base_result(&mut self, change_id: &str) -> PortResult<UpstreamStepOutcome> {
        let head = self.git.head_sha().await?;
        match self
            .verify_with_semantic_repair_for_base_result(&head)
            .await?
        {
            None => Ok(UpstreamStepOutcome::Integrated { merge_sha: head }),
            Some(UpstreamStepOutcome::Stalled { reason }) => Ok(UpstreamStepOutcome::Stalled {
                reason: format!("after integrating '{}': {}", change_id, reason),
            }),
            Some(other) => Ok(other),
        }
    }

    /// Base-result verification has no fetched-SHA identity to preserve, so the
    /// goal is forward-only ancestry plus clean state plus a passing command.
    async fn verify_with_semantic_repair_for_base_result(
        &mut self,
        repair_start_head: &str,
    ) -> PortResult<Option<UpstreamStepOutcome>> {
        let max_attempts = self.repair_agent.max_attempts();

        self.observer
            .observe(UpstreamEvent::Reverifying {
                command: self.config.verify_command.clone(),
            })
            .await;
        let mut outcome = self.verifier.verify().await?;

        let mut attempt = 0;
        while !outcome.success {
            self.observer
                .observe(UpstreamEvent::VerificationFailed {
                    output_tail: outcome.output_tail.clone(),
                })
                .await;
            if attempt >= max_attempts {
                break;
            }
            attempt += 1;

            self.observer
                .observe(UpstreamEvent::Resolving {
                    cause: "semantic".to_string(),
                    attempt,
                })
                .await;

            let request = RepairRequest {
                cause: RepairCause::SemanticVerification,
                remote: self.config.remote.clone(),
                branch: self.branch.clone(),
                local_revision_before: repair_start_head.to_string(),
                fetched_sha: String::new(),
                conflict_files: Vec::new(),
                status: self.git.status_porcelain_v2().await.unwrap_or_default(),
                verify_command: self.config.verify_command.clone(),
                verify_output_tail: outcome.output_tail.clone(),
                push_diagnostics: String::new(),
            };
            let _ = self.repair_agent.repair(&request).await?;

            let head = self.git.head_sha().await?;
            let forward_only = self.git.is_ancestor(repair_start_head, &head).await?;
            let clean = self.git.is_working_tree_clean().await?;
            let state = self.git.merge_repository_state().await?;
            if !forward_only || !clean || state.merge_head_present || state.has_unmerged_entries {
                let reason = format!(
                    "base-result semantic repair violated forward-only convergence after attempt {}",
                    attempt
                );
                self.observer
                    .observe(UpstreamEvent::Stalled {
                        reason: reason.clone(),
                    })
                    .await;
                return Ok(Some(UpstreamStepOutcome::Stalled { reason }));
            }

            self.observer
                .observe(UpstreamEvent::Reverifying {
                    command: self.config.verify_command.clone(),
                })
                .await;
            outcome = self.verifier.verify().await?;
        }

        if outcome.success {
            return Ok(None);
        }

        let reason = format!(
            "complete verification command did not pass within {} repair attempts",
            max_attempts
        );
        self.observer
            .observe(UpstreamEvent::Stalled {
                reason: reason.clone(),
            })
            .await;
        Ok(Some(UpstreamStepOutcome::Stalled { reason }))
    }

    // ── Finalization ───────────────────────────────────────────────────────

    /// Own finalization ordering for the run.
    ///
    /// Only a successful drain may enter final checkpoint, verification, native
    /// non-force push, and remote confirmation.
    pub async fn finalize(&mut self, outcome: SchedulerOutcome) -> PortResult<FinalizeOutcome> {
        match outcome {
            SchedulerOutcome::BlockedOrStalled => {
                return Ok(FinalizeOutcome::Skipped {
                    reason: "scheduler exited blocked or stalled".to_string(),
                })
            }
            SchedulerOutcome::Cancelled => {
                return Ok(FinalizeOutcome::Skipped {
                    reason: "run was cancelled".to_string(),
                })
            }
            SchedulerOutcome::DrainedSuccessfully => {}
        }

        if let Some(head) = self.pushed_head.clone() {
            // At most one successful push per run; never retried after confirmation.
            return Ok(FinalizeOutcome::Completed { pushed_head: head });
        }

        let remote = self.config.remote.clone();
        let branch = self.branch.clone();

        for attempt in 1..=MAX_FINALIZE_ATTEMPTS {
            // Zero-change no-work probe. Performed before any integration so a
            // remote-only advance never manufactures a synthetic local merge.
            self.git.fetch(&remote, &branch).await?;
            let Some(fetched_sha) = self.git.fetched_sha(&remote, &branch).await? else {
                return Ok(FinalizeOutcome::Stalled {
                    reason: format!("remote '{}' has no branch '{}'", remote, branch),
                });
            };
            let head = self.git.head_sha().await?;
            if self.git.is_ancestor(&head, &fetched_sha).await? {
                self.observer
                    .observe(UpstreamEvent::NoOp {
                        fetched_sha: fetched_sha.clone(),
                    })
                    .await;
                return Ok(FinalizeOutcome::NoWork);
            }

            // Final checkpoint: integrate any remote advance and reverify.
            let trigger = if attempt == 1 {
                CheckpointTrigger::AfterDrain
            } else {
                CheckpointTrigger::PrePushRemoteAdvance
            };
            match self
                .checkpoint(trigger, &BaseLaneState::clean(), None)
                .await?
            {
                UpstreamStepOutcome::Stalled { reason } => {
                    return Ok(FinalizeOutcome::Stalled { reason })
                }
                UpstreamStepOutcome::Deferred { reason } => {
                    return Ok(FinalizeOutcome::Stalled { reason })
                }
                UpstreamStepOutcome::NoOp { .. } => {
                    // Ancestry-proven upstream no-op still runs the complete
                    // command against final cumulative HEAD before push.
                    let head = self.git.head_sha().await?;
                    if let Some(UpstreamStepOutcome::Stalled { reason }) = self
                        .verify_with_semantic_repair_for_base_result(&head)
                        .await?
                    {
                        return Ok(FinalizeOutcome::Stalled { reason });
                    }
                }
                UpstreamStepOutcome::Integrated { .. } => {
                    // The checkpoint already ran the complete command against the
                    // integrated tree, which is final cumulative HEAD here.
                }
            }

            // Fresh pre-push ancestry check.
            self.git.fetch(&remote, &branch).await?;
            let Some(fresh_sha) = self.git.fetched_sha(&remote, &branch).await? else {
                return Ok(FinalizeOutcome::Stalled {
                    reason: format!("remote '{}' has no branch '{}'", remote, branch),
                });
            };
            let head = self.git.head_sha().await?;
            if !self.git.is_ancestor(&fresh_sha, &head).await? {
                // Remote advanced before the check: suppress stale push and
                // return to integration.
                continue;
            }

            self.observer
                .observe(UpstreamEvent::Pushing {
                    remote: remote.clone(),
                    branch: branch.clone(),
                    head: head.clone(),
                })
                .await;
            let push = self.git.push_porcelain(&remote, &branch).await?;
            let report = parse_push_porcelain(&push.porcelain_stdout);

            if !push.exit_success || report.has_rejection() {
                let status = parse_porcelain_v2(&self.git.status_porcelain_v2().await?);
                let class = classify_push_failure(&report, status);
                self.observer
                    .observe(UpstreamEvent::PushFailed {
                        classification: format!("{:?}", class),
                    })
                    .await;

                match class {
                    PushFailureClass::Race => continue,
                    PushFailureClass::Stalled => {
                        return Ok(FinalizeOutcome::Stalled {
                            reason: format!(
                            "native push to {}/{} failed without repairable repository evidence",
                            remote, branch
                        ),
                        })
                    }
                    PushFailureClass::RepositoryRepairable => {
                        if let Some(stall) = self.repair_push_repository(&head).await? {
                            return Ok(FinalizeOutcome::Stalled { reason: stall });
                        }
                        continue;
                    }
                }
            }

            // Remote confirmation. Completion is a network observation, not the
            // push command's exit status.
            let observed = self.git.ls_remote_sha(&remote, &branch).await?;
            let Some(observed) = observed else {
                continue;
            };
            let confirmation = classify_push_confirmation(
                &head,
                &observed,
                self.git.is_ancestor(&head, &observed).await?,
            );
            if confirmation == PushConfirmation::NotConfirmed {
                continue;
            }

            self.pushed_head = Some(head.clone());
            self.observer
                .observe(UpstreamEvent::PushConfirmed {
                    remote: remote.clone(),
                    branch: branch.clone(),
                    head: head.clone(),
                })
                .await;
            self.observer.observe(UpstreamEvent::Completed).await;
            return Ok(FinalizeOutcome::Completed { pushed_head: head });
        }

        Ok(FinalizeOutcome::Stalled {
            reason: format!(
                "upstream finalization did not converge within {} attempts",
                MAX_FINALIZE_ATTEMPTS
            ),
        })
    }

    /// Bounded repository repair after a push failure.
    ///
    /// The agent never executes the push. Conflux reruns convergence and the
    /// complete verification command, then retries the native push itself.
    async fn repair_push_repository(
        &mut self,
        repair_start_head: &str,
    ) -> PortResult<Option<String>> {
        let request = RepairRequest {
            cause: RepairCause::PushRepository,
            remote: self.config.remote.clone(),
            branch: self.branch.clone(),
            local_revision_before: repair_start_head.to_string(),
            fetched_sha: String::new(),
            conflict_files: Vec::new(),
            status: self.git.status_porcelain_v2().await.unwrap_or_default(),
            verify_command: self.config.verify_command.clone(),
            verify_output_tail: String::new(),
            push_diagnostics: "native non-force push failed with local repository mutation"
                .to_string(),
        };
        let _ = self.repair_agent.repair(&request).await?;

        let head = self.git.head_sha().await?;
        let forward_only = self.git.is_ancestor(repair_start_head, &head).await?;
        let clean = self.git.is_working_tree_clean().await?;
        let state = self.git.merge_repository_state().await?;
        if !forward_only || !clean || state.merge_head_present || state.has_unmerged_entries {
            return Ok(Some(
                "push repair did not restore a clean forward-only repository state".to_string(),
            ));
        }

        self.observer
            .observe(UpstreamEvent::Reverifying {
                command: self.config.verify_command.clone(),
            })
            .await;
        if !self.verifier.verify().await?.success {
            return Ok(Some(
                "complete verification command failed after push repair".to_string(),
            ));
        }
        Ok(None)
    }
}

/// Convert a port error into an operator-visible orchestrator error.
impl From<UpstreamPortError> for crate::error::OrchestratorError {
    fn from(err: UpstreamPortError) -> Self {
        crate::error::OrchestratorError::GitCommand(err.to_string())
    }
}

#[cfg(test)]
mod tests;
