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
//! So this is a thin, intent-shaped client: three commands, stable JSON, stable
//! exit statuses, and no protocol details in the public surface at all.
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

#[cfg(feature = "web-monitoring")]
mod enqueue;
#[cfg(feature = "web-monitoring")]
mod repo;
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

/// Run one `cflx client` invocation and return its exit status.
///
/// Returns rather than exits so the entrypoint owns process termination and a
/// test can drive the same function the binary does.
pub async fn run(args: ClientArgs) -> i32 {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
