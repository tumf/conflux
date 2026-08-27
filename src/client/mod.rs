//! `cflx client` — a client of an existing owner, never an owner itself.
//!
//! # What this namespace is for
//!
//! An external agent wants to hand work to the Conflux process that already
//! holds this repository. Before this existed it had two bad options: run
//! `cflx run`, which is an *owner* of a finite explicit-target run and contends
//! for the repository lock, or speak `/api/v2` directly and reimplement
//! optimistic revisions, idempotency identity, command settlement, mode-aware
//! mark/queue/start routing, and truthful completion. The second option makes
//! every caller break whenever the orchestration state model moves.
//!
//! So this is a thin, control-shaped client whose verbs are the operator's own:
//! `status`, `mark`, `unmark`, `start`, `stop`, `force-stop`,
//! `force-stop-change`, `wait`, the nested
//! `subscribe` group, and `mcp` — stable JSON, stable exit statuses, and no
//! protocol details in the public surface at all.
//!
//! The shape is deliberately the TUI's rather than an "admit this change"
//! abstraction over it. Marking is operator selection; Start is the explicit
//! lifecycle control that consumes the authoritative mark set; admission is the
//! owner's own conclusion. A client that decided admission policy itself — as
//! this one used to, by writing queue intent directly — could move work into the
//! scheduler's admitted set along a path no keypress can take.
//!
//! `subscribe` manages the completion callback for a *proposal*, which is
//! observability rather than workflow: it submits no command and cannot move
//! anything. `mcp` is the same boundary spoken as Model Context Protocol, so an
//! agent gets the identical routing and the identical typed outcomes whether it
//! types a command or calls a tool.
//!
//! # What it must never do
//!
//! Nothing here acquires the orchestration repository lock, binds a listener,
//! loads orchestration configuration, initializes runtime logging, starts a
//! lifecycle adapter, or launches an AI subprocess. Git is touched read-only,
//! and only to derive the repository's canonical common directory and to
//! *verify* completion — never to produce it.
//!
//! # Truthfulness
//!
//! A settled command record proves the owner accepted an intent, not that a
//! change was implemented, accepted, archived, or integrated — and a settled
//! mark proves less still, because a mark is next-run intent that the owner may
//! never admit. `wait` therefore certifies success from current repository
//! evidence for the owner's declared terminal mode, and treats owner
//! disappearance, owner restart, rejection, process failure, and timeout as
//! distinct unsuccessful outcomes rather than as completion.

pub mod envelope;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::CommandFactory;

#[cfg(feature = "web-monitoring")]
pub mod completion;
#[cfg(feature = "web-monitoring")]
pub mod control;
#[cfg(feature = "web-monitoring")]
pub mod mcp;
#[cfg(feature = "web-monitoring")]
pub mod repo;
#[cfg(feature = "web-monitoring")]
mod session;
#[cfg(feature = "web-monitoring")]
pub mod subscribe;
#[cfg(feature = "web-monitoring")]
mod transport;
#[cfg(feature = "web-monitoring")]
mod wait;

use envelope::{Operation, Outcome, ResultEnvelope};

use crate::cli::{ClientArgs, ClientCommands, ClientSubscribeCommands};

// ============================================================================
// Which owner one operation talks to
// ============================================================================

/// The route one client operation takes to an owner.
///
/// # Why the public selector is a directory rather than a socket
///
/// A socket path is an implementation detail of one owner incarnation: it lives
/// under a Git common directory a caller has to discover, and naming it forces
/// every agent to reimplement that derivation. The stable identity of the work
/// is the *project* — a directory inside the repository the owner holds. So
/// `project_dir` is the normal selector, and `unix_socket` stays as the
/// low-level override for diagnostics, tests, and non-repository transports.
///
/// # Why one selector rather than two merged fields
///
/// A route is one decision. Modelling it as two optional fields would let a
/// call arrive naming both, and the only honest answers to that are "refuse"
/// or "silently prefer one" — and silently preferring one is exactly how a
/// registration for project B reaches project A's owner. Building the selector
/// is therefore where the conflict is caught, once, before any owner is
/// contacted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RouteSelector {
    /// No explicit selector: fall back to the namespace default, then to
    /// current-working-directory repository discovery.
    #[default]
    Default,
    /// An absolute directory inside the project's Git working tree.
    Project(PathBuf),
    /// An explicit owner `/api/v2` Unix socket.
    Socket(PathBuf),
}

