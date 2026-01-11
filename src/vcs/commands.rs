//! Common command execution helpers for VCS operations.
//!
//! This module provides shared utilities for running VCS commands,
//! reducing code duplication between jj and Git implementations.

use super::{VcsBackend, VcsError, VcsResult};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Execute a VCS command and return the trimmed stdout output.
///
/// # Arguments
/// * `program` - The VCS program to run (e.g., "jj", "git")
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
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::Command {
            backend,
            message: format!("Failed to execute {}: {}", program, e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VcsError::Command {
            backend,
            message: format!("{} {} failed: {}", program, args.join(" "), stderr),
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
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| VcsError::Command {
            backend,
            message: format!("Failed to execute {}: {}", program, e),
        })?;

    if !output.status.success() {
        return Err(VcsError::Command {
            backend,
            message: format!("{} {} failed", program, args.join(" ")),
        });
    }

    Ok(())
}

/// Execute a VCS command, ignoring errors.
///
/// Useful for cleanup operations where failure is acceptable.
#[allow(dead_code)]
pub async fn run_vcs_command_ignore_error<P: AsRef<Path>>(program: &str, args: &[&str], cwd: P) {
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
}
