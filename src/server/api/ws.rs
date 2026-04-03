use super::*;

/// GET /api/v1/ws - WebSocket stream of remote state updates
///
/// Current behavior:
/// - Sends periodic FullState snapshots (simple, reliable for clients).
/// - Also sends Ping messages to keep the connection alive.
/// - Streams Log entries from the execution log broadcast channel.
pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let registry = state.registry.clone();
    let log_rx = state.log_tx.subscribe();
    let sync_available = state.resolve_command.is_some();
    let orchestration_status = state.orchestration_status.clone();
    let active_commands = state.active_commands.clone();
    let db = state.db.clone();
    ws.on_upgrade(move |socket| {
        handle_ws(
            socket,
            registry,
            log_rx,
            sync_available,
            orchestration_status,
            active_commands,
            db,
        )
    })
}

async fn handle_ws(
    mut socket: WebSocket,
    registry: SharedRegistry,
    mut log_rx: tokio::sync::broadcast::Receiver<RemoteLogEntry>,
    sync_available: bool,
    orchestration_status: Arc<tokio::sync::RwLock<OrchestrationStatus>>,
    active_commands: SharedActiveCommands,
    db: Option<Arc<ServerDb>>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Snapshot
                let (entries, data_dir, all_selections, all_errors) = {
                    let reg = registry.read().await;
                    let entries = reg.list();
                    let data_dir = reg.data_dir().to_path_buf();
                    let all_selections: std::collections::HashMap<String, std::collections::HashMap<String, bool>> = entries
                        .iter()
                        .filter_map(|e| {
                            reg.change_selections_for_project(&e.id)
                                .map(|s| (e.id.clone(), s.clone()))
                        })
                        .collect();
                    let all_errors: std::collections::HashMap<String, std::collections::HashMap<String, String>> = entries
                        .iter()
                        .filter_map(|e| {
                            reg.error_changes_for_project(&e.id)
                                .map(|s| (e.id.clone(), s.clone()))
                        })
                        .collect();
                    (entries, data_dir, all_selections, all_errors)
                };

                let mut snapshot = Vec::new();
                for entry in &entries {
                    let selections = all_selections.get(&entry.id);
                    let errors = all_errors.get(&entry.id);
                    snapshot.push(build_remote_project_snapshot_async(&data_dir, entry, selections, errors).await);
                }

                // Collect worktree information for each project
                let mut worktrees_map = std::collections::HashMap::new();
                for entry in &entries {
                    let wt_path = data_dir
                        .join("worktrees")
                        .join(&entry.id)
                        .join(&entry.branch);
                    if wt_path.exists() {
                        if let Ok(wts) = crate::worktree_ops::get_worktrees(&wt_path).await {
                            let remote_wts: Vec<crate::remote::types::RemoteWorktreeInfo> =
                                wts.into_iter().map(Into::into).collect();
                            if !remote_wts.is_empty() {
                                worktrees_map.insert(entry.id.clone(), remote_wts);
                            }
                        }
                    }
                }
                let worktrees = if worktrees_map.is_empty() {
                    None
                } else {
                    Some(worktrees_map)
                };

                let ui_state = if let Some(db) = &db {
                    match db.get_all_ui_state() {
                        Ok(state) => state,
                        Err(e) => {
                            warn!(error = %e, "Failed to load ui_state for websocket snapshot");
                            std::collections::HashMap::new()
                        }
                    }
                } else {
                    std::collections::HashMap::new()
                };

                let orch_status = orchestration_status.read().await.as_str().to_string();
                let active_cmds = {
                    let ac = active_commands.read().await;
                    ac.snapshot()
                };
                if let Ok(payload) = serde_json::to_string(&RemoteStateUpdate::FullState {
                    projects: snapshot,
                    worktrees,
                    ui_state,
                    sync_available,
                    orchestration_status: orch_status,
                    active_commands: active_cmds,
                }) {
                    if socket.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }

                // Keep-alive ping
                if let Ok(ping) = serde_json::to_string(&RemoteStateUpdate::Ping) {
                    if socket.send(Message::Text(ping.into())).await.is_err() {
                        break;
                    }
                }
            }

            log_result = log_rx.recv() => {
                match log_result {
                    Ok(entry) => {
                        if let Ok(payload) = serde_json::to_string(&RemoteStateUpdate::Log { entry }) {
                            if socket.send(Message::Text(payload.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("WS log receiver lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Sender closed – stop streaming logs but keep WS open
                    }
                }
            }

            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {
                        // Ignore client messages for now.
                    }
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

pub(super) async fn build_remote_project_snapshot_async(
    data_dir: &std::path::Path,
    entry: &ProjectEntry,
    change_selections: Option<&std::collections::HashMap<String, bool>>,
    error_changes: Option<&std::collections::HashMap<String, String>>,
) -> RemoteProject {
    let name = project_display_name(&entry.remote_url, &entry.branch);
    let repo = extract_repo_name(&entry.remote_url);
    let worktree_path = data_dir
        .join("worktrees")
        .join(&entry.id)
        .join(&entry.branch);

    let mut changes =
        list_remote_changes_in_worktree(&worktree_path, &entry.id, &entry.branch).await;

    // Apply selected/error state from registry.
    for change in &mut changes {
        let is_error = error_changes
            .and_then(|m| m.get(&change.id))
            .map(|error| {
                change.status = "error".to_string();
                change.iteration_number = None;
                !error.is_empty()
            })
            .unwrap_or(false);
        let default_selected = !is_error;
        change.selected = change_selections
            .and_then(|m| m.get(&change.id))
            .copied()
            .unwrap_or(default_selected);
    }

    let status_str = match entry.status {
        ProjectStatus::Idle => "idle",
        ProjectStatus::Running => "running",
        ProjectStatus::Stopped => "stopped",
    };
    let is_busy = matches!(entry.status, ProjectStatus::Running);

    RemoteProject {
        id: entry.id.clone(),
        name,
        repo,
        branch: entry.branch.clone(),
        status: status_str.to_string(),
        is_busy,
        error: None,
        sync_state: entry.sync_metadata.sync_state.as_str().to_string(),
        ahead_count: entry.sync_metadata.ahead_count,
        behind_count: entry.sync_metadata.behind_count,
        sync_required: entry.sync_metadata.sync_required,
        local_sha: entry.sync_metadata.local_sha.clone(),
        remote_sha: entry.sync_metadata.remote_sha.clone(),
        last_remote_check_at: entry.sync_metadata.last_remote_check_at.clone(),
        remote_check_error: entry.sync_metadata.remote_check_error.clone(),
        changes,
    }
}

/// Extract the repository name from a remote URL (last path segment without .git suffix).
///
/// For standard git remotes ending in `.git`, the `.git` suffix is stripped.
/// For unusual URLs, falls back to the best available non-empty label.
pub(super) fn extract_repo_name(remote_url: &str) -> String {
    let basename = remote_url
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .unwrap_or(remote_url)
        .trim_end_matches(".git");

    if basename.is_empty() {
        // Fallback: use the full URL rather than an empty string
        remote_url.to_string()
    } else {
        basename.to_string()
    }
}

pub(super) fn project_display_name(remote_url: &str, branch: &str) -> String {
    // Keep it short but recognizable: repo@branch
    let repo = extract_repo_name(remote_url);
    format!("{}@{}", repo, branch)
}

pub(super) async fn list_remote_changes_in_worktree(
    worktree_path: &std::path::Path,
    project_id: &str,
    base_branch: &str,
) -> Vec<RemoteChange> {
    let changes_dir = worktree_path.join("openspec/changes");
    if !changes_dir.exists() {
        return Vec::new();
    }

    // Build a mapping from sanitized change_id -> worktree path.
    // This allows us to read progress from active worktrees created during parallel execution.
    let mut worktree_by_change: std::collections::HashMap<String, std::path::PathBuf> =
        std::collections::HashMap::new();
    if let Ok(worktrees) = crate::vcs::git::commands::list_worktrees(worktree_path).await {
        for (path, _head, branch, _is_detached, is_main) in worktrees {
            if is_main {
                continue;
            }
            if let Some(change_id) =
                GitWorkspaceManager::extract_change_id_from_worktree_name(&branch)
            {
                worktree_by_change.insert(change_id, std::path::PathBuf::from(path));
            }
        }
    }

    let entries = match std::fs::read_dir(&changes_dir) {
        Ok(e) => e,
        Err(e) => {
            debug!("Failed to read changes dir {:?}: {}", changes_dir, e);
            return Vec::new();
        }
    };

    let mut changes = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if dir_name == "archive" || dir_name.starts_with('.') {
            continue;
        }

        // Only include directories that look like an active change (proposal.md exists).
        let proposal_path = path.join("proposal.md");
        if !proposal_path.exists() {
            continue;
        }

        let tasks_path = path.join("tasks.md");

        // Prefer worktree progress if the change is currently executing in a worktree.
        let wt_path_opt = worktree_by_change.get(dir_name).map(|p| p.as_path());
        let (completed, total) =
            match task_parser::parse_progress_with_fallback(dir_name, wt_path_opt) {
                Ok(p) => (p.completed, p.total),
                Err(_) => {
                    // Last resort: try base tasks.md directly.
                    match task_parser::parse_file(&tasks_path, Some(dir_name)) {
                        Ok(p) => (p.completed, p.total),
                        Err(_) => (0, 0),
                    }
                }
            };

        let last_modified = latest_modified_rfc3339(&[&proposal_path, &tasks_path])
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        let (status, iteration_number) = if path.join("REJECTED.md").exists() {
            ("rejected".to_string(), None)
        } else if let Some(wt_path) = worktree_by_change.get(dir_name) {
            match detect_workspace_state(dir_name, wt_path, base_branch).await {
                Ok(WorkspaceState::Created) => ("queued".to_string(), None),
                Ok(WorkspaceState::Applying { iteration }) => {
                    ("applying".to_string(), Some(iteration))
                }
                Ok(WorkspaceState::Applied) => ("archiving".to_string(), None),
                Ok(WorkspaceState::Archiving) => ("archiving".to_string(), None),
                Ok(WorkspaceState::Archived) => ("archived".to_string(), None),
                Ok(WorkspaceState::Merged) => ("merged".to_string(), None),
                Err(_) => ("idle".to_string(), None),
            }
        } else {
            ("idle".to_string(), None)
        };

        changes.push(RemoteChange {
            id: dir_name.to_string(),
            project: project_id.to_string(),
            completed_tasks: completed,
            total_tasks: total,
            last_modified,
            status,
            iteration_number,
            selected: true,
        });
    }

    changes.sort_by(|a, b| a.id.cmp(&b.id));
    changes
}

fn latest_modified_rfc3339(paths: &[&std::path::Path]) -> Option<String> {
    use std::time::SystemTime;

    let mut latest: Option<SystemTime> = None;
    for p in paths {
        let m = std::fs::metadata(p).and_then(|m| m.modified()).ok();
        if let Some(ts) = m {
            latest = Some(match latest {
                Some(cur) if cur >= ts => cur,
                _ => ts,
            });
        }
    }

    latest.map(|ts| chrono::DateTime::<chrono::Utc>::from(ts).to_rfc3339())
}
