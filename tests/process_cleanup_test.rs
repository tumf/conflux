#![cfg(feature = "heavy-tests")]

//! Integration tests for process cleanup functionality
//!
//! Tests verify that child processes are properly cleaned up across platforms:
//! - Unix: Process group cleanup via setpgid/killpg
//! - Windows: Job object automatic termination

use std::time::Duration;
use tokio::process::Command;

#[cfg(unix)]
#[tokio::test]
async fn test_unix_process_group_cleanup() {
    use std::process::Stdio;

    // Spawn a shell command that creates child processes
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("sleep 30 & sleep 30 & wait")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());

    // Configure process group (simulating what agent.rs does)
    unsafe {
        cmd.pre_exec(|| {
            use nix::unistd::{setpgid, Pid};
            setpgid(Pid::from_raw(0), Pid::from_raw(0)).map_err(std::io::Error::other)?;
            Ok(())
        });
    }

    let mut child = cmd.spawn().expect("Failed to spawn test process");
    let pid = child.id().expect("Failed to get PID");
    let pid_string = pid.to_string();

    // Give processes time to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify process group exists
    let check_output = std::process::Command::new("ps")
        .args(["-o", "pid,pgid", "-p", &pid_string])
        .output()
        .expect("Failed to check process group");

    assert!(
        check_output.status.success(),
        "Process should be running before termination"
    );

    // Terminate the process group
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::Pid;
    killpg(Pid::from_raw(pid as i32), Signal::SIGTERM).expect("Failed to kill process group");

    // Wait for the child to actually terminate
    let _ = child.wait().await;

    // Wait for cleanup with retries
    let mut terminated = false;
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let check_output = std::process::Command::new("ps")
            .args(["-p", &pid_string])
            .output()
            .expect("Failed to check process");

        if !check_output.status.success() {
            terminated = true;
            break;
        }
    }

    assert!(terminated, "Process should be terminated after killpg");
}

#[cfg(windows)]
#[tokio::test]
async fn test_windows_job_object_cleanup() {
    // TODO: Implement Windows-specific test for job object cleanup
    // This requires creating a job object, spawning a process, and verifying
    // that the process terminates when the job handle is closed
    println!("Windows job object test not yet implemented");
}

/// Apply-completion cleanup: a descendant that keeps the managed worktree
/// `index.lock` after the process-group leader exits must gate Git finalization.
///
/// The leader is terminated on its own so the "leader exited, descendant alive"
/// state is deterministic rather than racy.
#[cfg(unix)]
mod apply_completion_cleanup {
    use conflux::process_manager::{
        cleanup_process_group_verified, configure_process_group, ManagedChild,
        ProcessGroupQuiescence,
    };
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::process::Command;

    fn git(workspace: &Path, args: &[&str]) -> std::process::Output {
        std::process::Command::new("git")
            .args(args)
            .current_dir(workspace)
            .output()
            .expect("git should run")
    }

    /// Creates a managed worktree with one commit, mirroring what Apply finalizes.
    fn init_worktree(workspace: &Path) {
        git(workspace, &["init"]);
        git(workspace, &["config", "user.email", "test@example.com"]);
        git(workspace, &["config", "user.name", "Test User"]);
        std::fs::write(workspace.join("README.md"), "initial\n").unwrap();
        git(workspace, &["add", "README.md"]);
        let out = git(workspace, &["commit", "-m", "initial"]);
        assert!(out.status.success(), "initial commit must succeed");
    }

    fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).expect("script should be written");
        path
    }

    fn group_has_members(pgid: u32) -> bool {
        unsafe { libc::killpg(pgid as i32, 0) == 0 }
    }

    /// Spawns a leader in its own process group that starts `descendant_script`
    /// and then sleeps. Returns the managed leader and its pgid.
    async fn spawn_leader(dir: &Path, descendant_script: &Path) -> (ManagedChild, u32) {
        let leader_script = write_script(
            dir,
            "leader.sh",
            &format!(
                "#!/bin/sh\n\
                 sh {descendant} >/dev/null 2>&1 </dev/null &\n\
                 sleep 120\n",
                descendant = descendant_script.display()
            ),
        );

        let mut cmd = Command::new("sh");
        cmd.arg(leader_script.as_os_str())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_process_group(&mut cmd);
        let child = cmd.spawn().expect("leader should spawn");
        let managed = ManagedChild::new(child).expect("managed leader");
        let pgid = managed.id().expect("leader pid");
        (managed, pgid)
    }

    /// Waits until `path` exists (bounded), so tests never assert on a
    /// descendant that has not started yet.
    async fn wait_for(path: &Path) {
        for _ in 0..100 {
            if path.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("expected {} to appear", path.display());
    }

    /// Leader exit alone is not quiescence: the descendant still holds
    /// `index.lock`, Git finalization would fail, and only confirmed cleanup
    /// makes the worktree safe to commit.
    #[tokio::test]
    async fn apply_completion_cleanup_waits_for_descendant_holding_index_lock() {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path().join("worktree");
        std::fs::create_dir_all(&workspace).unwrap();
        init_worktree(&workspace);

        let index_lock = workspace.join(".git").join("index.lock");
        let released = temp.path().join("released");
        let holder = write_script(
            temp.path(),
            "holder.sh",
            &format!(
                "#!/bin/sh\n\
                 release() {{ rm -f {lock}; echo released > {released}; exit 0; }}\n\
                 trap release TERM\n\
                 : > {lock}\n\
                 while :; do sleep 0.2; done\n",
                lock = index_lock.display(),
                released = released.display()
            ),
        );

        let (mut leader, pgid) = spawn_leader(temp.path(), &holder).await;
        wait_for(&index_lock).await;

        // Terminate and reap only the leader — exactly the state the Apply loop
        // used to treat as "the command is done".
        let leader_pid = leader.id().expect("leader pid") as i32;
        unsafe { libc::kill(leader_pid, libc::SIGTERM) };
        let _ = tokio::time::timeout(Duration::from_secs(5), leader.wait())
            .await
            .expect("leader should exit");

        assert!(
            index_lock.exists(),
            "the descendant must still hold index.lock after leader exit"
        );
        assert!(
            group_has_members(pgid),
            "leader exit must not be mistaken for process-group quiescence"
        );
        let blocked = git(&workspace, &["commit", "--allow-empty", "-m", "would race"]);
        assert!(
            !blocked.status.success(),
            "Git finalization must be impossible while the descendant holds index.lock"
        );

        let report = cleanup_process_group_verified(pgid, 2_000, 10_000, Some("apply"), None).await;

        assert_eq!(
            report.quiescence(),
            ProcessGroupQuiescence::Confirmed,
            "cleanup must confirm quiescence: {}",
            report.diagnostics()
        );
        assert!(
            released.exists() && !index_lock.exists(),
            "the descendant must have released the lock itself before quiescence was confirmed"
        );
        assert!(
            !group_has_members(pgid),
            "confirmed quiescence must mean no owned members remain"
        );

        let finalized = git(
            &workspace,
            &["commit", "--allow-empty", "-m", "Apply: change"],
        );
        assert!(
            finalized.status.success(),
            "Git finalization must succeed after confirmed cleanup: {}",
            String::from_utf8_lossy(&finalized.stderr)
        );
    }

    /// A group that cannot be proven quiescent inside the budget is reported as
    /// unconfirmed, and the worktree is left untouched for the retry.
    #[tokio::test]
    async fn apply_completion_cleanup_reports_unconfirmed_when_budget_exhausted() {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path().join("worktree");
        std::fs::create_dir_all(&workspace).unwrap();
        init_worktree(&workspace);

        let index_lock = workspace.join(".git").join("index.lock");
        let holder = write_script(
            temp.path(),
            "holder.sh",
            &format!(
                "#!/bin/sh\n\
                 trap '' TERM\n\
                 : > {lock}\n\
                 while :; do sleep 0.2; done\n",
                lock = index_lock.display()
            ),
        );

        let (mut leader, pgid) = spawn_leader(temp.path(), &holder).await;
        wait_for(&index_lock).await;

        let leader_pid = leader.id().expect("leader pid") as i32;
        unsafe { libc::kill(leader_pid, libc::SIGTERM) };
        let _ = tokio::time::timeout(Duration::from_secs(5), leader.wait())
            .await
            .expect("leader should exit");

        // Zero budget: quiescence cannot be proven.
        let report = cleanup_process_group_verified(pgid, 2_000, 0, Some("apply"), None).await;

        let members_after = group_has_members(pgid);
        let lock_after = index_lock.exists();
        unsafe { libc::killpg(pgid as i32, libc::SIGKILL) };

        assert!(
            !report.is_confirmed(),
            "an unswept group must never be reported as quiescent: {}",
            report.diagnostics()
        );
        assert_eq!(report.quiescence(), ProcessGroupQuiescence::MembersRemain);
        assert!(
            report.diagnostics().contains("cleanup budget expired"),
            "diagnostics must be actionable: {}",
            report.diagnostics()
        );
        assert!(members_after, "the survivor must still be observable");
        assert!(
            lock_after,
            "an unconfirmed cleanup must leave the worktree untouched, including its lock file"
        );
    }

    /// A descendant that ignores SIGTERM still reaches confirmed quiescence
    /// through the bounded force-kill escalation.
    #[tokio::test]
    async fn apply_completion_cleanup_confirms_after_forced_termination() {
        let temp = TempDir::new().unwrap();
        let started = temp.path().join("started");
        let holder = write_script(
            temp.path(),
            "holder.sh",
            &format!(
                "#!/bin/sh\n\
                 trap '' TERM\n\
                 : > {started}\n\
                 while :; do sleep 0.2; done\n",
                started = started.display()
            ),
        );

        let (mut leader, pgid) = spawn_leader(temp.path(), &holder).await;
        wait_for(&started).await;

        let leader_pid = leader.id().expect("leader pid") as i32;
        unsafe { libc::kill(leader_pid, libc::SIGTERM) };
        let _ = tokio::time::timeout(Duration::from_secs(5), leader.wait())
            .await
            .expect("leader should exit");

        let report = cleanup_process_group_verified(pgid, 200, 10_000, Some("apply"), None).await;

        assert_eq!(
            report.quiescence(),
            ProcessGroupQuiescence::Confirmed,
            "force-kill escalation must reach quiescence: {}",
            report.diagnostics()
        );
        assert!(
            report.force_killed(),
            "a SIGTERM-immune descendant must be recorded as force killed"
        );
        assert!(!group_has_members(pgid));
    }
}

