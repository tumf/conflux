//! Connecting to one owner, and reading it coherently.
//!
//! # Why a "coherent observation" needs its own type
//!
//! `/api/v2` publishes capabilities, instance identity, authoritative state,
//! execution status, and the owner execution contract as *separate* resources.
//! Each one is internally consistent; a set of them read one after another is
//! not. A client that mixed a snapshot from revision 8 with an execution
//! contract from revision 12 would be building a completion claim out of two
//! different instants, which is exactly the failure the API's revision and
//! incarnation tokens exist to make detectable.
//!
//! So the reads are joined here, once, under three rules:
//!
//! * every resource must carry the same `instance_id` — a restart invalidates
//!   the whole observation, not just the cursor;
//! * every revision-bearing resource must agree on one `state_revision`, which
//!   is why `/state` is read *last* and the others are re-read against it;
//! * `event_sequence` must not move backwards.
//!
//! `status` gives up after a bounded number of rereads and says
//! `observation_conflict`; `wait` keeps reconciling until its own deadline,
//! because a busy owner legitimately advances between reads and a wait that
//! failed on the first race would be useless.

use std::path::{Path, PathBuf};

use crate::client::envelope::{Operation, Outcome, ResultEnvelope};
use crate::client::transport::{TransportError, UnixApiClient};
use crate::web::remote_control_api::dto::{
    ApiError, CapabilitiesResponse, ChangeResource, ExecutionContractResponse,
    ExecutionStatusResponse, InstanceResponse, StateResponse, API_VERSION,
};

/// How many times `status` rereads before reporting an observation conflict.
///
/// Bounded because an owner under sustained load can advance faster than a
/// client can read, and an unbounded reconciliation loop would present that as
/// a hang instead of as the typed conflict it is.
pub const MAX_RECONCILE_ATTEMPTS: usize = 5;

/// A refusal raised before any owner conversation happened.
#[derive(Debug)]
pub struct ConnectionRefusal {
    outcome: Outcome,
    message: String,
}

impl ConnectionRefusal {
    /// Project the refusal onto the operation the caller asked for.
    pub fn into_envelope(self, operation: Operation) -> ResultEnvelope {
        ResultEnvelope::new(operation, self.outcome).with_message(self.message)
    }
}

/// One resolved client connection: where the owner is, and how to authenticate.
///
/// Safe to `Debug`-print: the transport redacts the bearer token in its own
/// hand-written `Debug`, so no rendering of a connection can leak one.
#[derive(Debug)]
pub struct Connection {
    client: UnixApiClient,
    /// Repository root, when the client is inside one.
    ///
    /// `wait` needs it to verify completion from repository evidence. An
    /// explicit `--unix-socket` outside a repository is still a usable
    /// connection for `status`, so this stays optional rather than a hard
    /// requirement of connecting.
    repo_root: Option<PathBuf>,
}

impl Connection {
    /// Resolve the socket path and the bearer token without contacting anyone.
    ///
    /// The default path is derived from the canonical Git common directory —
    /// the same identity the repository lock excludes on — so every linked
    /// worktree of one repository reaches the one owner that lock permits.
    pub fn resolve(
        explicit_socket: Option<&Path>,
        auth_token_env: Option<&str>,
    ) -> Result<Self, ConnectionRefusal> {
        let workspace = std::env::current_dir().map_err(|error| ConnectionRefusal {
            outcome: Outcome::NotInRepository,
            message: format!("the current directory could not be resolved: {error}"),
        })?;
        let common_dir = crate::repo_lock::discover_common_dir(&workspace);
        let repo_root = discover_repo_root(&workspace);

        let socket =
            match explicit_socket {
                Some(path) => path.to_path_buf(),
                None => match &common_dir {
                    Some(common_dir) => crate::web::unix_socket::default_socket_path(common_dir),
                    None => return Err(ConnectionRefusal {
                        outcome: Outcome::NotInRepository,
                        message:
                            "the default owner socket needs a Git repository: run inside one, or \
                             name the socket with --unix-socket PATH"
                                .to_string(),
                    }),
                },
            };

        let token = match auth_token_env {
            Some(name) => match std::env::var(name) {
                Ok(value) if !value.is_empty() => Some(value),
                // An empty or absent variable is a configuration mistake, not a
                // reason to silently connect anonymously and then report the
                // owner's 401 as if the owner were at fault.
                _ => {
                    return Err(ConnectionRefusal {
                        outcome: Outcome::AuthenticationFailed,
                        message: format!(
                            "environment variable '{name}' is unset or empty, so no bearer token \
                             could be presented"
                        ),
                    })
                }
            },
            None => None,
        };

        // The token is framed into an HTTP header, so a value carrying CR, LF,
        // another control byte, or DEL is refused here — before a connection is
        // opened and before any request bytes exist. The diagnostic names the
        // variable and the byte class only: echoing the value would leak the
        // credential, and echoing an injection attempt would leak it into a log.
        let client = UnixApiClient::new(socket, token).map_err(|rejection| ConnectionRefusal {
            outcome: Outcome::AuthenticationFailed,
            message: match auth_token_env {
                Some(name) => format!(
                    "the bearer token in environment variable '{name}' contains {rejection}, \
                     which cannot be sent as an HTTP header value. The value is not shown"
                ),
                None => format!(
                    "the configured bearer token contains {rejection}, which cannot be sent as \
                     an HTTP header value. The value is not shown"
                ),
            },
        })?;

        Ok(Self { client, repo_root })
    }

