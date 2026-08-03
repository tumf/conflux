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
