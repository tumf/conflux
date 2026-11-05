//! `/api/v2` configured-command worktree sessions.
//!
//! The adapter between the v2 contract and
//! [`crate::worktree_ops::session`]: the late-bound session port the executor
//! and the read routes share, the production port over the shared worktree
//! service and the session registry, the redacted DTO projection, and the two
//! read routes.
//!
//! The command text is always the owner's configured `worktree_command`
//! template with `{workspace_dir}` and `{repo_root}` expanded here, server-side.
//! Nothing a client sends reaches it. Every owner-generated string this module
//! publishes — results, error detail, events, the session resource's own
//! members — is built from opaque IDs, repository-relative display paths, and
//! stable tokens, and any diagnostic that came from Git or a hook is passed
//! through [`PathRedactor`] first. `output.lines` is the one exception, by
//! design: it is the command's own stdout/stderr, sanitized for control
//! sequences and bounded, but not redacted.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{Path as AxumPath, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;

use crate::worktree_ops::service::WorktreeService;
use crate::worktree_ops::session::{
    CleanupOutcome, GitSessionProvisionRuntime, LaunchSpec, ProvisionError, SessionEvent,
    SessionEventSink, SessionKind, SessionProvisionRuntime, SessionProvisioner, SessionRefusal,
    SessionSnapshot, SetupFacts, WorktreeSessionRegistry, MAX_OUTPUT_BYTES, MAX_RETAINED_FINISHED,
    MAX_RUNNING_SESSIONS, SESSION_REASONS,
};

use super::auth::CorrelationId;
use super::dto::{
    ApiError, CommandResult, ErrorCode, SessionCleanupFacts, SessionOutput, SessionOutputLine,
    SessionSetupFacts, WorktreeSessionCapability, WorktreeSessionEventType,
    WorktreeSessionResource, WorktreeSessionResponse, WorktreeSessionResult,
    WorktreeSessionsResponse,
};
use super::executor::{CommandFailure, ExecutionSummary};
use super::projection::Projection;
use super::worktrees::{map_worktree_error, repository_relative_display, RemoteWorktreeOperations};
use super::RemoteControlState;

/// The three session command types, in capability order.
pub const SESSION_COMMANDS: [&str; 3] = [
    "run_worktree_command",
    "create_session_worktree",
    "stop_worktree_session",
];

/// The two session read routes.
pub const SESSION_ROUTES: [&str; 2] = [
    "/api/v2/worktree-sessions",
    "/api/v2/worktree-sessions/{session_id}",
];

// ============================================================================
// Port
// ============================================================================

/// The session port the executor and the read routes share.
#[async_trait]
pub trait WorktreeSessionOperations: Send + Sync {
    /// `worktree_command` is configured, the project is a Git repository, and
    /// owner shutdown has not closed admission.
    fn launchable(&self) -> bool;
    /// Every retained session, newest first.
    fn sessions(&self) -> Vec<SessionSnapshot>;
    /// One session.
    fn session(&self, session_id: &str) -> Option<SessionSnapshot>;
    /// Sessions currently running.
    fn running(&self) -> usize;
    /// `run_worktree_command`.
    async fn run(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure>;
    /// `create_session_worktree`.
    async fn create(&self) -> Result<ExecutionSummary, CommandFailure>;
    /// `stop_worktree_session`.
    async fn stop(&self, session_id: &str) -> Result<ExecutionSummary, CommandFailure>;
}

/// Late-bound holder of this incarnation's session port.
///
/// Late for the same reason the completion-sink registry is: the listener is
/// bound before an orchestration runtime exists. Unbound is a process that can
/// launch nothing and holds no session, which is what the capability and the
/// read routes report.
#[derive(Default)]
pub struct WorktreeSessionHandle {
    port: std::sync::RwLock<Option<Arc<dyn WorktreeSessionOperations>>>,
}

impl WorktreeSessionHandle {
    /// Bind the port this incarnation serves.
    pub fn bind(&self, port: Arc<dyn WorktreeSessionOperations>) {
        *self
            .port
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(port);
    }

