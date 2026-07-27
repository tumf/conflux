//! Shared serial execution service for CLI and TUI modes.
//!
//! This module provides a unified service for running serial execution
//! that can be used by both CLI and TUI orchestrators, eliminating
//! code duplication between the two modes.
//!
//! The service provides helper functions for:
//! - Change selection based on progress and dependencies
//! - State tracking (apply counts, completed/stalled changes)
//! - Iteration limit checking
//! - Hook execution helpers
//!
//! The actual orchestration loop remains in the orchestrators for now,
//! as they have mode-specific concerns (WIP commits for CLI, DynamicQueue for TUI).

use crate::agent::{AgentRunner, OutputLine};
use crate::ai_command_runner::AiCommandRunner;
use crate::config::OrchestratorConfig;
use crate::error::Result;
use crate::execution::apply as common_apply;
use crate::hooks::{HookContext, HookRunner, HookType};
use crate::openspec::{self, Change};
use crate::orchestration::acceptance::{
    decide_acceptance_blocker, decide_acceptance_retry, missing_verdict_exhausted_error,
    normalize_findings, repository_findings, semantic_progress_fingerprint,
    AcceptanceBlockerDecision, AcceptanceProtocolDriver, AcceptanceRetryDecision,
    MissingVerdictRetryStep, MAX_MISSING_VERDICT_RETRIES,
};
use crate::orchestration::{
    acceptance_test_streaming, archive_change, AcceptanceResult, ArchiveContext, ArchiveResult,
    OutputHandler,
};
use crate::parallel::acceptance_state::{
    parse_blocked_marker, AcceptanceRetryContext, AcceptanceStallStore,
};
use crate::stall::{StallDetector, StallPhase};
use crate::task_parser;
use crate::task_parser::TaskProgress;
use crate::vcs::VcsBackend;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Service for serial execution of changes.
///
/// This service encapsulates the shared logic between CLI and TUI
/// serial execution modes, including:
/// - Change selection
/// - Apply/archive flow
/// - Acceptance testing
/// - Hook execution
/// - Iteration tracking
/// - Stall detection
pub struct SerialRunService {
    /// Configuration for the orchestrator
    config: OrchestratorConfig,
    /// Repository root directory
    repo_root: PathBuf,
    /// Apply count per change
    apply_counts: HashMap<String, u32>,
    /// Currently processing change ID
    current_change_id: Option<String>,
    /// Completed change IDs
    completed_change_ids: HashSet<String>,
    /// Stalled change IDs
    stalled_change_ids: HashSet<String>,
    /// Stall detector for monitoring progress
    stall_detector: StallDetector,
    /// Changes processed count
    changes_processed: usize,
    /// Current iteration
    iteration: u32,
    /// In-memory acceptance retry context for the active run, keyed by change ID.
    ///
    /// This is deliberately not persisted: a restarted process starts a fresh
    /// acceptance sequence instead of trusting a generated checkpoint.
    acceptance_retry: HashMap<String, AcceptanceRetryContext>,
    /// Override for the acceptance stall state root.
    ///
    /// `None` uses Conflux's XDG state area. Tests set an isolated temporary
    /// root so runtime stall state never leaks into a developer's real state
    /// directory and concurrent tests cannot see each other's holds.
    acceptance_stall_state_root: Option<PathBuf>,
    /// Changes whose next processing step must be acceptance, not apply or
    /// archive, because an explicit retry consumed a resumable runtime hold.
    ///
    /// Serial's counterpart to parallel's `ResumeAction::Acceptance`.
    acceptance_resume: HashSet<String>,
    /// Changes that acceptance passed during *this* run.
    ///
    /// Archive is reachable only through an observed PASS. Nothing durable
    /// records the verdict, so a restart finds this set empty and re-runs
    /// acceptance for complete unarchived work instead of inferring PASS.
    accepted_change_ids: HashSet<String>,
}

impl SerialRunService {
    /// Create a new serial run service
    pub fn new(repo_root: PathBuf, config: OrchestratorConfig) -> Self {
        let stall_config = config.get_stall_detection();
        Self {
            config,
            repo_root,
            apply_counts: HashMap::new(),
            current_change_id: None,
            completed_change_ids: HashSet::new(),
            stalled_change_ids: HashSet::new(),
            stall_detector: StallDetector::new(stall_config),
            changes_processed: 0,
            iteration: 0,
            acceptance_retry: HashMap::new(),
            acceptance_stall_state_root: None,
            acceptance_resume: HashSet::new(),
            accepted_change_ids: HashSet::new(),
        }
    }

    /// Get the repository root path
    #[allow(dead_code)] // Reserved for future TUI integration
    pub fn repo_root(&self) -> &PathBuf {
        &self.repo_root
    }

    /// Point acceptance stall state at an isolated root (tests only).
    #[cfg(test)]
    pub fn set_acceptance_stall_state_root(&mut self, root: PathBuf) {
        self.acceptance_stall_state_root = Some(root);
    }

    /// Open the acceptance stall store for this service.
    fn acceptance_stall_store(&self) -> Result<AcceptanceStallStore> {
        match &self.acceptance_stall_state_root {
            Some(root) => Ok(AcceptanceStallStore::new(root.clone())),
            None => AcceptanceStallStore::discover(),
        }
    }

    /// Get the current iteration number
    #[allow(dead_code)] // Reserved for future TUI integration
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// Get the number of changes processed
    #[allow(dead_code)] // Reserved for future TUI integration
    pub fn changes_processed(&self) -> usize {
        self.changes_processed
    }

    /// Get the current change ID being processed
    #[allow(dead_code)] // Reserved for future TUI integration
    pub fn current_change_id(&self) -> Option<&String> {
        self.current_change_id.as_ref()
    }

    /// Get apply count for a change
    pub fn apply_count(&self, change_id: &str) -> u32 {
        *self.apply_counts.get(change_id).unwrap_or(&0)
    }

    /// Increment apply count for a change
    fn increment_apply_count(&mut self, change_id: &str) {
        let count = self.apply_counts.entry(change_id.to_string()).or_insert(0);
        *count += 1;
    }

    /// Check if a change is stalled
    pub fn is_stalled(&self, change_id: &str) -> bool {
        self.stalled_change_ids.contains(change_id)
    }

    /// Check if a change is completed
    pub fn is_completed(&self, change_id: &str) -> bool {
        self.completed_change_ids.contains(change_id)
    }

