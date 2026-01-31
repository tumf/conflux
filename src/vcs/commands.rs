//! Common command execution helpers for VCS operations.
//!
//! This module provides shared utilities for running VCS commands,
//! reducing code duplication in Git implementations.

use super::{VcsBackend, VcsError, VcsResult};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

/// Execute a VCS command and return the trimmed stdout output.
///
/// # Arguments
/// * `program` - The VCS program to run (e.g., "git")
/// * `args` - Arguments to pass to the program
/// * `cwd` - Working directory for the command
/// * `backend` - VCS backend type for error context
///
/// # Returns
/// The trimmed stdout output on success, or an error if the command fails.
pub async fn run_vcs_command<P: AsRef<Path>>(
    program: &str,
    args: &[&str],
    cwd: P,
    backend: VcsBackend,
) -> VcsResult<String> {
    let cwd_path = cwd.as_ref();
    let command_str = format!("{} {}", program, args.join(" "));

    debug!(
        module = module_path!(),
        "Executing {} command: {} (cwd: {:?})",
        program,
        args.join(" "),
        cwd_path
    );
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd_path)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::Command {
            backend,
            message: format!("Failed to execute {}: {}", program, e),
            command: Some(command_str.clone()),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: None,
            stdout: None,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        return Err(VcsError::Command {
            backend,
            message: format!("{} {} failed: {}", program, args.join(" "), stderr),
            command: Some(command_str),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: Some(stderr),
            stdout: Some(stdout),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Execute a VCS command without capturing output (fire-and-forget).
///
/// Returns Ok(()) on success, error on failure.
#[allow(dead_code)]
pub async fn run_vcs_command_silent<P: AsRef<Path>>(
    program: &str,
    args: &[&str],
    cwd: P,
    backend: VcsBackend,
) -> VcsResult<()> {
    let cwd_path = cwd.as_ref();
    let command_str = format!("{} {}", program, args.join(" "));

    debug!(
        module = module_path!(),
        "Executing {} command (silent): {} (cwd: {:?})",
        program,
        args.join(" "),
        cwd_path
    );
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::Command {
            backend,
            message: format!("Failed to execute {}: {}", program, e),
            command: Some(command_str.clone()),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: None,
            stdout: None,
        })?;

    if !output.status.success() {
        return Err(VcsError::Command {
            backend,
            message: format!("{} {} failed", program, args.join(" ")),
            command: Some(command_str),
            working_dir: Some(cwd_path.to_path_buf()),
            stderr: None,
            stdout: None,
        });
    }

    Ok(())
}

/// Execute a VCS command, ignoring errors.
///
/// Useful for cleanup operations where failure is acceptable.
#[allow(dead_code)]
pub async fn run_vcs_command_ignore_error<P: AsRef<Path>>(program: &str, args: &[&str], cwd: P) {
    debug!(
        module = module_path!(),
        "Executing {} command (ignore errors): {} (cwd: {:?})",
        program,
        args.join(" "),
        cwd.as_ref()
    );
    let _ = Command::new(program)
        .args(args)
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await;
}

/// Check if a VCS program is available.
///
/// Returns true if the program can be executed with --version.
pub async fn check_vcs_available<P: AsRef<Path>>(program: &str, cwd: P) -> VcsResult<bool> {
    debug!(
        module = module_path!(),
        "Executing {} command: {} (cwd: {:?})",
        program,
        "--version",
        cwd.as_ref()
    );
    let version_result = Command::new(program)
        .arg("--version")
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await;

    match version_result {
        Ok(out) if out.status.success() => Ok(true),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_check_vcs_available_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        // Non-existent program should return false
        let result = check_vcs_available("nonexistent-vcs-program", temp_dir.path()).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_vcs_error_includes_command_context() {
        let temp_dir = TempDir::new().unwrap();

        // Run a git command that will fail (invalid subcommand)
        let result = run_vcs_command(
            "git",
            &["invalid-subcommand-xyz"],
            temp_dir.path(),
            VcsBackend::Git,
        )
        .await;

        // Verify the command failed
        assert!(result.is_err());

        let err = result.unwrap_err();
        match err {
            VcsError::Command {
                command,
                working_dir,
                stderr,
                stdout,
                ..
            } => {
                // Verify command context is included
                assert!(command.is_some());
                let cmd = command.unwrap();
                assert!(cmd.contains("git"));
                assert!(cmd.contains("invalid-subcommand-xyz"));

                // Verify working directory is included
                assert!(working_dir.is_some());
                assert_eq!(working_dir.unwrap(), temp_dir.path());

                // Verify stderr is captured
                assert!(stderr.is_some());
                let stderr_str = stderr.unwrap();
                assert!(!stderr_str.is_empty());

                // stdout may or may not be present
                assert!(stdout.is_some());
            }
            _ => panic!("Expected VcsError::Command variant"),
        }
    }
}