    /// The bound port, if any.
    pub fn get(&self) -> Option<Arc<dyn WorktreeSessionOperations>> {
        self.port
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Sessions currently running; `0` when unbound.
    pub fn running(&self) -> usize {
        self.get().map_or(0, |port| port.running())
    }
}

/// The capability block, given whether a command executor is bound.
pub fn capability(
    handle: &WorktreeSessionHandle,
    executor_bound: bool,
) -> WorktreeSessionCapability {
    WorktreeSessionCapability {
        available: executor_bound && handle.get().is_some_and(|port| port.launchable()),
        commands: SESSION_COMMANDS.iter().map(|c| c.to_string()).collect(),
        routes: SESSION_ROUTES.iter().map(|r| r.to_string()).collect(),
        max_running: MAX_RUNNING_SESSIONS,
        max_retained: MAX_RETAINED_FINISHED,
        max_output_bytes: MAX_OUTPUT_BYTES,
        stdin: "null".to_string(),
        reasons: SESSION_REASONS.iter().map(|r| r.to_string()).collect(),
        event_types: vec![
            WorktreeSessionEventType::WorktreeSessionStarted,
            WorktreeSessionEventType::WorktreeSessionFinished,
        ],
    }
}

/// The refusal an owner with no session port returns.
pub fn unavailable(detail: &str) -> CommandFailure {
    CommandFailure::new(
        ErrorCode::WorktreeSessionUnavailable,
        format!("reason=unavailable: {detail}"),
    )
}

/// The port before a session registry is bound.
pub struct UnboundWorktreeSessions;

#[async_trait]
impl WorktreeSessionOperations for UnboundWorktreeSessions {
    fn launchable(&self) -> bool {
        false
    }
    fn sessions(&self) -> Vec<SessionSnapshot> {
        Vec::new()
    }
    fn session(&self, _session_id: &str) -> Option<SessionSnapshot> {
        None
    }
    fn running(&self) -> usize {
        0
    }
    async fn run(&self, _worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        Err(unavailable(
            "this instance has no worktree session runtime bound",
        ))
    }
    async fn create(&self) -> Result<ExecutionSummary, CommandFailure> {
        Err(unavailable(
            "this instance has no worktree session runtime bound",
        ))
    }
    async fn stop(&self, session_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        Err(not_found(session_id))
    }
}

fn not_found(session_id: &str) -> CommandFailure {
    CommandFailure::new(
        ErrorCode::NotFound,
        format!("worktree session '{session_id}' is not known to this instance"),
    )
}

fn map_refusal(refusal: &SessionRefusal) -> CommandFailure {
    match refusal {
        SessionRefusal::WorktreeBusy => CommandFailure::new(
            ErrorCode::WorktreeSessionBusy,
            "reason=worktree_busy: a worktree session is already running in that worktree",
        ),
        SessionRefusal::Capacity => CommandFailure::new(
            ErrorCode::WorktreeSessionBusy,
            format!(
                "reason=capacity: {MAX_RUNNING_SESSIONS} worktree sessions are already running"
            ),
        ),
        SessionRefusal::Closed => unavailable("the owner is shutting down"),
        // The runner's own error never names the command; it is still a
        // launch the owner could not perform, not a client mistake.
        SessionRefusal::LaunchFailed(_) => CommandFailure::new(
            ErrorCode::InternalError,
            "the worktree session runner could not start",
        ),
    }
}

// ============================================================================
// Redaction
// ============================================================================

/// Replaces every absolute path an owner diagnostic could carry with its
/// repository-relative display form.
///
/// Git and `.wt/setup` diagnostics name their paths; the remote contract never
/// carries one. Longest match first, so a worktree under the repository is
/// rewritten as itself rather than as a fragment of the root.
pub struct PathRedactor {
    replacements: Vec<(String, String)>,
}

impl PathRedactor {
    /// Rewrite each `(absolute, display)` pair.
    pub fn new(pairs: Vec<(PathBuf, String)>) -> Self {
        let mut replacements: Vec<(String, String)> = pairs
            .into_iter()
            .flat_map(|(path, display)| {
                let canonical = std::fs::canonicalize(&path).ok();
                [Some(path), canonical]
                    .into_iter()
                    .flatten()
                    .map(move |p| (p.to_string_lossy().to_string(), display.clone()))
            })
            .filter(|(from, _)| !from.is_empty() && from != "/")
            .collect();
        replacements.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
        replacements.dedup();
        Self { replacements }
    }