/// Why a route could not be built, stated without echoing a credential.
///
/// Deliberately not an [`Outcome`]: a rejected selector never reached an owner,
/// so it is a bounded validation failure on the caller's own arguments — the
/// existing MCP `ToolError` / CLI usage-error channel — rather than a new
/// stable-envelope outcome describing an owner conversation that never happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteError {
    /// Sanitized explanation. May name the rejected path; never a secret.
    pub message: String,
}

impl RouteError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl RouteSelector {
    /// Build one call's selector from the two optional inputs it may carry.
    ///
    /// Pure: it inspects the arguments and nothing else — no filesystem, no
    /// Git, no owner. That is what lets the conflict and the relative-path
    /// refusals happen provably *before* contact.
    pub fn from_inputs(
        project_dir: Option<&Path>,
        unix_socket: Option<&Path>,
    ) -> Result<Self, RouteError> {
        match (project_dir, unix_socket) {
            (Some(_), Some(_)) => Err(RouteError::new(
                "project_dir and unix_socket select two different routes, so supplying both in \
                 one call is ambiguous. Name the project directory, or the socket, not both",
            )),
            (Some(project), None) => {
                if project.as_os_str().is_empty() {
                    return Err(RouteError::new("project_dir is empty"));
                }
                if !project.is_absolute() {
                    return Err(RouteError::new(format!(
                        "project_dir '{}' is relative. It must be absolute: this process's \
                         working directory has nothing to do with the project the work belongs to",
                        project.display()
                    )));
                }
                Ok(Self::Project(project.to_path_buf()))
            }
            (None, Some(socket)) => {
                if socket.as_os_str().is_empty() {
                    return Err(RouteError::new("unix_socket is empty"));
                }
                Ok(Self::Socket(socket.to_path_buf()))
            }
            (None, None) => Ok(Self::Default),
        }
    }

    /// This selector when it names a route, otherwise the fallback.
    ///
    /// The precedence the contract requires: a call-scoped selector overrides a
    /// namespace-level default, and it does so by *shadowing* it rather than by
    /// writing to it — nothing here mutates the default, so two concurrent
    /// calls cannot change each other's route.
    pub fn or_default(self, fallback: &RouteSelector) -> Self {
        match self {
            Self::Default => fallback.clone(),
            explicit => explicit,
        }
    }
}

/// One project's owner socket and the repository that proves completion.
///
/// Both come from the *same* selected project on purpose. A route that took the
/// socket from one repository and the completion evidence from another would
/// certify project A's archive as project B's success, which is the exact
/// failure truthful `wait` exists to prevent.
#[cfg(feature = "web-monitoring")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoute {
    /// `<git-common-dir>/cflx-api.sock` for the selected project.
    pub socket: PathBuf,
    /// The selected project's canonical working-tree root.
    pub repo_root: PathBuf,
}

/// Resolve one absolute project directory into an owner route.
///
/// Read-only by construction: it stats a path, asks Git where the working tree
/// and the common directory are, and derives a socket path. It creates nothing,
/// starts no owner, and never infers a project from a change ID.
///
/// A linked worktree, a submodule, a symlinked path, and a directory below the
/// working-tree root all resolve, because the common directory is derived the
/// same way the repository lock derives it — and the lock is what guarantees
/// there is only one default owner to reach.
#[cfg(feature = "web-monitoring")]
pub fn resolve_project(project_dir: &Path) -> Result<ProjectRoute, RouteError> {
    if !project_dir.is_absolute() {
        return Err(RouteError::new(format!(
            "project_dir '{}' is relative. It must be absolute",
            project_dir.display()
        )));
    }
    let metadata = std::fs::metadata(project_dir).map_err(|error| {
        RouteError::new(format!(
            "project_dir '{}' could not be read: {error}",
            project_dir.display()
        ))
    })?;
    if !metadata.is_dir() {
        return Err(RouteError::new(format!(
            "project_dir '{}' is not a directory",
            project_dir.display()
        )));
    }
    let canonical =
        std::fs::canonicalize(project_dir).unwrap_or_else(|_| project_dir.to_path_buf());

    // Asked of Git rather than walked by hand: a bare repository and a path
    // outside any repository both fail here, and neither may be answered with a
    // guessed socket.
    let repo_root = session::discover_repo_root(&canonical).ok_or_else(|| {
        RouteError::new(format!(
            "project_dir '{}' is not inside a usable Git working tree, so no owner socket can \
             be derived from it",
            project_dir.display()
        ))
    })?;
    let common_dir = crate::repo_lock::discover_common_dir(&canonical).ok_or_else(|| {
        RouteError::new(format!(
            "the Git common directory of project_dir '{}' could not be resolved",
            project_dir.display()
        ))
    })?;
    Ok(ProjectRoute {
        socket: crate::web::unix_socket::default_socket_path(&common_dir),
        repo_root,
    })
}

