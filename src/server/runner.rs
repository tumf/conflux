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

        match run_cflx_in_worktree(&project_id, &worktree_path, changes, cancel_child).await {
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

async fn run_cflx_in_worktree(
    project_id: &str,
    worktree_path: &Path,
    changes: Option<Vec<String>>,
    cancel: CancellationToken,
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
    if let Some(changes) = changes {
        if !changes.is_empty() {
            cmd.arg("--change").arg(changes.join(","));
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

    // Drain stdout/stderr to avoid blocking.
    if let Some(stdout) = child.child.stdout.take() {
        let pid = project_id.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("[{} stdout] {}", pid, line);
            }
        });
    }
    if let Some(stderr) = child.child.stderr.take() {
        let pid = project_id.to_string();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("[{} stderr] {}", pid, line);
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
                }
                Err(e) => {
                    warn!("Failed waiting for cflx run: project_id={} err={}", project_id, e);
                }
            }
        }
    }

    Ok(())
}
