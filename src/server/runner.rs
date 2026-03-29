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
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::Result;
use crate::process_manager::{configure_process_group, ManagedChild};
use crate::remote::types::RemoteLogEntry;
use crate::server::db::ServerDb;
use crate::server::registry::{ProjectStatus, SharedRegistry};

#[derive(Debug)]
pub struct ProjectRunRequest {
    pub project_id: String,
    pub worktree_path: PathBuf,
    pub changes: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct RunnerEntry {
    cancel: CancellationToken,
    // Presence indicates a task is (or was) running.
    #[allow(dead_code)]
    handle: tokio::task::JoinHandle<()>,
}

pub type SharedRunners = Arc<RwLock<HashMap<String, RunnerEntry>>>;

pub fn create_shared_runners() -> SharedRunners {
    Arc::new(RwLock::new(HashMap::new()))
}

pub(crate) async fn start_project_run(
    runners: &SharedRunners,
    registry: SharedRegistry,
    db: Option<Arc<ServerDb>>,
    req: ProjectRunRequest,
    log_tx: tokio::sync::broadcast::Sender<RemoteLogEntry>,
) -> Result<()> {
    // If already running, cancel existing runner first.
    stop_project_run(runners, req.project_id.clone()).await;

    let cancel = CancellationToken::new();
    let cancel_child = cancel.clone();
    let registry_child = registry.clone();
    let db_child = db.clone();

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
            db_child,
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
    operation: Option<String>,
    iteration: Option<u32>,
) -> RemoteLogEntry {
    RemoteLogEntry {
        message,
        level: level.to_string(),
        change_id,
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_id,
        operation,
        iteration,
    }
}

async fn mark_selected_changes_as_error(
    registry: &SharedRegistry,
    db: Option<&Arc<ServerDb>>,
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
        if let Some(db) = db {
            if let Err(e) = db.upsert_change_state(project_id, change_id, false, Some(&error)) {
                warn!(project_id, change_id, error = %e, "Failed to persist change error state");
            }
        }
    }
}

async fn run_cflx_in_worktree(
    registry: &SharedRegistry,
    db: Option<Arc<ServerDb>>,
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
    let started_at = Instant::now();

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
        let db_stdout = db.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("[{} stdout] {}", pid, line);
                let entry = make_log_entry(
                    line.clone(),
                    "info",
                    None,
                    Some(pid.clone()),
                    Some("apply".to_string()),
                    None,
                );
                if let Some(db) = db_stdout.as_ref() {
                    if let Err(e) =
                        db.insert_log(Some(&pid), "info", &line, None, Some("apply"), None)
                    {
                        warn!(project_id = %pid, error = %e, "Failed to persist stdout log entry");
                    }
                }
                // Ignore errors (no subscribers is fine)
                let _ = tx.send(entry);
            }
        });
    }
    if let Some(stderr) = child.child.stderr.take() {
        let pid = project_id.to_string();
        let tx = log_tx.clone();
        let db_stderr = db.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!("[{} stderr] {}", pid, line);
                let entry = make_log_entry(
                    line.clone(),
                    "warn",
                    None,
                    Some(pid.clone()),
                    Some("apply".to_string()),
                    None,
                );
                if let Some(db) = db_stderr.as_ref() {
                    if let Err(e) =
                        db.insert_log(Some(&pid), "warn", &line, None, Some("apply"), None)
                    {
                        warn!(project_id = %pid, error = %e, "Failed to persist stderr log entry");
                    }
                }
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
            let duration_ms = i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX);
            match status {
                Ok(s) if s.success() => {
                    info!("cflx run exited successfully: project_id={}", project_id);
                    if let (Some(db), Some(change_ids)) = (db.as_ref(), changes.as_ref()) {
                        for change_id in change_ids {
                            if let Err(e) = db.insert_change_event(
                                project_id,
                                change_id,
                                None,
                                "apply",
                                1,
                                true,
                                duration_ms,
                                s.code().map(i64::from),
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                            ) {
                                warn!(project_id, change_id, error = %e, "Failed to persist apply success event");
                            }
                        }
                    }
                }
                Ok(s) => {
                    warn!("cflx run exited with failure: project_id={} status={}", project_id, s);
                    mark_selected_changes_as_error(
                        registry,
                        db.as_ref(),
                        project_id,
                        &changes,
                        s.to_string(),
                    )
                    .await;
                    if let (Some(db), Some(change_ids)) = (db.as_ref(), changes.as_ref()) {
                        let error_text = s.to_string();
                        for change_id in change_ids {
                            if let Err(e) = db.insert_change_event(
                                project_id,
                                change_id,
                                None,
                                "apply",
                                1,
                                false,
                                duration_ms,
                                s.code().map(i64::from),
                                Some(&error_text),
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                            ) {
                                warn!(project_id, change_id, error = %e, "Failed to persist apply failure event");
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed waiting for cflx run: project_id={} err={}", project_id, e);
                    mark_selected_changes_as_error(
                        registry,
                        db.as_ref(),
                        project_id,
                        &changes,
                        e.to_string(),
                    )
                    .await;
                    if let (Some(db), Some(change_ids)) = (db.as_ref(), changes.as_ref()) {
                        let error_text = e.to_string();
                        for change_id in change_ids {
                            if let Err(db_err) = db.insert_change_event(
                                project_id,
                                change_id,
                                None,
                                "apply",
                                1,
                                false,
                                duration_ms,
                                None,
                                Some(&error_text),
                                None,
                                None,
                                None,
                                None,
                                None,
                                None,
                            ) {
                                warn!(project_id, change_id, error = %db_err, "Failed to persist apply wait error event");
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