    /// Redact and sanitize one diagnostic.
    pub fn redact(&self, text: &str) -> String {
        let mut redacted = text.to_string();
        for (from, to) in &self.replacements {
            redacted = redacted.replace(from.as_str(), to.as_str());
        }
        crate::events::sanitize_detail(&redacted)
    }
}

// ============================================================================
// Production port
// ============================================================================

/// Production port over the shared worktree service and the session registry.
pub struct RemoteSessionOperations {
    registry: Arc<WorktreeSessionRegistry>,
    worktrees: Arc<RemoteWorktreeOperations>,
    service: Arc<WorktreeService>,
    /// As configured, for expansion and validation — exactly what the TUI passes.
    repo_root: PathBuf,
    /// Canonical, for display relativization.
    display_root: PathBuf,
    template: Option<String>,
    is_git_repo: bool,
    provision: Arc<dyn SessionProvisionRuntime>,
}

impl RemoteSessionOperations {
    /// Wire the port. Observes once whether the project is a Git repository.
    pub async fn new(
        registry: Arc<WorktreeSessionRegistry>,
        worktrees: Arc<RemoteWorktreeOperations>,
        service: Arc<WorktreeService>,
        repo_root: PathBuf,
        template: Option<String>,
    ) -> Self {
        let is_git_repo = crate::vcs::git::commands::check_git_repo(&repo_root)
            .await
            .unwrap_or(false);
        let provision: Arc<dyn SessionProvisionRuntime> =
            Arc::new(GitSessionProvisionRuntime::new(service.backend()));
        Self {
            display_root: std::fs::canonicalize(&repo_root).unwrap_or_else(|_| repo_root.clone()),
            registry,
            worktrees,
            service,
            repo_root,
            template,
            is_git_repo,
            provision,
        }
    }

    /// Replace the provisioning seam, for tests that need to move the
    /// repository between two of its steps.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_provision_runtime(mut self, runtime: Arc<dyn SessionProvisionRuntime>) -> Self {
        self.provision = runtime;
        self
    }

    fn display(&self, path: &Path) -> String {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        repository_relative_display(&self.display_root, &canonical)
    }

    fn redactor(&self, worktree: Option<&Path>) -> PathRedactor {
        let mut pairs = vec![
            (self.repo_root.clone(), ".".to_string()),
            (self.display_root.clone(), ".".to_string()),
            (
                self.service.workspace_base_dir().to_path_buf(),
                self.display(self.service.workspace_base_dir()),
            ),
        ];
        if let Some(path) = worktree {
            pairs.push((path.to_path_buf(), self.display(path)));
        }
        PathRedactor::new(pairs)
    }

    fn template(&self) -> Result<&str, CommandFailure> {
        if self.registry.is_closed() {
            return Err(unavailable("the owner is shutting down"));
        }
        if !self.is_git_repo {
            return Err(unavailable("the project is not a Git repository"));
        }
        self.template
            .as_deref()
            .ok_or_else(|| unavailable("worktree_command is not configured"))
    }

    fn expand(&self, template: &str, path: &Path) -> Result<String, CommandFailure> {
        let internal =
            || CommandFailure::new(ErrorCode::InternalError, "a path is not valid UTF-8");
        let path = path.to_str().ok_or_else(internal)?;
        let root = self.repo_root.to_str().ok_or_else(internal)?;
        Ok(crate::config::OrchestratorConfig::expand_worktree_command(
            template, path, root,
        ))
    }