/// How the caller asked for output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Concise single-line human output.
    Human,
    /// Exactly one versioned JSON envelope on stdout.
    Json,
}

impl OutputMode {
    /// Select the mode from a `--json` flag.
    pub fn from_json_flag(json: bool) -> Self {
        if json {
            Self::Json
        } else {
            Self::Human
        }
    }
}

/// Write the one result and return the process exit status.
///
/// stdout carries the result and nothing else. Every diagnostic — including the
/// human-readable rendering of a failure — is a stderr concern, so a caller
/// parsing stdout never has to strip progress text out of its JSON.
pub fn emit(envelope: &ResultEnvelope, mode: OutputMode) -> i32 {
    use std::io::Write;

    let line = match mode {
        OutputMode::Json => envelope.to_json_line(),
        OutputMode::Human => envelope.to_human_line(),
    };
    let mut stdout = std::io::stdout();
    // A closed stdout must not be reported as a successful operation: a caller
    // that never received the envelope has no result.
    if writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return Outcome::TransportError.exit_code();
    }
    if !envelope.ok {
        if let Some(message) = &envelope.message {
            eprintln!("cflx client: {}: {message}", envelope.outcome.as_str());
        } else {
            eprintln!("cflx client: {}", envelope.outcome.as_str());
        }
    }
    envelope.exit_code()
}

/// The exact spelling that selects machine output.
///
/// Matched as a whole argument and never as a substring: a change ID or a socket
/// path that merely *contains* `--json` is data, and rewriting an unrelated
/// error into a JSON envelope because of it would corrupt the human contract of
/// every other command.
const JSON_FLAG: &str = "--json";

/// The end-of-options separator.
///
/// Everything after it is a value by definition, so the argv scan for
/// [`JSON_FLAG`] stops there: `cflx client subscribe set alpha -- /cb --json`
/// asks the owner to run a callback whose second argument happens to be spelled
/// `--json`, and reading that as a request for machine output would let a
/// *value* select the output contract — the exact inference the whole-argument
/// rule below exists to forbid.
const END_OF_OPTIONS: &str = "--";

/// Map a client subcommand name onto the operation an envelope reports.
///
/// `subscribe` is a group rather than an operation, so it is resolved by
/// [`subscribe_operation_of`] against the nested subcommand instead.
fn operation_of(subcommand: &str) -> Option<Operation> {
    match subcommand {
        "status" => Some(Operation::Status),
        "mark" => Some(Operation::ControlMark),
        "unmark" => Some(Operation::ControlUnmark),
        "start" => Some(Operation::ControlStart),
        "stop" => Some(Operation::ControlStop),
        "force-stop" => Some(Operation::ControlForceStop),
        "force-stop-change" => Some(Operation::ControlForceStopChange),
        "wait" => Some(Operation::Wait),
        // `mcp` is a server, not an operation: a usage failure there has no
        // envelope to belong to, so it keeps Clap's human behavior.
        _ => None,
    }
}

/// Map a `subscribe` subcommand name onto the operation an envelope reports.
///
/// A `subscribe` invocation that named no operation — `cflx client subscribe
/// --json` — reports [`Operation::SubscribeGet`]: of the three it is the only
/// one that neither registers nor removes anything, so a caller branching on
/// `operation` cannot read a refused invocation as an attempted registration.
fn subscribe_operation_of(subcommand: Option<&str>) -> Operation {
    match subcommand {
        Some("set") => Operation::SubscribeSet,
        Some("clear") => Operation::SubscribeClear,
        _ => Operation::SubscribeGet,
    }
}

