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

/// Run one read-only `git` command, bounded by `deadline` when there is one.
///
/// `Ok(GitOutcome::DeadlineExpired)` means the child was killed and waited for,
/// so no `git` process outlives this call. `Err` is reserved for a `git` that
/// could not be spawned or whose pipes could not be read — the same failure the
/// unbounded `output()` reports.
pub async fn run_git(
    repo_root: &Path,
    args: &[&str],
    deadline: Option<Instant>,
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
        let outcome = run_git(tmp.path(), &["--version"], None).await.unwrap();
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
        let outcome = run_git(tmp.path(), &["--version"], Some(deadline))
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
        let outcome = run_git(tmp.path(), &["--version"], Some(deadline))
            .await
            .unwrap();
        assert!(matches!(outcome, GitOutcome::Finished(_)), "{outcome:?}");
    }

    #[tokio::test]
    async fn a_missing_git_is_an_error_rather_than_a_silent_expiry() {
        let tmp = tempfile::tempdir().unwrap();
        // A path that cannot be a working directory fails at spawn, which is the
        // same class of failure a missing `git` produces.
        let outcome = run_git(&tmp.path().join("absent"), &["--version"], None).await;
        assert!(outcome.is_err());
    }
}