    fn launched(&self, snapshot: &SessionSnapshot) -> ExecutionSummary {
        ExecutionSummary::changed(format!(
            "worktree session {} launched ({}) in worktree_id {}",
            snapshot.session_id,
            snapshot.kind.as_str(),
            snapshot.worktree_id.as_deref().unwrap_or("unknown"),
        ))
        .with_result(CommandResult::WorktreeSession(Box::new(
            WorktreeSessionResult {
                session_id: Some(snapshot.session_id.clone()),
                session_kind: snapshot.kind.as_str().to_string(),
                worktree_id: snapshot.worktree_id.clone(),
                branch: snapshot.branch.clone(),
                path: snapshot.path.clone(),
                setup: snapshot.setup.map(setup_dto),
                cleanup: None,
            },
        )))
    }
}

#[async_trait]
impl WorktreeSessionOperations for RemoteSessionOperations {
    fn launchable(&self) -> bool {
        self.template().is_ok()
    }

    fn sessions(&self) -> Vec<SessionSnapshot> {
        self.registry.list()
    }

    fn session(&self, session_id: &str) -> Option<SessionSnapshot> {
        self.registry.get(session_id)
    }

    fn running(&self) -> usize {
        self.registry.running_count()
    }

    async fn run(&self, worktree_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        let template = self.template()?.to_string();
        let resource = self.worktrees.resolve_live(worktree_id).await?;
        let path = resource.0.path.clone();

        // The slot and the worktree-busy fact are taken before any effect, and
        // released on every refusal below by dropping the reservation.
        let reservation = self
            .registry
            .reserve(Some(&path))
            .map_err(|refusal| map_refusal(&refusal))?;

        // A managed deletion or merge holds the root guard. Probed, not held:
        // the reservation is what keeps a later deletion from removing this cwd.
        drop(
            self.service
                .acquire_mutation_guard()
                .map_err(|error| map_worktree_error(&error))?,
        );

        if let Err(error) =
            crate::vcs::git::commands::validate_worktree_command_cwd(&self.repo_root, &path).await
        {
            return Err(CommandFailure::new(
                ErrorCode::TargetIneligible,
                format!(
                    "reason=cwd_invalid: {}",
                    self.redactor(Some(&path)).redact(&error.to_string())
                ),
            ));
        }

        let command = self.expand(&template, &path)?;
        let snapshot = self
            .registry
            .launch(
                reservation,
                LaunchSpec {
                    kind: SessionKind::Existing,
                    worktree_id: Some(worktree_id.to_string()),
                    display_path: resource.1.path.clone(),
                    branch: resource.1.branch.clone(),
                    path,
                    command,
                    setup: None,
                },
            )
            .await
            .map_err(|refusal| map_refusal(&refusal))?;
        Ok(self.launched(&snapshot))
    }

    async fn create(&self) -> Result<ExecutionSummary, CommandFailure> {
        let template = self.template()?.to_string();
        let reservation = self
            .registry
            .reserve(None)
            .map_err(|refusal| map_refusal(&refusal))?;

        let provisioned = {
            // The creation steps run under the repository mutation guard, so a
            // session worktree is `root_busy` while a merge or deletion runs.
            let _guard = self
                .service
                .acquire_mutation_guard()
                .map_err(|error| map_worktree_error(&error))?;
            let provisioner = SessionProvisioner::new(
                self.provision.as_ref(),
                &self.repo_root,
                self.service.workspace_base_dir(),
                Some(&template),
            );
            let provisioned = provisioner.provision().await;
            if let Ok(created) = &provisioned {
                // Bound before the guard is released, so no deletion can slip
                // in between creation and launch.
                reservation.bind_path(&created.path);
            }
            provisioned
        };

        let created = match provisioned {
            Ok(created) => created,
            Err(error) => return Err(self.provision_failure(error).await),
        };

        let worktree_id = self.worktrees.identify(&created.path).await;
        let snapshot = self
            .registry
            .launch(
                reservation,
                LaunchSpec {
                    kind: SessionKind::Session,
                    worktree_id,
                    display_path: self.display(&created.path),
                    branch: created.branch.clone(),
                    path: created.path.clone(),
                    command: created.command.clone(),
                    setup: Some(created.setup),
                },
            )
            .await
            .map_err(|refusal| map_refusal(&refusal))?;
        Ok(self.launched(&snapshot))
    }