/// Whether this argv selected `cflx client` in JSON mode, and which operation.
///
/// Clap's own parse decides the namespace, so an option *value* that happens to
/// read `client` cannot fake one. The `--json` flag is looked for in argv rather
/// than in the parsed matches on purpose: parsing stops at the first rejected
/// argument, so `cflx client mark ../escape --json` never records the flag
/// even though the caller plainly asked for machine output.
///
/// When the namespace is selected but no operation is named — `cflx client
/// --json` — the envelope still has to exist, and it reports [`Operation::Status`]:
/// it is the only operation that neither mutates nor waits, so a caller
/// branching on `operation` cannot read a refused invocation as an attempted
/// admission. The nested `subscribe` group answers the same way through
/// [`subscribe_operation_of`]. `outcome` is `usage_error` either way, and `message`
/// carries Clap's own statement of what was wrong.
pub fn json_usage_operation(argv: &[OsString]) -> Option<Operation> {
    let selects_json = argv
        .iter()
        .skip(1)
        .take_while(|arg| *arg != END_OF_OPTIONS)
        .any(|arg| arg == JSON_FLAG);
    if !selects_json {
        return None;
    }
    let matches = crate::cli::Cli::command()
        .ignore_errors(true)
        .try_get_matches_from(argv)
        .ok()?;
    let client = matches.subcommand_matches("client")?;
    if let Some(subscribe) = client.subcommand_matches("subscribe") {
        return Some(subscribe_operation_of(subscribe.subcommand_name()));
    }
    Some(
        client
            .subcommand_name()
            .and_then(operation_of)
            .unwrap_or(Operation::Status),
    )
}

/// The one-line problem statement from a Clap error.
///
/// Clap renders a problem line, a usage block, and a help hint. Only the problem
/// belongs in an envelope: the rest is human help, and a caller reading
/// `message` wants the reason, not a rendering of the CLI.
fn summarize_parse_error(error: &clap::Error) -> String {
    error
        .render()
        .to_string()
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.strip_prefix("error: ").unwrap_or(line).to_string())
        .unwrap_or_else(|| "the invocation could not be parsed".to_string())
}

/// Build the usage-failure envelope for a rejected client invocation.
pub fn usage_error_envelope(error: &clap::Error, operation: Operation) -> ResultEnvelope {
    ResultEnvelope::new(operation, Outcome::UsageError).with_message(summarize_parse_error(error))
}

/// Whether a Clap outcome is a usage *failure* rather than requested output.
///
/// `--help` and `--version` arrive as errors too, and both are successful
/// invocations that print what was asked for. Rewriting either into a
/// `usage_error` envelope would break the one thing a caller uses them for.
fn is_usage_failure(kind: clap::error::ErrorKind) -> bool {
    use clap::error::ErrorKind;
    !matches!(
        kind,
        ErrorKind::DisplayHelp
            | ErrorKind::DisplayVersion
            | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    )
}

/// Answer a Clap parse failure and terminate the process.
///
/// This is the entrypoint's replacement for `Cli::parse()`'s implicit exit, and
/// it runs before *anything* observable: no logging, no configuration, no
/// repository lock, no listener, no lifecycle work. A `cflx client ... --json`
/// invocation gets its promised single envelope on stdout and the outcome's own
/// exit status; every other invocation keeps Clap's existing human behavior
/// byte for byte.
pub fn exit_on_parse_error(error: clap::Error) -> ! {
    if is_usage_failure(error.kind()) {
        let argv: Vec<OsString> = std::env::args_os().collect();
        if let Some(operation) = json_usage_operation(&argv) {
            let envelope = usage_error_envelope(&error, operation);
            std::process::exit(emit(&envelope, OutputMode::Json));
        }
    }
    error.exit()
}

