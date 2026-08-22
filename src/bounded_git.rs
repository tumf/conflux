//! Read-only Git subprocesses that a caller's deadline actually owns.
//!
//! `tokio::process::Command::output()` under a `timeout` looks bounded and is
//! not: cancelling the future drops the handle, and the `git` process keeps
//! running with nobody waiting on it. For `cflx client wait` that is the whole
//! bug — a `git ls-remote` against an unreachable remote outlives the operation
//! that asked for it, so the caller's `--timeout` bounds only the *reply*, not
//! the work.
//!
//! So the deadline is passed down to the spawn site instead of wrapped around
//! it. On expiry the child is signalled and then reaped here, before the caller
//! is told the deadline passed.
//!
//! Every command routed through this module is read-only by construction of its
//! caller; nothing here fetches, writes a ref, or touches a working tree.

use std::path::Path;
use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::Instant;

/// What became of one bounded Git invocation.
#[derive(Debug)]
pub enum GitOutcome {
    /// The command ran to completion.
    Finished(Output),
    /// The deadline passed first; the child was terminated and reaped.
    DeadlineExpired,
}

/// How a caller bounds the Git children it spawns.
///
/// The three variants exist because "how long may this child run" and "how long
/// may the caller's operation run" are different questions, and only one of them
/// has a `timeout` outcome attached to it.
#[derive(Debug, Clone, Copy)]
pub enum GitDeadline {
    /// No bound at all, for callers that supervise the child some other way.
    Unbounded,
    /// One shared instant for every child: the caller's whole-operation deadline.
    ///
    /// Expiry is the *operation's* answer, so a caller holding one of these
    /// reports its own timeout rather than retrying.
    Operation(Instant),
    /// A fresh finite budget for each child, for a caller with no operation
    /// deadline.
    ///
    /// "Wait as long as it takes" is a promise about the operation. Turning it
    /// into an unkillable `git ls-remote` against an unreachable remote would be
    /// a different promise entirely, so each child still gets its own bound —
    /// and its expiry means only that this attempt gave up, never that the
    /// operation did.
    PerChild(Duration),
}

impl GitDeadline {
    /// The instant the next child must not outlive.
    fn next(self) -> Option<Instant> {
        match self {
            Self::Unbounded => None,
            Self::Operation(at) => Some(at),
            Self::PerChild(budget) => Some(Instant::now() + budget),
        }
    }

    /// Whether an expiry means the caller's whole operation ran out of time.
    ///
    /// This is the distinction that keeps the `timeout` outcome reserved for an
    /// explicitly configured deadline: a per-child expiry killed one `git`, and
    /// proves nothing about how long the caller has been waiting.
    pub fn is_operation_deadline(self) -> bool {
        matches!(self, Self::Operation(_))
    }
}