    /// Select the next change to process.
    ///
    /// Prioritizes changes by highest progress percentage.
    /// Filters out stalled changes and their dependencies.
    pub fn select_next_change<'a>(&self, changes: &'a [Change]) -> Option<&'a Change> {
        // Filter out completed and stalled changes
        let eligible: Vec<_> = changes
            .iter()
            .filter(|c| !self.is_completed(&c.id) && !self.is_stalled(&c.id))
            .collect();

        // Further filter out changes that depend on stalled changes
        let filtered: Vec<_> = eligible
            .iter()
            .filter(|c| {
                !c.dependencies
                    .iter()
                    .any(|dep| self.stalled_change_ids.contains(dep))
            })
            .copied()
            .collect();

        if filtered.is_empty() {
            return None;
        }

        // Find incomplete changes and prioritize by progress
        let incomplete: Vec<_> = filtered.iter().filter(|c| !c.is_complete()).collect();

        if !incomplete.is_empty() {
            // Prioritize incomplete changes by highest progress percentage
            return incomplete
                .into_iter()
                .max_by(|a, b| {
                    let a_progress = if a.total_tasks > 0 {
                        a.completed_tasks as f32 / a.total_tasks as f32
                    } else {
                        0.0
                    };
                    let b_progress = if b.total_tasks > 0 {
                        b.completed_tasks as f32 / b.total_tasks as f32
                    } else {
                        0.0
                    };
                    a_progress
                        .partial_cmp(&b_progress)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied();
        }

        // If all are complete, select the first one for archiving
        filtered.first().copied()
    }

    /// Mark a change as stalled
    pub fn mark_stalled(&mut self, change_id: &str, reason: &str) {
        warn!("Marking {} as stalled: {}", change_id, reason);
        self.stalled_change_ids.insert(change_id.to_string());
    }

    /// Consume a resumable acceptance stall for an explicit serial retry.
    ///
    /// The hold lives outside the worktree, so consuming it leaves the managed
    /// worktree untouched. A non-resumable or absent hold is refused rather than
    /// silently discarded, and a refusal keeps the change stalled with its
    /// blocker evidence intact.
    pub async fn consume_explicit_acceptance_retry(&mut self, change_id: &str) -> Result<bool> {
        let store = self.acceptance_stall_store()?;
        let base_branch = self.current_branch_for_facts().await;
        // A legacy acceptance-origin marker must become runtime state before
        // retry looks for a record, otherwise an operator retrying a legacy hold
        // is refused for a hold that is really there.
        self.migrate_legacy_acceptance_marker(&store, change_id, &base_branch)
            .await;
        let Some(record) = crate::execution::state::load_valid_acceptance_stall(
            &store,
            &self.repo_root,
            &self.repo_root,
            change_id,
            &base_branch,
        )
        .await?
        else {
            return Ok(false);
        };
        if !record.resumable {
            return Ok(false);
        }
        let consumed = store.consume(&record.repository_id, &record.change_id)?;
        if consumed {
            self.stalled_change_ids.remove(change_id);
            // The consumed hold proved a complete apply revision, so the next
            // pass resumes at acceptance instead of rerunning apply or falling
            // through to archive on `change.is_complete()`.
            self.acceptance_resume.insert(change_id.to_string());
            info!(
                "Explicit retry for {}: resuming at acceptance against apply revision {} without \
                 rerunning apply",
                change_id, record.apply_revision
            );
        }
        Ok(consumed)
    }

    /// Branch used as the base reference when gathering reconciliation facts.
    ///
    /// Serial mode runs directly in the repository, so the checked-out branch is
    /// the best available base reference; `main` is a safe fallback because the
    /// derived facts only ever *reduce* a record's authority.
    async fn current_branch_for_facts(&self) -> String {
        crate::vcs::git::commands::get_current_branch(&self.repo_root)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "main".to_string())
    }

    /// Persist a validated external acceptance blocker as a runtime stall hold.
    async fn record_acceptance_stall(
        &mut self,
        change_id: &str,
        blocker: &crate::acceptance::AcceptanceBlocker,
    ) -> ChangeProcessResult {
        let store = match self.acceptance_stall_store() {
            Ok(store) => store,
            Err(error) => {
                return ChangeProcessResult::AcceptanceCommandFailed {
                    error: format!("Failed to open acceptance stall state: {error}"),
                }
            }
        };
        let base_branch = self.current_branch_for_facts().await;
        // A record born without a revision can never reconcile: `revision_exists`
        // rejects an empty revision, so the hold would be quarantined on the next
        // load and the blocker evidence would silently vanish. Fail loudly
        // instead, mirroring parallel dispatch, which persists only a revision it
        // actually resolved.
        let apply_revision = match crate::vcs::git::commands::get_current_commit(&self.repo_root)
            .await
        {
            Ok(revision) if !revision.trim().is_empty() => revision,
            Ok(_) => {
                return ChangeProcessResult::AcceptanceCommandFailed {
                    error: "Failed to persist acceptance stalled evidence: current apply revision \
                            resolved to an empty commit id"
                        .to_string(),
                }
            }
            Err(error) => {
                return ChangeProcessResult::AcceptanceCommandFailed {
                    error: format!(
                        "Failed to persist acceptance stalled evidence: could not resolve the \
                         current apply revision: {error}"
                    ),
                }
            }
        };
        let retry_count = self
            .acceptance_retry
            .get(change_id)
            .map_or(0, |context| context.cycle_count);

        match crate::execution::state::persist_acceptance_stall(
            &store,
            &self.repo_root,
            &self.repo_root,
            change_id,
            &base_branch,
            &apply_revision,
            blocker,
            retry_count,
        )
        .await
        {
            Ok(record) => {
                warn!(
                    "Acceptance stalled for {} on a validated external blocker ({}); worktree \
                     preserved and apply revision {} unchanged",
                    change_id, record.category, record.apply_revision
                );
                self.mark_stalled(change_id, &record.next_action);
                ChangeProcessResult::AcceptanceStalled {
                    blocker: record.to_stalled_blocker(),
                    error: format!("{}: {}", record.category, record.next_action),
                }
            }
            Err(error) => ChangeProcessResult::AcceptanceCommandFailed {
                error: format!("Failed to persist acceptance stalled evidence: {error}"),
            },
        }
    }

    /// Acceptance retry context accumulated during the active run.
    ///
    /// Returns `None` after a restart, which forces a fresh acceptance sequence.
    #[allow(dead_code)] // Consumed by active-run acceptance retry regression coverage.
    pub fn acceptance_retry_context(&self, change_id: &str) -> Option<&AcceptanceRetryContext> {
        self.acceptance_retry.get(change_id)
    }

    /// Record acceptance retry context for the active run only.
    pub fn set_acceptance_retry_context(
        &mut self,
        change_id: &str,
        context: AcceptanceRetryContext,
    ) {
        self.acceptance_retry.insert(change_id.to_string(), context);
    }

    /// Seed the agent with acceptance retry context gathered earlier in this run.
    ///
    /// Nothing is restored after a restart: the in-memory map is empty, so the
    /// next acceptance runs without a reconstructed baseline.
    fn seed_active_run_acceptance_history(&self, change_id: &str, agent: &mut AgentRunner) {
        let Some(context) = self.acceptance_retry.get(change_id) else {
            return;
        };
        if context.finding_identities.is_empty() {
            return;
        }

        let mut history = crate::history::AcceptanceHistory::new();
        history.set_checkpoint(
            change_id,
            context.cycle_count,
            context.finding_identities.clone(),
            context.semantic_fingerprint.clone(),
        );
        agent.seed_acceptance_history(history);
    }

    /// One-time migration of a legacy acceptance-origin marker into runtime
    /// stall state, so serial recovers the same hold parallel dispatch does.
    ///
    /// A stalled acceptance hold never commits, so current HEAD is the apply
    /// revision the marker was written against. Migration is best-effort:
    /// anything it cannot prove is acceptance-owned stays exactly where it is
    /// and keeps its conservative blocked handling.
    async fn migrate_legacy_acceptance_marker(
        &self,
        store: &AcceptanceStallStore,
        change_id: &str,
        base_branch: &str,
    ) {
        use crate::parallel::acceptance_state::MarkerMigration;

        let Ok(head) = crate::vcs::git::commands::get_current_commit(&self.repo_root).await else {
            return;
        };
        let facts = match crate::execution::state::gather_workspace_facts(
            &self.repo_root,
            &self.repo_root,
            change_id,
            &head,
            base_branch,
        )
        .await
        {
            Ok(facts) => facts,
            Err(error) => {
                warn!(
                    "Could not gather workspace facts for {change_id} stall reconciliation: {error}"
                );
                return;
            }
        };

        match crate::parallel::acceptance_state::migrate_legacy_acceptance_marker(
            store,
            &self.repo_root,
            &facts,
            &head,
        ) {
            Ok(MarkerMigration::Migrated { category }) => info!(
                "Migrated legacy acceptance blocker marker for {change_id} into runtime stall \
                 state (category {category})"
            ),
            Ok(MarkerMigration::Preserved { reason }) => debug!(
                change_id = %change_id,
                reason = %reason,
                "Preserved non-acceptance blocked marker unchanged"
            ),
            Ok(MarkerMigration::NotApplicable) => {}
            Err(error) => {
                warn!("Legacy acceptance marker migration degraded for {change_id}: {error}")
            }
        }
    }

    /// Reconcile runtime acceptance stall state before any serial routing.
    ///
    /// Serial's counterpart to the parallel dispatch reconciliation block. The
    /// record lives outside the worktree and may only suppress dispatch and
    /// restore the operator-facing `stalled` hold; it never advances the
    /// workflow and never touches the worktree. A legacy acceptance-origin
    /// marker is migrated first so both modes recover the same hold.
    async fn preflight_acceptance_stall(
        &mut self,
        change_id: &str,
    ) -> Result<Option<ChangeProcessResult>> {
        let store = self.acceptance_stall_store()?;
        let base_branch = self.current_branch_for_facts().await;

        self.migrate_legacy_acceptance_marker(&store, change_id, &base_branch)
            .await;

        // An explicit retry already consumed the hold for this change; the
        // resume request wins so retry cannot be re-suppressed by a stale read.
        if self.acceptance_resume.contains(change_id) {
            return Ok(None);
        }

        let record = crate::execution::state::load_valid_acceptance_stall(
            &store,
            &self.repo_root,
            &self.repo_root,
            change_id,
            &base_branch,
        )
        .await
        .unwrap_or_else(|error| {
            warn!("Could not load acceptance stall state for {change_id}: {error}");
            None
        });

        let Some(record) = record else {
            return Ok(None);
        };
        let error = format!("{}: {}", record.category, record.next_action);
        self.mark_stalled(change_id, &record.next_action);
        Ok(Some(ChangeProcessResult::AcceptanceStalled {
            blocker: record.to_stalled_blocker(),
            error,
        }))
    }

    fn preflight_blocked_marker(&mut self, change_id: &str) -> Result<Option<ChangeProcessResult>> {
        if let Some(marker) = parse_blocked_marker(&self.repo_root, change_id)? {
            let error = format!("Blocked marker ({:?}): {}", marker.origin, marker.reason);
            self.mark_stalled(change_id, &error);
            return Ok(Some(ChangeProcessResult::Stalled { error }));
        }
        Ok(None)
    }

    /// Process a single iteration for a change.
    ///
    /// This includes:
    /// - Running hooks (on_change_start, pre_apply, post_apply, etc.)
    /// - Applying or archiving the change
    /// - Running acceptance tests
    /// - Stall detection
    ///
    /// Returns `Ok(ChangeProcessResult)` indicating the outcome.
    /// Callers should handle the result and decide whether to continue the loop.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_change<O: OutputHandler, F, G>(
        &mut self,
        change: &Change,
        agent: &mut AgentRunner,
        ai_runner: &AiCommandRunner,
        hooks: &HookRunner,
        output: &O,
        total_changes: usize,
        remaining_changes: usize,
        cancel_check: F,
        is_single_change_stopped: G,
        operation_tracker: Option<std::sync::Arc<std::sync::RwLock<String>>>,
    ) -> Result<ChangeProcessResult>
    where
        F: Fn() -> bool + Clone + Send + 'static,
        G: Fn() -> bool + Clone,
    {
        self.iteration += 1;
        let change_id = &change.id;

        if let Some(result) = self.preflight_acceptance_stall(change_id).await? {
            return Ok(result);
        }

        if let Some(result) = self.preflight_blocked_marker(change_id)? {
            return Ok(result);
        }

        // Check if this is a new change
        let is_new_change = self.current_change_id.as_ref() != Some(change_id);
        if is_new_change {
            // Run on_change_start hook
            let change_start_context = HookContext::new(
                self.changes_processed,
                total_changes,
                remaining_changes,
                false,
            )
            .with_change(change_id, change.completed_tasks, change.total_tasks)
            .with_apply_count(0);

            hooks
                .run_hook(HookType::OnChangeStart, &change_start_context)
                .await?;

            self.current_change_id = Some(change_id.clone());
        }

        let apply_count = self.apply_count(change_id);

        // An explicit retry consumed a resumable hold, so acceptance is the next
        // step for this change and apply is not repeated.
        let resume_at_acceptance = self.acceptance_resume.remove(change_id);

        // Process the change
        if change.is_complete() {
            // Archive is reachable only through an acceptance PASS observed in
            // this run. After a restart — or after an explicit acceptance retry —
            // complete unarchived work is accepted again rather than archived on
            // an inferred prior verdict (constitutional law 1a fail-safe).
            if resume_at_acceptance || !self.accepted_change_ids.contains(change_id) {
                info!(
                    "Change {} is complete but unaccepted in this run, running acceptance before \
                     archive",
                    change_id
                );
                Self::update_operation_tracker(&operation_tracker, "acceptance");
                self.seed_active_run_acceptance_history(change_id, agent);
                return self
                    .run_acceptance_loop(
                        change,
                        agent,
                        ai_runner,
                        output,
                        &cancel_check,
                        &is_single_change_stopped,
                    )
                    .await;
            }

            // Archive completed change
            self.archive_change_internal(
                change,
                agent,
                ai_runner,
                hooks,
                output,
                total_changes,
                remaining_changes,
                apply_count,
                operation_tracker,
            )
            .await
        } else {
            // Apply incomplete change
            self.apply_change_internal(
                change,
                agent,
                ai_runner,
                hooks,
                output,
                total_changes,
                remaining_changes,
                apply_count,
                &cancel_check,
                &is_single_change_stopped,
                operation_tracker,
            )
            .await
        }
    }

    /// Internal method to archive a change
    #[allow(clippy::too_many_arguments)]
    async fn archive_change_internal<O: OutputHandler>(
        &mut self,
        change: &Change,
        agent: &mut AgentRunner,
        ai_runner: &AiCommandRunner,
        hooks: &HookRunner,
        output: &O,
        total_changes: usize,
        remaining_changes: usize,
        apply_count: u32,
        operation_tracker: Option<std::sync::Arc<std::sync::RwLock<String>>>,
    ) -> Result<ChangeProcessResult> {
        info!("Change {} is complete, archiving...", change.id);

        // Update operation to "archive" before running archive
        Self::update_operation_tracker(&operation_tracker, "archive");

        let archive_ctx = ArchiveContext::new(
            self.changes_processed,
            total_changes,
            remaining_changes,
            apply_count,
        );

        let stall_config = self.config.get_stall_detection();

        match archive_change(
            change,
            agent,
            ai_runner,
            hooks,
            &archive_ctx,
            output,
            None,
            &stall_config,
        )
        .await
        {
            Ok(ArchiveResult::Success) => {
                // Update changes_processed count
                self.changes_processed += 1;

                // Clear acceptance history after successful archive
                agent.clear_acceptance_history(&change.id);

                // Run on_change_end hook (not included in shared archive_change)
                let new_remaining = remaining_changes.saturating_sub(1);
                let change_end_context =
                    HookContext::new(self.changes_processed, total_changes, new_remaining, false)
                        .with_change(&change.id, change.completed_tasks, change.total_tasks)
                        .with_apply_count(apply_count);
                hooks
                    .run_hook(HookType::OnChangeEnd, &change_end_context)
                    .await?;

                // Run on_merged hook after on_change_end (serial mode: archive success = merge complete equivalent)
                let merged_context =
                    HookContext::new(self.changes_processed, total_changes, new_remaining, false)
                        .with_change(&change.id, change.completed_tasks, change.total_tasks)
                        .with_apply_count(apply_count);
                hooks.run_hook(HookType::OnMerged, &merged_context).await?;

                // Mark change as completed and clear current
                self.completed_change_ids.insert(change.id.clone());
                self.current_change_id = None;
                self.apply_counts.remove(&change.id);
                self.stall_detector.clear_change(&change.id);

                Ok(ChangeProcessResult::Archived)
            }
            Ok(ArchiveResult::Stalled { error }) => {
                self.mark_stalled(&change.id, &error);
                Ok(ChangeProcessResult::Stalled { error })
            }
            Ok(ArchiveResult::Failed { error }) => Ok(ChangeProcessResult::Failed { error }),
            Ok(ArchiveResult::Cancelled) => Ok(ChangeProcessResult::Cancelled),
            Err(e) => Err(e),
        }
    }

    /// Internal method to apply a change
    #[allow(clippy::too_many_arguments)]
    async fn apply_change_internal<O: OutputHandler, F, G>(
        &mut self,
        change: &Change,
        agent: &mut AgentRunner,
        ai_runner: &AiCommandRunner,
        hooks: &HookRunner,
        output: &O,
        total_changes: usize,
        remaining_changes: usize,
        _apply_count: u32,
        cancel_check: &F,
        is_single_change_stopped: &G,
        operation_tracker: Option<std::sync::Arc<std::sync::RwLock<String>>>,
    ) -> Result<ChangeProcessResult>
    where
        F: Fn() -> bool + Clone + Send + 'static,
        G: Fn() -> bool + Clone,
    {
        info!("Applying change: {}", change.id);

        // Create event handler for apply loop
        let event_handler = SerialApplyEventHandler::new(output);

        // Create hook context for apply loop
        let hook_ctx = common_apply::ApplyLoopHookContext::serial(
            self.changes_processed,
            total_changes,
            remaining_changes,
        );

        // Create a cancellation token and spawn a background task to poll cancel_check
        // This allows us to bridge the cancel_check closure to CancellationToken
        let cancel_token = CancellationToken::new();
        let cancel_token_for_task = cancel_token.clone();
        let cancel_check_clone = cancel_check.clone();
        let cancel_task = tokio::spawn(async move {
            loop {
                if cancel_check_clone() {
                    cancel_token_for_task.cancel();
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });

        // Execute apply loop using common implementation
        let apply_result = match common_apply::execute_apply_loop(
            &change.id,
            &self.repo_root,
            &self.config,
            agent,
            VcsBackend::Git,
            None, // workspace_manager (None for serial mode)
            Some(hooks),
            &hook_ctx,
            &event_handler,
            Some(&cancel_token), // Pass cancel_token to enable apply loop cancellation
            ai_runner,
            |line| async move {
                match &line {
                    OutputLine::Stdout(s) => output.on_stdout(s),
                    OutputLine::Stderr(s) => output.on_agent_stderr(s),
                }
            },
        )
        .await
        {
            Ok(result) => result,
            Err(crate::error::OrchestratorError::PermissionBlocked {
                denied_path,
                guidance,
            }) => {
                // Abort the background cancel monitoring task
                cancel_task.abort();

                // Mark as stalled with permission guidance
                let error_message = format!(
                    "Permission auto-rejected for: {}\n{}",
                    denied_path, guidance
                );
                self.mark_stalled(&change.id, &error_message);
                return Ok(ChangeProcessResult::Stalled {
                    error: error_message,
                });
            }
            Err(e) => {
                // Abort the background cancel monitoring task
                cancel_task.abort();
                return Err(e);
            }
        };

        // Abort the background cancel monitoring task now that apply is complete
        cancel_task.abort();

        let apply_blocked_handoff = apply_result.blocked_handoff.clone();

        // Check if apply loop completed successfully or detected blocked handoff.
        if apply_result.completed || apply_blocked_handoff.is_some() {
            if apply_result.completed {
                info!(
                    "Apply loop completed for {} after {} iterations",
                    change.id, apply_result.iterations
                );
            } else if let Some(ref handoff) = apply_blocked_handoff {
                warn!(
                    change_id = %change.id,
                    blocker_path = %handoff.blocker_path.display(),
                    iterations = apply_result.iterations,
                    "Apply blocked handoff detected; keeping change blocked with preserved worktree context"
                );
            }

            // Increment apply count for this change
            self.increment_apply_count(&change.id);

            // Re-fetch change to get updated task counts after apply
            let (updated_change, is_complete) = self.refetch_change_after_apply(&change.id);

            if is_complete || apply_blocked_handoff.is_some() {
                let updated_change = updated_change.unwrap_or_else(|| change.clone());

                if let Some(ref handoff) = apply_blocked_handoff {
                    warn!(
                        change_id = %change.id,
                        blocker_path = %handoff.blocker_path.display(),
                        "Apply reported recoverable blocker; leaving change stalled for explicit unblock/resume"
                    );

                    Ok(ChangeProcessResult::Stalled {
                        error: format!(
                            "Apply blocked handoff recorded at {}",
                            handoff.blocker_path.display()
                        ),
                    })
                } else {
                    info!(
                        "Tasks complete for {}, running acceptance test...",
                        change.id
                    );

                    self.seed_active_run_acceptance_history(&change.id, agent);

                    // Update operation to "acceptance" before running acceptance test
                    Self::update_operation_tracker(&operation_tracker, "acceptance");

                    self.run_acceptance_loop(
                        &updated_change,
                        agent,
                        ai_runner,
                        output,
                        cancel_check,
                        is_single_change_stopped,
                    )
                    .await
                }
            } else {
                info!(
                    "Apply completed for {}, but tasks not yet complete",
                    change.id
                );
                Ok(ChangeProcessResult::ApplySuccessIncomplete)
            }
        } else {
            error!(
                "Apply loop did not complete for {} after {} iterations",
                change.id, apply_result.iterations
            );
            Ok(ChangeProcessResult::ApplyFailed {
                error: format!(
                    "Apply loop did not complete after {} iterations",
                    apply_result.iterations
                ),
            })
        }
    }

    /// Run acceptance for a change whose tasks are complete.
    ///
    /// Shared by the post-apply path, the restart fail-safe rerun, and explicit
    /// acceptance-only retry, so every serial entry into acceptance uses the same
    /// protocol budget and the same blocker decisions as parallel dispatch.
    ///
    /// A completed command with no canonical verdict is a protocol failure, so
    /// the normal configured acceptance command is re-invoked with continuation
    /// context while the shared missing-verdict budget remains. That budget is
    /// active-run memory only and never touches the configured explicit-CONTINUE
    /// budget.
    async fn run_acceptance_loop<O: OutputHandler, F, G>(
        &mut self,
        change: &Change,
        agent: &mut AgentRunner,
        ai_runner: &AiCommandRunner,
        output: &O,
        cancel_check: &F,
        is_single_change_stopped: &G,
    ) -> Result<ChangeProcessResult>
    where
        F: Fn() -> bool + Clone + Send + 'static,
        G: Fn() -> bool + Clone,
    {
        let mut protocol = AcceptanceProtocolDriver::default();
        loop {
            match acceptance_test_streaming(
                change,
                agent,
                ai_runner,
                &self.config,
                output,
                cancel_check,
                protocol.take_protocol_retry(),
            )
            .await
            {
                Ok((AcceptanceResult::MissingVerdict { findings }, _attempt_number, _command)) => {
                    match protocol.observe_missing_verdict(&findings) {
                        MissingVerdictRetryStep::Retry { progress, .. } => {
                            warn!("{} for {}", progress, change.id);
                            output.on_warn(&progress);
                            continue;
                        }
                        MissingVerdictRetryStep::Exhausted { error } => {
                            error!("{} for {}", error, change.id);
                            break Ok(ChangeProcessResult::AcceptanceCommandFailed { error });
                        }
                    }
                }
                Ok((
                    result @ (AcceptanceResult::BareBlocker { .. }
                    | AcceptanceResult::Stalled { .. }),
                    _attempt_number,
                    _command,
                )) => {
                    // Same shared decision API as parallel dispatch: bare or
                    // invalid compatibility input gets bounded acceptance-only
                    // retry; only a validated blocker becomes a durable
                    // revision-bound stall.
                    match decide_acceptance_blocker(&mut protocol, &result) {
                        Some(AcceptanceBlockerDecision::ProtocolRetry { progress, .. }) => {
                            warn!("{} for {}", progress, change.id);
                            output.on_warn(&progress);
                            continue;
                        }
                        Some(AcceptanceBlockerDecision::ProtocolExhausted { error }) => {
                            error!("{} for {}", error, change.id);
                            break Ok(ChangeProcessResult::AcceptanceCommandFailed { error });
                        }
                        Some(AcceptanceBlockerDecision::Stall { blocker }) => {
                            break Ok(self.record_acceptance_stall(&change.id, &blocker).await);
                        }
                        None => unreachable!(
                            "decide_acceptance_blocker owns every blocker-bearing result"
                        ),
                    }
                }
                Ok((
                    AcceptanceResult::PermissionStalled { blocker },
                    _attempt_number,
                    _command,
                )) => {
                    // A repeated unresolved permission/tool-policy denial carries
                    // an explicitly classified category, so it enters the same
                    // durable hold without prose inference.
                    protocol.observe_canonical_verdict();
                    let stall = crate::acceptance::AcceptanceBlocker {
                        category: "policy".to_string(),
                        evidence: blocker.evidence.clone(),
                        next_action: blocker.next_action.clone(),
                        resumable: blocker.resumable,
                        prerequisite_owner: None,
                        evidence_ids: vec![blocker.category.clone()],
                    };
                    break Ok(self.record_acceptance_stall(&change.id, &stall).await);
                }
                Ok((result, _attempt_number, _command)) => {
                    protocol.observe_canonical_verdict();
                    let repo_root = self.repo_root.clone();
                    break Ok(self.process_acceptance_result(
                        &change.id,
                        &repo_root,
                        agent,
                        result,
                        is_single_change_stopped,
                    ));
                }
                Err(e) => {
                    error!("Acceptance error for {}: {}", change.id, e);
                    break Err(e);
                }
            }
        }
    }

    /// Check stall detection after apply
    pub fn check_stall_after_apply(
        &mut self,
        change_id: &str,
        progress: &TaskProgress,
        is_empty_commit: Option<bool>,
    ) -> Option<String> {
        if let Some(is_empty) = is_empty_commit {
            if !is_progress_complete(progress)
                && self
                    .stall_detector
                    .register_commit(change_id, StallPhase::Apply, is_empty)
            {
                let count = self
                    .stall_detector
                    .current_count(change_id, StallPhase::Apply);
                let threshold = self.stall_detector.config().threshold;
                let message = format!(
                    "Stall detected for {} after {} empty WIP commits (apply)",
                    change_id, count
                );
                return Some(format!("{} (threshold {})", message, threshold));
            }
        }
        None
    }

    /// Re-fetch change to get updated task counts after apply.
    ///
    /// Returns the updated change and whether it's complete.
    fn refetch_change_after_apply(&self, change_id: &str) -> (Option<Change>, bool) {
        let updated_changes =
            openspec::list_changes_native_from(&self.repo_root).unwrap_or_default();
        let updated_change = updated_changes.iter().find(|c| c.id == change_id).cloned();
        let is_complete = updated_change.as_ref().is_some_and(|c| c.is_complete());
        (updated_change, is_complete)
    }

    /// Process acceptance test result and determine outcome.
    ///
    /// Handles Pass, Continue, Fail, CommandFailed, and Cancelled results,
    /// applying max_continues logic for Continue results.
    fn process_acceptance_result<F>(
        &mut self,
        change_id: &str,
        workspace_path: &std::path::Path,
        agent: &AgentRunner,
        acceptance_result: AcceptanceResult,
        is_single_change_stopped: F,
    ) -> ChangeProcessResult
    where
        F: Fn() -> bool,
    {
        match acceptance_result {
            AcceptanceResult::Pass => {
                // PASS is handed off to archive through active-run control flow
                // only; nothing durable records the verdict, so a restart
                // re-runs acceptance instead of archiving on an inferred PASS.
                self.acceptance_retry.remove(change_id);
                self.accepted_change_ids.insert(change_id.to_string());
                info!("Acceptance passed for {}, ready for archive", change_id);
                match task_parser::resolve_acceptance_follow_up_tasks_path_for_cleanup(
                    change_id,
                    workspace_path,
                ) {
                    Ok(Some(tasks_path)) => {
                        match task_parser::clear_acceptance_follow_up(&tasks_path) {
                            Ok(recovery) => {
                                if let Some(warning) = recovery.warning() {
                                    warn!(
                                        "Acceptance follow-up recovery for {} at {}: {}",
                                        change_id,
                                        tasks_path.display(),
                                        warning
                                    );
                                }
                            }
                            Err(err) => {
                                return ChangeProcessResult::AcceptanceCommandFailed {
                                    error: format!(
                                        "Acceptance passed but follow-up cleanup failed at {}: {}",
                                        tasks_path.display(),
                                        err
                                    ),
                                };
                            }
                        }
                    }
                    Ok(None) => debug!("No acceptance follow-up to clear for {}", change_id),
                    Err(err) => {
                        return ChangeProcessResult::AcceptanceCommandFailed {
                            error: format!(
                                "Acceptance passed but follow-up path resolution failed: {}",
                                err
                            ),
                        };
                    }
                }
                ChangeProcessResult::AcceptancePassed
            }
            AcceptanceResult::Continue => {
                let continue_count = agent.count_consecutive_acceptance_continues(change_id);
                let max_continues = self.config.get_acceptance_max_continues();

                if continue_count >= max_continues {
                    let semantic_fingerprint = semantic_progress_fingerprint(workspace_path).ok();
                    self.set_acceptance_retry_context(
                        change_id,
                        AcceptanceRetryContext {
                            finding_identities: Vec::new(),
                            semantic_fingerprint,
                            cycle_count: continue_count,
                        },
                    );
                    warn!(
                        "Acceptance CONTINUE limit ({}) exceeded for {}, treating as FAIL",
                        max_continues, change_id
                    );
                    ChangeProcessResult::AcceptanceContinueExceeded
                } else {
                    info!(
                        "Acceptance requires continuation for {} (attempt {}/{}), retrying...",
                        change_id, continue_count, max_continues
                    );
                    ChangeProcessResult::AcceptanceContinue
                }
            }
            AcceptanceResult::BareBlocker { rejection } => {
                // The acceptance loop owns the bounded protocol budget for this
                // result and only reaches terminal routing after exhaustion, so
                // this arm reports the exhausted diagnostic. No stalled state,
                // blocker category, or change-directory artifact is created.
                let error = crate::orchestration::acceptance::protocol_exhausted_error(
                    crate::orchestration::acceptance::AcceptanceProtocolError::BareBlocker,
                    MAX_MISSING_VERDICT_RETRIES.saturating_add(1),
                    MAX_MISSING_VERDICT_RETRIES,
                    &[rejection.reason()],
                );
                error!("{} for {}", error, change_id);
                ChangeProcessResult::AcceptanceCommandFailed { error }
            }
            AcceptanceResult::Stalled { blocker } => {
                // Durable persistence happens in the async acceptance loop, which
                // owns repository facts and the revision binding. Reaching here
                // means a caller bypassed that loop; report the hold truthfully
                // without inventing a record.
                warn!(
                    "Acceptance stalled for {} on a validated external blocker ({})",
                    change_id, blocker.category
                );
                ChangeProcessResult::Stalled {
                    error: format!("{}: {}", blocker.category, blocker.next_action),
                }
            }
            AcceptanceResult::Fail { findings } => {
                // Retry context comes from this run only. After a restart the
                // map is empty, so the next failure is treated as the first one
                // and acceptance is retried rather than skipped.
                let previous = self.acceptance_retry.get(change_id).cloned();
                let retry_count = previous.as_ref().map_or_else(
                    || {
                        agent
                            .get_last_acceptance_attempt(change_id)
                            .map(|attempt| attempt.attempt)
                            .unwrap_or(1)
                    },
                    |context| context.cycle_count.saturating_add(1),
                );
                let fingerprint = match semantic_progress_fingerprint(workspace_path) {
                    Ok(fingerprint) => fingerprint,
                    Err(error) => {
                        return ChangeProcessResult::AcceptanceCommandFailed {
                            error: format!("Failed to fingerprint acceptance progress: {error}"),
                        };
                    }
                };
                let normalized = normalize_findings(&findings);
                let identities = normalized
                    .iter()
                    .map(|finding| finding.identity.clone())
                    .collect::<Vec<_>>();
                let decision = decide_acceptance_retry(
                    previous.as_ref().map_or(
                        &[] as &[String],
                        AcceptanceRetryContext::previous_identities,
                    ),
                    previous
                        .as_ref()
                        .and_then(AcceptanceRetryContext::previous_fingerprint),
                    &normalized,
                    &fingerprint,
                    retry_count,
                );
                let retry = AcceptanceRetryContext {
                    finding_identities: identities.clone(),
                    semantic_fingerprint: Some(fingerprint),
                    cycle_count: retry_count,
                };
                self.set_acceptance_retry_context(change_id, retry.clone());
                if let AcceptanceRetryDecision::Stall {
                    reason,
                    external_blockers,
                } = decision
                {
                    // Repeated findings and cycle exhaustion are runtime retry
                    // judgements, not reviewer-validated external blockers. They
                    // carry no explicit category, so they must not fabricate one
                    // or create a durable hold; they stop and require explicit
                    // retry. Parallel dispatch makes the identical decision.
                    let error = format!(
                        "Acceptance stopped retrying {change_id} ({reason}). External blocker \
                         context: {}. Explicit retry is required.",
                        if external_blockers.is_empty() {
                            "none reported".to_string()
                        } else {
                            external_blockers.join(" | ")
                        }
                    );
                    error!("{error}");
                    return ChangeProcessResult::AcceptanceCommandFailed { error };
                }
                let blocking_gate_context = findings
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "no acceptance findings captured".to_string());
                warn!(
                    "Acceptance failed for {} ({} findings), blocking gate context: {}; will retry apply",
                    change_id,
                    findings.len(),
                    blocking_gate_context
                );
                let repository_findings = repository_findings(&findings);
                if !findings.is_empty() {
                    if let Ok(tasks_path) = task_parser::resolve_acceptance_follow_up_tasks_path(
                        change_id,
                        workspace_path,
                    ) {
                        match task_parser::replace_acceptance_follow_up_from_latest_fail(
                            &tasks_path,
                            agent
                                .get_last_acceptance_attempt(change_id)
                                .map(|attempt| attempt.attempt)
                                .unwrap_or(1),
                            &findings,
                        ) {
                            Ok(recovery) => {
                                if let Some(warning) = recovery.warning() {
                                    warn!(
                                        "Acceptance follow-up recovery for {} at {}: {}",
                                        change_id,
                                        tasks_path.display(),
                                        warning
                                    );
                                }
                            }
                            // Acceptance FAIL remains the primary diagnosis; persistence
                            // degradation is supplemental context only.
                            Err(err) => warn!(
                                "Acceptance follow-up persistence degraded for {} at {}: {}",
                                change_id,
                                tasks_path.display(),
                                err
                            ),
                        }
                    }
                }
                ChangeProcessResult::AcceptanceFailed {
                    findings: repository_findings,
                }
            }
            AcceptanceResult::CommandFailed {
                error,
                findings: _findings,
            } => {
                error!("Acceptance command failed for {}: {}", change_id, error);
                // Canonical owner note: runtime appends follow-up tasks for FAIL verdicts,
                // while command-level failures are surfaced without forcing local tasks.md updates.
                ChangeProcessResult::AcceptanceCommandFailed { error }
            }
            AcceptanceResult::MissingVerdict { findings } => {
                // A completed acceptance command with no canonical verdict is a
                // protocol failure, not an intentional CONTINUE. The acceptance
                // retry loop owns the dedicated protocol budget and only reaches
                // terminal routing after exhaustion, so this arm reports the
                // exhausted diagnostic with bounded output evidence.
                let error = missing_verdict_exhausted_error(
                    MAX_MISSING_VERDICT_RETRIES.saturating_add(1),
                    MAX_MISSING_VERDICT_RETRIES,
                    &findings,
                );
                error!("{} for {}", error, change_id);
                ChangeProcessResult::AcceptanceCommandFailed { error }
            }
            AcceptanceResult::PermissionStalled { blocker } => {
                // Durable persistence happens in the async acceptance loop; this
                // arm only classifies the outcome. Nothing is written into the
                // change directory.
                warn!(
                    "Acceptance stalled for {} due to repeated unresolved permission/tool policy blocker: {}",
                    change_id, blocker.next_action
                );
                ChangeProcessResult::Stalled {
                    error: blocker.next_action,
                }
            }
            AcceptanceResult::Cancelled => {
                // Check if this is a single-change stop or global cancel
                if is_single_change_stopped() {
                    info!("Single change {} stopped during acceptance", change_id);
                    ChangeProcessResult::ChangeStopped
                } else {
                    info!("Acceptance cancelled for {} (global cancel)", change_id);
                    ChangeProcessResult::Cancelled
                }
            }
        }
    }

    /// Update operation tracker with the current operation name.
    ///
    /// This is a helper to centralize tracker updates for both apply and acceptance flows.
    fn update_operation_tracker(
        operation_tracker: &Option<std::sync::Arc<std::sync::RwLock<String>>>,
        operation: &str,
    ) {
        if let Some(ref tracker) = operation_tracker {
            *tracker.write().unwrap() = operation.to_string();
        }
    }
}