    async fn stop(&self, session_id: &str) -> Result<ExecutionSummary, CommandFailure> {
        let outcome = self
            .registry
            .stop(session_id)
            .await
            .ok_or_else(|| not_found(session_id))?;
        let result = CommandResult::WorktreeSessionStop {
            session_id: session_id.to_string(),
            terminated: outcome.terminated,
            state_after: outcome.state_after.as_str().to_string(),
        };
        let summary = if outcome.was_running {
            ExecutionSummary::changed(format!(
                "worktree session {session_id} stopped (terminated={}, state={})",
                outcome.terminated,
                outcome.state_after.as_str()
            ))
        } else {
            ExecutionSummary::no_op(format!(
                "worktree session {session_id} had already finished ({})",
                outcome.state_after.as_str()
            ))
        };
        Ok(summary.with_result(result))
    }
}

impl RemoteSessionOperations {
    /// Map a provisioning refusal, carrying the created worktree when there is one.
    async fn provision_failure(&self, error: ProvisionError) -> CommandFailure {
        match error {
            ProvisionError::Unavailable(detail) => {
                unavailable(&self.redactor(None).redact(&detail))
            }
            ProvisionError::Internal(detail) => CommandFailure::new(
                ErrorCode::InternalError,
                self.redactor(None).redact(&detail),
            ),
            ProvisionError::CwdInvalid {
                phase,
                detail,
                path,
                branch,
                setup,
            } => {
                let worktree_id = self.worktrees.identify(&path).await;
                let message = format!(
                    "reason=cwd_invalid phase={}: {}; nothing was launched",
                    phase.as_str(),
                    self.redactor(Some(&path)).redact(&detail)
                );
                CommandFailure::new(ErrorCode::TargetIneligible, message).with_result(
                    CommandResult::WorktreeSession(Box::new(WorktreeSessionResult {
                        session_id: None,
                        session_kind: SessionKind::Session.as_str().to_string(),
                        worktree_id,
                        branch,
                        path: self.display(&path),
                        setup: setup.map(setup_dto),
                        cleanup: None,
                    })),
                )
            }
            ProvisionError::SetupFailed {
                detail,
                cleanup,
                path,
                branch,
            } => {
                let redactor = self.redactor(Some(&path));
                let worktree_id = match cleanup {
                    CleanupOutcome::Removed => None,
                    CleanupOutcome::Retained { .. } => self.worktrees.identify(&path).await,
                };
                let cleanup = cleanup_dto(&cleanup, &redactor);
                let message = format!(
                    "reason=setup_failed: {}; cleanup {}; nothing was launched",
                    redactor.redact(&detail),
                    cleanup.outcome
                );
                CommandFailure::new(ErrorCode::WorktreeSessionSetupFailed, message).with_result(
                    CommandResult::WorktreeSession(Box::new(WorktreeSessionResult {
                        session_id: None,
                        session_kind: SessionKind::Session.as_str().to_string(),
                        worktree_id,
                        branch,
                        path: self.display(&path),
                        setup: Some(SessionSetupFacts {
                            ran: true,
                            succeeded: false,
                        }),
                        cleanup: Some(cleanup),
                    })),
                )
            }
        }
    }
}

fn setup_dto(setup: SetupFacts) -> SessionSetupFacts {
    SessionSetupFacts {
        ran: setup.ran,
        succeeded: setup.succeeded,
    }
}

fn cleanup_dto(cleanup: &CleanupOutcome, redactor: &PathRedactor) -> SessionCleanupFacts {
    match cleanup {
        CleanupOutcome::Removed => SessionCleanupFacts {
            outcome: "removed".to_string(),
            removed: true,
            reason: None,
        },
        CleanupOutcome::Retained { reason } => SessionCleanupFacts {
            outcome: "retained".to_string(),
            removed: false,
            reason: Some(redactor.redact(reason)),
        },
    }
}

/// Project one session onto the wire.
pub fn resource(snapshot: &SessionSnapshot, instance_id: &str) -> WorktreeSessionResource {
    WorktreeSessionResource {
        session_id: snapshot.session_id.clone(),
        instance_id: instance_id.to_string(),
        kind: snapshot.kind.as_str().to_string(),
        worktree_id: snapshot.worktree_id.clone(),
        path: snapshot.path.clone(),
        branch: snapshot.branch.clone(),
        state: snapshot.state.as_str().to_string(),
        attempt: snapshot.attempt,
        started_at: snapshot.started_at.to_rfc3339(),
        finished_at: snapshot.finished_at.map(|at| at.to_rfc3339()),
        exit_code: snapshot.exit_code,
        signal: snapshot.signal.clone(),
        stop_requested: snapshot.stop_requested,
        setup: snapshot.setup.map(setup_dto),
        // Always `None` for a launched session: setup-failure cleanup is
        // reported on the refused command's result, through the redactor.
        cleanup: snapshot.cleanup.as_ref().map(|cleanup| match cleanup {
            CleanupOutcome::Removed => SessionCleanupFacts {
                outcome: "removed".to_string(),
                removed: true,
                reason: None,
            },
            CleanupOutcome::Retained { reason } => SessionCleanupFacts {
                outcome: "retained".to_string(),
                removed: false,
                reason: Some(reason.clone()),
            },
        }),
        output: SessionOutput {
            lines: snapshot
                .output
                .lines
                .iter()
                .map(|line| SessionOutputLine {
                    stream: line.stream.as_str().to_string(),
                    text: line.text.clone(),
                })
                .collect(),
            retained_bytes: snapshot.output.retained_bytes,
            truncated_bytes: snapshot.output.truncated_bytes,
        },
    }
}

// ============================================================================
// Events
// ============================================================================

/// Publishes session state events as presentation events: they take a
/// sequence but never compare or replace the snapshot, so a session never moves
/// the revision. Output never travels here.
pub struct ProjectionSessionEvents {
    projection: Arc<Projection>,
}

impl ProjectionSessionEvents {
    /// Publish into `projection`.
    pub fn new(projection: Arc<Projection>) -> Self {
        Self { projection }
    }
}

impl SessionEventSink for ProjectionSessionEvents {
    fn publish(&self, event: &SessionEvent) {
        self.projection.apply_presentation(
            event.event_type,
            None,
            json!({
                "session_id": event.session_id,
                "worktree_id": event.worktree_id,
                "state": event.state.as_str(),
                "exit_code": event.exit_code,
            }),
        );
    }
}

// ============================================================================
// Read routes
// ============================================================================

fn no_store<T: serde::Serialize>(body: T) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(body)).into_response()
}