/// Run one `cflx client` invocation and return its exit status.
///
/// Returns rather than exits so the entrypoint owns process termination and a
/// test can drive the same function the binary does.
pub async fn run(args: ClientArgs) -> i32 {
    // The MCP server is not an operation with an envelope: it is a long-lived
    // protocol session whose *tools* produce envelopes. Answering it here keeps
    // the single-envelope contract of the other three exactly as it was.
    if matches!(args.command, ClientCommands::Mcp(_)) {
        return serve_mcp(args).await;
    }

    let (operation, mode, change_id) = match &args.command {
        ClientCommands::Status(status) => (
            Operation::Status,
            OutputMode::from_json_flag(status.json),
            None,
        ),
        // A single-target request names its target the way every other
        // operation does; a multi-target one cannot, and the per-target list in
        // `detail` is the answer instead.
        ClientCommands::Mark(mark) => (
            Operation::ControlMark,
            OutputMode::from_json_flag(mark.json),
            single(&mark.change_ids),
        ),
        ClientCommands::Unmark(unmark) => (
            Operation::ControlUnmark,
            OutputMode::from_json_flag(unmark.json),
            single(&unmark.change_ids),
        ),
        ClientCommands::Start(start) => (
            Operation::ControlStart,
            OutputMode::from_json_flag(start.json),
            None,
        ),
        ClientCommands::Stop(stop) => (
            Operation::ControlStop,
            OutputMode::from_json_flag(stop.json),
            None,
        ),
        ClientCommands::ForceStop(force) => (
            Operation::ControlForceStop,
            OutputMode::from_json_flag(force.json),
            None,
        ),
        ClientCommands::ForceStopChange(force) => (
            Operation::ControlForceStopChange,
            OutputMode::from_json_flag(force.json),
            Some(force.change_id.clone()),
        ),
        ClientCommands::Wait(wait) => (
            Operation::Wait,
            OutputMode::from_json_flag(wait.json),
            Some(wait.change_id.clone()),
        ),
        ClientCommands::Subscribe(subscribe) => match &subscribe.command {
            ClientSubscribeCommands::Set(set) => (
                Operation::SubscribeSet,
                OutputMode::from_json_flag(set.json),
                single(&set.change_ids),
            ),
            ClientSubscribeCommands::Get(get) => (
                Operation::SubscribeGet,
                OutputMode::from_json_flag(get.json),
                single(&get.change_ids),
            ),
            ClientSubscribeCommands::Clear(clear) => (
                Operation::SubscribeClear,
                OutputMode::from_json_flag(clear.json),
                single(&clear.change_ids),
            ),
        },
        // Answered above; reaching here would mean the guard was removed.
        ClientCommands::Mcp(_) => unreachable!("the MCP session is served before this point"),
    };

    let envelope = execute(args, operation).await;
    let envelope = match change_id {
        Some(change_id) if envelope.change_id.is_none() => envelope.with_change(change_id),
        _ => envelope,
    };
    emit(&envelope, mode)
}

/// The feature-disabled refusal.
///
/// It runs before anything observable: no repository lock, no socket, no log,
/// no workspace write. A build that cannot speak the local API has nothing
/// useful to attempt, and attempting anyway would leave state behind for an
/// operation that was never going to work.
#[cfg(not(feature = "web-monitoring"))]
async fn execute(_args: ClientArgs, operation: Operation) -> ResultEnvelope {
    ResultEnvelope::new(operation, Outcome::FeatureUnavailable).with_message(
        "this build has no local /api/v2 support, so it cannot reach an existing owner. \
         Rebuild with `--features web-monitoring`",
    )
}

/// Serve one stdio MCP session, or refuse in a build that cannot reach an owner.
///
/// The refusal is written to stderr rather than to the protocol stream: a host
/// parsing stdout as JSON-RPC must not be handed prose, and a server that cannot
/// serve has no frame it could honestly send.
#[cfg(feature = "web-monitoring")]
async fn serve_mcp(args: ClientArgs) -> i32 {
    // Clap already refused the two-selector conflict, so this cannot fail for
    // ambiguity; a relative `--project-dir` is still worth refusing before a
    // long-lived protocol session starts advertising tools it cannot route.
    let default_route = match RouteSelector::from_inputs(
        args.project_dir.as_deref(),
        args.unix_socket.as_deref(),
    ) {
        Ok(selector) => selector,
        Err(error) => {
            eprintln!("cflx client mcp: {error}");
            return Outcome::UsageError.exit_code();
        }
    };
    mcp::run(default_route, args.auth_token_env).await
}

#[cfg(not(feature = "web-monitoring"))]
async fn serve_mcp(_args: ClientArgs) -> i32 {
    eprintln!(
        "cflx client mcp: this build has no local /api/v2 support, so it cannot reach an \
         existing owner. Rebuild with `--features web-monitoring`"
    );
    Outcome::FeatureUnavailable.exit_code()
}

