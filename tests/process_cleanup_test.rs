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