/// Result of processing a single change
#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants may not be used yet depending on mode
pub enum ChangeProcessResult {
    /// Change was successfully archived
    Archived,
    /// Change was stalled
    Stalled { error: String },
    /// Acceptance is holding on a validated external blocker.
    ///
    /// Carries the structured blocker so serial consumers can present the same
    /// operator-facing `stalled` state as parallel's `AcceptanceGated` event
    /// instead of degrading a validated hold into an opaque processing error.
    AcceptanceStalled {
        blocker: crate::events::StalledBlocker,
        error: String,
    },
    /// Archive or apply failed
    Failed { error: String },
    /// Operation was cancelled (global stop)
    Cancelled,
    /// Single change was stopped (not a global cancel)
    ChangeStopped,
    /// Apply succeeded but tasks not yet complete
    ApplySuccessIncomplete,
    /// Apply failed
    ApplyFailed { error: String },
    /// Acceptance test passed
    AcceptancePassed,
    /// Acceptance test failed
    AcceptanceFailed { findings: Vec<String> },
    /// Acceptance test command failed
    AcceptanceCommandFailed { error: String },
    /// Acceptance test requires continuation
    AcceptanceContinue,
    /// Acceptance CONTINUE limit exceeded
    AcceptanceContinueExceeded,
    /// Acceptance gated and change was rejected
    Rejected { reason: String },
}