/// Retained configured-command worktree sessions, newest first.
#[utoipa::path(
    get,
    path = "/api/v2/worktree-sessions",
    tag = "remote-control",
    responses((status = 200, description = "Retained worktree sessions, newest first", body = WorktreeSessionsResponse))
)]
pub async fn list_sessions(State(state): State<RemoteControlState>) -> Response {
    let instance_id = state.projection.instance_id().to_string();
    let sessions = state
        .worktree_sessions
        .get()
        .map(|port| port.sessions())
        .unwrap_or_default();
    no_store(WorktreeSessionsResponse {
        state_revision: state.projection.revision(),
        sessions: sessions
            .iter()
            .map(|snapshot| resource(snapshot, &instance_id))
            .collect(),
        instance_id,
    })
}

/// One configured-command worktree session.
#[utoipa::path(
    get,
    path = "/api/v2/worktree-sessions/{session_id}",
    tag = "remote-control",
    params(("session_id" = String, Path, description = "Opaque process-local session ID")),
    responses(
        (status = 200, description = "The requested session", body = WorktreeSessionResponse),
        (status = 404, description = "No such session in this incarnation", body = ApiError)
    )
)]
pub async fn get_session(
    State(state): State<RemoteControlState>,
    Extension(correlation): Extension<CorrelationId>,
    AxumPath(session_id): AxumPath<String>,
) -> Response {
    let instance_id = state.projection.instance_id().to_string();
    let state_revision = state.projection.revision();
    match state
        .worktree_sessions
        .get()
        .and_then(|port| port.session(&session_id))
    {
        Some(snapshot) => no_store(WorktreeSessionResponse {
            session: resource(&snapshot, &instance_id),
            instance_id,
            state_revision,
        }),
        None => ApiError::new(
            ErrorCode::NotFound,
            format!("worktree session '{session_id}' is not known to this instance"),
            &correlation.0,
        )
        .with_revision(state_revision)
        .into_response(),
    }
}