    /// The underlying transport.
    pub fn client(&self) -> &UnixApiClient {
        &self.client
    }

    /// The repository root, when the client is inside one.
    pub fn repo_root(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }
}

/// Locate the repository root without mutating anything.
///
/// A plain `rev-parse` rather than a walk up the tree, so a worktree, a
/// submodule, and a plain clone all resolve the way Git itself resolves them.
fn discover_repo_root(workspace: &Path) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(workspace)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

/// One coherent multi-resource observation of a single owner incarnation.
pub struct Observation {
    /// Incarnation every resource in this observation agreed on.
    pub instance_id: String,
    /// Revision every revision-bearing resource agreed on.
    pub state_revision: u64,
    /// Latest event sequence at the reconciled revision; a valid resume cursor.
    pub event_sequence: u64,
    /// What the owner accepts.
    pub capabilities: CapabilitiesResponse,
    /// Who the owner is.
    pub instance: InstanceResponse,
    /// Authoritative operator state.
    pub state: StateResponse,
    /// What is actually running.
    pub execution: ExecutionStatusResponse,
    /// What would prove a change finished.
    pub contract: ExecutionContractResponse,
}

impl Observation {
    /// One projected change, when the owner tracks it.
    pub fn change(&self, change_id: &str) -> Option<&ChangeResource> {
        self.state
            .snapshot
            .changes
            .iter()
            .find(|change| change.id == change_id)
    }

    /// Whether this owner can execute a command at all.
    pub fn command_capable(&self) -> bool {
        self.capabilities.command_execution.available
    }

    /// The process-local execution episode the owner publishes for one change.
    ///
    /// `None` on an owner that predates execution identity, and on a change
    /// this incarnation never admitted. Both are honest absences rather than
    /// something to synthesize: an episode ID this client invented would name
    /// no subscription the owner could ever deliver.
    pub fn execution_id(&self, change_id: &str) -> Option<String> {
        self.execution
            .changes
            .iter()
            .find(|status| status.id == change_id)
            .and_then(|status| status.execution_id.clone())
    }

    /// Whether this owner serves execution-scoped completion sinks.
    pub fn execution_sinks_available(&self) -> bool {
        self.capabilities.execution_sinks.available
    }
}

/// Why one observation attempt did not produce a usable result.
#[derive(Debug)]
pub enum ObserveError {
    /// The reads could not be joined at one incarnation and revision.
    Incoherent(String),
    /// The owner is not one this build can operate.
    Incompatible(String),
    /// Credentials were missing, rejected, or not accepted.
    Unauthenticated(String),
    /// Nothing is listening.
    NotRunning(String),
    /// The exchange itself failed.
    Transport(String),
}

