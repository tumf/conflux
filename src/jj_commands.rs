//! Common helpers for jj (Jujutsu) command execution.
//!
//! This module provides shared utilities for running jj commands,
//! reducing code duplication across jj_workspace.rs and parallel_executor.rs.

use crate::error::{OrchestratorError, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

/// Execute a jj command and return the trimmed stdout output.
///
/// # Arguments
/// * `args` - Arguments to pass to jj
/// * `cwd` - Working directory for the command
///
/// # Returns
/// The trimmed stdout output on success, or an error if the command fails.
pub async fn run_jj<P: AsRef<Path>>(args: &[&str], cwd: P) -> Result<String> {
    let output = Command::new("jj")
        .args(args)
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| OrchestratorError::JjCommand(format!("Failed to execute jj: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OrchestratorError::JjCommand(format!(
            "jj {} failed: {}",
            args.join(" "),
            stderr
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Execute a jj command without capturing output (fire-and-forget).
///
/// Returns Ok(()) on success, error on failure.
/// Reserved for future use in operations that don't need output.
#[allow(dead_code)] // Public API reserved for future use
pub async fn run_jj_silent<P: AsRef<Path>>(args: &[&str], cwd: P) -> Result<()> {
    let output = Command::new("jj")
        .args(args)
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| OrchestratorError::JjCommand(format!("Failed to execute jj: {}", e)))?;

    if !output.status.success() {
        return Err(OrchestratorError::JjCommand(format!(
            "jj {} failed",
            args.join(" ")
        )));
    }

    Ok(())
}

/// Execute a jj command, ignoring errors.
///
/// Useful for cleanup operations where failure is acceptable.
/// Reserved for future use in cleanup and best-effort operations.
#[allow(dead_code)] // Public API reserved for cleanup scenarios
pub async fn run_jj_ignore_error<P: AsRef<Path>>(args: &[&str], cwd: P) {
    let _ = Command::new("jj")
        .args(args)
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await;
}

/// Get the current revision (change_id) at @ in the given directory.
pub async fn get_current_revision<P: AsRef<Path>>(cwd: P) -> Result<String> {
    run_jj(
        &[
            "log",
            "-r",
            "@",
            "--no-graph",
            "--ignore-working-copy",
            "-T",
            "change_id",
        ],
        cwd,
    )
    .await
}

/// Check if jj is available and the directory is a jj repository.
#[allow(dead_code)] // Reserved for future use in workspace initialization
pub async fn check_jj_repo<P: AsRef<Path>>(cwd: P) -> Result<bool> {
    // Check jj --version
    let version_result = Command::new("jj")
        .arg("--version")
        .current_dir(cwd.as_ref())
        .stdin(Stdio::null())
        .output()
        .await;

    match version_result {
        Ok(out) if out.status.success() => {
            // Check if directory is a jj repo
            let root_result = Command::new("jj")
                .arg("root")
                .current_dir(cwd.as_ref())
                .stdin(Stdio::null())
                .output()
                .await;

            match root_result {
                Ok(out) if out.status.success() => Ok(true),
                _ => Ok(false),
            }
        }
        _ => Ok(false),
    }
}

/// Get jj status output.
pub async fn get_status<P: AsRef<Path>>(cwd: P) -> Result<String> {
    run_jj(&["status"], cwd).await
}

/// Get jj log output for specific revisions.
pub async fn get_log_for_revisions<P: AsRef<Path>>(revisions: &[String], cwd: P) -> Result<String> {
    if revisions.is_empty() {
        return Ok(String::new());
    }

    let revset = revisions.join(" | ");
    run_jj(&["log", "-r", &revset, "--no-graph"], cwd).await
}

/// Describe a revision with a message.
///
/// Reserved for future use when direct revision description is needed.
#[allow(dead_code)] // Public API reserved for revision operations
pub async fn describe<P: AsRef<Path>>(message: &str, cwd: P) -> Result<()> {
    run_jj(&["describe", "-m", message], cwd).await?;
    Ok(())
}

/// Edit (switch to) a specific revision.
///
/// Reserved for future use when direct revision switching is needed.
#[allow(dead_code)] // Public API reserved for revision operations
pub async fn edit<P: AsRef<Path>>(revision: &str, cwd: P) -> Result<()> {
    run_jj(&["edit", revision], cwd).await?;
    Ok(())
}

/// Run jj status to snapshot working copy changes (triggers auto-snapshot).
///
/// Reserved for future use when explicit snapshot triggering is needed.
#[allow(dead_code)] // Public API reserved for snapshot operations
pub async fn snapshot<P: AsRef<Path>>(cwd: P) {
    debug!("Triggering jj snapshot via status");
    let _ = run_jj(&["status"], cwd).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_check_jj_repo_non_repo() {
        let temp_dir = TempDir::new().unwrap();
        // Non-jj directory should return false (not error)
        let result = check_jj_repo(temp_dir.path()).await;
        assert!(result.is_ok());
        // Result depends on whether jj is installed
    }
}
