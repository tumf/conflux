//! Builder and initialization methods for [`super::ParallelExecutor`].
//!
//! This module provides the constructor and setter API for `ParallelExecutor`,
//! separating initialization concerns from execution logic.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use crate::hooks::HookRunner;
use crate::vcs::{GitWorkspaceManager, VcsBackend, WorkspaceManager};

use super::{FailedChangeTracker, ParallelEvent, ParallelExecutor, DEFAULT_MAX_CONFLICT_RETRIES};

impl ParallelExecutor {
    /// Create a new parallel executor with automatic VCS detection
    pub fn new(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
    ) -> Self {
        // Auto-detect VCS backend
        let vcs_backend = config.get_vcs_backend();
        Self::with_backend(repo_root, config, event_tx, vcs_backend)
    }

    /// Create a new parallel executor with a specific VCS backend and optional shared queue change
    /// timestamp
    pub fn with_backend_and_queue_state(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
        vcs_backend: VcsBackend,
        shared_queue_change: Option<Arc<Mutex<Option<std::time::Instant>>>>,
    ) -> Self {
        Self::with_backend_and_queue_and_stagger(
            repo_root,
            config,
            event_tx,
            vcs_backend,
            shared_queue_change,
            None,
        )
    }

    /// Create a new parallel executor with a specific VCS backend, optional shared queue change
    /// timestamp, and optional shared stagger state
    pub fn with_backend_and_queue_and_stagger(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
        vcs_backend: VcsBackend,
        shared_queue_change: Option<Arc<Mutex<Option<std::time::Instant>>>>,
        shared_stagger_state: Option<SharedStaggerState>,
    ) -> Self {
        // Resolve workspace base directory
        let base_dir = if let Some(configured_dir) = config.get_workspace_base_dir() {
            // User configured a specific directory
            PathBuf::from(configured_dir)
        } else {
            // Use OS-specific default workspace directory
            crate::config::defaults::default_workspace_base_dir(Some(&repo_root))
        };
        info!("Using workspace base directory: {:?}", base_dir);

        let max_concurrent = config.get_max_concurrent_workspaces();
        let apply_command = config
            .get_apply_command()
            .expect("apply_command must be configured before creating ParallelExecutor")
            .to_string();
        let archive_command = config
            .get_archive_command()
            .expect("archive_command must be configured before creating ParallelExecutor")
            .to_string();

        // Resolve the VCS backend (handle Auto)
        let resolved_backend = Self::resolve_backend(vcs_backend, &repo_root);
        info!("Using VCS backend: {:?}", resolved_backend);

        let workspace_manager: Box<dyn WorkspaceManager> = match resolved_backend {
            VcsBackend::Git | VcsBackend::Auto => Box::new(GitWorkspaceManager::new(
                base_dir,
                repo_root.clone(),
                max_concurrent,
                config.clone(),
            )),
        };

        let last_queue_change_at =
            shared_queue_change.unwrap_or_else(|| Arc::new(Mutex::new(None)));

        // Use provided shared stagger state or create a new one
        let shared_stagger_state =
            shared_stagger_state.unwrap_or_else(|| Arc::new(Mutex::new(None)));

        // Build CommandQueue configuration from orchestrator config
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
            inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
            inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
            inactivity_timeout_max_retries: config.get_command_inactivity_timeout_max_retries(),
            strict_process_cleanup: config.get_command_strict_process_cleanup(),
        };

        // Create shared AI command runner
        let ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state.clone());

        Self {
            workspace_manager,
            config,
            apply_command,
            archive_command,
            event_tx,
            max_conflict_retries: DEFAULT_MAX_CONFLICT_RETRIES,
            repo_root,
            no_resume: false,
            failed_tracker: FailedChangeTracker::new(),
            change_dependencies: HashMap::new(),
            resolve_wait_changes: HashSet::new(),
            merge_wait_changes: HashSet::new(),
            previously_blocked_changes: HashSet::new(),
            force_recreate_worktree: HashSet::new(),
            hooks: None,
            cancel_token: None,
            last_queue_change_at,
            last_available_slots: None,
            dynamic_queue: None,
            ai_runner,
            shared_stagger_state,
            apply_history: Arc::new(Mutex::new(crate::history::ApplyHistory::new())),
            archive_history: Arc::new(Mutex::new(crate::history::ArchiveHistory::new())),
            acceptance_history: Arc::new(Mutex::new(crate::history::AcceptanceHistory::new())),
            acceptance_tail_injected: Arc::new(Mutex::new(std::collections::HashMap::new())),
            needs_reanalysis: false,
            manual_resolve_count: None,
            auto_resolve_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            pending_merge_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Create a new parallel executor with a specific VCS backend
    pub fn with_backend(
        repo_root: PathBuf,
        config: OrchestratorConfig,
        event_tx: Option<mpsc::Sender<ParallelEvent>>,
        vcs_backend: VcsBackend,
    ) -> Self {
        Self::with_backend_and_queue_state(repo_root, config, event_tx, vcs_backend, None)
    }

    /// Set the hook runner for executing hooks during parallel execution.
    #[allow(dead_code)] // Public API for future integration with CLI/TUI
    pub fn set_hooks(&mut self, hooks: HookRunner) {
        self.hooks = Some(Arc::new(hooks));
    }

    /// Set whether to disable automatic workspace resume.
    ///
    /// When `no_resume` is true, existing workspaces are always deleted
    /// and new ones are created. When false (default), existing workspaces
    /// are reused to resume interrupted work.
    pub fn set_no_resume(&mut self, no_resume: bool) {
        self.no_resume = no_resume;
    }

    /// Set the cancellation token for force stop cleanup.
    pub fn set_cancel_token(&mut self, cancel_token: CancellationToken) {
        self.cancel_token = Some(cancel_token);
    }

    /// Set the dynamic queue for runtime change additions (TUI mode).
    pub fn set_dynamic_queue(&mut self, dynamic_queue: Arc<crate::tui::queue::DynamicQueue>) {
        self.dynamic_queue = Some(dynamic_queue);
    }

    /// Set the manual resolve counter for tracking active manual resolve operations (TUI mode).
    /// This allows manual resolves to consume parallel execution slots.
    pub fn set_manual_resolve_counter(&mut self, counter: Arc<std::sync::atomic::AtomicUsize>) {
        self.manual_resolve_count = Some(counter);
    }

    /// Get a clone of the automatic resolve counter for testing or external tracking.
    #[cfg(test)]
    pub fn get_auto_resolve_counter(&self) -> Arc<std::sync::atomic::AtomicUsize> {
        self.auto_resolve_count.clone()
    }

    /// Get the VCS backend type
    #[allow(dead_code)] // Public API for external callers
    pub fn backend_type(&self) -> VcsBackend {
        self.workspace_manager.backend_type()
    }

    /// Check if VCS is available for parallel execution
    #[allow(dead_code)] // Public API, used via ParallelRunService
    pub async fn check_vcs_available(&self) -> crate::error::Result<bool> {
        self.workspace_manager
            .check_available()
            .await
            .map_err(Into::into)
    }

    /// Resolve VCS backend (convert Auto to concrete backend)
    pub(super) fn resolve_backend(backend: VcsBackend, _repo_root: &Path) -> VcsBackend {
        match backend {
            VcsBackend::Auto => VcsBackend::Git,
            other => other,
        }
    }
}