impl ObserveError {
    /// Stable outcome for this failure.
    pub fn outcome(&self) -> Outcome {
        match self {
            Self::Incoherent(_) => Outcome::ObservationConflict,
            Self::Incompatible(_) => Outcome::IncompatibleOwner,
            Self::Unauthenticated(_) => Outcome::AuthenticationFailed,
            Self::NotRunning(_) => Outcome::OwnerNotRunning,
            Self::Transport(_) => Outcome::TransportError,
        }
    }

    /// Sanitized explanation.
    pub fn message(&self) -> &str {
        match self {
            Self::Incoherent(detail)
            | Self::Incompatible(detail)
            | Self::Unauthenticated(detail)
            | Self::NotRunning(detail)
            | Self::Transport(detail) => detail,
        }
    }

    /// Whether rereading could plausibly succeed.
    ///
    /// Only incoherence is transient: an incompatible owner, a rejected
    /// credential, and an absent socket all stay that way for this invocation.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Incoherent(_))
    }

    /// Build the envelope for this failure.
    pub fn into_envelope(self, operation: Operation) -> ResultEnvelope {
        let outcome = self.outcome();
        let message = self.message().to_string();
        ResultEnvelope::new(operation, outcome).with_message(message)
    }
}

impl From<TransportError> for ObserveError {
    fn from(error: TransportError) -> Self {
        match &error {
            TransportError::NotListening { .. } => Self::NotRunning(error.to_string()),
            _ => Self::Transport(error.to_string()),
        }
    }
}

/// Read one resource, mapping the owner's typed refusals onto client outcomes.
async fn read<T: serde::de::DeserializeOwned>(
    client: &UnixApiClient,
    path_and_query: &str,
) -> Result<T, ObserveError> {
    let response = client.get(path_and_query).await?;
    match response.status {
        200 => response.json::<T>().map_err(|error| {
            // A read that parses on a matching build and not on this one means
            // the shapes diverged, which is an owner-compatibility fact rather
            // than a transport fault.
            ObserveError::Incompatible(format!(
                "'{path_and_query}' did not match this build's contract: {error}"
            ))
        }),
        401 => Err(ObserveError::Unauthenticated(describe_api_error(
            &response.body,
            "the owner requires a bearer token; supply one with --auth-token-env NAME",
        ))),
        403 => Err(ObserveError::Unauthenticated(describe_api_error(
            &response.body,
            "the owner refused the presented credentials",
        ))),
        404 => Err(ObserveError::Incompatible(format!(
            "the owner does not serve '{path_and_query}', so it is not a compatible build"
        ))),
        status => Err(ObserveError::Transport(format!(
            "'{path_and_query}' returned unexpected status {status}"
        ))),
    }
}

/// Read a typed API error's message, or fall back to a client-side explanation.
///
/// The owner's own message is preferred because it names the actual refusal, but
/// a body that is not a typed error must never be echoed verbatim: it is
/// untrusted bytes.
pub fn describe_api_error(body: &[u8], fallback: &str) -> String {
    match serde_json::from_slice::<ApiError>(body) {
        Ok(error) => format!("{} ({})", error.message, error.error_code.as_str()),
        Err(_) => fallback.to_string(),
    }
}