/// Run one read-only `git` command under `deadline`.
///
/// `Ok(GitOutcome::DeadlineExpired)` means the child was killed and waited for,
/// so no `git` process outlives this call. `Err` is reserved for a `git` that
/// could not be spawned or whose pipes could not be read — the same failure the
/// unbounded `output()` reports.
pub async fn run_git(
    repo_root: &Path,
    args: &[&str],
    deadline: GitDeadline,
) -> std::io::Result<GitOutcome> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Belt and braces: an unwind or an early return between spawn and reap must
    // not leave the child behind either.
    command.kill_on_drop(true);

    // Resolved before the spawn so a per-child budget measures the child's own
    // life rather than whatever the spawn itself cost.
    let deadline = deadline.next();

    let mut child = command.spawn()?;

    let Some(deadline) = deadline else {
        return child.wait_with_output().await.map(GitOutcome::Finished);
    };

    // The pipes are drained concurrently with the wait, exactly as
    // `wait_with_output` does, because a child that fills a pipe buffer while
    // nobody reads it would deadlock instead of finishing.
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    // Bound in its own statement so the timeout future — which holds the only
    // mutable borrow of `child` — is dropped before the expiry branch reaps it.
    let completed = {
        let collect = async {
            let read_out = async {
                match stdout_pipe.as_mut() {
                    Some(pipe) => pipe.read_to_end(&mut stdout).await.map(|_| ()),
                    None => Ok(()),
                }
            };
            let read_err = async {
                match stderr_pipe.as_mut() {
                    Some(pipe) => pipe.read_to_end(&mut stderr).await.map(|_| ()),
                    None => Ok(()),
                }
            };
            let (out, err) = tokio::join!(read_out, read_err);
            out?;
            err?;
            child.wait().await
        };
        tokio::time::timeout_at(deadline, collect).await
    };

    match completed {
        Ok(status) => Ok(GitOutcome::Finished(Output {
            status: status?,
            stdout,
            stderr,
        })),
        Err(_elapsed) => {
            // Signal, then wait: `start_kill` only asks, and a child that is
            // never waited for stays a zombie owned by this process.
            let _ = child.start_kill();
            let _ = child.wait().await;
            Ok(GitOutcome::DeadlineExpired)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn a_completed_command_reports_its_own_streams_and_status() {
        let tmp = tempfile::tempdir().unwrap();
        let outcome = run_git(tmp.path(), &["--version"], GitDeadline::Unbounded)
            .await
            .unwrap();
        let GitOutcome::Finished(output) = outcome else {
            panic!("`git --version` must finish");
        };
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("git version"));
    }

    #[tokio::test]
    async fn a_deadline_that_already_passed_reports_expiry_rather_than_output() {
        let tmp = tempfile::tempdir().unwrap();
        // Already elapsed, so the outcome is decided by the deadline and not by
        // how fast this machine runs `git`.
        let deadline = Instant::now() - Duration::from_secs(1);
        let outcome = run_git(tmp.path(), &["--version"], GitDeadline::Operation(deadline))
            .await
            .unwrap();
        assert!(
            matches!(outcome, GitOutcome::DeadlineExpired),
            "{outcome:?}"
        );
    }

    #[tokio::test]
    async fn a_generous_deadline_does_not_disturb_a_fast_command() {
        let tmp = tempfile::tempdir().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let outcome = run_git(tmp.path(), &["--version"], GitDeadline::Operation(deadline))
            .await
            .unwrap();
        assert!(matches!(outcome, GitOutcome::Finished(_)), "{outcome:?}");
    }

    #[tokio::test]
    async fn a_missing_git_is_an_error_rather_than_a_silent_expiry() {
        let tmp = tempfile::tempdir().unwrap();
        // A path that cannot be a working directory fails at spawn, which is the
        // same class of failure a missing `git` produces.
        let outcome = run_git(
            &tmp.path().join("absent"),
            &["--version"],
            GitDeadline::Unbounded,
        )
        .await;
        assert!(outcome.is_err());
    }

    #[tokio::test]
    async fn a_per_child_budget_bounds_each_invocation_from_its_own_start() {
        let tmp = tempfile::tempdir().unwrap();
        // Each call gets the full budget rather than sharing one instant, so a
        // second invocation cannot inherit an already-elapsed deadline from the
        // first the way `Operation` deliberately does.
        for _ in 0..2 {
            let outcome = run_git(
                tmp.path(),
                &["--version"],
                GitDeadline::PerChild(Duration::from_secs(30)),
            )
            .await
            .unwrap();
            assert!(matches!(outcome, GitOutcome::Finished(_)), "{outcome:?}");
        }
    }

    #[tokio::test]
    async fn a_per_child_budget_terminates_and_reaps_a_child_that_never_finishes() {
        // A `git://` endpoint that accepts and then says nothing, so `ls-remote`
        // blocks on the ref advertisement instead of on anything this test
        // controls. The child is only reachable through its own budget.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (connected_tx, connected_rx) = tokio::sync::oneshot::channel();
        let accepting = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("git must connect");
            let _ = connected_tx.send(());
            // Held, not dropped: closing it would end the child for the wrong
            // reason.
            std::future::pending::<()>().await;
            drop(stream);
        });

        let tmp = tempfile::tempdir().unwrap();
        let url = format!("git://127.0.0.1:{port}/stalled.git");
        let outcome = run_git(
            tmp.path(),
            &["ls-remote", &url],
            GitDeadline::PerChild(Duration::from_millis(200)),
        )
        .await
        .unwrap();

        assert!(
            matches!(outcome, GitOutcome::DeadlineExpired),
            "a per-child budget must expire a child that never finishes: {outcome:?}"
        );
        // `run_git` returning at all proves the reap happened: it waits on the
        // child after signalling it, so a surviving `git` would still be blocking
        // this call.
        connected_rx
            .await
            .expect("git must have reached the stalled endpoint");
        accepting.abort();
    }

    #[test]
    fn only_a_shared_operation_deadline_claims_the_operation_ran_out_of_time() {
        assert!(GitDeadline::Operation(Instant::now()).is_operation_deadline());
        assert!(!GitDeadline::PerChild(Duration::from_secs(1)).is_operation_deadline());
        assert!(!GitDeadline::Unbounded.is_operation_deadline());
    }
}