/// The absolute per-invocation runtime limit, proven against real process groups.
///
/// The unit coverage in `src/` proves the deadline is computed from child spawn
/// and that a disabled limit never resolves. These prove the half that only a
/// real process can show: that a command which never stops *talking* is still
/// stopped, that its owned group is actually empty afterwards, and that the
/// reason it ended is reported as a boundary decision rather than as a crash the
/// run could retry.
///
/// Every assertion path force-kills the group first, so a failure cannot leak a
/// `sleep` into the test runner's process table.
#[cfg(unix)]
mod absolute_runtime_limit {
    use conflux::ai_command_runner::{AiCommandRunner, OutputLine, RunCommandScope};
    use conflux::config::OrchestratorConfig;
    use conflux::process_manager::CommandTermination;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    /// A command that never stops emitting output and never exits.
    ///
    /// Output is what an inactivity timeout watches, so a group shaped like this
    /// is invisible to every existing limit: only an absolute deadline can end it.
    const CHATTY_FOREVER: &str = "while :; do echo tick; sleep 0.05; done";

    /// The same, plus a descendant that ignores SIGTERM.
    ///
    /// Graceful termination alone cannot clear this group, so it distinguishes
    /// "we signalled" from "we verified".
    const CHATTY_FOREVER_SIGTERM_IMMUNE: &str =
        "sh -c 'trap \"\" TERM; while :; do sleep 0.2; done' >/dev/null 2>&1 </dev/null & \
         while :; do echo tick; sleep 0.05; done";

