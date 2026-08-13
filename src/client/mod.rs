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
//! So this is a thin, intent-shaped client: four commands, stable JSON, stable
//! exit statuses, and no protocol details in the public surface at all. The
//! fourth, `mcp`, is the same boundary spoken as Model Context Protocol, so an
//! agent gets the identical routing and the identical typed outcomes.
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
//! Admission is not completion. A settled command record proves the owner
//! accepted an intent, not that a change was implemented, accepted, archived, or
//! integrated. `wait` therefore certifies success from current repository
//! evidence for the owner's declared terminal mode, and treats owner
//! disappearance, owner restart, rejection, process failure, and timeout as
//! distinct unsuccessful outcomes rather than as completion.

pub mod envelope;

use std::ffi::OsString;

use clap::CommandFactory;

#[cfg(feature = "web-monitoring")]
pub mod completion;
#[cfg(feature = "web-monitoring")]
mod enqueue;
#[cfg(feature = "web-monitoring")]
pub mod mcp;
#[cfg(feature = "web-monitoring")]
mod notify;
#[cfg(feature = "web-monitoring")]
pub mod repo;
#[cfg(feature = "web-monitoring")]
mod session;
#[cfg(feature = "web-monitoring")]
mod transport;
#[cfg(feature = "web-monitoring")]
mod wait;

use envelope::{Operation, Outcome, ResultEnvelope};

use crate::cli::{ClientArgs, ClientCommands};

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

/// Map a client subcommand name onto the operation an envelope reports.
fn operation_of(subcommand: &str) -> Option<Operation> {
    match subcommand {
        "status" => Some(Operation::Status),
        "enqueue" => Some(Operation::Enqueue),
        "wait" => Some(Operation::Wait),
        // `mcp` is a server, not an operation: a usage failure there has no
        // envelope to belong to, so it keeps Clap's human behavior.
        _ => None,
    }
}

/// Whether this argv selected `cflx client` in JSON mode, and which operation.
///
/// Clap's own parse decides the namespace, so an option *value* that happens to
/// read `client` cannot fake one. The `--json` flag is looked for in argv rather
/// than in the parsed matches on purpose: parsing stops at the first rejected
/// argument, so `cflx client enqueue ../escape --json` never records the flag
/// even though the caller plainly asked for machine output.
///
/// When the namespace is selected but no operation is named — `cflx client
/// --json` — the envelope still has to exist, and it reports [`Operation::Status`]:
/// of the three, it is the only one that neither mutates nor waits, so a caller
/// branching on `operation` cannot read a refused invocation as an attempted
/// admission. `outcome` is `usage_error` either way, and `message` carries
/// Clap's own statement of what was wrong.
pub fn json_usage_operation(argv: &[OsString]) -> Option<Operation> {
    if !argv.iter().skip(1).any(|arg| arg == JSON_FLAG) {
        return None;
    }
    let matches = crate::cli::Cli::command()
        .ignore_errors(true)
        .try_get_matches_from(argv)
        .ok()?;
    let client = matches.subcommand_matches("client")?;
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
        ClientCommands::Enqueue(enqueue) => (
            Operation::Enqueue,
            OutputMode::from_json_flag(enqueue.json),
            Some(enqueue.change_id.clone()),
        ),
        ClientCommands::Wait(wait) => (
            Operation::Wait,
            OutputMode::from_json_flag(wait.json),
            Some(wait.change_id.clone()),
        ),
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
    mcp::run(args.unix_socket, args.auth_token_env).await
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
    let connection = match session::Connection::resolve(
        args.unix_socket.as_deref(),
        args.auth_token_env.as_deref(),
    ) {
        Ok(connection) => connection,
        Err(refusal) => return refusal.into_envelope(operation),
    };

    match args.command {
        ClientCommands::Status(_) => session::status(&connection).await,
        ClientCommands::Enqueue(enqueue) => enqueue::run(&connection, &enqueue.change_id).await,
        ClientCommands::Wait(wait) => wait::run(&connection, &wait.change_id, wait.timeout).await,
        ClientCommands::Mcp(_) => unreachable!("the MCP session is served before this point"),
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
            json_usage_operation(&argv(&["client", "enqueue", "../escape", "--json"])),
            Some(Operation::Enqueue)
        );
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
            json_usage_operation(&argv(&["client", "enqueue", "--json"])),
            Some(Operation::Enqueue)
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
            json_usage_operation(&argv(&["client", "enqueue", "../escape"])),
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
                json_usage_operation(&argv(&["client", "enqueue", value])),
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
            "enqueue",
            "../escape",
            "--json",
        ]))
        .expect_err("an escaping change ID must be rejected");
        let envelope = usage_error_envelope(&error, Operation::Enqueue);
        assert_eq!(envelope.outcome, Outcome::UsageError);
        assert!(!envelope.ok);
        assert_eq!(envelope.exit_code(), 2);
        let message = envelope.message.clone().unwrap();
        assert!(!message.contains('\n'), "{message}");
        assert!(!message.starts_with("error: "), "{message}");
        assert!(!message.contains("Usage:"), "{message}");
        assert!(!envelope.to_json_line().contains('\n'));
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