/// Take one coherent observation, or say why it could not be taken.
///
/// Read order is deliberate: everything that does not carry a revision first,
/// then the revision-bearing resources, with `/state` last. A mismatch is
/// reported rather than papered over — the caller decides whether to reread.
pub async fn observe(
    connection: &Connection,
    change_id: Option<&str>,
) -> Result<Observation, ObserveError> {
    let client = connection.client();

    let capabilities: CapabilitiesResponse = read(client, "/api/v2/capabilities").await?;
    if capabilities.api_version != API_VERSION {
        return Err(ObserveError::Incompatible(format!(
            "the owner serves API version '{}', but this client speaks '{API_VERSION}'",
            capabilities.api_version
        )));
    }
    let instance: InstanceResponse = read(client, "/api/v2/instance").await?;

    let contract_path = match change_id {
        Some(change_id) => format!(
            "/api/v2/execution-contract?change_id={}",
            crate::client::transport::encode_query_value(change_id)
        ),
        None => "/api/v2/execution-contract".to_string(),
    };
    let contract: ExecutionContractResponse = read(client, &contract_path).await?;
    let execution: ExecutionStatusResponse = read(client, "/api/v2/execution-status").await?;
    let state: StateResponse = read(client, "/api/v2/state").await?;

    let instances = [
        capabilities.instance_id.as_str(),
        instance.instance_id.as_str(),
        contract.instance_id.as_str(),
        execution.instance_id.as_str(),
        state.instance_id.as_str(),
    ];
    if instances.iter().any(|id| *id != state.instance_id) {
        return Err(ObserveError::Incoherent(
            "the owner's resources reported different process incarnations, so they cannot be \
             one observation"
                .to_string(),
        ));
    }

    // A cursor that moved backwards means the reads did not come from one
    // ordered history, whatever the revisions say.
    if execution.event_sequence > state.event_sequence {
        return Err(ObserveError::Incoherent(
            "the owner's event cursor moved backwards between reads".to_string(),
        ));
    }

    // `/state` is read last, so an earlier resource may legitimately be one
    // revision behind — but only until it is re-read at the final revision.
    if contract.state_revision != state.state_revision
        || execution.state_revision != state.state_revision
    {
        let contract: ExecutionContractResponse = read(client, &contract_path).await?;
        let execution: ExecutionStatusResponse = read(client, "/api/v2/execution-status").await?;
        if contract.instance_id != state.instance_id || execution.instance_id != state.instance_id {
            return Err(ObserveError::Incoherent(
                "the owner restarted while the observation was being reconciled".to_string(),
            ));
        }
        if contract.state_revision != state.state_revision
            || execution.state_revision != state.state_revision
        {
            return Err(ObserveError::Incoherent(format!(
                "the owner advanced past revision {} while the observation was being reconciled",
                state.state_revision
            )));
        }
        return Ok(Observation {
            instance_id: state.instance_id.clone(),
            state_revision: state.state_revision,
            event_sequence: state.event_sequence,
            capabilities,
            instance,
            state,
            execution,
            contract,
        });
    }

    Ok(Observation {
        instance_id: state.instance_id.clone(),
        state_revision: state.state_revision,
        event_sequence: state.event_sequence,
        capabilities,
        instance,
        state,
        execution,
        contract,
    })
}

