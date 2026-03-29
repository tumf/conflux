//! Server-side project runner.
//!
//! Spawns `cflx run` in a project's worktree and supports stop/retry.
//!
//! Design goals:
//! - Keep server daemon directory-independent.
//! - Isolate orchestration execution to the project's worktree path.
//! - Allow cancellation by terminating the spawned process group.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::Result;
use crate::process_manager::{configure_process_group, ManagedChild};
use crate::remote::types::RemoteLogEntry;
use crate::server::registry::{ProjectStatus, SharedRegistry};

#[derive(Debug)]
pub struct ProjectRunRequest {
    pub project_id: String,
    pub worktree_path: PathBuf,
    pub changes: Option<Vec<String>>,
}

#[derive(Debug)]
pub(crate) struct RunnerEntry {
    cancel: CancellationToken,
    // Presence indicates a task is (or was) running.
    #[allow(dead_code)]
    handle: tokio::task::JoinHandle<()>,
}

pub(crate) type SharedRunners = Arc<RwLock<HashMap<String, RunnerEntry>>>;

pub(crate) fn create_shared_runners() -> SharedRunners {
    Arc::new(RwLock::new(HashMap::new()))
}

pub(crate) async fn start_project_run(
    runners: &SharedRunners,
    registry: SharedRegistry,
    req: ProjectRunRequest,
    log_tx: tokio::sync::broadcast::Sender<RemoteLogEntry>,
) -> Result<()> {
    // If already running, cancel existing runner first.
    stop_project_run(runners, req.project_id.clone()).await;

    let cancel = CancellationToken::new();
    let cancel_child = cancel.clone();
    let registry_child = registry.clone();

    let project_id = req.project_id.clone();
    let worktree_path = req.worktree_path.clone();
    let changes = req.changes.clone();

    let handle = tokio::spawn(async move {
        if let Err(e) =
            set_project_status(&registry_child, &project_id, ProjectStatus::Running).await
        {
            warn!(
                "Failed to set project status running: project_id={} err={}",
                project_id, e
            );
        }

        match run_cflx_in_worktree(
            &registry_child,
            &project_id,
            &worktree_path,
            changes,
            cancel_child,
            log_tx,
        )
        .await
        {
            Ok(()) => {
                if let Err(e) =
                    set_project_status(&registry_child, &project_id, ProjectStatus::Idle).await
                {
                    warn!(
                        "Failed to set project status idle: project_id={} err={}",
                        project_id, e
                    );
                }
            }
            Err(e) => {
                warn!("Project run failed: project_id={} err={}", project_id, e);
                // Leave the status as-is; caller can Retry/Stop.
            }
        }
    });

    let mut map = runners.write().await;
    map.insert(req.project_id, RunnerEntry { cancel, handle });

    Ok(())
}

pub(crate) async fn stop_project_run(runners: &SharedRunners, project_id: String) {
    let entry = { runners.write().await.remove(&project_id) };
    if let Some(entry) = entry {
        entry.cancel.cancel();
        // Detach; task will observe cancel and terminate the child.
    }
}

async fn set_project_status(
    registry: &SharedRegistry,
    project_id: &str,
    status: ProjectStatus,
) -> Result<()> {
    let mut reg = registry.write().await;
    reg.set_status(project_id, status)
}

fn make_log_entry(
    message: String,
    level: &str,
    change_id: Option<String>,
    project_id: Option<String>,
) -> RemoteLogEntry {
    RemoteLogEntry {
        message,
        level: level.to_string(),
        change_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_id,
        operation: None,
        iteration: None,
    }
}

async fn mark_selected_changes_as_error(
    registry: &SharedRegistry,
    project_id: &str,
    changes: &Option<Vec<String>>,
    error: String,
) {
    let Some(change_ids) = changes.as_ref() else {
        return;
    };

    let mut reg = registry.write().await;
    for change_id in change_ids {
        reg.mark_change_error(project_id, change_id, error.clone());
    }
}

async fn run_cflx_in_worktree(
    registry: &SharedRegistry,
    project_id: &str,
    worktree_path: &Path,
    changes: Option<Vec<String>>,
    cancel: CancellationToken,
    log_tx: tokio::sync::broadcast::Sender<RemoteLogEntry>,
) -> Result<()> {
    if !worktree_path.exists() {
        return Err(crate::error::OrchestratorError::ConfigLoad(format!(
            "Worktree path does not exist: {}",
            worktree_path.display()
        )));
    }

    let exe = std::env::current_exe().map_err(|e| {
        crate::error::OrchestratorError::Io(std::io::Error::other(format!(
            "Failed to resolve current executable: {}",
            e
        )))
    })?;

    let mut cmd = tokio::process::Command::new(exe);
    cmd.arg("run");
    if let Some(change_ids) = changes.as_ref() {
        if !change_ids.is_empty() {
            cmd.arg("--change").arg(change_ids.join(","));
        }
    }
    cmd.current_dir(worktree_path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(unix)]
    configure_process_group(&mut cmd);

    info!(
        "Starting cflx run for project_id={} (cwd={})",
        project_id,
        worktree_path.display()
    );

    let child = cmd.spawn().map_err(|e| {
        crate::error::OrchestratorError::Io(std::io::Error::other(format!(
            "Failed to spawn cflx run: {}",
            e
        )))
    })?;

    let mut child = ManagedChild::new(child).map_err(|e| {
        crate::error::OrchestratorError::Io(std::io::Error::other(format!(
            "Failed to wrap child process: {}",
            e
        )))
    })?;

    // Drain stdout/stderr to avoid blocking, and stream to WebSocket clients.
    if let Some(stdout) = child.child.stdout.take() {
        let pid = project_id.to_string();
        let tx = log_tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("[{} stdout] {}", pid, line);
                let entry = make_log_entry(line, "info", None, Some(pid.clone()));
                // Ignore errors (no subscribers is fine)
                let _ = tx.send(entry);
            }
        });
    }
    if let Some(stderr) = child.child.stderr.take() {
        let pid = project_id.to_string();
        let tx = log_tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("[{} stderr] {}", pid, line);
                let entry = make_log_entry(line, "warn", None, Some(pid.clone()));
                // Ignore errors (no subscribers is fine)
                let _ = tx.send(entry);
            }
        });
    }

    tokio::select! {
        _ = cancel.cancelled() => {
            info!("Stopping cflx run: project_id={}", project_id);
            let _ = child.terminate_with_timeout(Duration::from_secs(5)).await;
        }
        status = child.wait() => {
            match status {
                Ok(s) if s.success() => {
                    info!("cflx run exited successfully: project_id={}", project_id);
                }
                Ok(s) => {
                    warn!("cflx run exited with failure: project_id={} status={}", project_id, s);
                    mark_selected_changes_as_error(registry, project_id, &changes, s.to_string()).await;
                }
                Err(e) => {
                    warn!("Failed waiting for cflx run: project_id={} err={}", project_id, e);
                    mark_selected_changes_as_error(registry, project_id, &changes, e.to_string()).await;
                }
            }
        }
    }

    Ok(())
}