impl ChangeProcessResult {
    /// Lifecycle events a serial consumer must publish for this result.
    ///
    /// A validated acceptance stall maps to the same `AcceptanceGated` event
    /// parallel dispatch emits, so both modes reach the operator-facing
    /// `stalled` state carrying the same structured evidence.
    ///
    /// Deliberately *not* followed by a generic `Blocked` workspace status:
    /// serial has no managed worktree to report on, and that handler rewrites
    /// the blocker metadata with a generic summary, discarding the category,
    /// evidence, and next action this hold exists to preserve.
    ///
    /// Every other result publishes nothing here and keeps its existing routing.
    pub fn stalled_lifecycle_events(&self, change_id: &str) -> Vec<crate::events::ExecutionEvent> {
        match self {
            ChangeProcessResult::AcceptanceStalled { blocker, .. } => {
                vec![crate::events::ExecutionEvent::AcceptanceGated {
                    change_id: change_id.to_string(),
                    blocker: blocker.clone(),
                }]
            }
            _ => Vec::new(),
        }
    }
}

/// Helper function to check if progress is complete
fn is_progress_complete(progress: &TaskProgress) -> bool {
    progress.total > 0 && progress.completed >= progress.total
}

/// Event handler for serial apply loop that delegates to OutputHandler
struct SerialApplyEventHandler<'a, O: OutputHandler> {
    #[allow(dead_code)] // Kept for type safety but not used since output is handled via closure
    output: &'a O,
}

impl<'a, O: OutputHandler> SerialApplyEventHandler<'a, O> {
    fn new(output: &'a O) -> Self {
        Self { output }
    }
}

impl<'a, O: OutputHandler> common_apply::ApplyEventHandler for SerialApplyEventHandler<'a, O> {
    fn on_apply_started(&self, _change_id: &str, _command: &str) {
        // No-op for serial mode - output is handled via output_handler closure
    }

    fn on_progress_updated(&self, _change_id: &str, _completed: u32, _total: u32) {
        // No-op for serial mode - progress is logged in execute_apply_loop
    }

    fn on_hook_started(&self, _change_id: &str, _hook_type: &str) {
        // No-op for serial mode - hooks log themselves
    }

    fn on_hook_completed(&self, _change_id: &str, _hook_type: &str) {
        // No-op for serial mode - hooks log themselves
    }

    fn on_hook_failed(&self, _change_id: &str, _hook_type: &str, _error: &str) {
        // No-op for serial mode - hooks log themselves
    }