#[cfg(feature = "web-monitoring")]
async fn execute(args: ClientArgs, operation: Operation) -> ResultEnvelope {
    // One selector, built once, before anything is contacted. Clap enforces the
    // mutual exclusion at parse time, so reaching here with both is impossible;
    // the refusal below covers the remaining shape rules.
    let selector = match RouteSelector::from_inputs(
        args.project_dir.as_deref(),
        args.unix_socket.as_deref(),
    ) {
        Ok(selector) => selector,
        Err(error) => {
            return ResultEnvelope::new(operation, Outcome::UsageError).with_message(error.message)
        }
    };
    let connection =
        match session::Connection::resolve_route(&selector, args.auth_token_env.as_deref()) {
            Ok(connection) => connection,
            Err(refusal) => return refusal.into_envelope(operation),
        };

    match args.command {
        ClientCommands::Status(_) => session::status(&connection).await,
        // The same modules `cflx client mcp` calls, with the same arguments: a
        // command and a tool that disagreed about routing, transport, or typed
        // outcomes would be two contracts wearing one name.
        ClientCommands::Mark(mark) => {
            control::run(&connection, control::Action::Mark, &mark.change_ids).await
        }
        ClientCommands::Unmark(unmark) => {
            control::run(&connection, control::Action::Unmark, &unmark.change_ids).await
        }
        ClientCommands::Start(_) => control::run(&connection, control::Action::Start, &[]).await,
        ClientCommands::Stop(_) => control::run(&connection, control::Action::Stop, &[]).await,
        ClientCommands::ForceStop(_) => {
            control::run(&connection, control::Action::ForceStop, &[]).await
        }
        ClientCommands::ForceStopChange(force) => {
            control::run(
                &connection,
                control::Action::ForceStopChange,
                std::slice::from_ref(&force.change_id),
            )
            .await
        }
        ClientCommands::Wait(wait) => {
            wait::run(&connection, &wait.change_id, wait.timeout.deadline()).await
        }
        ClientCommands::Subscribe(args) => match args.command {
            ClientSubscribeCommands::Set(set) => {
                subscribe::run(
                    &connection,
                    &set.change_ids,
                    Some(&set.instance_id),
                    subscribe::Intent::Set {
                        command: set.command,
                        notify_blocked: set.blocked,
                    },
                )
                .await
            }
            ClientSubscribeCommands::Get(get) => {
                subscribe::run(
                    &connection,
                    &get.change_ids,
                    Some(&get.instance_id),
                    subscribe::Intent::Get,
                )
                .await
            }
            ClientSubscribeCommands::Clear(clear) => {
                subscribe::run(
                    &connection,
                    &clear.change_ids,
                    Some(&clear.instance_id),
                    subscribe::Intent::Clear,
                )
                .await
            }
        },
        ClientCommands::Mcp(_) => unreachable!("the MCP session is served before this point"),
    }
}

