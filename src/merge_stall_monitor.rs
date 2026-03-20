//! Merge stall detection monitor.
//!
//! This module monitors the progress of merge commits to the base branch
//! and triggers a stall when no merge activity is detected within a threshold period.

use crate::config::MergeStallDetectionConfig;
use crate::error::{OrchestratorError, Result};
use std::path::Path;
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Monitors merge commit progress and triggers cancellation on stall.
///
/// The monitor periodically checks for new merge commits matching the pattern
/// `Merge change: <change_id>` on the base branch. If no new merge commits
/// are found within the configured threshold, it triggers the cancellation token.
pub struct MergeStallMonitor {
    config: MergeStallDetectionConfig,
    repo_root: std::path::PathBuf,
    base_branch: String,
}

impl MergeStallMonitor {
    /// Create a new merge stall monitor.
    ///
    /// # Arguments
    /// * `config` - Merge stall detection configuration
    /// * `repo_root` - Repository root directory
    /// * `base_branch` - Base branch name to monitor
    pub fn new(
        config: MergeStallDetectionConfig,
        repo_root: impl AsRef<Path>,
        base_branch: String,
    ) -> Self {
        Self {
            config,
            repo_root: repo_root.as_ref().to_path_buf(),
            base_branch,
        }
    }

    /// Start monitoring merge progress in a background task.
    ///
    /// Returns a task handle that can be awaited or aborted.
    ///
    /// # Arguments
    /// * `cancel_token` - Cancellation token to trigger on stall detection
    ///
    /// # Behavior
    /// - Checks for merge commits every `check_interval_seconds`
    /// - Triggers `cancel_token` if no merge within `threshold_minutes`
    /// - Stops monitoring when `cancel_token` is already cancelled
    pub fn spawn_monitor(
        self,
        cancel_token: CancellationToken,
    ) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move {
            if !self.config.enabled {
                debug!("Merge stall detection is disabled");
                return Ok(());
            }

            info!(
                threshold_minutes = self.config.threshold_minutes,
                check_interval_seconds = self.config.check_interval_seconds,
                base_branch = %self.base_branch,
                "Starting merge stall monitor"
            );

            let mut interval = interval(Duration::from_secs(self.config.check_interval_seconds));
            let threshold = Duration::from_secs(self.config.threshold_minutes * 60);
            let mut last_merge_time: Option<SystemTime> = None;

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        debug!("Merge stall monitor stopping (cancellation requested)");
                        return Ok(());
                    }
                    _ = interval.tick() => {
                        // Check for latest merge commit
                        match self.get_latest_merge_commit_time().await {
                            Ok(Some(commit_time)) => {
                                debug!(
                                    commit_time = ?commit_time,
                                    "Found merge commit on base branch"
                                );

                                // Update last_merge_time if this is the first or newer
                                if last_merge_time.is_none() || Some(commit_time) > last_merge_time {
                                    last_merge_time = Some(commit_time);
                                }
                            }
                            Ok(None) => {
                                debug!("No merge commits found yet");
                            }
                            Err(e) => {
                                warn!("Failed to check merge commits: {}", e);
                                continue;
                            }
                        }

                        // Check for stall
                        if let Some(last_time) = last_merge_time {
                            if let Ok(elapsed) = SystemTime::now().duration_since(last_time) {
                                if elapsed > threshold {
                                    let elapsed_minutes = elapsed.as_secs() / 60;
                                    error!(
                                        elapsed_minutes = elapsed_minutes,
                                        threshold_minutes = self.config.threshold_minutes,
                                        base_branch = %self.base_branch,
                                        "Merge stall detected: no merge progress for {} minutes (threshold: {} minutes)",
                                        elapsed_minutes,
                                        self.config.threshold_minutes
                                    );
                                    // Cancel execution due to merge stall
                                    // The cancellation will be picked up by the executor's is_cancelled() check
                                    // which will log the cancellation and stop processing
                                    cancel_token.cancel();
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    /// Get the timestamp of the most recent merge commit on the base branch.
    ///
    /// Returns:
    /// - `Ok(Some(time))` if merge commits exist
    /// - `Ok(None)` if no merge commits found
    /// - `Err` if git command fails
    async fn get_latest_merge_commit_time(&self) -> Result<Option<SystemTime>> {
        // Search for merge commits with pattern "Merge change: *"
        let output = Command::new("git")
            .args([
                "log",
                &self.base_branch,
                "--format=%ct", // Commit timestamp (Unix epoch)
                "--grep",
                "^Merge change: ",
                "-1", // Only the most recent
            ])
            .current_dir(&self.repo_root)
            .output()
            .await
            .map_err(|e| OrchestratorError::GitCommand(format!("Failed to run git log: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OrchestratorError::GitCommand(format!(
                "git log failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let timestamp_str = stdout.trim();

        if timestamp_str.is_empty() {
            return Ok(None);
        }

        // Parse Unix timestamp
        let timestamp: u64 = timestamp_str.parse().map_err(|e| {
            OrchestratorError::GitCommand(format!("Failed to parse commit timestamp: {}", e))
        })?;

        let commit_time = SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp);
        Ok(Some(commit_time))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    async fn init_git_repo(dir: &Path) -> Result<()> {
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir)
            .output()
            .await
            .map_err(|e| OrchestratorError::GitCommand(format!("git init failed: {}", e)))?;

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir)
            .output()
            .await?;

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(dir)
            .output()
            .await?;

        Ok(())
    }

    async fn create_commit(dir: &Path, message: &str) -> Result<()> {
        fs::write(dir.join("test.txt"), message)?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .output()
            .await?;
        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(dir)
            .output()
            .await
            .map_err(|e| OrchestratorError::GitCommand(format!("git commit failed: {}", e)))?;
        Ok(())
    }

    #[tokio::test]
    async fn test_get_latest_merge_commit_time_with_merge() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path()).await.unwrap();
        create_commit(temp_dir.path(), "Initial commit")
            .await
            .unwrap();
        create_commit(temp_dir.path(), "Merge change: test-change")
            .await
            .unwrap();

        let config = MergeStallDetectionConfig::default();
        let monitor = MergeStallMonitor::new(config, temp_dir.path(), "main".to_string());

        let result = monitor.get_latest_merge_commit_time().await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_get_latest_merge_commit_time_no_merge() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path()).await.unwrap();
        create_commit(temp_dir.path(), "Initial commit")
            .await
            .unwrap();

        let config = MergeStallDetectionConfig::default();
        let monitor = MergeStallMonitor::new(config, temp_dir.path(), "main".to_string());

        let result = monitor.get_latest_merge_commit_time().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_monitor_disabled_exits_immediately() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path()).await.unwrap();

        let config = MergeStallDetectionConfig {
            enabled: false,
            ..Default::default()
        };
        let monitor = MergeStallMonitor::new(config, temp_dir.path(), "main".to_string());
        let cancel_token = CancellationToken::new();

        let handle = monitor.spawn_monitor(cancel_token.clone());
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_monitor_stops_on_external_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path()).await.unwrap();
        create_commit(temp_dir.path(), "Initial commit")
            .await
            .unwrap();

        let config = MergeStallDetectionConfig {
            enabled: true,
            threshold_minutes: 60, // High threshold so it won't trigger during test
            check_interval_seconds: 1,
        };
        let monitor = MergeStallMonitor::new(config, temp_dir.path(), "main".to_string());
        let cancel_token = CancellationToken::new();

        let handle = monitor.spawn_monitor(cancel_token.clone());

        // Give monitor time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Cancel externally
        cancel_token.cancel();

        // Monitor should exit cleanly
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(2), handle).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_monitor_triggers_cancellation_on_stall() {
        let temp_dir = TempDir::new().unwrap();
        init_git_repo(temp_dir.path()).await.unwrap();
        create_commit(temp_dir.path(), "Initial commit")
            .await
            .unwrap();

        // Create an old merge commit (simulating no recent merge activity)
        create_commit(temp_dir.path(), "Merge change: test-old")
            .await
            .unwrap();

        let config = MergeStallDetectionConfig {
            enabled: true,
            threshold_minutes: 0, // Zero threshold means immediate stall
            check_interval_seconds: 1,
        };
        let monitor = MergeStallMonitor::new(config, temp_dir.path(), "main".to_string());
        let cancel_token = CancellationToken::new();

        let handle = monitor.spawn_monitor(cancel_token.clone());

        // Wait for monitor to detect stall and cancel
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Cancel token should be triggered by the monitor
        assert!(cancel_token.is_cancelled());

        // Monitor should exit cleanly
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), handle).await;
        assert!(result.is_ok());
    }
}