    fn on_apply_output(&self, _change_id: &str, _line: &OutputLine, _iteration: u32) {
        // No-op: Output is already handled by the output_handler closure passed to execute_apply_loop
        // (lines 398-403). Having both would cause duplicate output.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::default_retry_patterns;
    use crate::config::OrchestratorConfig;
    use crate::hooks::{HookRunner, HooksConfig};
    use crate::openspec::ProposalMetadata;
    use crate::orchestration::output::NullOutputHandler;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    fn create_test_change(id: &str, completed: u32, total: u32) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified: "1m ago".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }
    }

    #[test]
    fn test_select_next_change_prioritizes_progress() {
        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let changes = vec![
            create_test_change("a", 1, 10), // 10% progress
            create_test_change("b", 5, 10), // 50% progress
            create_test_change("c", 8, 10), // 80% progress (highest)
        ];

        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("c"));
    }

    #[test]
    fn test_select_next_change_excludes_stalled() {
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        service.mark_stalled("b", "test");

        let changes = vec![
            create_test_change("a", 1, 10),
            create_test_change("b", 8, 10), // Highest progress but stalled
            create_test_change("c", 5, 10),
        ];

        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("c")); // Should pick 'c', not 'b'
    }

    #[test]
    fn test_select_next_change_prioritizes_complete_for_archive() {
        let temp_dir = TempDir::new().unwrap();
        let service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let changes = vec![
            create_test_change("a", 5, 10),  // 50% progress, incomplete
            create_test_change("b", 10, 10), // 100% complete
        ];

        let next = service.select_next_change(&changes);
        // Should select the incomplete one first (archive happens in a separate phase in practice,
        // but select_next_change returns the first match which would be 'b' if it's complete)
        // Actually, reading the implementation, it prioritizes incomplete first, so should be 'a'
        assert_eq!(next.map(|c| c.id.as_str()), Some("a"));
    }

    fn serial_test_ai_runner() -> AiCommandRunner {
        AiCommandRunner::new(
            CommandQueueConfig {
                stagger_delay_ms: 0,
                max_retries: 0,
                retry_delay_ms: 0,
                retry_error_patterns: default_retry_patterns(),
                retry_if_duration_under_secs: 0,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 0,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
            },
            Arc::new(Mutex::new(None)),
        )
    }

    fn init_serial_repo(root: &std::path::Path, change_id: &str) -> std::path::PathBuf {
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test User"],
        ] {
            std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap();
        }
        let change_dir = root.join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("proposal.md"), "# serial restart\n").unwrap();
        std::fs::write(change_dir.join("tasks.md"), "- [ ] pending\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(root)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(root)
            .output()
            .unwrap();
        change_dir
    }

    fn serial_failing_acceptance_config(change_id: &str) -> OrchestratorConfig {
        OrchestratorConfig {
            // Apply checks off every open box, including acceptance follow-up
            // entries, so repeated cycles converge instead of exhausting the
            // apply iteration budget.
            apply_command: Some(format!(
                "sh -c \"sed 's/- \\[ \\]/- [x]/g' openspec/changes/{change_id}/tasks.md \
                 > openspec/changes/{change_id}/tasks.next \
                 && mv openspec/changes/{change_id}/tasks.next openspec/changes/{change_id}/tasks.md\""
            )),
            acceptance_command: Some(
                "sh -c 'echo ACCEPTANCE: FAIL; echo FINDINGS:; echo - repeated serial finding'"
                    .to_string(),
            ),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn serial_active_run_accumulates_acceptance_retry_context_without_a_checkpoint_file() {
        let temp_dir = TempDir::new().unwrap();
        let change_id = "serial-restart";
        init_serial_repo(temp_dir.path(), change_id);

        let config = serial_failing_acceptance_config(change_id);
        let mut service = SerialRunService::new(temp_dir.path().to_path_buf(), config.clone());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = serial_test_ai_runner();

        // First failure of this run: retry context does not exist yet, so the
        // change returns to apply instead of stalling.
        assert!(service.acceptance_retry_context(change_id).is_none());
        let result = service
            .process_change(
                &create_test_change(change_id, 0, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { .. }
        ));
        let context = service.acceptance_retry_context(change_id).unwrap();
        assert_eq!(context.cycle_count, 1);
        assert_eq!(
            context.finding_identities,
            ["repository|repeated serial finding|implementation"]
        );
        assert!(!temp_dir.path().join(".cflx/acceptance-state.json").exists());

        // Second identical failure in the same run reuses the in-memory
        // baseline and stalls with a tracked marker.
        let result = service
            .process_change(
                &create_test_change(change_id, 0, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        // Repeated findings without progress stop the retry loop and require an
        // explicit retry. No category is invented, and nothing is written into
        // the change directory or a generated checkpoint.
        match &result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(error.contains("repeated_acceptance_findings"), "{error}");
                assert!(error.contains("Explicit retry is required"), "{error}");
            }
            other => panic!("expected a retry-exhaustion stop, got {other:?}"),
        }
        assert!(crate::parallel::acceptance_state::parse_blocked_marker(
            temp_dir.path(),
            change_id
        )
        .unwrap()
        .is_none());
        assert!(!temp_dir.path().join(".cflx/acceptance-state.json").exists());
        assert_eq!(
            service
                .acceptance_retry_context(change_id)
                .map(|context| context.cycle_count),
            Some(2),
            "in-memory retry context still tracks the sequence"
        );
    }

    #[tokio::test]
    async fn serial_restart_reruns_acceptance_without_reconstructing_retry_context() {
        let temp_dir = TempDir::new().unwrap();
        let change_id = "serial-restart";
        init_serial_repo(temp_dir.path(), change_id);

        // A leftover checkpoint from an older Conflux version claims a nearly
        // exhausted retry budget. It must be ignored entirely.
        let stale_checkpoint = temp_dir.path().join(".cflx/acceptance-state.json");
        std::fs::create_dir_all(stale_checkpoint.parent().unwrap()).unwrap();
        std::fs::write(
            &stale_checkpoint,
            "{\"state\":\"failed\",\"revision\":\"old\",\"updated_at\":\"now\",             \"workspace_path\":\".\",\"change_id\":\"serial-restart\",             \"previous_finding_identities\":[\"repository|repeated serial finding|implementation\"],             \"semantic_fingerprint\":\"stale\",\"cycle_count\":9}",
        )
        .unwrap();

        let config = serial_failing_acceptance_config(change_id);
        let mut service = SerialRunService::new(temp_dir.path().to_path_buf(), config.clone());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = serial_test_ai_runner();

        let result = service
            .process_change(
                &create_test_change(change_id, 0, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        // Acceptance actually ran and produced a first-failure verdict rather
        // than resuming the stale cycle count or inferring a prior PASS.
        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { .. }
        ));
        assert_eq!(
            service
                .acceptance_retry_context(change_id)
                .unwrap()
                .cycle_count,
            1
        );
        assert!(crate::parallel::acceptance_state::parse_blocked_marker(
            temp_dir.path(),
            change_id
        )
        .unwrap()
        .is_none());
    }

    /// Configure a stateful fake acceptance command that withholds a canonical
    /// verdict for its first `missing_attempts` invocations, then emits
    /// `ACCEPTANCE: PASS`. It is the ordinary configured acceptance command on
    /// every invocation — no harness session, resume flag, or job identifier.
    fn serial_missing_verdict_config(
        change_id: &str,
        state_dir: &std::path::Path,
        missing_attempts: u32,
    ) -> OrchestratorConfig {
        let counter = state_dir.join("attempts").display().to_string();
        let prompts = state_dir.join("prompts").display().to_string();
        std::fs::create_dir_all(state_dir.join("prompts")).unwrap();
        OrchestratorConfig {
            acceptance_command: Some(format!(
                "sh -c 'n=$(cat \"{counter}\" 2>/dev/null || echo 0); n=$((n+1)); \
                 echo $n > \"{counter}\"; printf \"%s\" \"$0\" > \"{prompts}/attempt-$n.txt\"; \
                 if [ $n -gt {missing_attempts} ]; then echo \"ACCEPTANCE: PASS\"; \
                 else echo \"Monitoring verification, waiting for the owned job to finish\"; fi' \
                 {{prompt}}"
            )),
            ..serial_failing_acceptance_config(change_id)
        }
    }

    fn serial_acceptance_invocations(state_dir: &std::path::Path) -> u32 {
        std::fs::read_to_string(state_dir.join("attempts"))
            .map(|text| text.trim().parse().unwrap_or(0))
            .unwrap_or(0)
    }

    fn serial_acceptance_prompt(state_dir: &std::path::Path, attempt: u32) -> String {
        std::fs::read_to_string(
            state_dir
                .join("prompts")
                .join(format!("attempt-{attempt}.txt")),
        )
        .unwrap_or_default()
    }

    async fn run_serial_missing_verdict_change(
        temp_dir: &std::path::Path,
        state_dir: &std::path::Path,
        change_id: &str,
        missing_attempts: u32,
    ) -> ChangeProcessResult {
        let config = serial_missing_verdict_config(change_id, state_dir, missing_attempts);
        let mut service = SerialRunService::new(temp_dir.to_path_buf(), config.clone());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = serial_test_ai_runner();

        service
            .process_change(
                &create_test_change(change_id, 0, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap()
    }

    /// Configure a stateful fake acceptance command that emits bare
    /// `ACCEPTANCE: GATED` for its first `bare_attempts` invocations and then
    /// the `final_verdict` payload.
    fn serial_bare_gated_config(
        change_id: &str,
        state_dir: &std::path::Path,
        bare_attempts: u32,
        final_verdict: &str,
    ) -> OrchestratorConfig {
        let counter = state_dir.join("attempts").display().to_string();
        let prompts = state_dir.join("prompts").display().to_string();
        std::fs::create_dir_all(state_dir.join("prompts")).unwrap();
        // The final verdict is read from a file so JSON quoting survives the
        // shell round trip intact.
        let verdict_file = state_dir.join("final-verdict.txt");
        std::fs::write(&verdict_file, final_verdict).unwrap();
        let verdict_file = verdict_file.display().to_string();
        OrchestratorConfig {
            acceptance_command: Some(format!(
                "sh -c 'n=$(cat \"{counter}\" 2>/dev/null || echo 0); n=$((n+1)); \
                 echo $n > \"{counter}\"; printf \"%s\" \"$0\" > \"{prompts}/attempt-$n.txt\"; \
                 if [ $n -gt {bare_attempts} ]; then cat \"{verdict_file}\"; \
                 else echo \"ACCEPTANCE: GATED\"; fi' {{prompt}}"
            )),
            ..serial_failing_acceptance_config(change_id)
        }
    }

    async fn run_serial_bare_gated_change(
        temp_dir: &std::path::Path,
        state_dir: &std::path::Path,
        change_id: &str,
        bare_attempts: u32,
        final_verdict: &str,
        stall_state_root: &std::path::Path,
    ) -> ChangeProcessResult {
        let config = serial_bare_gated_config(change_id, state_dir, bare_attempts, final_verdict);
        let mut service = SerialRunService::new(temp_dir.to_path_buf(), config.clone());
        service.set_acceptance_stall_state_root(stall_state_root.to_path_buf());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = serial_test_ai_runner();

        service
            .process_change(
                &create_test_change(change_id, 0, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap()
    }

    fn serial_porcelain_status(repo_root: &std::path::Path) -> String {
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo_root)
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    }

    /// Assert acceptance contributed nothing to the working tree. Serial mode
    /// does not commit apply, so the apply-owned `tasks.md` edit is expected;
    /// what must never appear is an acceptance-generated artifact.
    fn assert_acceptance_added_no_worktree_artifact(repo_root: &std::path::Path, change_id: &str) {
        let status = serial_porcelain_status(repo_root);
        assert!(
            !status.contains("APPLY_BLOCKED"),
            "acceptance must not create a blocked marker: {status}"
        );
        assert!(
            !status.contains(".cflx/"),
            "acceptance must not create a generated checkpoint: {status}"
        );
        assert!(
            !repo_root
                .join("openspec/changes")
                .join(change_id)
                .join("APPLY_BLOCKED")
                .exists(),
            "no acceptance marker directory may exist"
        );
    }

    /// gated → gated → PASS: two acceptance-only retries, no apply rerun, no
    /// stalled transition, and a clean worktree throughout.
    #[tokio::test]
    async fn serial_bare_gated_retries_then_accepts_canonical_pass() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let change_id = "serial-bare-gated-pass";
        init_serial_repo(temp_dir.path(), change_id);

        let stall_state = TempDir::new().unwrap();
        let result = run_serial_bare_gated_change(
            temp_dir.path(),
            state_dir.path(),
            change_id,
            2,
            "ACCEPTANCE: PASS\n",
            stall_state.path(),
        )
        .await;

        assert!(
            matches!(result, ChangeProcessResult::AcceptancePassed),
            "a canonical PASS after bare-gated retries must pass, got {result:?}"
        );
        assert_eq!(
            serial_acceptance_invocations(state_dir.path()),
            3,
            "the initial attempt plus exactly two protocol retries must run"
        );

        assert!(
            !serial_acceptance_prompt(state_dir.path(), 1).contains("<acceptance_protocol_retry>")
        );
        for attempt in [2, 3] {
            let prompt = serial_acceptance_prompt(state_dir.path(), attempt);
            assert!(
                prompt.contains("<acceptance_protocol_retry>"),
                "retry {attempt} must carry corrective context"
            );
            assert!(
                prompt.contains("without a validated structured blocker"),
                "retry {attempt} must name the bare-blocker protocol failure"
            );
            assert!(
                prompt.contains("Supported categories:"),
                "retry {attempt} must list the supported categories"
            );
        }

        // No change-directory artifact and no durable stalled record.
        assert_acceptance_added_no_worktree_artifact(temp_dir.path(), change_id);
    }

    /// Three consecutive bare-gated results exhaust the shared budget and become
    /// a terminal protocol error requiring explicit retry.
    #[tokio::test]
    async fn serial_bare_gated_exhaustion_is_terminal_and_creates_no_hold() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let change_id = "serial-bare-gated-exhausted";
        init_serial_repo(temp_dir.path(), change_id);

        let stall_state = TempDir::new().unwrap();
        let result = run_serial_bare_gated_change(
            temp_dir.path(),
            state_dir.path(),
            change_id,
            5,
            "ACCEPTANCE: PASS\n",
            stall_state.path(),
        )
        .await;

        match &result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(error.contains("bare-blocker protocol failure"), "{error}");
                assert!(
                    error.contains("Exhausted 3 consecutive attempts after 2 protocol retries"),
                    "{error}"
                );
            }
            other => panic!("exhausted bare-gated retries must be terminal, got {other:?}"),
        }
        assert!(
            !matches!(result, ChangeProcessResult::Stalled { .. }),
            "evidence-free gated input must never present as a stalled hold"
        );
        assert_eq!(
            serial_acceptance_invocations(state_dir.path()),
            3,
            "no fourth protocol retry may start after exhaustion"
        );
        assert_acceptance_added_no_worktree_artifact(temp_dir.path(), change_id);
    }

    /// A validated structured blocker stalls immediately — no protocol retry —
    /// and leaves the worktree clean with the apply revision intact.
    #[tokio::test]
    async fn serial_structured_blocker_stalls_on_the_first_result() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let stall_state = TempDir::new().unwrap();
        let change_id = "serial-structured-stall";
        init_serial_repo(temp_dir.path(), change_id);

        let verdict = concat!(
            r#"{"acceptance":"gated","blocker":{"category":"external_approval","#,
            r#""evidence":["change board ticket CB-42 awaits sign-off"],"#,
            r#""next_action":"await CB-42 then retry acceptance","resumable":true}}"#,
            "\n"
        );
        let result = run_serial_bare_gated_change(
            temp_dir.path(),
            state_dir.path(),
            change_id,
            0,
            verdict,
            stall_state.path(),
        )
        .await;

        match &result {
            ChangeProcessResult::AcceptanceStalled { blocker, error } => {
                assert!(error.starts_with("external_approval:"), "{error}");
                assert!(error.contains("await CB-42"), "{error}");
                // The structured payload survives to the consumer, so serial can
                // display `stalled` with the same evidence parallel emits.
                assert_eq!(blocker.category, "external_approval");
                assert_eq!(blocker.phase, "acceptance");
                assert_eq!(
                    blocker.evidence,
                    ["change board ticket CB-42 awaits sign-off"]
                );
                assert_eq!(blocker.next_action, "await CB-42 then retry acceptance");
                assert!(blocker.resumable);
                assert!(blocker.worktree_preserved);
            }
            other => panic!("a validated blocker must stall, got {other:?}"),
        }
        assert_eq!(
            serial_acceptance_invocations(state_dir.path()),
            1,
            "a validated blocker must not consume protocol retry budget"
        );
        assert_acceptance_added_no_worktree_artifact(temp_dir.path(), change_id);

        // The hold lives outside the worktree and carries the explicit category.
        let store =
            crate::parallel::acceptance_state::AcceptanceStallStore::new(stall_state.path());
        let record = store
            .load(
                &crate::parallel::acceptance_state::repository_identity(temp_dir.path()),
                change_id,
            )
            .unwrap()
            .expect("a validated blocker must persist a runtime hold");
        assert_eq!(record.category, "external_approval");
        assert_eq!(record.phase, "acceptance");
        assert!(record.resumable);
        assert_eq!(
            record.evidence,
            ["change board ticket CB-42 awaits sign-off"]
        );
    }

    /// Serial parity with parallel: a status-only acceptance exit re-runs the
    /// normal configured acceptance command with continuation context, and a
    /// later canonical PASS keeps its existing routing.
    #[tokio::test]
    async fn serial_missing_verdict_retries_then_passes() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let change_id = "serial-missing-verdict-pass";
        init_serial_repo(temp_dir.path(), change_id);

        let result =
            run_serial_missing_verdict_change(temp_dir.path(), state_dir.path(), change_id, 2)
                .await;

        assert!(
            matches!(result, ChangeProcessResult::AcceptancePassed),
            "a canonical PASS after protocol retries must route as AcceptancePassed, got {result:?}"
        );
        assert_eq!(
            serial_acceptance_invocations(state_dir.path()),
            3,
            "the initial attempt plus two protocol retries must run the acceptance command"
        );

        assert!(
            !serial_acceptance_prompt(state_dir.path(), 1).contains("<acceptance_protocol_retry>"),
            "the initial attempt must not receive corrective retry context"
        );
        for attempt in [2, 3] {
            let prompt = serial_acceptance_prompt(state_dir.path(), attempt);
            assert!(
                prompt.contains("<acceptance_protocol_retry>"),
                "retry {attempt} must carry the continuation context"
            );
            assert!(prompt.contains("emit exactly one canonical verdict"));
            assert!(
                prompt.contains("Monitoring verification"),
                "retry {attempt} must carry bounded prior acceptance output"
            );
            let lower = prompt.to_ascii_lowercase();
            for forbidden in ["session_id", "--resume", "job_id"] {
                assert!(
                    !lower.contains(forbidden),
                    "continuation must stay harness neutral, found `{forbidden}`"
                );
            }
        }

        assert!(
            !temp_dir.path().join("ACCEPTANCE_REPORT.json").exists(),
            "protocol retries must not create an acceptance report artifact"
        );
    }

    /// Three consecutive missing verdicts exhaust the dedicated budget and
    /// become the terminal protocol failure.
    #[tokio::test]
    async fn serial_missing_verdict_exhaustion_is_terminal() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let change_id = "serial-missing-verdict-exhausted";
        init_serial_repo(temp_dir.path(), change_id);

        let result = run_serial_missing_verdict_change(
            temp_dir.path(),
            state_dir.path(),
            change_id,
            u32::MAX,
        )
        .await;

        match result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(error.contains("missing-verdict protocol failure"));
                assert!(error.contains("Exhausted 3 consecutive attempts after 2 protocol retries"));
                assert!(
                    error.contains("Monitoring verification"),
                    "terminal diagnostic must retain bounded evidence, got: {error}"
                );
            }
            other => panic!("exhausted protocol retries must be terminal, got {other:?}"),
        }
        assert_eq!(
            serial_acceptance_invocations(state_dir.path()),
            3,
            "no fourth protocol retry may start"
        );
        assert!(!temp_dir.path().join("ACCEPTANCE_REPORT.json").exists());
    }

    /// Constitutional restart behavior: the protocol counter is active-run
    /// memory, so a fresh run re-runs acceptance with a full budget and cannot
    /// infer PASS from the previous run's narrative output.
    #[tokio::test]
    async fn serial_restart_reruns_acceptance_after_missing_verdict_exhaustion() {
        let temp_dir = TempDir::new().unwrap();
        let change_id = "serial-missing-verdict-restart";
        init_serial_repo(temp_dir.path(), change_id);

        let first_state = TempDir::new().unwrap();
        let first = run_serial_missing_verdict_change(
            temp_dir.path(),
            first_state.path(),
            change_id,
            u32::MAX,
        )
        .await;
        assert!(matches!(
            first,
            ChangeProcessResult::AcceptanceCommandFailed { .. }
        ));
        assert_eq!(serial_acceptance_invocations(first_state.path()), 3);

        // A fresh service/agent owns no prior runtime state.
        let second_state = TempDir::new().unwrap();
        let second = run_serial_missing_verdict_change(
            temp_dir.path(),
            second_state.path(),
            change_id,
            u32::MAX,
        )
        .await;

        assert!(
            matches!(second, ChangeProcessResult::AcceptanceCommandFailed { .. }),
            "an unarchived change must not be treated as accepted from prior output, got {second:?}"
        );
        assert_eq!(
            serial_acceptance_invocations(second_state.path()),
            3,
            "a restarted run must re-run acceptance with a full, fresh protocol budget"
        );
        assert!(
            !temp_dir.path().join(".cflx/acceptance-state.json").exists(),
            "protocol retries must not create a durable acceptance checkpoint"
        );
    }

    #[test]
    fn serial_acceptance_pass_hands_off_in_memory_without_writing_a_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        service.set_acceptance_retry_context(
            "test-change",
            AcceptanceRetryContext {
                finding_identities: vec!["repository|old finding|implementation".to_string()],
                semantic_fingerprint: Some("baseline".to_string()),
                cycle_count: 1,
            },
        );

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Pass,
            || false,
        );

        assert!(matches!(result, ChangeProcessResult::AcceptancePassed));
        assert!(service.acceptance_retry_context("test-change").is_none());
        assert!(!temp_dir.path().join(".cflx/acceptance-state.json").exists());
    }

    #[test]
    fn serial_repeated_findings_without_progress_stop_before_another_apply() {
        use crate::orchestration::acceptance::normalize_findings;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec!["src/lib.rs:10 missing regression coverage".to_string()];
        let fingerprint = semantic_progress_fingerprint(temp_dir.path()).unwrap();
        service.set_acceptance_retry_context(
            "test-change",
            AcceptanceRetryContext {
                finding_identities: normalize_findings(&findings)
                    .into_iter()
                    .map(|finding| finding.identity)
                    .collect(),
                semantic_fingerprint: Some(fingerprint),
                cycle_count: 1,
            },
        );

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail { findings },
            || false,
        );

        match &result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(error.contains("repeated_acceptance_findings"), "{error}");
            }
            other => panic!("expected a retry-exhaustion stop, got {other:?}"),
        }
        assert!(crate::parallel::acceptance_state::parse_blocked_marker(
            temp_dir.path(),
            "test-change"
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn serial_external_only_failure_stops_without_a_durable_hold() {
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail {
                findings: vec!["external non-mockable prerequisite unavailable".to_string()],
            },
            || false,
        );

        // External-only FAIL findings are not a validated structured blocker: the
        // reviewer supplied no explicit category, evidence contract, or next
        // action. The loop stops without fabricating a durable stalled hold.
        match &result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(error.contains("external_acceptance_blocker"), "{error}");
                assert!(
                    error.contains("external non-mockable prerequisite unavailable"),
                    "the external blocker context must be preserved: {error}"
                );
            }
            other => panic!("expected a retry-exhaustion stop, got {other:?}"),
        }
        assert!(crate::parallel::acceptance_state::parse_blocked_marker(
            temp_dir.path(),
            "test-change"
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn serial_missing_verdict_routes_as_protocol_failure_not_continue() {
        // A completed acceptance command without a canonical verdict must be
        // surfaced as an explicit protocol/command failure with actionable
        // evidence — never as the intentional-CONTINUE retry path.
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::MissingVerdict {
                findings: vec!["Monitoring verification, will report when complete".to_string()],
            },
            || false,
        );

        match result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(
                    error.contains("missing-verdict protocol failure"),
                    "diagnostic must identify the missing verdict, got: {error}"
                );
                assert!(
                    error.contains("Exhausted 3 consecutive attempts after 2 protocol retries"),
                    "terminal routing must report the exhausted attempts, got: {error}"
                );
                assert!(
                    error.contains("Monitoring verification, will report when complete"),
                    "diagnostic must retain bounded output evidence, got: {error}"
                );
            }
            other => panic!(
                "missing verdict must route as acceptance command failure, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn serial_explicit_continue_still_uses_continue_retry_path() {
        // Control: an explicit canonical CONTINUE keeps its intentional
        // continuation routing and configured retry policy.
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Continue,
            || false,
        );

        assert!(
            matches!(result, ChangeProcessResult::AcceptanceContinue),
            "explicit CONTINUE below the retry limit must retry acceptance, got {:?}",
            result
        );
    }

    #[test]
    fn serial_cycle_limit_stops_without_a_workspace_marker() {
        use crate::orchestration::acceptance::{normalize_findings, MAX_ACCEPTANCE_RETRY_CYCLES};

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec!["new finding at ceiling".to_string()];
        service.set_acceptance_retry_context(
            "test-change",
            AcceptanceRetryContext {
                finding_identities: normalize_findings(&["older finding".to_string()])
                    .into_iter()
                    .map(|finding| finding.identity)
                    .collect(),
                semantic_fingerprint: Some("previous-progress".to_string()),
                cycle_count: MAX_ACCEPTANCE_RETRY_CYCLES - 1,
            },
        );

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail { findings },
            || false,
        );

        // Cycle exhaustion is a runtime safety ceiling with no reviewer evidence,
        // so it must not synthesize a blocker category or a durable hold.
        match &result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(
                    error.contains("acceptance_cycle_limit_exhausted"),
                    "{error}"
                );
                assert!(error.contains("Explicit retry is required"), "{error}");
            }
            other => panic!("expected a cycle-limit stop, got {other:?}"),
        }
        assert!(crate::parallel::acceptance_state::parse_blocked_marker(
            temp_dir.path(),
            "test-change"
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn test_process_acceptance_result_archive_readiness_fail_blocks_archive_progression() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec![
            "blocking gate: cargo clippy -- -D warnings".to_string(),
            "src/orchestration/archive.rs:459".to_string(),
        ];
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("test-change");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n",
        )
        .unwrap();

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail {
                findings: findings.clone(),
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { findings: returned }
            if returned == findings
        ));
    }

    #[test]
    fn serial_latest_fail_reconciles_completed_findings_with_parallel_parity() {
        let temp_dir = TempDir::new().unwrap();
        let change_id = "test-change";
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join(change_id);
        std::fs::create_dir_all(&change_dir).unwrap();
        let tasks_path = change_dir.join("tasks.md");
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- attempt: 1\n- [x] [SAME_FINDING] fixed wording\n- [x] [RETIRED_FINDING] fixed and not reported again\n- [x] [DIFFERENT_FINDING] unrelated completed defect\n",
        )
        .unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec![
            "[SAME_FINDING] defect still present with new evidence".to_string(),
            "[NEW_FINDING] distinct newly reported defect".to_string(),
        ];

        let result = service.process_acceptance_result(
            change_id,
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail {
                findings: findings.clone(),
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { findings: returned }
            if returned == findings
        ));
        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [ ] [SAME_FINDING] defect still present with new evidence"));
        assert!(content.contains("- [ ] [NEW_FINDING] distinct newly reported defect"));
        assert!(!content.contains("RETIRED_FINDING"));
        assert!(!content.contains("DIFFERENT_FINDING"));
        assert_eq!(
            crate::task_parser::parse_file(&tasks_path, None).unwrap(),
            TaskProgress::with_counts(1, 3)
        );
    }

    #[test]
    fn acceptance_fail_uses_recorded_attempt_number_for_follow_up() {
        use crate::agent::AgentRunner;
        use crate::history::AcceptanceAttempt;
        use crate::orchestration::AcceptanceResult;
        use std::time::Duration;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let mut agent = AgentRunner::new(OrchestratorConfig::default());
        agent.record_acceptance_attempt(
            "test-change",
            AcceptanceAttempt {
                attempt: 1,
                passed: false,
                duration: Duration::from_secs(1),
                findings: Some(vec!["first".to_string()]),
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: None,
            },
        );
        agent.record_acceptance_attempt(
            "test-change",
            AcceptanceAttempt {
                attempt: 2,
                passed: false,
                duration: Duration::from_secs(1),
                findings: Some(vec!["second".to_string()]),
                exit_code: Some(0),
                stdout_tail: None,
                stderr_tail: None,
                commit_hash: None,
            },
        );
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("test-change");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(change_dir.join("tasks.md"), "- [x] done\n").unwrap();

        service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail {
                findings: vec!["canonical second".to_string()],
            },
            || false,
        );

        let content = std::fs::read_to_string(change_dir.join("tasks.md")).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- attempt: 2"));
        assert_eq!(
            content.matches("## Current Acceptance Follow-up").count(),
            1
        );
    }

    #[test]
    fn test_process_acceptance_result_fail_uses_archive_tasks_fallback_when_active_missing() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let archive_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("archive")
            .join("test-change");
        std::fs::create_dir_all(&archive_dir).unwrap();
        std::fs::write(
            archive_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n",
        )
        .unwrap();

        let findings = vec!["archive fallback finding".to_string()];
        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail {
                findings: findings.clone(),
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { findings: returned }
            if returned == findings
        ));

        let content = std::fs::read_to_string(archive_dir.join("tasks.md")).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- attempt: 1"));
        assert!(content.contains("- [ ] archive fallback finding"));
    }

    #[test]
    fn test_process_acceptance_result_fail_degrades_when_no_tasks_path_available() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let findings = vec!["missing tasks path finding".to_string()];

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail {
                findings: findings.clone(),
            },
            || false,
        );

        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { findings: returned }
            if returned == findings
        ));
    }

    #[test]
    fn acceptance_pass_clears_runtime_follow_up() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("test-change");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n\n## Acceptance #2 Failure Follow-up\n- [x] fixed\n",
        )
        .unwrap();

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Pass,
            || false,
        );

        assert!(matches!(result, ChangeProcessResult::AcceptancePassed));
        let content = std::fs::read_to_string(change_dir.join("tasks.md")).unwrap();
        assert!(!content.contains("Failure Follow-up"));
        assert!(content.contains("## Implementation Tasks\n- [x] done"));
    }

    /// `tasks.md` whose runtime-owned follow-up carries harmless unknown
    /// reviewer prose that older runtime versions refused outright.
    const DRIFTED_TASKS_MD: &str = concat!(
        "## Implementation Tasks\n",
        "- [x] done\n",
        "\n",
        "## Current Acceptance Follow-up\n",
        "- attempt: 1\n",
        "- [x] earlier finding\n",
        "### Reviewer notes\n",
        "Free-form evidence the runtime does not recognize.\n",
    );

    const DRIFTED_PAYLOAD: &str = concat!(
        "### Reviewer notes\n",
        "Free-form evidence the runtime does not recognize."
    );

    fn seed_serial_tasks(temp_dir: &TempDir, content: &str) -> std::path::PathBuf {
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("test-change");
        std::fs::create_dir_all(&change_dir).unwrap();
        let tasks_path = change_dir.join("tasks.md");
        std::fs::write(&tasks_path, content).unwrap();
        tasks_path
    }

    #[test]
    fn serial_acceptance_fail_recovers_unknown_follow_up_content_and_keeps_retrying() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let tasks_path = seed_serial_tasks(&temp_dir, DRIFTED_TASKS_MD);

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Fail {
                findings: vec!["latest finding".to_string()],
            },
            || false,
        );

        // The workflow continues on the acceptance verdict instead of terminating.
        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceFailed { .. }
        ));
        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Recovered Acceptance Notes"));
        assert!(content.contains(DRIFTED_PAYLOAD));
        assert!(content.contains("## Current Acceptance Follow-up\n- attempt: 1"));
        assert!(content.contains("- [ ] latest finding"));
        // Recovered checkbox-free prose leaves task accounting untouched.
        assert_eq!(
            crate::task_parser::parse_content(&content, None),
            crate::task_parser::TaskProgress::with_counts(1, 2)
        );
    }

    #[test]
    fn serial_acceptance_pass_cleanup_retains_recovered_notes() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let tasks_path = seed_serial_tasks(&temp_dir, DRIFTED_TASKS_MD);

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Pass,
            || false,
        );

        assert!(matches!(result, ChangeProcessResult::AcceptancePassed));
        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(!content.contains("Acceptance Follow-up"));
        assert!(content.contains("## Recovered Acceptance Notes"));
        assert!(content.contains(DRIFTED_PAYLOAD));
    }

    #[test]
    fn serial_acceptance_pass_reports_hard_error_for_ambiguous_boundary() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let ambiguous = concat!(
            "## Implementation Tasks\n",
            "- [x] done\n",
            "\n",
            "## Current Acceptance Follow-up\n",
            "- [x] fixed\n",
            "```text\n",
            "unterminated reviewer dump\n",
        );
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());
        let tasks_path = seed_serial_tasks(&temp_dir, ambiguous);

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Pass,
            || false,
        );

        match result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(error.contains("unclosed code fence"), "{error}");
            }
            other => panic!("expected cleanup failure, got {other:?}"),
        }
        assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), ambiguous);
    }

    #[test]
    fn test_process_acceptance_result_archive_readiness_pass_allows_archive_progression() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let agent = AgentRunner::new(OrchestratorConfig::default());

        let result = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Pass,
            || false,
        );

        assert!(matches!(result, ChangeProcessResult::AcceptancePassed));
    }

    /// Bare and validated blocker results must never touch the change directory
    /// in serial mode, and a bare token must not be reported as a stalled hold.
    #[test]
    fn serial_blocker_results_create_no_change_directory_artifact() {
        use crate::agent::AgentRunner;
        use crate::orchestration::AcceptanceResult;

        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let agent = AgentRunner::new(OrchestratorConfig::default());

        let bare = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::BareBlocker {
                rejection: crate::acceptance::BlockerRejection::Missing,
            },
            || false,
        );
        match &bare {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(error.contains("bare-blocker protocol failure"), "{error}");
                assert!(error.contains("no structured blocker"), "{error}");
            }
            other => panic!("bare blocker must be a protocol error, got {other:?}"),
        }
        assert!(
            !matches!(bare, ChangeProcessResult::Stalled { .. }),
            "bare blocker input must never present as a stalled hold"
        );

        let stalled = service.process_acceptance_result(
            "test-change",
            temp_dir.path(),
            &agent,
            AcceptanceResult::Stalled {
                blocker: crate::acceptance::AcceptanceBlocker {
                    category: "external_service".to_string(),
                    evidence: vec!["registry returned 503".to_string()],
                    next_action: "wait for the registry then retry acceptance".to_string(),
                    resumable: true,
                    prerequisite_owner: None,
                    evidence_ids: Vec::new(),
                },
            },
            || false,
        );
        match &stalled {
            ChangeProcessResult::Stalled { error } => {
                assert!(error.starts_with("external_service:"), "{error}");
            }
            other => panic!("validated blocker must stall, got {other:?}"),
        }

        // Neither path wrote anything into the change directory.
        assert!(!temp_dir.path().join("openspec/changes").exists());
    }

    /// Build a service whose acceptance command records every invocation and
    /// always passes, so a test can prove whether acceptance ran at all.
    fn serial_counting_pass_config(
        change_id: &str,
        state_dir: &std::path::Path,
    ) -> OrchestratorConfig {
        let counter = state_dir.join("attempts").display().to_string();
        std::fs::create_dir_all(state_dir.join("prompts")).unwrap();
        OrchestratorConfig {
            acceptance_command: Some(format!(
                "sh -c 'n=$(cat \"{counter}\" 2>/dev/null || echo 0); n=$((n+1)); \
                 echo $n > \"{counter}\"; echo \"ACCEPTANCE: PASS\"'"
            )),
            ..serial_failing_acceptance_config(change_id)
        }
    }

    async fn persist_serial_stall(
        repo_root: &std::path::Path,
        stall_state: &std::path::Path,
        change_id: &str,
        resumable: bool,
    ) -> crate::parallel::acceptance_state::AcceptanceStallRecord {
        let store = AcceptanceStallStore::new(stall_state);
        let apply_revision = crate::vcs::git::commands::get_current_commit(repo_root)
            .await
            .unwrap();
        crate::execution::state::persist_acceptance_stall(
            &store,
            repo_root,
            repo_root,
            change_id,
            "main",
            &apply_revision,
            &crate::acceptance::AcceptanceBlocker {
                category: "external_service".to_string(),
                evidence: vec!["staging registry returned 503".to_string()],
                next_action: "wait for the registry then retry acceptance".to_string(),
                resumable,
                prerequisite_owner: Some("platform".to_string()),
                evidence_ids: Vec::new(),
            },
            0,
        )
        .await
        .unwrap()
    }

    /// Restart must reconstruct a runtime acceptance stall instead of archiving
    /// the complete change: the hold survives the process, and archive is not
    /// reachable while it holds.
    #[tokio::test]
    async fn serial_restart_restores_runtime_acceptance_stall_before_archive() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let stall_state = TempDir::new().unwrap();
        let change_id = "serial-restart-stall";
        init_serial_repo(temp_dir.path(), change_id);
        let apply_revision = crate::vcs::git::commands::get_current_commit(temp_dir.path())
            .await
            .unwrap();

        persist_serial_stall(temp_dir.path(), stall_state.path(), change_id, true).await;

        // A brand-new service stands in for a restarted process: it has no
        // in-memory acceptance history at all.
        let config = serial_counting_pass_config(change_id, state_dir.path());
        let mut service = SerialRunService::new(temp_dir.path().to_path_buf(), config.clone());
        service.set_acceptance_stall_state_root(stall_state.path().to_path_buf());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = serial_test_ai_runner();

        let result = service
            .process_change(
                &create_test_change(change_id, 1, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        match &result {
            ChangeProcessResult::AcceptanceStalled { blocker, error } => {
                assert_eq!(blocker.category, "external_service");
                assert_eq!(blocker.evidence, ["staging registry returned 503"]);
                assert!(error.starts_with("external_service:"), "{error}");
            }
            other => panic!("restart must restore the runtime stall, got {other:?}"),
        }
        assert!(service.is_stalled(change_id));
        assert_eq!(
            serial_acceptance_invocations(state_dir.path()),
            0,
            "a restored hold must not start acceptance"
        );
        // The hold advanced nothing: no archive, clean worktree, apply revision
        // untouched.
        assert!(temp_dir
            .path()
            .join("openspec/changes")
            .join(change_id)
            .exists());
        assert_eq!(serial_porcelain_status(temp_dir.path()), "");
        assert_eq!(
            crate::vcs::git::commands::get_current_commit(temp_dir.path())
                .await
                .unwrap(),
            apply_revision
        );
        assert_acceptance_added_no_worktree_artifact(temp_dir.path(), change_id);
    }

    /// The blocker a serial stall carries must survive all the way to the
    /// operator-facing lifecycle: `stalled`, not an opaque processing error, and
    /// not cleared immediately after being marked.
    #[tokio::test]
    async fn serial_acceptance_stall_displays_as_stalled_with_structured_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let stall_state = TempDir::new().unwrap();
        let change_id = "serial-stall-display";
        init_serial_repo(temp_dir.path(), change_id);

        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        service.set_acceptance_stall_state_root(stall_state.path().to_path_buf());

        let result = service
            .record_acceptance_stall(
                change_id,
                &crate::acceptance::AcceptanceBlocker {
                    category: "external_service".to_string(),
                    evidence: vec!["staging registry returned 503".to_string()],
                    next_action: "wait for the registry then retry acceptance".to_string(),
                    resumable: true,
                    prerequisite_owner: Some("platform".to_string()),
                    evidence_ids: Vec::new(),
                },
            )
            .await;
        assert!(matches!(
            result,
            ChangeProcessResult::AcceptanceStalled { .. }
        ));

        // Consumers publish the same acceptance-gated event parallel emits, and
        // nothing that would overwrite its structured metadata.
        let events = result.stalled_lifecycle_events(change_id);
        assert!(
            matches!(
                events.as_slice(),
                [crate::events::ExecutionEvent::AcceptanceGated { .. }]
            ),
            "a validated serial stall must publish acceptance-gated, got {events:?}"
        );

        let mut state =
            crate::orchestration::state::OrchestratorState::new(vec![change_id.to_string()], 0);
        state.mark_stalled(change_id.to_string());
        for event in &events {
            state.apply_execution_event(event);
        }

        assert_eq!(state.display_status(change_id), "stalled");
        let runtime = state
            .change_runtime(change_id)
            .expect("runtime entry for a stalled serial change");
        assert_eq!(
            runtime.blocked_metadata.blocker_reason.as_deref(),
            Some("acceptance-gated:external_service")
        );
        let unblock = runtime
            .blocked_metadata
            .unblock_metadata
            .as_deref()
            .expect("structured unblock metadata");
        assert!(unblock.contains("resumable=true"), "{unblock}");
        assert!(
            unblock.contains("staging registry returned 503"),
            "{unblock}"
        );

        // Every other result keeps its existing routing and publishes nothing.
        assert!(ChangeProcessResult::Stalled {
            error: "apply blocked".to_string()
        }
        .stalled_lifecycle_events(change_id)
        .is_empty());
    }

    /// A successful explicit retry resumes at acceptance: apply is not rerun and
    /// the complete change is not archived on an inferred prior PASS.
    #[tokio::test]
    async fn explicit_serial_retry_resumes_at_acceptance_without_rerunning_apply() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let stall_state = TempDir::new().unwrap();
        let change_id = "serial-retry-resume";
        init_serial_repo(temp_dir.path(), change_id);
        let apply_revision = crate::vcs::git::commands::get_current_commit(temp_dir.path())
            .await
            .unwrap();

        let record =
            persist_serial_stall(temp_dir.path(), stall_state.path(), change_id, true).await;

        let config = serial_counting_pass_config(change_id, state_dir.path());
        let mut service = SerialRunService::new(temp_dir.path().to_path_buf(), config.clone());
        service.set_acceptance_stall_state_root(stall_state.path().to_path_buf());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = serial_test_ai_runner();

        assert!(
            service
                .consume_explicit_acceptance_retry(change_id)
                .await
                .unwrap(),
            "a resumable hold must accept explicit retry"
        );
        assert!(!service.is_stalled(change_id));
        let store = AcceptanceStallStore::new(stall_state.path());
        assert_eq!(
            store.load(&record.repository_id, change_id).unwrap(),
            None,
            "a successful retry consumes the hold"
        );

        let result = service
            .process_change(
                &create_test_change(change_id, 1, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        assert!(
            matches!(result, ChangeProcessResult::AcceptancePassed),
            "explicit retry must resume at acceptance, got {result:?}"
        );
        assert_eq!(
            serial_acceptance_invocations(state_dir.path()),
            1,
            "explicit retry must run acceptance exactly once"
        );
        assert_eq!(
            service.apply_count(change_id),
            0,
            "explicit retry must not rerun apply"
        );
        assert_eq!(
            crate::vcs::git::commands::get_current_commit(temp_dir.path())
                .await
                .unwrap(),
            apply_revision,
            "the preserved apply revision must be unchanged"
        );
    }

    /// Without runtime state, a complete unarchived change is accepted again
    /// rather than archived on an inferred prior PASS (constitutional fail-safe).
    #[tokio::test]
    async fn serial_complete_change_reruns_acceptance_before_archive_after_restart() {
        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let stall_state = TempDir::new().unwrap();
        let change_id = "serial-failsafe-rerun";
        init_serial_repo(temp_dir.path(), change_id);

        let config = serial_counting_pass_config(change_id, state_dir.path());
        let mut service = SerialRunService::new(temp_dir.path().to_path_buf(), config.clone());
        service.set_acceptance_stall_state_root(stall_state.path().to_path_buf());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = serial_test_ai_runner();

        let result = service
            .process_change(
                &create_test_change(change_id, 1, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        assert!(
            matches!(result, ChangeProcessResult::AcceptancePassed),
            "a complete but unaccepted change must run acceptance, got {result:?}"
        );
        assert_eq!(serial_acceptance_invocations(state_dir.path()), 1);
    }

    /// A validated hold cannot be born invalid: when the current apply revision
    /// cannot be resolved, acceptance reports a command failure instead of
    /// persisting a record that reconciliation will always quarantine.
    #[tokio::test]
    async fn serial_stall_refuses_to_persist_without_an_apply_revision() {
        // No git repository at all, so `get_current_commit` cannot resolve HEAD.
        let temp_dir = TempDir::new().unwrap();
        let stall_state = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        service.set_acceptance_stall_state_root(stall_state.path().to_path_buf());

        let result = service
            .record_acceptance_stall(
                "no-revision",
                &crate::acceptance::AcceptanceBlocker {
                    category: "credential".to_string(),
                    evidence: vec!["STAGING_API_KEY is unset".to_string()],
                    next_action: "provision STAGING_API_KEY then retry acceptance".to_string(),
                    resumable: true,
                    prerequisite_owner: None,
                    evidence_ids: Vec::new(),
                },
            )
            .await;

        match &result {
            ChangeProcessResult::AcceptanceCommandFailed { error } => {
                assert!(error.contains("apply revision"), "{error}");
            }
            other => panic!("an unresolvable revision must fail loudly, got {other:?}"),
        }
        assert!(!service.is_stalled("no-revision"));
        let store = AcceptanceStallStore::new(stall_state.path());
        assert_eq!(
            crate::execution::state::load_valid_acceptance_stall(
                &store,
                temp_dir.path(),
                temp_dir.path(),
                "no-revision",
                "main",
            )
            .await
            .unwrap(),
            None,
            "no hold may be created from an unresolved revision"
        );
    }

    /// Serial must migrate a legacy acceptance-origin marker into runtime state
    /// exactly like parallel dispatch does, so the same hold is recoverable in
    /// both modes and explicit retry can consume it.
    #[tokio::test]
    async fn serial_migrates_a_legacy_acceptance_marker_into_runtime_state() {
        use crate::parallel::acceptance_state::write_legacy_acceptance_marker;

        let temp_dir = TempDir::new().unwrap();
        let state_dir = TempDir::new().unwrap();
        let stall_state = TempDir::new().unwrap();
        let change_id = "serial-legacy-marker";
        init_serial_repo(temp_dir.path(), change_id);
        let apply_revision = crate::vcs::git::commands::get_current_commit(temp_dir.path())
            .await
            .unwrap();

        write_legacy_acceptance_marker(
            temp_dir.path(),
            change_id,
            "acceptance_gated",
            &["managed verification job 42 is still running".to_string()],
            &AcceptanceRetryContext {
                finding_identities: vec!["external|job 42|verification".to_string()],
                semantic_fingerprint: Some("baseline".to_string()),
                cycle_count: 2,
            },
            "no_semantic_progress",
            &["verification job 42".to_string()],
            true,
            "wait for job 42 then retry acceptance",
        )
        .unwrap();

        let config = serial_counting_pass_config(change_id, state_dir.path());
        let mut service = SerialRunService::new(temp_dir.path().to_path_buf(), config.clone());
        service.set_acceptance_stall_state_root(stall_state.path().to_path_buf());
        let mut agent = AgentRunner::new(config.clone());
        let ai_runner = serial_test_ai_runner();

        let result = service
            .process_change(
                &create_test_change(change_id, 1, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        // The legacy marker became a structured runtime hold instead of an
        // opaque marker-based stall, and the marker residue is gone.
        match &result {
            ChangeProcessResult::AcceptanceStalled { blocker, .. } => {
                assert_eq!(blocker.category, "pending_verification");
                assert_eq!(blocker.next_action, "wait for job 42 then retry acceptance");
            }
            other => panic!("a legacy acceptance marker must migrate, got {other:?}"),
        }
        assert_eq!(serial_acceptance_invocations(state_dir.path()), 0);
        assert_acceptance_added_no_worktree_artifact(temp_dir.path(), change_id);
        assert_eq!(serial_porcelain_status(temp_dir.path()), "");

        // Explicit retry now finds the migrated hold instead of refusing it.
        assert!(
            service
                .consume_explicit_acceptance_retry(change_id)
                .await
                .unwrap(),
            "a migrated legacy hold must be retryable"
        );
        assert_eq!(
            crate::vcs::git::commands::get_current_commit(temp_dir.path())
                .await
                .unwrap(),
            apply_revision
        );
    }

    #[tokio::test]
    async fn serial_process_change_stops_at_workspace_marker_before_archive() {
        use crate::parallel::acceptance_state::write_legacy_acceptance_marker;

        let temp_dir = TempDir::new().unwrap();
        write_legacy_acceptance_marker(
            temp_dir.path(),
            "complete-change",
            "stalled",
            &[],
            &AcceptanceRetryContext::default(),
            "no_semantic_progress",
            &[],
            true,
            "explicit retry",
        )
        .unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        let mut agent = AgentRunner::new(OrchestratorConfig::default());
        let ai_runner = AiCommandRunner::new(
            CommandQueueConfig {
                stagger_delay_ms: 0,
                max_retries: 0,
                retry_delay_ms: 0,
                retry_error_patterns: default_retry_patterns(),
                retry_if_duration_under_secs: 0,
                inactivity_timeout_secs: 0,
                inactivity_kill_grace_secs: 0,
                inactivity_timeout_max_retries: 0,
                strict_process_cleanup: true,
            },
            Arc::new(Mutex::new(None)),
        );
        let result = service
            .process_change(
                &create_test_change("complete-change", 1, 1),
                &mut agent,
                &ai_runner,
                &HookRunner::new(HooksConfig::default(), temp_dir.path()),
                &NullOutputHandler::new(),
                1,
                1,
                || false,
                || false,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(result, ChangeProcessResult::Stalled { .. }));
        assert!(service.is_stalled("complete-change"));
        assert!(crate::parallel::acceptance_state::parse_blocked_marker(
            temp_dir.path(),
            "complete-change"
        )
        .unwrap()
        .is_some());
    }

    #[test]
    fn serial_preflight_suppresses_apply_and_archive_for_any_marker() {
        use crate::parallel::acceptance_state::write_legacy_acceptance_marker;

        let temp_dir = TempDir::new().unwrap();
        write_legacy_acceptance_marker(
            temp_dir.path(),
            "complete-change",
            "stalled",
            &[],
            &AcceptanceRetryContext::default(),
            "no_semantic_progress",
            &[],
            true,
            "explicit retry",
        )
        .unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let result = service.preflight_blocked_marker("complete-change").unwrap();

        assert!(matches!(result, Some(ChangeProcessResult::Stalled { .. })));
        assert!(service.is_stalled("complete-change"));
    }

    #[test]
    fn malformed_marker_stops_serial_preflight_and_is_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir
            .path()
            .join("openspec/changes/blocked/APPLY_BLOCKED/marker.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ malformed").unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        assert!(service.preflight_blocked_marker("blocked").is_err());
        assert!(path.exists());
    }

    /// A non-resumable hold is refused by explicit retry and keeps its evidence:
    /// retry must not silently discard a blocker an operator still needs.
    #[tokio::test]
    async fn explicit_serial_retry_refuses_a_non_resumable_hold_and_keeps_it() {
        let temp_dir = TempDir::new().unwrap();
        let stall_state = TempDir::new().unwrap();
        init_serial_repo(temp_dir.path(), "non-resumable");

        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());
        service.set_acceptance_stall_state_root(stall_state.path().to_path_buf());

        let store =
            crate::parallel::acceptance_state::AcceptanceStallStore::new(stall_state.path());
        let apply_revision = crate::vcs::git::commands::get_current_commit(temp_dir.path())
            .await
            .unwrap();
        let record = crate::execution::state::persist_acceptance_stall(
            &store,
            temp_dir.path(),
            temp_dir.path(),
            "non-resumable",
            "main",
            &apply_revision,
            &crate::acceptance::AcceptanceBlocker {
                category: "human_decision".to_string(),
                evidence: vec!["the approach needs an owner decision".to_string()],
                next_action: "owner decides the approach".to_string(),
                resumable: false,
                prerequisite_owner: Some("architecture".to_string()),
                evidence_ids: Vec::new(),
            },
            0,
        )
        .await
        .unwrap();
        assert!(!record.resumable);

        assert!(
            !service
                .consume_explicit_acceptance_retry("non-resumable")
                .await
                .unwrap(),
            "a non-resumable hold must refuse explicit retry"
        );

        let preserved = store
            .load(&record.repository_id, "non-resumable")
            .unwrap()
            .expect("a refused retry must keep the blocker evidence");
        assert_eq!(preserved, record);
    }

    /// An apply-origin marker is never treated as an acceptance hold, so an
    /// explicit acceptance retry refuses it and leaves it in place.
    #[tokio::test]
    async fn explicit_serial_retry_refuses_apply_origin_marker() {
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let apply_marker = temp_dir
            .path()
            .join("openspec/changes/apply/APPLY_BLOCKED/marker.md");
        std::fs::create_dir_all(apply_marker.parent().unwrap()).unwrap();
        std::fs::write(&apply_marker, "origin: apply\nreason: blocked\n").unwrap();

        assert!(!service
            .consume_explicit_acceptance_retry("apply")
            .await
            .unwrap());
        assert!(
            apply_marker.exists(),
            "an apply-origin marker must survive an acceptance retry"
        );
    }

    #[test]
    fn test_mark_stalled_prevents_reselection() {
        let temp_dir = TempDir::new().unwrap();
        let mut service =
            SerialRunService::new(temp_dir.path().to_path_buf(), OrchestratorConfig::default());

        let changes = vec![
            create_test_change("a", 5, 10),
            create_test_change("b", 8, 10), // Highest progress
        ];

        // Initially, highest progress change should be selected
        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("b"));

        // Mark 'b' as stalled (simulating GATED acceptance)
        service.mark_stalled("b", "Implementation blocker detected");

        // After marking as stalled, 'b' should not be selected
        let next = service.select_next_change(&changes);
        assert_eq!(next.map(|c| c.id.as_str()), Some("a"));

        // Verify 'b' is marked as stalled
        assert!(service.is_stalled("b"));
        assert!(!service.is_stalled("a"));
    }
}