/// The one change an envelope may name, when a request addressed exactly one.
///
/// A multi-target request deliberately names none: `change_id` is a scalar, and
/// picking the first of several would tell a caller its request was about one
/// proposal when it was about five.
#[cfg(feature = "web-monitoring")]
fn single(change_ids: &[String]) -> Option<String> {
    match change_ids {
        [only] => Some(only.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<OsString> {
        std::iter::once("cflx")
            .chain(args.iter().copied())
            .map(OsString::from)
            .collect()
    }

    #[test]
    fn a_client_json_invocation_is_recognized_with_the_operation_it_named() {
        assert_eq!(
            json_usage_operation(&argv(&["client", "status", "--json"])),
            Some(Operation::Status)
        );
        // The rejected value stops Clap's parse before `--json`, which is
        // exactly the case the argv scan exists for.
        assert_eq!(
            json_usage_operation(&argv(&["client", "mark", "../escape", "--json"])),
            Some(Operation::ControlMark)
        );
        assert_eq!(
            json_usage_operation(&argv(&["client", "unmark", "../escape", "--json"])),
            Some(Operation::ControlUnmark)
        );
        // A lifecycle verb has no argument to reject, so an unknown flag is the
        // failure that still has to name its operation.
        for (verb, operation) in [
            ("start", Operation::ControlStart),
            ("stop", Operation::ControlStop),
            ("force-stop", Operation::ControlForceStop),
        ] {
            assert_eq!(
                json_usage_operation(&argv(&["client", verb, "--nope", "--json"])),
                Some(operation),
                "{verb}"
            );
        }
        assert_eq!(
            json_usage_operation(&argv(&[
                "client",
                "wait",
                "alpha",
                "--timeout",
                "abc",
                "--json"
            ])),
            Some(Operation::Wait)
        );
        // A missing required argument still names its operation.
        assert_eq!(
            json_usage_operation(&argv(&["client", "mark", "--json"])),
            Some(Operation::ControlMark)
        );
        // The nested group answers with the operation it named, and a group
        // that named none reports the one that neither registers nor removes.
        assert_eq!(
            json_usage_operation(&argv(&["client", "subscribe", "set", "--json"])),
            Some(Operation::SubscribeSet)
        );
        assert_eq!(
            json_usage_operation(&argv(&["client", "subscribe", "clear", "--json"])),
            Some(Operation::SubscribeClear)
        );
        assert_eq!(
            json_usage_operation(&argv(&["client", "subscribe", "--json"])),
            Some(Operation::SubscribeGet)
        );
        // The namespace with no operation still owes the caller an envelope.
        assert_eq!(
            json_usage_operation(&argv(&["client", "--json"])),
            Some(Operation::Status)
        );
    }

    #[test]
    fn human_and_non_client_invocations_keep_clap_to_themselves() {
        // No `--json` at all.
        assert_eq!(json_usage_operation(&argv(&["client", "status"])), None);
        assert_eq!(
            json_usage_operation(&argv(&["client", "mark", "../escape"])),
            None
        );
        // JSON, but not this namespace.
        assert_eq!(
            json_usage_operation(&argv(&["openspec", "show", "alpha", "--json"])),
            None
        );
        assert_eq!(json_usage_operation(&argv(&["--json"])), None);
    }

    #[test]
    fn json_intent_is_a_whole_argument_never_a_substring_of_a_value() {
        // Values that merely contain the spelling are data. Rewriting these into
        // JSON would be exactly the "infer intent from a substring" failure the
        // contract forbids.
        for value in ["--jsonish", "not--json", "x--json", "--json=true"] {
            assert_eq!(
                json_usage_operation(&argv(&["client", "mark", value])),
                None,
                "{value} must not select JSON mode"
            );
        }
    }

    #[test]
    fn help_and_version_are_answers_rather_than_usage_failures() {
        use clap::error::ErrorKind;
        assert!(!is_usage_failure(ErrorKind::DisplayHelp));
        assert!(!is_usage_failure(ErrorKind::DisplayVersion));
        assert!(!is_usage_failure(
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        ));
        for kind in [
            ErrorKind::InvalidValue,
            ErrorKind::UnknownArgument,
            ErrorKind::MissingRequiredArgument,
            ErrorKind::ValueValidation,
            ErrorKind::InvalidSubcommand,
        ] {
            assert!(is_usage_failure(kind), "{kind:?}");
        }
    }

    #[test]
    fn a_usage_envelope_is_one_line_and_carries_only_the_problem() {
        let error = <crate::cli::Cli as clap::Parser>::try_parse_from(argv(&[
            "client",
            "mark",
            "../escape",
            "--json",
        ]))
        .expect_err("an escaping change ID must be rejected");
        let envelope = usage_error_envelope(&error, Operation::ControlMark);
        assert_eq!(envelope.outcome, Outcome::UsageError);
        assert!(!envelope.ok);
        assert_eq!(envelope.exit_code(), 2);
        let message = envelope.message.clone().unwrap();
        assert!(!message.contains('\n'), "{message}");
        assert!(!message.starts_with("error: "), "{message}");
        assert!(!message.contains("Usage:"), "{message}");
        assert!(!envelope.to_json_line().contains('\n'));
    }

    // ------------------------------------------------------------------
    // Route selection
    //
    // Unit-scoped on purpose: every assertion below is about the *arguments*
    // a call carried, so none of it touches Git, a filesystem repository, a
    // socket, or an owner. That is the same boundary the contract draws —
    // these refusals must be provable before contact — so proving them here
    // without a repository is the honest test, not a convenience.
    // ------------------------------------------------------------------

    #[test]
    fn a_call_naming_no_selector_takes_the_default_route() {
        assert_eq!(
            RouteSelector::from_inputs(None, None).unwrap(),
            RouteSelector::Default
        );
    }

    #[test]
    fn a_project_directory_is_the_normal_selector_and_a_socket_the_override() {
        assert_eq!(
            RouteSelector::from_inputs(Some(Path::new("/srv/project-b")), None).unwrap(),
            RouteSelector::Project(PathBuf::from("/srv/project-b"))
        );
        assert_eq!(
            RouteSelector::from_inputs(None, Some(Path::new("/tmp/owner.sock"))).unwrap(),
            RouteSelector::Socket(PathBuf::from("/tmp/owner.sock"))
        );
    }

    #[test]
    fn two_selectors_in_one_call_are_refused_rather_than_silently_ranked() {
        // Preferring one would be the cross-project misroute the selector
        // exists to prevent: a registration meant for the project directory
        // would reach whatever owner the socket named.
        let error = RouteSelector::from_inputs(
            Some(Path::new("/srv/project-b")),
            Some(Path::new("/tmp/project-a.sock")),
        )
        .expect_err("two routes in one call are ambiguous");
        assert!(error.message.contains("project_dir"), "{error}");
        assert!(error.message.contains("unix_socket"), "{error}");
    }

    #[test]
    fn a_relative_or_empty_project_directory_is_refused() {
        // This process's working directory is the Hermes gateway's, or an MCP
        // host's — never the project's — so a relative path resolves against
        // the wrong tree by construction.
        for value in ["relative/project", "./project", "..", ""] {
            let error = RouteSelector::from_inputs(Some(Path::new(value)), None)
                .expect_err("{value} must be refused");
            assert!(
                error.message.contains("empty") || error.message.contains("relative"),
                "{value}: {error}"
            );
        }
        assert!(RouteSelector::from_inputs(None, Some(Path::new(""))).is_err());
    }

    #[test]
    fn a_call_scoped_selector_overrides_the_namespace_default_without_mutating_it() {
        let namespace = RouteSelector::Socket(PathBuf::from("/tmp/project-a.sock"));

        let call = RouteSelector::from_inputs(Some(Path::new("/srv/project-b")), None)
            .unwrap()
            .or_default(&namespace);
        assert_eq!(
            call,
            RouteSelector::Project(PathBuf::from("/srv/project-b"))
        );

        // The default is a shared value, not a cursor: the next call still sees
        // it, which is what keeps two concurrent calls from changing each
        // other's route.
        let plain = RouteSelector::from_inputs(None, None)
            .unwrap()
            .or_default(&namespace);
        assert_eq!(plain, namespace);
    }

    #[test]
    fn the_cli_refuses_both_route_options_through_the_existing_usage_contract() {
        let error = <crate::cli::Cli as clap::Parser>::try_parse_from(argv(&[
            "client",
            "--project-dir",
            "/srv/project-b",
            "--unix-socket",
            "/tmp/project-a.sock",
            "status",
        ]))
        .expect_err("two routes on one namespace are ambiguous");
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        // A usage error, not a new envelope outcome: nothing was contacted.
        let envelope = usage_error_envelope(&error, Operation::Status);
        assert_eq!(envelope.outcome, Outcome::UsageError);
        assert_eq!(envelope.exit_code(), 2);

        // Either one alone parses.
        for route in [
            ["--project-dir", "/srv/project-b"],
            ["--unix-socket", "/tmp/project-a.sock"],
        ] {
            <crate::cli::Cli as clap::Parser>::try_parse_from(argv(&[
                "client", route[0], route[1], "status",
            ]))
            .unwrap_or_else(|error| panic!("{route:?} must parse: {error}"));
        }
    }

    #[test]
    fn the_json_flag_selects_the_machine_contract() {
        assert_eq!(OutputMode::from_json_flag(true), OutputMode::Json);
        assert_eq!(OutputMode::from_json_flag(false), OutputMode::Human);
    }

    #[test]
    fn an_unsuccessful_envelope_reports_its_own_exit_status() {
        let envelope = ResultEnvelope::new(Operation::Wait, Outcome::Timeout);
        assert_eq!(envelope.exit_code(), Outcome::Timeout.exit_code());
        assert!(!envelope.ok);
    }
}