/// Reread until one coherent observation is produced, up to `attempts` times.
pub async fn observe_bounded(
    connection: &Connection,
    change_id: Option<&str>,
    attempts: usize,
) -> Result<Observation, ObserveError> {
    let mut last = None;
    for _ in 0..attempts.max(1) {
        match observe(connection, change_id).await {
            Ok(observation) => return Ok(observation),
            Err(error) if error.is_transient() => last = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last.unwrap_or_else(|| {
        ObserveError::Incoherent("no coherent observation was produced".to_string())
    }))
}

/// `cflx client status`: read the owner, mutate nothing.
pub async fn status(connection: &Connection) -> ResultEnvelope {
    let observation = match observe_bounded(connection, None, MAX_RECONCILE_ATTEMPTS).await {
        Ok(observation) => observation,
        Err(error) => return error.into_envelope(Operation::Status),
    };

    let changes: Vec<serde_json::Value> = observation
        .state
        .snapshot
        .changes
        .iter()
        .map(|change| {
            let execution = observation
                .execution
                .changes
                .iter()
                .find(|status| status.id == change.id);
            serde_json::json!({
                "id": change.id,
                "display_status": change.display_status,
                "queue_intent": change.queue_intent,
                "execution_marked": change.execution_marked,
                "parallel_eligible": change.parallel.eligible,
                "actions": change.actions,
                "blocker": change.blocker,
                "execution_state": execution.map(|status| status.execution_state),
                "current_phase": execution.map(|status| status.current_phase),
                "last_completed_phase": execution.and_then(|status| status.last_completed_phase),
            })
        })
        .collect();

    let detail = serde_json::json!({
        "socket": connection.client().socket().display().to_string(),
        "state_revision": observation.state_revision,
        "event_sequence": observation.event_sequence,
        "owner": {
            "pid": observation.instance.pid,
            "version": observation.instance.version,
            "started_at": observation.instance.started_at,
            "api_version": observation.instance.api_version,
        },
        "process": {
            "app_mode": observation.execution.process.app_mode,
            "scheduler_running": observation.execution.process.scheduler_running,
            "has_active_work": observation.execution.process.has_active_work,
            "active_activities": observation.execution.process.active_activities,
            "process_error": observation.state.snapshot.process_error,
            "is_resolving": observation.state.snapshot.is_resolving,
            "persistent_scheduler_idle": observation.state.snapshot.persistent_scheduler_idle,
        },
        "command_execution_available": observation.command_capable(),
        "authentication_required": observation.capabilities.authentication_required,
        "execution_contract": observation.contract.contract,
        "totals": observation.state.snapshot.totals,
        "changes": changes,
    });

    ResultEnvelope::new(Operation::Status, Outcome::Observed)
        .with_instance(Some(observation.instance_id))
        .with_detail(detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::remote_control_api::dto::ErrorCode;

    #[test]
    fn an_absent_socket_outside_a_repository_names_both_choices() {
        let tmp = tempfile::tempdir().unwrap();
        let previous = std::env::current_dir().unwrap();
        // A temporary directory outside any repository is the only way to
        // observe the no-identity refusal, and the directory is restored before
        // the assertion so a failure cannot strand the test process.
        std::env::set_current_dir(tmp.path()).unwrap();
        let resolved = Connection::resolve(None, None);
        std::env::set_current_dir(previous).unwrap();

        // A temp dir on a machine whose /tmp happens to be inside a repository
        // would resolve; only assert the refusal shape when it did refuse.
        if let Err(refusal) = resolved {
            assert_eq!(refusal.outcome, Outcome::NotInRepository);
            assert!(refusal.message.contains("--unix-socket"), "{refusal:?}");
        }
    }

    #[test]
    fn an_unset_token_variable_fails_closed_instead_of_connecting_anonymously() {
        let refusal = Connection::resolve(
            Some(Path::new("/tmp/cflx-client-test.sock")),
            Some("CFLX_CLIENT_TEST_TOKEN_THAT_IS_NOT_SET"),
        )
        .expect_err("an unset variable must refuse");
        assert_eq!(refusal.outcome, Outcome::AuthenticationFailed);
        assert!(refusal.message.contains("unset or empty"), "{refusal:?}");
    }

    #[test]
    fn an_explicit_socket_needs_no_repository_identity() {
        let connection = Connection::resolve(Some(Path::new("/tmp/explicit.sock")), None)
            .expect("an explicit socket always resolves");
        assert_eq!(
            connection.client().socket(),
            Path::new("/tmp/explicit.sock")
        );
        assert!(!connection.client().has_token());
    }

    #[test]
    fn a_typed_api_error_is_described_by_its_own_message_and_code() {
        let body = serde_json::to_vec(&ApiError::new(
            ErrorCode::Unauthorized,
            "missing bearer credentials",
            "corr-1",
        ))
        .unwrap();
        let described = describe_api_error(&body, "fallback");
        assert!(described.contains("missing bearer credentials"));
        assert!(described.contains("unauthorized"));
    }

    #[test]
    fn an_untyped_body_is_never_echoed_back_to_the_caller() {
        let described = describe_api_error(b"<html>secret</html>", "fallback text");
        assert_eq!(described, "fallback text");
    }

    #[test]
    fn only_incoherence_is_worth_rereading() {
        assert!(ObserveError::Incoherent(String::new()).is_transient());
        for error in [
            ObserveError::Incompatible(String::new()),
            ObserveError::Unauthenticated(String::new()),
            ObserveError::NotRunning(String::new()),
            ObserveError::Transport(String::new()),
        ] {
            assert!(!error.is_transient(), "{error:?}");
        }
    }

    #[test]
    fn observation_failures_map_to_stable_outcomes() {
        assert_eq!(
            ObserveError::Incoherent(String::new()).outcome(),
            Outcome::ObservationConflict
        );
        assert_eq!(
            ObserveError::Incompatible(String::new()).outcome(),
            Outcome::IncompatibleOwner
        );
        assert_eq!(
            ObserveError::Unauthenticated(String::new()).outcome(),
            Outcome::AuthenticationFailed
        );
        assert_eq!(
            ObserveError::NotRunning(String::new()).outcome(),
            Outcome::OwnerNotRunning
        );
        assert_eq!(
            ObserveError::Transport(String::new()).outcome(),
            Outcome::TransportError
        );
    }
}