    /// Generous safety bound. Nothing asserts *when* the limit fires — only that
    /// it does — so this exists purely to fail a hang instead of hanging CI.
    const SAFETY: Duration = Duration::from_secs(60);

    fn group_has_members(pgid: i32) -> bool {
        unsafe { libc::killpg(pgid, 0) == 0 }
    }

    fn reap_and_report_survival(pgid: i32) -> bool {
        let survived = group_has_members(pgid);
        if survived {
            unsafe { libc::killpg(pgid, libc::SIGKILL) };
        }
        survived
    }

    /// A run-owned runner with the absolute limit set and every other lifecycle
    /// limit disabled, so a terminated command is unambiguous evidence.
    fn runner_with_limit(scope: &RunCommandScope, max_runtime_secs: u64) -> AiCommandRunner {
        let config = OrchestratorConfig {
            command_queue_stagger_delay_ms: Some(0),
            command_queue_max_retries: Some(1),
            command_queue_retry_delay_ms: Some(0),
            // Disabled: only the absolute deadline may end these commands.
            command_inactivity_timeout_secs: Some(0),
            command_inactivity_timeout_max_retries: Some(0),
            command_max_runtime_secs: Some(max_runtime_secs),
            ..OrchestratorConfig::default()
        };
        AiCommandRunner::for_run(&config, Arc::new(Mutex::new(None)), scope.clone())
    }

    /// Wait until the leader PID is published, so the PGID is recorded before
    /// anything can terminate it.
    async fn wait_for_pgid(handle: &conflux::process_manager::StreamingChildHandle) -> i32 {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(pid) = handle.id() {
                return pid as i32;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the command never reported a real pid"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Continuous output cannot postpone the absolute deadline, and the owned
    /// group is empty once it fires.
    #[tokio::test]
    async fn continuous_output_does_not_extend_the_absolute_deadline() {
        let scope = RunCommandScope::new();
        let runner = runner_with_limit(&scope, 1);

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(CHATTY_FOREVER, None, Some("apply"), Some("change-a"))
            .await
            .expect("an open scope admits the command");
        let pgid = wait_for_pgid(&handle).await;

        // Drain so the runner task is never blocked on a full output channel:
        // a command that cannot write is a command that stopped talking, which
        // is the one thing this test must not accidentally arrange.
        let mut lines = 0usize;
        let drain = tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                if matches!(line, OutputLine::Stdout(_)) {
                    lines += 1;
                }
            }
            lines
        });

        let status = tokio::time::timeout(SAFETY, handle.wait())
            .await
            .expect("the absolute limit must end a command that never stops emitting output")
            .expect("the runner publishes a final status");
        let termination = handle.termination().await;
        let cleanup = handle.process_group_cleanup().await;
        let emitted = drain.await.expect("the drain task joins");

        let survived = reap_and_report_survival(pgid);
        assert!(
            emitted > 0,
            "arrangement failed: the command must have been emitting output"
        );
        assert!(
            !status.success(),
            "a command stopped by its runtime limit is not a success"
        );
        assert_eq!(
            termination,
            CommandTermination::RuntimeLimit,
            "the reason must be the runtime limit, not an ordinary exit"
        );
        assert!(
            !termination.permits_retry(),
            "runtime-limit termination closes retry admission for the invocation"
        );
        assert!(
            cleanup.is_confirmed(),
            "the owned group must be proven quiescent: {}",
            cleanup.diagnostics()
        );
        assert!(!survived, "process group {pgid} survived its runtime limit");
    }

    /// Acceptance carries its own deadline, and continuous output cannot
    /// postpone that one either.
    ///
    /// The common budget is disabled here, so nothing but the dedicated
    /// Acceptance limit can end this command — which is exactly the case a
    /// legacy or malformed proposal produces. The bound comes from the same
    /// precedence rule the Acceptance call site uses, not from a hand-picked
    /// number.
    #[tokio::test]
    async fn acceptance_stays_bounded_when_the_common_limit_is_disabled() {
        use conflux::orchestration::acceptance::acceptance_runtime_limit_secs;

        let scope = RunCommandScope::new();
        // `0` = the shared command budget is disabled for every class.
        let runner = runner_with_limit(&scope, 0);
        assert_eq!(runner.queue_config().max_runtime_secs, 0);

        let effective = acceptance_runtime_limit_secs(1, 0);
        assert_eq!(
            effective, 1,
            "a disabled common limit leaves the dedicated Acceptance limit binding"
        );
        let acceptance = runner.with_max_runtime_secs(effective);

        let (mut handle, mut rx) = acceptance
            .execute_streaming_with_retry(
                CHATTY_FOREVER,
                None,
                Some("acceptance"),
                Some("change-a"),
            )
            .await
            .expect("an open scope admits the command");
        let pgid = wait_for_pgid(&handle).await;

        let mut lines = 0usize;
        let drain = tokio::spawn(async move {
            while let Some(line) = rx.recv().await {
                if matches!(line, OutputLine::Stdout(_)) {
                    lines += 1;
                }
            }
            lines
        });

        let status = tokio::time::timeout(SAFETY, handle.wait())
            .await
            .expect("the dedicated Acceptance limit must end a command that never stops emitting")
            .expect("the runner publishes a final status");
        let termination = handle.termination().await;
        let cleanup = handle.process_group_cleanup().await;
        let emitted = drain.await.expect("the drain task joins");

        let survived = reap_and_report_survival(pgid);
        assert!(
            emitted > 0,
            "arrangement failed: the command must have been emitting output"
        );
        assert!(!status.success());
        assert_eq!(
            termination,
            CommandTermination::RuntimeLimit,
            "Acceptance ends on its own absolute limit, not on an ordinary exit"
        );
        assert!(
            !termination.permits_retry(),
            "retry admission closes for a timed-out Acceptance invocation"
        );
        assert!(
            cleanup.is_confirmed(),
            "the owned group must be proven quiescent: {}",
            cleanup.diagnostics()
        );
        assert!(
            !survived,
            "process group {pgid} survived the Acceptance runtime limit"
        );

        // The runner every other command class uses is untouched: building the
        // Acceptance-bounded runner must not move Apply's budget.
        assert_eq!(
            runner.queue_config().max_runtime_secs,
            0,
            "the dedicated Acceptance limit is applied at the Acceptance call site only"
        );
    }

    /// `0` is an explicit disable: total elapsed runtime alone never ends the
    /// command, and cancellation stays independently effective.
    #[tokio::test]
    async fn zero_disables_the_absolute_deadline() {
        let scope = RunCommandScope::new();
        let runner = runner_with_limit(&scope, 0);

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(CHATTY_FOREVER, None, Some("apply"), Some("change-a"))
            .await
            .expect("an open scope admits the command");
        let pgid = wait_for_pgid(&handle).await;
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });

        // Long enough to be past any plausible accidental default, short enough
        // to stay a bounded test. Nothing here asserts a duration threshold: the
        // claim is that the command is *still alive*, which the group probe proves.
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            group_has_members(pgid),
            "a disabled deadline must not terminate the command"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), handle.wait())
                .await
                .is_err(),
            "a disabled deadline must leave the invocation running"
        );

        // Explicit cancellation remains enforceable while the deadline is off.
        handle.terminate().expect("cancellation is admissible");
        let _ = tokio::time::timeout(SAFETY, handle.wait())
            .await
            .expect("cancellation must end the command");
        let termination = handle.termination().await;
        drain.await.expect("the drain task joins");

        let survived = reap_and_report_survival(pgid);
        assert_eq!(
            termination,
            CommandTermination::Cancelled,
            "with the deadline disabled the command ends by cancellation, not by runtime limit"
        );
        assert!(!survived, "process group {pgid} survived cancellation");
    }

    /// Cleanup that cannot be proven is reported as unconfirmed, and the
    /// invocation still admits no retry.
    #[tokio::test]
    async fn runtime_limit_without_provable_cleanup_reports_diagnostics() {
        let scope = RunCommandScope::new();
        let mut runner = runner_with_limit(&scope, 1);
        // Zero verification budget: a SIGTERM-immune descendant can never be
        // proven gone inside it.
        runner.set_process_group_cleanup_timeout_ms(0);

        let (mut handle, mut rx) = runner
            .execute_streaming_with_retry(
                CHATTY_FOREVER_SIGTERM_IMMUNE,
                None,
                Some("apply"),
                Some("change-a"),
            )
            .await
            .expect("an open scope admits the command");
        let pgid = wait_for_pgid(&handle).await;
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });

        let _ = tokio::time::timeout(SAFETY, handle.wait())
            .await
            .expect("the absolute limit must still end the invocation");
        let termination = handle.termination().await;
        let cleanup = handle.process_group_cleanup().await;
        drain.await.expect("the drain task joins");

        reap_and_report_survival(pgid);
        assert_eq!(
            termination,
            CommandTermination::RuntimeLimit,
            "the reason stays the runtime limit even when cleanup is unprovable"
        );
        assert!(
            !termination.permits_retry(),
            "no retry may be admitted for an invocation stopped by its runtime limit"
        );
        assert!(
            !cleanup.is_confirmed(),
            "an unswept group must never be acknowledged as terminated: {}",
            cleanup.diagnostics()
        );
        assert!(
            !cleanup.diagnostics().is_empty(),
            "unconfirmed cleanup must carry actionable diagnostics"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn test_process_group_isolation() {
    use std::process::Stdio;
    use tokio::process::Command;

    // Verify that the process group is different from parent
    let parent_pgid = unsafe { libc::getpgid(0) };

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("echo $PPID")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    unsafe {
        cmd.pre_exec(|| {
            use nix::unistd::{setpgid, Pid};
            setpgid(Pid::from_raw(0), Pid::from_raw(0)).map_err(std::io::Error::other)?;
            Ok(())
        });
    }

    let child = cmd.spawn().expect("Failed to spawn test process");
    let child_pid = child.id().expect("Failed to get child PID");
    let child_pgid = unsafe { libc::getpgid(child_pid as i32) };

    assert_ne!(
        parent_pgid, child_pgid,
        "Child should be in a different process group"
    );

    // Clean up
    let _ = child.wait_with_output().await;
}

/// Run-owned process cleanup at the three shutdown boundaries.
///
/// The unit coverage in `src/` proves that the scheduler and the TUI *consult*
/// the run command scope in the right order. These prove the other half with
/// real processes: that when each boundary is reached, the owned process group
/// is actually empty — including a descendant that ignores SIGTERM, which is the
/// case a graceful-only path silently leaves behind.
///
/// Each test records the PGID before shutdown starts and probes `killpg(pgid, 0)`
/// after the asserted boundary. Every assertion path force-kills the group first
/// so a failure cannot leak a `sleep` into the test runner's process table.
#[cfg(unix)]
mod run_scope_process_cleanup {
    use conflux::ai_command_runner::{AiCommandRunner, OutputLine, RunCommandScope};
    use conflux::config::OrchestratorConfig;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use tokio_util::sync::CancellationToken;

    /// A leader that stays alive plus a descendant that ignores SIGTERM.
    ///
    /// Graceful termination alone cannot clear this group; only the SIGKILL
    /// escalation and verification the cleanup path owns can.
    const SIGTERM_IMMUNE_GROUP: &str =
        "sh -c 'trap \"\" TERM; while :; do sleep 0.2; done' >/dev/null 2>&1 </dev/null & sleep 300";

    /// A run-owned runner built the way production builds one.
    ///
    /// Inactivity timeout is off so nothing but the shutdown path can end the
    /// command, which is what makes a surviving group unambiguous evidence.
    fn scoped_runner(scope: &RunCommandScope) -> AiCommandRunner {
        let config = OrchestratorConfig {
            command_queue_stagger_delay_ms: Some(0),
            command_queue_max_retries: Some(1),
            command_queue_retry_delay_ms: Some(0),
            command_inactivity_timeout_secs: Some(0),
            command_inactivity_timeout_max_retries: Some(0),
            ..OrchestratorConfig::default()
        };
        AiCommandRunner::for_run(&config, Arc::new(Mutex::new(None)), scope.clone())
    }

    fn group_has_members(pgid: i32) -> bool {
        unsafe { libc::killpg(pgid, 0) == 0 }
    }

    /// Reap the group unconditionally, then report whether it had survived.
    ///
    /// Called before every terminal assertion so a failing test cleans up after
    /// itself instead of leaking the process it was written to detect.
    fn reap_and_report_survival(pgid: i32) -> bool {
        let survived = group_has_members(pgid);
        if survived {
            unsafe { libc::killpg(pgid, libc::SIGKILL) };
        }
        survived
    }

    /// Launch one run-owned command and return its PGID once the leader exists.
    ///
    /// The `StreamingChildHandle` is returned so the caller decides when the
    /// caller-side handle disappears — dropping it is how an aborted workspace
    /// future looks from the runner's side.
    async fn launch_owned_group(
        runner: &AiCommandRunner,
        change_id: &str,
    ) -> (
        conflux::process_manager::StreamingChildHandle,
        tokio::sync::mpsc::Receiver<OutputLine>,
        i32,
    ) {
        let (handle, rx) = runner
            .execute_streaming_with_retry(
                SIGTERM_IMMUNE_GROUP,
                None,
                Some("apply"),
                Some(change_id),
            )
            .await
            .expect("an open scope admits the command");

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(pid) = handle.id() {
                // `configure_process_group` makes the leader its own group.
                return (handle, rx, pid as i32);
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the command never reported a real pid"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Global cancellation: no member remains at the boundary where the
    /// scheduler is allowed to publish terminal `Stopped`.
    #[tokio::test]
    async fn run_scope_global_cancellation_cleans_process_group() {
        let scope = RunCommandScope::new();
        let cancel = CancellationToken::new();
        // The scope observes the run's global token directly, exactly as the
        // scheduler wires it at run start.
        scope.link_cancellation(cancel.clone());

        let runner = scoped_runner(&scope);

        let (handle, _rx, pgid) = launch_owned_group(&runner, "change-a").await;
        assert!(
            group_has_members(pgid),
            "arrangement failed: the owned group must exist before shutdown"
        );

        // Operator cancellation, then `JoinSet::abort_all` drops the workspace
        // future — and with it the only caller-held handle.
        cancel.cancel();
        drop(handle);

        // The scheduler's cleanup barrier. `Stopped` may not precede it.
        let cleanup = scope.wait_quiescent(Duration::from_secs(30)).await;

        let survived = reap_and_report_survival(pgid);
        assert!(
            cleanup.is_quiescent(),
            "the barrier must prove quiescence before terminal Stopped: {}",
            cleanup.diagnostics()
        );
        assert!(
            !survived,
            "process group {pgid} survived global cancellation"
        );
    }

    /// Run-fatal: the global Error is published promptly, and no member remains
    /// at the boundary where the scheduler returns its failure.
    ///
    /// The Error/barrier *ordering* against the real queue boundary is pinned by
    /// `run_fatal_error_precedes_cleanup_barrier` in the crate's unit tests;
    /// this models the same sequence to assert the process outcome at the
    /// failure-return boundary with a real SIGTERM-immune group.
    #[tokio::test]
    async fn run_scope_run_fatal_cleans_process_group() {
        let scope = RunCommandScope::new();
        let runner = scoped_runner(&scope);

        let (handle, _rx, pgid) = launch_owned_group(&runner, "change-a").await;
        assert!(
            group_has_members(pgid),
            "arrangement failed: the owned group must exist before shutdown"
        );

        // Step 1: the queue boundary publishes exactly one global Error without
        // waiting for cleanup.
        let (error_tx, mut errors) = tokio::sync::mpsc::channel::<String>(8);
        error_tx
            .send("Background merge failed for 'change-a'".to_string())
            .await
            .expect("the prompt Error is published before any waiting");
        drop(error_tx);

        // Step 2: the same boundary closes admission and signals runner tasks.
        scope.close();
        assert!(
            scope.is_closed(),
            "run-fatal shutdown closes command admission"
        );

        // Step 3: dispatch stops and the workspace futures are aborted.
        drop(handle);

        // Step 4: only now may the scheduler return its run-fatal failure.
        let cleanup = scope.wait_quiescent(Duration::from_secs(30)).await;

        let survived = reap_and_report_survival(pgid);
        assert!(
            cleanup.is_quiescent(),
            "failure return must follow proven cleanup: {}",
            cleanup.diagnostics()
        );
        assert!(
            !survived,
            "process group {pgid} survived run-fatal shutdown"
        );

        let mut emitted = Vec::new();
        while let Some(message) = errors.recv().await {
            emitted.push(message);
        }
        assert_eq!(
            emitted.len(),
            1,
            "exactly one prompt global Error for the run-fatal outcome, got {emitted:?}"
        );

        // No new command may be admitted after the failure either.
        let (mut refused, mut refused_rx) = runner
            .execute_streaming_with_retry("echo never", None, Some("archive"), Some("change-b"))
            .await
            .expect("the call returns a refusal rather than launching");
        while refused_rx.recv().await.is_some() {}
        assert!(!refused.wait().await.expect("status").success());
    }

    /// Local TUI quit: no member remains when the supervisor reports
    /// `AbortedAfterTimeout`.
    ///
    /// The orchestrator task here never finishes, which is exactly the case
    /// where aborting the task would otherwise be the end of the story: the
    /// retained scope outside that task is the only remaining path to the PGID.
    #[tokio::test]
    async fn run_scope_tui_quit_cleans_process_group_after_timeout() {
        let scope = RunCommandScope::new();
        let mut runner = scoped_runner(&scope);
        // A zero per-command verification budget makes the ordinary cleanup
        // unprovable, so the identity stays retained for managed escalation —
        // the state the TUI timeout path exists to resolve.
        runner.set_process_group_cleanup_timeout_ms(0);

        let (handle, mut rx, pgid) = launch_owned_group(&runner, "change-a").await;
        assert!(
            group_has_members(pgid),
            "arrangement failed: the owned group must exist before shutdown"
        );

        scope.close();
        drop(handle);
        // Drain so the runner task is never blocked on a full output channel.
        while rx.recv().await.is_some() {}

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while scope.retained_process_ids().is_empty() {
            if std::time::Instant::now() >= deadline {
                reap_and_report_survival(pgid);
                panic!("the unprovable cleanup must retain its owned identity");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // The local orchestrator task has stopped cooperating.
        let orchestrator = tokio::spawn(async move {
            std::future::pending::<()>().await;
            Ok(())
        });
        let outcome = conflux::tui::shutdown_local_orchestrator_task(
            Some(orchestrator),
            Some(CancellationToken::new()),
            Some(scope.clone()),
            Duration::from_millis(50),
        )
        .await;

        let survived = reap_and_report_survival(pgid);
        assert_eq!(
            outcome,
            conflux::tui::LocalOrchestratorShutdownOutcome::AbortedAfterTimeout,
            "the timeout outcome stays distinguishable from graceful completion"
        );
        assert!(
            !survived,
            "process group {pgid} survived the local TUI timeout abort"
        );
        assert!(
            scope.retained_process_ids().is_empty(),
            "forced cleanup must verify, not merely signal"
        );
    }
}
